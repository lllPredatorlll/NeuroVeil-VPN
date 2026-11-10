use crate::models::{is_awg_protocol, RoutingConfig, VpnProfile};
use serde_json::{json, Value};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use url::Url;
use uuid::Uuid;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn get_app_data_dir() -> PathBuf {
    let mut base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });

    // If running from src-tauri/target/debug or src-tauri/target/release, go up to project root
    if base.ends_with("debug") || base.ends_with("release") {
        if let Some(p1) = base.parent() {
            if let Some(p2) = p1.parent() {
                if let Some(p3) = p2.parent() {
                    base = p3.to_path_buf();
                }
            }
        }
    } else if base.ends_with("src-tauri") {
        if let Some(parent) = base.parent() {
            base = parent.to_path_buf();
        }
    }

    let data_dir = base.join("data");
    let _ = fs::create_dir_all(&data_dir);
    data_dir
}

pub fn get_singbox_bin_path() -> PathBuf {
    let dir = get_app_data_dir().join("bin");
    let _ = fs::create_dir_all(&dir);
    dir.join("sing-box.exe")
}

pub fn check_singbox_version() -> (bool, String) {
    let bin = get_singbox_bin_path();
    if !bin.exists() {
        return (false, "Не установлен".to_string());
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let first_line = text.lines().next().unwrap_or("sing-box").to_string();
            (true, first_line)
        }
        _ => (true, "sing-box (установлен)".to_string()),
    }
}

pub fn download_singbox_core() -> Result<String, String> {
    let bin_path = get_singbox_bin_path();

    // Query latest release from GitHub API (sing-box-lx with native AmneziaWG support)
    let release_resp: Value = ureq::get("https://api.github.com/repos/Leadaxe/sing-box-lx/releases/latest")
        .set("User-Agent", "NeuroVeilVPN-Client/0.4.3")
        .call()
        .or_else(|_| {
            ureq::get("https://api.github.com/repos/SagerNet/sing-box/releases/latest")
                .set("User-Agent", "NeuroVeilVPN-Client/0.4.3")
                .call()
        })
        .map_err(|e| format!("Ошибка запроса к GitHub API: {}", e))?
        .into_json()
        .map_err(|e| format!("Ошибка чтения ответа GitHub: {}", e))?;

    let assets = release_resp["assets"]
        .as_array()
        .ok_or_else(|| "В релизе sing-box не найден список файлов".to_string())?;

    let mut download_url = None;
    for asset in assets {
        if let Some(name) = asset["name"].as_str() {
            if name.contains("windows-amd64.zip") && !name.contains("legacy") {
                download_url = asset["browser_download_url"].as_str().map(|s| s.to_string());
                break;
            }
        }
    }

    let url = download_url.ok_or_else(|| "Не найден архив windows-amd64.zip в последнем релизе".to_string())?;

    let resp = ureq::get(&url)
        .set("User-Agent", "NeuroVeilVPN-Client/0.4.3")
        .call()
        .map_err(|e| format!("Ошибка скачивания архива sing-box: {}", e))?;

    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Ошибка чтения потока архива: {}", e))?;

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Ошибка распаковки ZIP: {}", e))?;

    let bin_dir = bin_path.parent().unwrap_or(&bin_path);
    let mut extracted = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        if name.ends_with("sing-box.exe") {
            let mut out_file = fs::File::create(&bin_path)
                .map_err(|e| format!("Не удалось создать sing-box.exe: {}", e))?;
            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Ошибка записи sing-box.exe: {}", e))?;
            extracted = true;
        } else if name.ends_with("wintun.dll") {
            let wintun_path = bin_dir.join("wintun.dll");
            if let Ok(mut out_file) = fs::File::create(&wintun_path) {
                let _ = std::io::copy(&mut file, &mut out_file);
            }
        } else if name.ends_with("libcronet.dll") {
            let cronet_path = bin_dir.join("libcronet.dll");
            if let Ok(mut out_file) = fs::File::create(&cronet_path) {
                let _ = std::io::copy(&mut file, &mut out_file);
            }
        }
    }

    if !extracted {
        return Err("В архиве не найден файл sing-box.exe".to_string());
    }

    let (_, ver) = check_singbox_version();
    Ok(ver)
}

pub fn set_windows_system_proxy(enable: bool, port: u16) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let reg_path = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        if enable {
            let proxy_server = format!("127.0.0.1:{}", port);
            let mut cmd1 = Command::new("reg");
            cmd1.args(["add", reg_path, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "1", "/f"])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = cmd1.output();

            let mut cmd2 = Command::new("reg");
            cmd2.args(["add", reg_path, "/v", "ProxyServer", "/t", "REG_SZ", "/d", &proxy_server, "/f"])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = cmd2.output();

            let mut cmd3 = Command::new("reg");
            cmd3.args([
                "add",
                reg_path,
                "/v",
                "ProxyOverride",
                "/t",
                "REG_SZ",
                "/d",
                "localhost;127.*;10.*;172.16.*;192.168.*;*.local;<local>",
                "/f",
            ])
            .creation_flags(CREATE_NO_WINDOW);
            let _ = cmd3.output();
        } else {
            let mut cmd1 = Command::new("reg");
            cmd1.args(["add", reg_path, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f"])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = cmd1.output();

            let mut cmd2 = Command::new("reg");
            cmd2.args(["delete", reg_path, "/v", "ProxyServer", "/f"])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = cmd2.output();
        }
    }
    Ok(())
}

fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

pub fn collect_exe_in_dir(dir: &std::path::Path, depth: u8, names: &mut Vec<String>) {
    if depth > 2 || names.len() >= 64 {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(dname) = p.file_name().and_then(|n| n.to_str()) {
                    let lower = dname.to_lowercase();
                    if matches!(
                        lower.as_str(),
                        "locales"
                            | "resources"
                            | "node_modules"
                            | "steamapps"
                            | "depotcache"
                            | "cache"
                            | "code cache"
                            | "gpucache"
                            | "logs"
                            | "crashpad"
                    ) {
                        continue;
                    }
                }
                collect_exe_in_dir(&p, depth + 1, names);
            } else if let Some(ext) = p.extension() {
                if ext.eq_ignore_ascii_case("exe") {
                    if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                        let s = fname.to_string();
                        if !names.contains(&s) {
                            names.push(s);
                        }
                    }
                }
            }
        }
    }
}

fn build_process_rules(entries: &[String], outbound: &str) -> (Vec<Value>, Vec<Value>) {
    let mut proc_names: Vec<String> = Vec::new();
    let mut proc_paths: Vec<String> = Vec::new();
    let mut proc_regexes: Vec<String> = Vec::new();
    let mut has_telegram = false;
    let mut has_discord = false;

    let dns_server = if outbound == "proxy" {
        "dns-remote"
    } else {
        "dns-direct"
    };

    for raw in entries {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.contains("telegram") {
            has_telegram = true;
        }
        if lower.contains("discord") {
            has_discord = true;
        }
        if trimmed.contains('\\') || trimmed.contains('/') {
            let path = std::path::Path::new(trimmed);
            if lower.ends_with(".exe") || path.is_file() {
                proc_paths.push(trimmed.to_string());
                if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                    let s = fname.to_string();
                    if !proc_names.contains(&s) {
                        proc_names.push(s);
                    }
                }
            } else {
                // Folder path
                let clean_folder = trimmed.trim_end_matches(&['\\', '/'][..]);
                let regex_str = format!("(?i)^{}[\\\\/].*", escape_regex(clean_folder));
                if !proc_regexes.contains(&regex_str) {
                    proc_regexes.push(regex_str);
                }
                // If user selected a versioned subfolder like ...\Discord\app-1.0.9266, also cover parent ...\Discord
                if let Some(idx) = lower.find("\\discord\\app-") {
                    let parent_discord = &trimmed[..idx + 8];
                    let parent_regex = format!("(?i)^{}[\\\\/].*", escape_regex(parent_discord));
                    if !proc_regexes.contains(&parent_regex) {
                        proc_regexes.push(parent_regex);
                    }
                }
                if path.is_dir() {
                    collect_exe_in_dir(path, 0, &mut proc_names);
                }
            }
        } else {
            let name = if lower.ends_with(".exe") {
                trimmed.to_string()
            } else {
                format!("{}.exe", trimmed)
            };
            let regex_str = format!("(?i)(^|[\\\\/]){}$", escape_regex(&name));
            if !proc_regexes.contains(&regex_str) {
                proc_regexes.push(regex_str);
            }
            if !proc_names.contains(&name) {
                proc_names.push(name);
            }
        }
    }

    if has_telegram && outbound == "proxy" {
        let tg_regex = "(?i).*\\\\telegram[\\\\/].*".to_string();
        if !proc_regexes.contains(&tg_regex) {
            proc_regexes.push(tg_regex);
        }
        for name in &["telegram.exe", "Telegram.exe"] {
            let s = name.to_string();
            if !proc_names.contains(&s) {
                proc_names.push(s);
            }
        }
    }
    if has_discord && outbound == "proxy" {
        let discord_regex = "(?i).*\\\\discord[\\\\/].*".to_string();
        if !proc_regexes.contains(&discord_regex) {
            proc_regexes.push(discord_regex);
        }
        for name in &["discord.exe", "Discord.exe", "DiscordSystemHelper.exe"] {
            let s = name.to_string();
            if !proc_names.contains(&s) {
                proc_names.push(s);
            }
        }
    }

    // Separate rules so sing-box evaluates them as OR instead of AND!
    let mut route_rules = Vec::new();
    let mut dns_rules = Vec::new();

    if !proc_names.is_empty() {
        route_rules.push(json!({
            "process_name": proc_names.clone(),
            "outbound": outbound
        }));
        dns_rules.push(json!({
            "process_name": proc_names,
            "server": dns_server
        }));
    }
    if !proc_paths.is_empty() {
        route_rules.push(json!({
            "process_path": proc_paths.clone(),
            "outbound": outbound
        }));
        dns_rules.push(json!({
            "process_path": proc_paths,
            "server": dns_server
        }));
    }
    if !proc_regexes.is_empty() {
        route_rules.push(json!({
            "process_path_regex": proc_regexes.clone(),
            "outbound": outbound
        }));
        dns_rules.push(json!({
            "process_path_regex": proc_regexes,
            "server": dns_server
        }));
    }
    if has_telegram && outbound == "proxy" {
        let tg_domains = vec![
            "telegram.org",
            "t.me",
            "tdesktop.com",
            "telegra.ph",
            "telesco.pe",
        ];
        dns_rules.push(json!({
            "domain_suffix": tg_domains.clone(),
            "server": "dns-remote"
        }));
        // Telegram Desktop uses direct MTProto connections to these official IPv4 CIDR blocks
        route_rules.push(json!({
            "ip_cidr": [
                "149.154.160.0/20",
                "91.108.4.0/22",
                "91.108.8.0/22",
                "91.108.12.0/22",
                "91.108.16.0/22",
                "91.108.20.0/22",
                "91.108.56.0/22",
                "95.161.64.0/20",
                "185.76.151.0/24",
                "194.221.248.0/21"
            ],
            "outbound": "proxy"
        }));
        route_rules.push(json!({
            "domain_suffix": tg_domains,
            "outbound": "proxy"
        }));
    }
    if has_discord && outbound == "proxy" {
        let discord_domains = vec![
            "discord.com",
            "discordapp.com",
            "discordapp.net",
            "discord.media",
            "discord.gg",
            "discordcdn.com",
            "discord.co",
            "discordstatus.com",
            "dis.gd",
        ];
        dns_rules.push(json!({
            "domain_suffix": discord_domains.clone(),
            "server": "dns-remote"
        }));
        // Discord Voice / WebRTC & API Cloudflare CIDRs (vital for UDP voice & process-lookup fallbacks)
        route_rules.push(json!({
            "ip_cidr": [
                "162.159.128.0/21",
                "162.159.136.0/21",
                "162.159.61.0/24",
                "66.22.196.0/22",
                "66.22.200.0/22",
                "66.22.204.0/22",
                "66.22.208.0/20"
            ],
            "outbound": "proxy"
        }));
        route_rules.push(json!({
            "domain_suffix": discord_domains,
            "outbound": "proxy"
        }));
    }
    (route_rules, dns_rules)
}

pub fn get_rules_proxy_path() -> PathBuf {
    get_app_data_dir().join("rules_proxy.json")
}

pub fn get_rules_direct_path() -> PathBuf {
    get_app_data_dir().join("rules_direct.json")
}

pub fn build_ruleset_content(
    processes: &[String],
    domains: &[String],
    ips: &[String],
    is_proxy: bool,
) -> Value {
    let mut rules: Vec<Value> = Vec::new();

    // 1. Convert process rules into rule_set items (strip "outbound" property)
    let outbound_tag = if is_proxy { "proxy" } else { "direct" };
    let (proc_routes, _) = build_process_rules(processes, outbound_tag);
    for mut r in proc_routes {
        if let Some(obj) = r.as_object_mut() {
            obj.remove("outbound");
            rules.push(r);
        }
    }

    // 2. Custom domains
    let mut domain_suffixes = Vec::new();
    let mut full_domains = Vec::new();
    for d in domains {
        let trimmed = d.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('.') {
            domain_suffixes.push(trimmed.to_string());
        } else {
            domain_suffixes.push(format!(".{}", trimmed));
            full_domains.push(trimmed.to_string());
        }
    }
    if !domain_suffixes.is_empty() || !full_domains.is_empty() {
        let mut d_rule = serde_json::Map::new();
        if !domain_suffixes.is_empty() {
            d_rule.insert("domain_suffix".to_string(), json!(domain_suffixes));
        }
        if !full_domains.is_empty() {
            d_rule.insert("domain".to_string(), json!(full_domains));
        }
        rules.push(Value::Object(d_rule));
    }

    // 3. Custom IPs / CIDRs
    let mut cidrs = Vec::new();
    for ip in ips {
        let t = ip.trim();
        if !t.is_empty() {
            if t.contains('/') {
                cidrs.push(t.to_string());
            } else {
                cidrs.push(format!("{}/32", t));
            }
        }
    }
    if !cidrs.is_empty() {
        rules.push(json!({ "ip_cidr": cidrs }));
    }

    json!({
        "version": 1,
        "rules": rules
    })
}

pub fn write_routing_rulesets(routing: &RoutingConfig) -> Result<(), String> {
    let proxy_path = get_rules_proxy_path();
    let direct_path = get_rules_direct_path();

    let proxy_content = if routing.split_tunneling {
        build_ruleset_content(
            &routing.proxy_processes,
            &routing.proxy_domains,
            &routing.proxy_ips,
            true,
        )
    } else {
        json!({ "version": 1, "rules": [] })
    };

    let direct_content = if routing.split_tunneling {
        build_ruleset_content(
            &routing.direct_processes,
            &routing.direct_domains,
            &routing.direct_ips,
            false,
        )
    } else {
        json!({ "version": 1, "rules": [] })
    };

    if let Some(parent) = proxy_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let write_and_sync = |p: &std::path::Path, content: &Value, name: &str| -> Result<(), String> {
        use std::io::Write;
        let data = serde_json::to_string_pretty(content).unwrap_or_default();
        let mut f = std::fs::File::create(p)
            .map_err(|e| format!("Не удалось открыть {}: {}", name, e))?;
        f.write_all(data.as_bytes())
            .map_err(|e| format!("Не удалось записать {}: {}", name, e))?;
        let _ = f.sync_all();
        Ok(())
    };

    write_and_sync(&proxy_path, &proxy_content, "rules_proxy.json")?;
    write_and_sync(&direct_path, &direct_content, "rules_direct.json")?;

    Ok(())
}

#[allow(dead_code)]
fn detect_physical_ipv4(server_ip: &str) -> Option<String> {
    let target = if server_ip.parse::<std::net::Ipv4Addr>().is_ok() {
        format!("{}:80", server_ip)
    } else {
        "8.8.8.8:80".to_string()
    };
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect(&target).is_ok() {
            if let Ok(local) = sock.local_addr() {
                let ip = local.ip().to_string();
                if !ip.starts_with("127.")
                    && !ip.starts_with("172.19.")
                    && !ip.starts_with("169.254.")
                    && ip != "0.0.0.0"
                {
                    return Some(ip);
                }
            }
        }
    }
    None
}

fn build_single_outbound(profile: &VpnProfile, tag: &str, bind_ipv4: Option<&str>) -> Value {
    let mut out = if profile.protocol == "vless" {
        let flow = profile.reality_flow.as_deref().unwrap_or("xtls-rprx-vision");
        let tls_frag = profile.tls_fragment.unwrap_or(true);
        let mut tls_obj = json!({
            "enabled": true,
            "server_name": profile.sni,
            "utls": {
                "enabled": true,
                "fingerprint": "chrome"
            }
        });
        if tls_frag {
            tls_obj["fragment"] = json!(true);
            tls_obj["record_fragment"] = json!(true);
            tls_obj["fragment_fallback_delay"] = json!("500ms");
        }
        let mut vless_out = json!({
            "type": "vless",
            "tag": tag,
            "server": profile.server,
            "server_port": profile.port,
            "uuid": profile.uuid.clone().unwrap_or_else(|| profile.password.clone()),
            "flow": flow,
            "tls": tls_obj
        });
        if let Some(ref pbk) = profile.reality_public_key {
            let mut reality_obj = json!({
                "enabled": true,
                "public_key": pbk
            });
            if let Some(ref sid) = profile.reality_short_id {
                if !sid.is_empty() {
                    reality_obj["short_id"] = json!(sid);
                }
            }
            vless_out["tls"]["reality"] = reality_obj;
        }
        vless_out
    } else if profile.protocol == "tuic" {
        let tls_frag = profile.tls_fragment.unwrap_or(true);
        let mut tls_obj = json!({
            "enabled": true,
            "server_name": profile.sni,
            "insecure": profile.insecure,
            "alpn": ["h3"]
        });
        if tls_frag {
            tls_obj["fragment"] = json!(true);
            tls_obj["record_fragment"] = json!(true);
        }
        json!({
            "type": "tuic",
            "tag": tag,
            "server": profile.server,
            "server_port": profile.port,
            "uuid": profile.uuid.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
            "password": profile.password,
            "congestion_control": "bbr",
            "udp_relay_mode": "native",
            "zero_rtt_handshake": false,
            "heartbeat": "10s",
            "tls": tls_obj
        })
    } else if profile.protocol == "shadowsocks" {
        let method = profile.ss_method.as_deref().unwrap_or("2022-blake3-aes-128-gcm");
        let mut ss_out = json!({
            "type": "shadowsocks",
            "tag": tag,
            "server": profile.server,
            "server_port": profile.port,
            "method": method,
            "password": profile.password
        });
        if let Some(ref plugin) = profile.ss_plugin {
            if !plugin.trim().is_empty() {
                ss_out["plugin"] = json!(plugin.trim());
                if let Some(ref opts) = profile.ss_plugin_opts {
                    if !opts.trim().is_empty() {
                        ss_out["plugin_opts"] = json!(opts.trim());
                    }
                }
            }
        }
        ss_out
    } else if profile.protocol == "ssh" {
        let user = if profile.sni.trim().is_empty() {
            "root"
        } else {
            profile.sni.trim()
        };
        let mut ssh_out = json!({
            "type": "ssh",
            "tag": tag,
            "server": profile.server,
            "server_port": profile.port,
            "user": user,
            "client_version": "SSH-2.0-OpenSSH_9.0"
        });
        let pass = profile.password.trim();
        if !pass.is_empty() {
            if pass.contains("BEGIN ") && pass.contains("PRIVATE KEY") {
                ssh_out["private_key"] = json!(pass);
            } else {
                ssh_out["password"] = json!(pass);
            }
        }
        ssh_out
    } else {
        let tls_frag = profile.tls_fragment.unwrap_or(true);
        let mut tls_obj = json!({
            "enabled": true,
            "server_name": profile.sni,
            "insecure": profile.insecure,
            "alpn": ["h3"]
        });
        if tls_frag {
            tls_obj["fragment"] = json!(true);
            tls_obj["record_fragment"] = json!(true);
        }
        let mut hy2 = json!({
            "type": "hysteria2",
            "tag": tag,
            "server": profile.server,
            "server_port": profile.port,
            "up_mbps": profile.up_mbps.max(10),
            "down_mbps": profile.down_mbps.max(50),
            "password": profile.password,
            "tls": tls_obj
        });
        if let Some(ref obfs_pw) = profile.obfs_password {
            if !obfs_pw.trim().is_empty() {
                hy2["obfs"] = json!({
                    "type": "salamander",
                    "password": obfs_pw.trim()
                });
            }
        }
        if let Some(ref hp) = profile.hop_ports {
            let clean_hp = hp.trim();
            if !clean_hp.is_empty() {
                if let Some(obj) = hy2.as_object_mut() {
                    obj.remove("server_port");
                    obj.insert("server_ports".to_string(), json!([clean_hp]));
                    let interval = profile.hop_interval.as_deref().unwrap_or("30s");
                    obj.insert("hop_interval".to_string(), json!(interval));
                }
            }
        }
        hy2
    };
    if let Some(ip) = bind_ipv4 {
        out["inet4_bind_address"] = json!(ip);
    }
    out
}

pub fn get_awg_candidate_ports(profile: &VpnProfile) -> Vec<u16> {
    let mut ports: Vec<u16> = Vec::new();
    ports.push(profile.port);

    if let Some(ref hop) = profile.awg_hop_ports {
        for part in hop.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            if let Some((start_s, end_s)) = p.split_once(':').or_else(|| p.split_once('-')) {
                if let (Ok(s), Ok(e)) = (start_s.trim().parse::<u16>(), end_s.trim().parse::<u16>()) {
                    let (min_p, max_p) = if s <= e { (s, e) } else { (e, s) };
                    let range_size = max_p.saturating_sub(min_p) as usize;
                    if range_size > 0 {
                        let step = (range_size / 12).max(1);
                        let mut curr = min_p;
                        while curr <= max_p && ports.len() < 16 {
                            if !ports.contains(&curr) {
                                ports.push(curr);
                            }
                            curr = curr.saturating_add(step as u16);
                        }
                    }
                }
            } else if let Ok(port_num) = p.parse::<u16>() {
                if !ports.contains(&port_num) {
                    ports.push(port_num);
                }
            }
        }
    }
    for common in [443, 53, 8080, 1194] {
        if !ports.contains(&common) && ports.len() < 20 {
            ports.push(common);
        }
    }

    ports
}

pub fn generate_singbox_config(
    profile: &VpnProfile,
    routing: &RoutingConfig,
) -> Value {
    generate_singbox_config_with_port(profile, routing, None)
}

pub fn generate_singbox_config_with_port(
    profile: &VpnProfile,
    routing: &RoutingConfig,
    override_port: Option<u16>,
) -> Value {
    generate_singbox_config_internal(profile, routing, override_port, false)
}

pub fn generate_singbox_config_internal(
    profile: &VpnProfile,
    routing: &RoutingConfig,
    override_port: Option<u16>,
    for_mobile: bool,
) -> Value {
    let direct_out = json!({
        "type": "direct",
        "tag": "direct"
    });
    let block_out = json!({
        "type": "block",
        "tag": "block"
    });

    let is_awg = is_awg_protocol(&profile.protocol);
    let active_peer_port = override_port.unwrap_or(profile.port);

    let mut outbounds = Vec::new();
    let mut endpoints = Vec::new();

    if is_awg {
        outbounds.push(direct_out);
        outbounds.push(block_out);
        let mut ep = json!({
            "type": "wireguard",
            "tag": "proxy",
            "address": [profile.awg_local_address.as_deref().unwrap_or("10.8.0.2/32")],
            "private_key": profile.awg_private_key.as_deref().unwrap_or(&profile.password),
            "listen_port": 0,
            "mtu": 1280,
            "jc": profile.awg_jc.unwrap_or(4),
            "jmin": profile.awg_jmin.unwrap_or(40),
            "jmax": profile.awg_jmax.unwrap_or(70),
            "s1": profile.awg_s1.unwrap_or(15),
            "s2": profile.awg_s2.unwrap_or(25),
            "peers": [
                {
                    "address": profile.server,
                    "port": active_peer_port,
                    "public_key": profile.awg_public_key.as_deref().unwrap_or(&profile.password),
                    "allowed_ips": ["0.0.0.0/0", "::/0"],
                    "persistent_keepalive_interval": 25
                }
            ]
        });
        if let Some(ref psk) = profile.awg_preshared_key {
            if !psk.trim().is_empty() {
                ep["peers"][0]["pre_shared_key"] = json!(psk.trim());
            }
        }
        if let Some(ref h1) = profile.awg_h1 {
            if let Ok(num) = h1.trim().parse::<u32>() {
                ep["h1"] = json!(num);
            } else if !h1.trim().is_empty() {
                ep["h1"] = json!(h1.trim());
            }
        }
        if let Some(ref h2) = profile.awg_h2 {
            if let Ok(num) = h2.trim().parse::<u32>() {
                ep["h2"] = json!(num);
            } else if !h2.trim().is_empty() {
                ep["h2"] = json!(h2.trim());
            }
        }
        if let Some(ref h3) = profile.awg_h3 {
            if let Ok(num) = h3.trim().parse::<u32>() {
                ep["h3"] = json!(num);
            } else if !h3.trim().is_empty() {
                ep["h3"] = json!(h3.trim());
            }
        }
        if let Some(ref h4) = profile.awg_h4 {
            if let Ok(num) = h4.trim().parse::<u32>() {
                ep["h4"] = json!(num);
            } else if !h4.trim().is_empty() {
                ep["h4"] = json!(h4.trim());
            }
        }
        if let Some(s3) = profile.awg_s3 {
            ep["s3"] = json!(s3);
        }
        if let Some(s4) = profile.awg_s4 {
            ep["s4"] = json!(s4);
        }

        // AWG 3.1 Header Protection & advanced obfuscation
        if let Some(ref hpk) = profile.awg_header_protection_key {
            let trimmed_hpk = hpk.trim();
            if !trimmed_hpk.is_empty() {
                ep["header_protection_key"] = json!(trimmed_hpk);
                // In AWG 3.1, S1-S4 carry the ChaCha20 nonce and must each be >= 12 bytes
                let s1 = profile.awg_s1.unwrap_or(12).max(12);
                let s2 = profile.awg_s2.unwrap_or(12).max(12);
                let s3 = profile.awg_s3.unwrap_or(12).max(12);
                let s4 = profile.awg_s4.unwrap_or(12).max(12);
                ep["s1"] = json!(s1);
                ep["s2"] = json!(s2);
                ep["s3"] = json!(s3);
                ep["s4"] = json!(s4);

                if profile.awg_random_trailers.unwrap_or(true) {
                    ep["random_trailers"] = json!(true);
                }
                if profile.awg_disable_cookies.unwrap_or(true) {
                    ep["disable_cookies"] = json!(true);
                }
                if let Some(ref r) = profile.awg_rekey_after_time {
                    if !r.trim().is_empty() { ep["rekey_after_time"] = json!(r.trim()); }
                }
                if let Some(ref r) = profile.awg_rekey_timeout {
                    if !r.trim().is_empty() { ep["rekey_timeout"] = json!(r.trim()); }
                }
                if let Some(ref r) = profile.awg_reject_after_time {
                    if !r.trim().is_empty() { ep["reject_after_time"] = json!(r.trim()); }
                }
                if let Some(ref r) = profile.awg_keepalive_timeout {
                    if !r.trim().is_empty() { ep["keepalive_timeout"] = json!(r.trim()); }
                }
            }
        }

        if let Some(ref i1) = profile.awg_i1 {
            if !i1.trim().is_empty() { ep["i1"] = json!(i1.trim()); }
        }
        if let Some(ref i2) = profile.awg_i2 {
            if !i2.trim().is_empty() { ep["i2"] = json!(i2.trim()); }
        }
        if let Some(ref i3) = profile.awg_i3 {
            if !i3.trim().is_empty() { ep["i3"] = json!(i3.trim()); }
        }
        if let Some(ref i4) = profile.awg_i4 {
            if !i4.trim().is_empty() { ep["i4"] = json!(i4.trim()); }
        }
        endpoints.push(ep);
    } else {
        outbounds.push(build_single_outbound(profile, "proxy", None));
        outbounds.push(direct_out);
        outbounds.push(block_out);
    }

    // 2. Inbounds
    let mut inbounds = vec![json!({
        "type": "mixed",
        "tag": "mixed-in",
        "listen": "127.0.0.1",
        "listen_port": routing.local_mixed_port
    })];

    if !for_mobile {
        // Dedicated internal proxy inbound that ALWAYS routes through the active VPN tunnel ("proxy")
        // regardless of split tunneling / whitelist settings, used for SSH fallback when ISP blocks direct SSH.
        inbounds.push(json!({
            "type": "mixed",
            "tag": "vpn-tunnel-in",
            "listen": "127.0.0.1",
            "listen_port": 2081
        }));
    }

    if routing.mode == "tun" {
        let mut tun_obj = json!({
            "type": "tun",
            "tag": "tun-in",
            "interface_name": "neuroveil-tun",
            "address": ["172.19.0.1/30"],
            "mtu": 1400,
            "auto_route": true,
            "strict_route": routing.kill_switch,
            "stack": "gvisor"
        });
        if profile.server.parse::<std::net::Ipv4Addr>().is_ok() {
            tun_obj["route_exclude_address"] = json!([format!("{}/32", profile.server)]);
        }
        inbounds.push(tun_obj);
    }

    // 3. Route & DNS rules
    // CRITICAL: Match port 53 and sniff BEFORE protocol=dns and BEFORE ip_is_private!
    // Otherwise 172.19.0.2:53 (TUN DNS IP in RFC1918 172.16.0.0/12) gets swallowed by ip_is_private -> direct!
    let mut route_rules = vec![
        json!({
            "port": 53,
            "action": "hijack-dns"
        }),
        json!({
            "action": "sniff",
            "timeout": "50ms"
        }),
        json!({
            "protocol": "dns",
            "action": "hijack-dns"
        }),
    ];

    if routing.block_ipv6 {
        route_rules.push(json!({
            "ip_version": 6,
            "outbound": "block"
        }));
    }

    if !for_mobile {
        route_rules.push(json!({
            "inbound": ["vpn-tunnel-in"],
            "outbound": "proxy"
        }));
    }

    route_rules.push(json!({
        "ip_is_private": true,
        "outbound": "direct"
    }));
    route_rules.push(json!({
        "domain_suffix": [
            "steamcontent.com",
            "steamserver.net"
        ],
        "outbound": "direct"
    }));

    if profile.protocol == "ssh" {
        // Since SSH only encapsulates TCP, route non-DNS UDP directly
        // so voice chats (Discord WebRTC) and UDP games continue to function smoothly
        route_rules.push(json!({
            "network": "udp",
            "outbound": "direct"
        }));
    }

    let mut dns_rules = vec![
        json!({
            "domain_suffix": [
                "steamcontent.com",
                "steamserver.net"
            ],
            "server": "dns-direct"
        }),
    ];

    let final_target = if routing.split_tunneling && routing.default_outbound == "direct" {
        "direct"
    } else {
        "proxy"
    };

    let mut route_obj = serde_json::Map::new();

    if for_mobile {
        // Mobile clients (Hiddify / sing-box mobile): Inline rules directly
        if routing.split_tunneling {
            let (proxy_proc_routes, proxy_proc_dns) =
                build_process_rules(&routing.proxy_processes, "proxy");
            route_rules.extend(proxy_proc_routes);
            dns_rules.extend(proxy_proc_dns);

            let (direct_proc_routes, direct_proc_dns) =
                build_process_rules(&routing.direct_processes, "direct");
            route_rules.extend(direct_proc_routes);
            dns_rules.extend(direct_proc_dns);

            let mut domain_suffixes = Vec::new();
            let mut full_domains = Vec::new();

            for d in &routing.direct_domains {
                let trimmed = d.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with('.') {
                    domain_suffixes.push(trimmed.to_string());
                } else {
                    domain_suffixes.push(format!(".{}", trimmed));
                    full_domains.push(trimmed.to_string());
                }
            }

            if !routing.proxy_domains.is_empty() {
                let mut proxy_suffixes = Vec::new();
                for d in &routing.proxy_domains {
                    let t = d.trim().trim_start_matches('.');
                    if !t.is_empty() {
                        proxy_suffixes.push(t.to_string());
                    }
                }
                if !proxy_suffixes.is_empty() {
                    dns_rules.push(json!({
                        "domain_suffix": proxy_suffixes.clone(),
                        "server": "dns-remote"
                    }));
                    route_rules.push(json!({
                        "domain_suffix": proxy_suffixes,
                        "outbound": "proxy"
                    }));
                }
            }

            if !domain_suffixes.is_empty() || !full_domains.is_empty() {
                dns_rules.push(json!({
                    "domain_suffix": domain_suffixes.clone(),
                    "domain": full_domains.clone(),
                    "server": "dns-direct"
                }));
                route_rules.push(json!({
                    "domain_suffix": domain_suffixes,
                    "domain": full_domains,
                    "outbound": "direct"
                }));
            }

            if !routing.proxy_ips.is_empty() {
                let mut proxy_cidrs = Vec::new();
                for ip in &routing.proxy_ips {
                    let t = ip.trim();
                    if !t.is_empty() {
                        if t.contains('/') {
                            proxy_cidrs.push(t.to_string());
                        } else {
                            proxy_cidrs.push(format!("{}/32", t));
                        }
                    }
                }
                if !proxy_cidrs.is_empty() {
                    route_rules.push(json!({
                        "ip_cidr": proxy_cidrs,
                        "outbound": "proxy"
                    }));
                }
            }

            if !routing.direct_ips.is_empty() {
                let mut direct_cidrs = Vec::new();
                for ip in &routing.direct_ips {
                    let t = ip.trim();
                    if !t.is_empty() {
                        if t.contains('/') {
                            direct_cidrs.push(t.to_string());
                        } else {
                            direct_cidrs.push(format!("{}/32", t));
                        }
                    }
                }
                if !direct_cidrs.is_empty() {
                    route_rules.push(json!({
                        "ip_cidr": direct_cidrs,
                        "outbound": "direct"
                    }));
                }
            }
        }
        route_obj.insert("rules".to_string(), json!(route_rules));
        route_obj.insert("final".to_string(), json!(final_target));
        route_obj.insert("auto_detect_interface".to_string(), json!(true));
        route_obj.insert("default_domain_resolver".to_string(), json!("dns-direct"));
    } else {
        // Desktop Windows client: Dynamic local rule_set watching via fsnotify + selector outbound
        let _ = write_routing_rulesets(routing);
        let proxy_path_str = get_rules_proxy_path().to_string_lossy().to_string();
        let direct_path_str = get_rules_direct_path().to_string_lossy().to_string();

        route_rules.push(json!({
            "rule_set": "direct-rules",
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "rule_set": "proxy-rules",
            "outbound": "proxy"
        }));

        dns_rules.push(json!({
            "rule_set": "direct-rules",
            "server": "dns-direct"
        }));
        dns_rules.push(json!({
            "rule_set": "proxy-rules",
            "server": "dns-remote"
        }));

        outbounds.push(json!({
            "type": "selector",
            "tag": "default-outbound",
            "outbounds": ["proxy", "direct"],
            "default": final_target
        }));

        route_obj.insert(
            "rule_set".to_string(),
            json!([
                {
                    "tag": "direct-rules",
                    "type": "local",
                    "format": "source",
                    "path": direct_path_str
                },
                {
                    "tag": "proxy-rules",
                    "type": "local",
                    "format": "source",
                    "path": proxy_path_str
                }
            ]),
        );
        route_obj.insert("rules".to_string(), json!(route_rules));
        route_obj.insert("final".to_string(), json!("default-outbound"));
        route_obj.insert("auto_detect_interface".to_string(), json!(true));
        route_obj.insert("default_domain_resolver".to_string(), json!("dns-direct"));
    }

    let final_dns = if final_target == "direct" {
        "dns-direct"
    } else {
        "dns-remote"
    };

    let remote_dns_host = routing
        .remote_dns
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("udp://")
        .split('/')
        .next()
        .unwrap_or("1.1.1.1")
        .to_string();

    let direct_dns_host = routing
        .direct_dns
        .trim()
        .trim_start_matches("udp://")
        .split('/')
        .next()
        .unwrap_or("77.88.8.8")
        .to_string();

    let is_ssh = profile.protocol == "ssh";
    let remote_dns_type = if is_ssh { "tcp" } else { "udp" };

    let mut cfg = json!({
        "log": {
            "level": "info",
            "timestamp": true
        },
        "dns": {
            "servers": [
                {
                    "type": remote_dns_type,
                    "tag": "dns-remote",
                    "server": remote_dns_host,
                    "detour": "proxy"
                },
                {
                    "type": "udp",
                    "tag": "dns-direct",
                    "server": direct_dns_host
                }
            ],
            "rules": dns_rules,
            "final": final_dns,
            "strategy": "ipv4_only"
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": Value::Object(route_obj),
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", routing.clash_api_port)
            }
        }
    });

    if !endpoints.is_empty() {
        cfg["endpoints"] = json!(endpoints);
    }
    cfg
}

pub fn generate_mobile_singbox_config(profile: &VpnProfile, routing: &RoutingConfig) -> Value {
    let mut cfg = generate_singbox_config_internal(profile, routing, None, true);
    if let Some(obj) = cfg.as_object_mut() {
        obj.remove("experimental");
        if let Some(outbounds) = obj.get_mut("outbounds").and_then(|v| v.as_array_mut()) {
            for ob in outbounds.iter_mut() {
                if let Some(ob_obj) = ob.as_object_mut() {
                    ob_obj.remove("inet4_bind_address");
                }
            }
        }
        if let Some(route) = obj.get_mut("route").and_then(|v| v.as_object_mut()) {
            route.insert("override_android_vpn".to_string(), json!(true));
            route.insert("auto_detect_interface".to_string(), json!(true));
        }
        if let Some(inbounds) = obj.get_mut("inbounds").and_then(|v| v.as_array_mut()) {
            let mut tun_map = serde_json::Map::new();
            tun_map.insert("type".to_string(), json!("tun"));
            tun_map.insert("tag".to_string(), json!("tun-in"));
            tun_map.insert(
                "address".to_string(),
                json!(["172.19.0.1/28", "fdfe:dcba:9876::1/126"]),
            );
            tun_map.insert("mtu".to_string(), json!(9000));
            tun_map.insert("auto_route".to_string(), json!(true));
            tun_map.insert("strict_route".to_string(), json!(false));
            tun_map.insert("stack".to_string(), json!("mixed"));
            if profile.server.parse::<std::net::Ipv4Addr>().is_ok() {
                tun_map.insert(
                    "route_exclude_address".to_string(),
                    json!([format!("{}/32", profile.server)]),
                );
            }
            *inbounds = vec![Value::Object(tun_map)];
        }
    }
    cfg
}

pub fn build_profile_uri(profile: &VpnProfile) -> String {
    let encoded_name = urlencoding::encode(&profile.name).replace('+', "%20");
    let insecure_val = if profile.insecure { "1" } else { "0" };
    if profile.protocol == "vless" {
        let uuid = profile.uuid.clone().unwrap_or_else(|| profile.password.clone());
        let pbk = profile.reality_public_key.as_deref().unwrap_or_default();
        let sid = profile.reality_short_id.as_deref().unwrap_or_default();
        let flow = profile.reality_flow.as_deref().unwrap_or("xtls-rprx-vision");
        format!(
            "vless://{}@{}:{}?security=reality&sni={}&fp=chrome&pbk={}&sid={}&type=tcp&flow={}#{}",
            urlencoding::encode(&uuid),
            profile.server,
            profile.port,
            urlencoding::encode(&profile.sni),
            urlencoding::encode(pbk),
            urlencoding::encode(sid),
            urlencoding::encode(flow),
            encoded_name
        )
    } else if profile.protocol == "tuic" {
        let uuid = profile.uuid.clone().unwrap_or_default();
        format!(
            "tuic://{}:{}@{}:{}/?congestion_control=bbr&alpn=h3&sni={}&udp_relay_mode=native&allow_insecure={}&insecure={}&allowInsecure={}#{}",
            urlencoding::encode(&uuid),
            urlencoding::encode(&profile.password),
            profile.server,
            profile.port,
            urlencoding::encode(&profile.sni),
            insecure_val,
            insecure_val,
            insecure_val,
            encoded_name
        )
    } else if profile.protocol == "shadowsocks" {
        use base64::Engine;
        let method = profile.ss_method.as_deref().unwrap_or("2022-blake3-aes-128-gcm");
        let raw_userinfo = format!("{}:{}", method, profile.password);
        let b64_userinfo = base64::engine::general_purpose::STANDARD.encode(raw_userinfo.as_bytes());
        let mut query = String::new();
        if let Some(ref plugin) = profile.ss_plugin {
            let p_trim = plugin.trim();
            if !p_trim.is_empty() {
                let mut p_val = p_trim.to_string();
                if let Some(ref opts) = profile.ss_plugin_opts {
                    let o_trim = opts.trim();
                    if !o_trim.is_empty() {
                        p_val.push(';');
                        p_val.push_str(o_trim);
                    }
                }
                query = format!("?plugin={}", urlencoding::encode(&p_val));
            }
        }
        format!(
            "ss://{}@{}:{}{}#{}",
            b64_userinfo,
            profile.server,
            profile.port,
            query,
            encoded_name
        )
    } else if is_awg_protocol(&profile.protocol) {
        let priv_key = profile.awg_private_key.as_deref().unwrap_or(&profile.password);
        let pub_key = profile.awg_public_key.as_deref().unwrap_or_default();
        let addr = profile.awg_local_address.as_deref().unwrap_or("10.8.0.2/32");
        let jc = profile.awg_jc.unwrap_or(4);
        let jmin = profile.awg_jmin.unwrap_or(40);
        let jmax = profile.awg_jmax.unwrap_or(70);
        let s1 = profile.awg_s1.unwrap_or(15);
        let s2 = profile.awg_s2.unwrap_or(25);
        let h1 = profile.awg_h1.as_deref().unwrap_or("1");
        let h2 = profile.awg_h2.as_deref().unwrap_or("2");
        let h3 = profile.awg_h3.as_deref().unwrap_or("3");
        let h4 = profile.awg_h4.as_deref().unwrap_or("4");
        let hop = profile.awg_hop_ports.as_deref().unwrap_or_default();
        let psk = profile.awg_preshared_key.as_deref().unwrap_or_default();

        let mut params = format!(
            "public_key={}&address={}&jc={}&jmin={}&jmax={}&s1={}&s2={}&h1={}&h2={}&h3={}&h4={}",
            urlencoding::encode(pub_key),
            urlencoding::encode(addr),
            jc, jmin, jmax, s1, s2,
            urlencoding::encode(h1),
            urlencoding::encode(h2),
            urlencoding::encode(h3),
            urlencoding::encode(h4)
        );
        if let Some(s3) = profile.awg_s3 {
            params.push_str(&format!("&s3={}", s3));
        }
        if let Some(s4) = profile.awg_s4 {
            params.push_str(&format!("&s4={}", s4));
        }
        if let Some(ref hpk) = profile.awg_header_protection_key {
            if !hpk.trim().is_empty() {
                params.push_str(&format!("&header_protection_key={}", urlencoding::encode(hpk.trim())));
            }
        }
        if let Some(ref r) = profile.awg_rekey_after_time {
            if !r.trim().is_empty() {
                params.push_str(&format!("&rekey_after_time={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(ref r) = profile.awg_rekey_timeout {
            if !r.trim().is_empty() {
                params.push_str(&format!("&rekey_timeout={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(ref r) = profile.awg_reject_after_time {
            if !r.trim().is_empty() {
                params.push_str(&format!("&reject_after_time={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(ref r) = profile.awg_keepalive_timeout {
            if !r.trim().is_empty() {
                params.push_str(&format!("&keepalive_timeout={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(ref r) = profile.awg_max_handshake_attempts {
            if !r.trim().is_empty() {
                params.push_str(&format!("&max_handshake_attempts={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(ref r) = profile.awg_persistent_keepalive {
            if !r.trim().is_empty() {
                params.push_str(&format!("&persistent_keepalive={}", urlencoding::encode(r.trim())));
            }
        }
        if let Some(rt) = profile.awg_random_trailers {
            params.push_str(&format!("&random_trailers={}", if rt { "on" } else { "off" }));
        }
        if let Some(dc) = profile.awg_disable_cookies {
            params.push_str(&format!("&disable_cookies={}", if dc { "on" } else { "off" }));
        }
        if let Some(ref v) = profile.awg_version {
            params.push_str(&format!("&version={}", urlencoding::encode(v.trim())));
        }
        if let Some(ref i1) = profile.awg_i1 {
            params.push_str(&format!("&i1={}", urlencoding::encode(i1)));
        }
        if let Some(ref i2) = profile.awg_i2 {
            params.push_str(&format!("&i2={}", urlencoding::encode(i2)));
        }
        if let Some(ref i3) = profile.awg_i3 {
            params.push_str(&format!("&i3={}", urlencoding::encode(i3)));
        }
        if let Some(ref i4) = profile.awg_i4 {
            params.push_str(&format!("&i4={}", urlencoding::encode(i4)));
        }
        if !hop.is_empty() {
            params.push_str(&format!("&hop_ports={}", urlencoding::encode(hop)));
        }
        if !psk.is_empty() {
            params.push_str(&format!("&psk={}", urlencoding::encode(psk)));
        }

        let scheme = if profile.protocol == "awg2" || profile.awg_version.as_deref() == Some("2.0") {
            "awg2"
        } else {
            "awg"
        };

        format!(
            "{}://{}@{}:{}?{}#{}",
            scheme,
            urlencoding::encode(priv_key),
            profile.server,
            profile.port,
            params,
            encoded_name
        )
    } else if profile.protocol == "ssh" {
        let user = if profile.sni.trim().is_empty() { "root" } else { profile.sni.trim() };
        format!(
            "ssh://{}:{}@{}:{}#{}",
            urlencoding::encode(user),
            urlencoding::encode(&profile.password),
            profile.server,
            profile.port,
            encoded_name
        )
    } else {
        let mut query = format!(
            "insecure={}&allow_insecure={}&allowInsecure={}&sni={}&alpn=h3",
            insecure_val,
            insecure_val,
            insecure_val,
            urlencoding::encode(&profile.sni)
        );
        if let Some(ref obfs) = profile.obfs_password {
            if !obfs.is_empty() {
                let encoded_obfs = urlencoding::encode(obfs);
                query.push_str(&format!(
                    "&obfs=salamander&obfs-password={}&obfspassword={}",
                    encoded_obfs, encoded_obfs
                ));
            }
        }
        if let Some(ref hp) = profile.hop_ports {
            if !hp.trim().is_empty() {
                let encoded_hp = urlencoding::encode(hp.trim());
                query.push_str(&format!("&mport={}&ports={}", encoded_hp, encoded_hp));
                if let Some(ref hi) = profile.hop_interval {
                    query.push_str(&format!("&hop_interval={}", urlencoding::encode(hi.trim())));
                }
            }
        }
        format!(
            "hysteria2://{}@{}:{}/?{}#{}",
            urlencoding::encode(&profile.password),
            profile.server,
            profile.port,
            query,
            encoded_name
        )
    }
}

pub fn build_awg_conf(profile: &VpnProfile) -> String {
    let priv_key = profile.awg_private_key.as_deref().unwrap_or(&profile.password);
    let pub_key = profile.awg_public_key.as_deref().unwrap_or_default();
    let addr = profile.awg_local_address.as_deref().unwrap_or("10.8.0.2/32");
    let jc = profile.awg_jc.unwrap_or(4);
    let jmin = profile.awg_jmin.unwrap_or(40);
    let jmax = profile.awg_jmax.unwrap_or(70);
    let s1 = profile.awg_s1.unwrap_or(15);
    let s2 = profile.awg_s2.unwrap_or(25);
    let h1 = profile.awg_h1.as_deref().unwrap_or("1");
    let h2 = profile.awg_h2.as_deref().unwrap_or("2");
    let h3 = profile.awg_h3.as_deref().unwrap_or("3");
    let h4 = profile.awg_h4.as_deref().unwrap_or("4");

    let is_v31 = profile.awg_header_protection_key.is_some()
        || profile.awg_version.as_deref() == Some("3.1")
        || (profile.protocol == "amneziawg" && profile.awg_version.as_deref() != Some("2.0"));

    let mut conf = String::new();
    conf.push_str("[Interface]\n");
    conf.push_str(&format!("Address = {}\n", addr));
    conf.push_str("DNS = 1.1.1.1, 1.0.0.1\n");
    conf.push_str(&format!("PrivateKey = {}\n", priv_key));
    conf.push_str(&format!("Jc = {}\n", jc));
    conf.push_str(&format!("Jmin = {}\n", jmin));
    conf.push_str(&format!("Jmax = {}\n", jmax));
    conf.push_str(&format!("S1 = {}\n", if is_v31 && s1 < 12 { 12 } else { s1 }));
    conf.push_str(&format!("S2 = {}\n", if is_v31 && s2 < 12 { 12 } else { s2 }));

    if is_v31 {
        let s3 = profile.awg_s3.unwrap_or(12).max(12);
        let s4 = profile.awg_s4.unwrap_or(12).max(12);
        conf.push_str(&format!("S3 = {}\n", s3));
        conf.push_str(&format!("S4 = {}\n", s4));
    } else {
        if let Some(s3) = profile.awg_s3 {
            conf.push_str(&format!("S3 = {}\n", s3));
        }
        if let Some(s4) = profile.awg_s4 {
            conf.push_str(&format!("S4 = {}\n", s4));
        }
    }

    conf.push_str(&format!("H1 = {}\n", h1));
    conf.push_str(&format!("H2 = {}\n", h2));
    conf.push_str(&format!("H3 = {}\n", h3));
    conf.push_str(&format!("H4 = {}\n", h4));

    if is_v31 {
        if let Some(ref hpk) = profile.awg_header_protection_key {
            conf.push_str(&format!("HeaderProtectionKey = {}\n", hpk.trim()));
        }
        let rekey = profile.awg_rekey_after_time.as_deref().unwrap_or("100-120");
        conf.push_str(&format!("RekeyAfterTime = {}\n", rekey));

        let rekey_to = profile.awg_rekey_timeout.as_deref().unwrap_or("3-7");
        conf.push_str(&format!("RekeyTimeout = {}\n", rekey_to));

        let reject = profile.awg_reject_after_time.as_deref().unwrap_or("150-180");
        conf.push_str(&format!("RejectAfterTime = {}\n", reject));

        let keepalive_to = profile.awg_keepalive_timeout.as_deref().unwrap_or("5-15");
        conf.push_str(&format!("KeepaliveTimeout = {}\n", keepalive_to));

        let max_hs = profile.awg_max_handshake_attempts.as_deref().unwrap_or("15-20");
        conf.push_str(&format!("MaxHandshakeAttempts = {}\n", max_hs));

        let random_trailers = if profile.awg_random_trailers.unwrap_or(true) { "on" } else { "off" };
        conf.push_str(&format!("RandomTrailers = {}\n", random_trailers));

        let disable_cookies = if profile.awg_disable_cookies.unwrap_or(true) { "on" } else { "off" };
        conf.push_str(&format!("DisableCookies = {}\n", disable_cookies));
    }

    if let Some(ref i1) = profile.awg_i1 {
        if !i1.trim().is_empty() { conf.push_str(&format!("I1 = {}\n", i1.trim())); }
    }
    if let Some(ref i2) = profile.awg_i2 {
        if !i2.trim().is_empty() { conf.push_str(&format!("I2 = {}\n", i2.trim())); }
    }
    if let Some(ref i3) = profile.awg_i3 {
        if !i3.trim().is_empty() { conf.push_str(&format!("I3 = {}\n", i3.trim())); }
    }
    if let Some(ref i4) = profile.awg_i4 {
        if !i4.trim().is_empty() { conf.push_str(&format!("I4 = {}\n", i4.trim())); }
    }

    conf.push_str("\n[Peer]\n");
    conf.push_str(&format!("PublicKey = {}\n", pub_key));
    if let Some(ref psk) = profile.awg_preshared_key {
        if !psk.trim().is_empty() {
            conf.push_str(&format!("PresharedKey = {}\n", psk.trim()));
        }
    }
    conf.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
    conf.push_str(&format!("Endpoint = {}:{}\n", profile.server, profile.port));

    let pk = profile.awg_persistent_keepalive.as_deref().unwrap_or_else(|| {
        if is_v31 { "25-35" } else { "25" }
    });
    conf.push_str(&format!("PersistentKeepalive = {}\n", pk));

    conf
}

pub fn parse_profile_uri(uri_str: &str) -> Result<VpnProfile, String> {
    let trimmed = uri_str.trim();

    // Support direct JSON import (either VpnProfile JSON or exported sing-box config JSON)
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(mut prof) = serde_json::from_str::<VpnProfile>(trimmed) {
            prof.id = Uuid::new_v4().to_string();
            prof.uri = build_profile_uri(&prof);
            return Ok(prof);
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if let Some(outbounds) = val["outbounds"].as_array() {
                for out in outbounds {
                    let out_type = out["type"].as_str().unwrap_or("");
                    if out_type == "vless" || out_type == "hysteria2" || out_type == "tuic" || out_type == "shadowsocks" || out_type == "ssh" {
                        let server = out["server"].as_str().unwrap_or("").to_string();
                        if server.is_empty() {
                            continue;
                        }
                        let default_port = match out_type {
                            "shadowsocks" => 8388,
                            "ssh" => 22,
                            _ => 443,
                        };
                        let port = out["server_port"].as_u64().unwrap_or(default_port) as u16;
                        let password = out["password"].as_str().or_else(|| out["private_key"].as_str()).unwrap_or("").to_string();
                        let sni = if out_type == "ssh" {
                            out["user"].as_str().unwrap_or("root").to_string()
                        } else {
                            out["tls"]["server_name"].as_str().unwrap_or("www.bing.com").to_string()
                        };
                        let insecure = out["tls"]["insecure"].as_bool().unwrap_or(out_type != "vless" && out_type != "ssh");
                        let uuid = out["uuid"].as_str().map(|s| s.to_string());
                        let obfs_password = out["obfs"]["password"].as_str().map(|s| s.to_string());
                        let reality_public_key = out["tls"]["reality"]["public_key"].as_str().map(|s| s.to_string());
                        let reality_short_id = out["tls"]["reality"]["short_id"].as_str().or_else(|| {
                            out["tls"]["reality"]["short_id"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str())
                        }).map(|s| s.to_string());
                        let reality_flow = out["flow"].as_str().map(|s| s.to_string());
                        let ss_method = out["method"].as_str().map(|s| s.to_string());
                        let ss_plugin = out["plugin"].as_str().map(|s| s.to_string());
                        let ss_plugin_opts = out["plugin_opts"].as_str().map(|s| s.to_string());
                        let name = out["tag"].as_str().filter(|t| !t.is_empty() && *t != "proxy").unwrap_or(&format!("{}-import", out_type)).to_string();
                        let mut prof = VpnProfile {
                            id: Uuid::new_v4().to_string(),
                            name,
                            protocol: out_type.to_string(),
                            server,
                            port,
                            sni,
                            insecure,
                            password: if out_type == "vless" { uuid.clone().unwrap_or_default() } else { password },
                            uuid,
                            obfs_password,
                            up_mbps: 100,
                            down_mbps: 100,
                            uri: String::new(),
                            ping_ms: None,
                            routing: None,
                            hop_ports: out["server_ports"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            hop_interval: out["hop_interval"].as_str().map(|s| s.to_string()),
                            reality_public_key,
                            reality_short_id,
                            reality_flow,
                            ss_method,
                            ss_plugin,
                            ss_plugin_opts,
                            ..Default::default()
                        };
                        prof.uri = build_profile_uri(&prof);
                        return Ok(prof);
                    }
                }
            }
            if let Some(endpoints) = val["endpoints"].as_array() {
                for ep in endpoints {
                    let ep_type = ep["type"].as_str().unwrap_or("");
                    if ep_type == "wireguard" {
                        let peer = ep["peers"].as_array().and_then(|a| a.first());
                        let server = peer.and_then(|p| p["address"].as_str()).unwrap_or("").to_string();
                        if server.is_empty() {
                            continue;
                        }
                        let port = peer.and_then(|p| p["port"].as_u64()).unwrap_or(51820) as u16;
                        let pub_key = peer.and_then(|p| p["public_key"].as_str()).map(|s| s.to_string());
                        let psk = peer.and_then(|p| p["pre_shared_key"].as_str()).map(|s| s.to_string());
                        let priv_key = ep["private_key"].as_str().unwrap_or("").to_string();
                        let local_addr = ep["address"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).map(|s| s.to_string());
                        let jc = ep["jc"].as_u64().map(|n| n as u16);
                        let jmin = ep["jmin"].as_u64().map(|n| n as u16);
                        let jmax = ep["jmax"].as_u64().map(|n| n as u16);
                        let s1 = ep["s1"].as_u64().map(|n| n as u16);
                        let s2 = ep["s2"].as_u64().map(|n| n as u16);
                        let h1 = ep["h1"].as_str().map(|s| s.to_string()).or_else(|| ep["h1"].as_u64().map(|n| n.to_string()));
                        let h2 = ep["h2"].as_str().map(|s| s.to_string()).or_else(|| ep["h2"].as_u64().map(|n| n.to_string()));
                        let h3 = ep["h3"].as_str().map(|s| s.to_string()).or_else(|| ep["h3"].as_u64().map(|n| n.to_string()));
                        let h4 = ep["h4"].as_str().map(|s| s.to_string()).or_else(|| ep["h4"].as_u64().map(|n| n.to_string()));
                        let name = ep["tag"].as_str().filter(|t| !t.is_empty() && *t != "proxy").unwrap_or("AmneziaWG-import").to_string();
                        let mut prof = VpnProfile {
                            id: Uuid::new_v4().to_string(),
                            name,
                            protocol: "amneziawg".to_string(),
                            server,
                            port,
                            sni: String::new(),
                            insecure: false,
                            password: priv_key.clone(),
                            uuid: None,
                            obfs_password: None,
                            hop_ports: Some("20000:50000".to_string()),
                            awg_private_key: Some(priv_key),
                            awg_public_key: pub_key,
                            awg_preshared_key: psk,
                            awg_local_address: local_addr.or_else(|| Some("10.8.0.2/32".to_string())),
                            awg_jc: jc.or(Some(4)),
                            awg_jmin: jmin.or(Some(40)),
                            awg_jmax: jmax.or(Some(70)),
                            awg_s1: s1.or(Some(15)),
                            awg_s2: s2.or(Some(25)),
                            awg_s3: ep["s3"].as_u64().map(|n| n as u16).or(Some(35)),
                            awg_s4: ep["s4"].as_u64().map(|n| n as u16).or(Some(45)),
                            awg_h1: h1.or_else(|| Some("1".to_string())),
                            awg_h2: h2.or_else(|| Some("2".to_string())),
                            awg_h3: h3.or_else(|| Some("3".to_string())),
                            awg_h4: h4.or_else(|| Some("4".to_string())),
                            awg_hop_ports: Some("20000:50000".to_string()),
                            awg_port_probing: Some(true),
                            up_mbps: 100,
                            down_mbps: 100,
                            ..Default::default()
                        };
                        prof.uri = build_profile_uri(&prof);
                        return Ok(prof);
                    }
                }
            }
        }
    }

    if trimmed.contains("[Interface]") && trimmed.contains("[Peer]") {
        return parse_amnezia_conf(trimmed);
    }

    if trimmed.to_lowercase().starts_with("awg://") || trimmed.to_lowercase().starts_with("amneziawg://") || trimmed.to_lowercase().starts_with("awg2://") {
        return parse_awg_uri(trimmed);
    }

    if trimmed.to_lowercase().starts_with("ss://") || trimmed.to_lowercase().starts_with("shadowsocks://") {
        return parse_shadowsocks_uri(trimmed);
    }

    let parsed = Url::parse(trimmed).map_err(|e| format!("Некорректный формат URI: {}", e))?;
    let scheme = parsed.scheme().to_lowercase();

    let server = parsed
        .host_str()
        .ok_or_else(|| "В ссылке не указан адрес сервера".to_string())?
        .to_string();
    let port = parsed.port().unwrap_or(443);
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_else(|_| f.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{} ({})", server, scheme.to_uppercase()));

    let mut sni = "www.bing.com".to_string();
    let mut insecure = true;
    let mut obfs_password = None;
    let mut hop_ports = None;
    let mut hop_interval = None;
    let mut reality_public_key = None;
    let mut reality_short_id = None;
    let mut reality_flow = Some("xtls-rprx-vision".to_string());

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" => sni = v.to_string(),
            "insecure" | "allow_insecure" | "allowInsecure" => {
                insecure = v == "1" || v.eq_ignore_ascii_case("true");
            }
            "obfs-password" => {
                if !v.is_empty() {
                    obfs_password = Some(v.to_string());
                }
            }
            "mport" | "ports" | "hop_ports" => {
                if !v.is_empty() {
                    hop_ports = Some(v.to_string());
                }
            }
            "hop_interval" => {
                if !v.is_empty() {
                    hop_interval = Some(v.to_string());
                }
            }
            "pbk" | "public_key" | "reality_public_key" => {
                if !v.is_empty() {
                    reality_public_key = Some(v.to_string());
                }
            }
            "sid" | "short_id" | "reality_short_id" => {
                if !v.is_empty() {
                    reality_short_id = Some(v.to_string());
                }
            }
            "flow" => {
                if !v.is_empty() {
                    reality_flow = Some(v.to_string());
                }
            }
            _ => {}
        }
    }

    if scheme == "vless" {
        let uuid = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        let mut prof = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: "vless".to_string(),
            server,
            port,
            sni,
            insecure: false,
            password: uuid.clone(),
            uuid: Some(uuid),
            reality_public_key,
            reality_short_id,
            reality_flow,
            ..Default::default()
        };
        prof.uri = build_profile_uri(&prof);
        Ok(prof)
    } else if scheme == "hy2" || scheme == "hysteria2" {
        let password = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        let mut prof = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: "hysteria2".to_string(),
            server,
            port,
            sni,
            insecure,
            password,
            obfs_password,
            hop_ports,
            hop_interval,
            ..Default::default()
        };
        prof.uri = build_profile_uri(&prof);
        Ok(prof)
    } else if scheme == "tuic" {
        let uuid = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        let password = parsed
            .password()
            .map(|p| urlencoding::decode(p).unwrap_or_else(|_| p.to_string()))
            .unwrap_or_default();
        let mut prof = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: "tuic".to_string(),
            server,
            port,
            sni,
            insecure,
            password,
            uuid: Some(uuid),
            ..Default::default()
        };
        prof.uri = build_profile_uri(&prof);
        Ok(prof)
    } else if scheme == "ssh" {
        let user = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        let password = parsed
            .password()
            .map(|p| urlencoding::decode(p).unwrap_or_else(|_| p.to_string()))
            .unwrap_or_default();
        let mut prof = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: "ssh".to_string(),
            server,
            port: if port == 443 { 22 } else { port },
            sni: if user.is_empty() { "root".to_string() } else { user },
            insecure: false,
            password,
            up_mbps: 100,
            down_mbps: 100,
            uri: String::new(),
            ping_ms: None,
            routing: None,
            tls_fragment: Some(false),
            ..Default::default()
        };
        prof.uri = build_profile_uri(&prof);
        Ok(prof)
    } else {
        Err(format!(
            "Неподдерживаемый протокол '{}'. Поддерживаются ssh://, awg://, vless://, ss://, hy2://, hysteria2:// и tuic://",
            scheme
        ))
    }
}

fn decode_b64_str(input: &str) -> Option<String> {
    use base64::Engine;
    let clean = input.trim().replace('-', "+").replace('_', "/");
    let padded = match clean.len() % 4 {
        2 => format!("{}==", clean),
        3 => format!("{}=", clean),
        _ => clean,
    };
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&padded) {
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
    }
    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE.decode(input.trim()) {
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
    }
    None
}

fn parse_host_port(hp: &str, default_port: u16) -> Result<(String, u16), String> {
    let clean = hp.trim().trim_end_matches('/');
    if let Some(bracket_end) = clean.rfind(']') {
        let host = &clean[1..bracket_end];
        let port = if let Some(colon) = clean[bracket_end..].find(':') {
            clean[bracket_end + colon + 1..].parse::<u16>().unwrap_or(default_port)
        } else {
            default_port
        };
        Ok((host.to_string(), port))
    } else if let Some((h, p)) = clean.rsplit_once(':') {
        let port = p.parse::<u16>().map_err(|_| format!("Некорректный порт: {}", p))?;
        Ok((h.to_string(), port))
    } else {
        Ok((clean.to_string(), default_port))
    }
}

fn parse_shadowsocks_uri(uri_str: &str) -> Result<VpnProfile, String> {
    let trimmed = uri_str.trim();
    let prefix = if trimmed.to_lowercase().starts_with("shadowsocks://") {
        "shadowsocks://"
    } else if trimmed.to_lowercase().starts_with("ss://") {
        "ss://"
    } else {
        return Err("Неверная схема Shadowsocks ссылки".to_string());
    };

    let without_scheme = &trimmed[prefix.len()..];
    let (body, fragment) = match without_scheme.find('#') {
        Some(idx) => (&without_scheme[..idx], Some(&without_scheme[idx + 1..])),
        None => (without_scheme, None),
    };

    let (user_host_part, query_part) = match body.find('?') {
        Some(idx) => (&body[..idx], Some(&body[idx + 1..])),
        None => (body, None),
    };

    let mut plugin: Option<String> = None;
    let mut plugin_opts: Option<String> = None;
    if let Some(q) = query_part {
        for (k, v) in url::form_urlencoded::parse(q.as_bytes()) {
            if k == "plugin" {
                let decoded_plugin = v.to_string();
                if let Some((p_name, p_opts)) = decoded_plugin.split_once(';') {
                    plugin = Some(p_name.to_string());
                    plugin_opts = Some(p_opts.to_string());
                } else {
                    plugin = Some(decoded_plugin);
                }
            }
        }
    }

    let (method, password, server, port) = if user_host_part.contains('@') {
        // Standard SIP002 format: [userinfo]@[server]:[port]
        let (userinfo, host_port) = user_host_part.split_once('@').unwrap();
        let (m, p) = if let Some(decoded) = decode_b64_str(userinfo) {
            if let Some((m, p)) = decoded.split_once(':') {
                (m.to_string(), p.to_string())
            } else {
                ("2022-blake3-aes-128-gcm".to_string(), decoded)
            }
        } else if let Some((m, p)) = userinfo.split_once(':') {
            (
                urlencoding::decode(m).unwrap_or_else(|_| m.to_string()),
                urlencoding::decode(p).unwrap_or_else(|_| p.to_string()),
            )
        } else {
            return Err("Не удалось распознать метод шифрования и пароль Shadowsocks".to_string());
        };

        let (srv, prt) = parse_host_port(host_port, 8388)?;
        (m, p, srv, prt)
    } else {
        // Legacy format: BASE64(method:password@server:port)
        if let Some(decoded) = decode_b64_str(user_host_part) {
            if let Some((userinfo, host_port)) = decoded.split_once('@') {
                let (m, p) = if let Some((m, p)) = userinfo.split_once(':') {
                    (m.to_string(), p.to_string())
                } else {
                    ("2022-blake3-aes-128-gcm".to_string(), userinfo.to_string())
                };
                let (srv, prt) = parse_host_port(host_port, 8388)?;
                (m, p, srv, prt)
            } else {
                return Err("Некорректная Shadowsocks ссылка (отсутствует разделитель @)".to_string());
            }
        } else {
            return Err("Не удалось декодировать Shadowsocks ссылку".to_string());
        }
    };

    let name = fragment
        .map(|f| urlencoding::decode(f).unwrap_or_else(|_| f.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{} (SS)", server, port));

    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol: "shadowsocks".to_string(),
        server: server.clone(),
        port,
        sni: "www.bing.com".to_string(),
        insecure: false,
        password: password.clone(),
        uuid: None,
        obfs_password: None,
        up_mbps: 100,
        down_mbps: 100,
        uri: String::new(),
        ping_ms: None,
        routing: None,
        hop_ports: None,
        hop_interval: None,
        reality_public_key: None,
        reality_short_id: None,
        reality_flow: None,
        ss_method: Some(method),
        ss_plugin: plugin,
        ss_plugin_opts: plugin_opts,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Ok(prof)
}

pub fn parse_awg_uri(uri_str: &str) -> Result<VpnProfile, String> {
    let raw = uri_str.trim();
    let is_explicit_awg2 = raw.to_lowercase().starts_with("awg2://");
    let without_scheme = if let Some(s) = raw.strip_prefix("awg://") {
        s
    } else if let Some(s) = raw.strip_prefix("amneziawg://") {
        s
    } else if let Some(s) = raw.strip_prefix("awg2://") {
        s
    } else {
        raw
    };

    let (body, fragment) = match without_scheme.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (without_scheme, None),
    };

    let name = fragment
        .map(|f| urlencoding::decode(f).unwrap_or_else(|_| f.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_explicit_awg2 {
                "AmneziaWG 2.0 Profile".to_string()
            } else {
                "Amnezia 3.1 Profile".to_string()
            }
        });

    let (user_host, query) = match body.split_once('?') {
        Some((uh, q)) => (uh, Some(q)),
        None => (body, None),
    };

    let (priv_key_raw, host_port) = match user_host.split_once('@') {
        Some((u, hp)) => (u, hp),
        None => return Err("Некорректная awg:// ссылка: отсутствует разделитель @".to_string()),
    };

    let priv_key = urlencoding::decode(priv_key_raw).unwrap_or_else(|_| priv_key_raw.to_string());
    let (server, port) = parse_host_port(host_port, 51820)?;

    let mut pub_key = None;
    let mut psk = None;
    let mut addr = Some("10.8.0.2/32".to_string());
    let mut jc = Some(4);
    let mut jmin = Some(40);
    let mut jmax = Some(70);
    let mut s1 = Some(15);
    let mut s2 = Some(25);
    let mut s3 = Some(35);
    let mut s4 = Some(45);
    let mut h1 = Some("1".to_string());
    let mut h2 = Some("2".to_string());
    let mut h3 = Some("3".to_string());
    let mut h4 = Some("4".to_string());
    let mut i1 = None;
    let mut i2 = None;
    let mut i3 = None;
    let mut i4 = None;
    let mut hop_ports = Some("20000:50000".to_string());
    let mut header_protection_key = None;
    let mut rekey_after_time = None;
    let mut rekey_timeout = None;
    let mut reject_after_time = None;
    let mut keepalive_timeout = None;
    let mut max_handshake_attempts = None;
    let mut random_trailers = None;
    let mut disable_cookies = None;
    let mut persistent_keepalive = None;
    let mut parsed_version = None;

    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                let dec_v = urlencoding::decode(v).unwrap_or_else(|_| v.to_string());
                match k.to_lowercase().as_str() {
                    "public_key" | "pubkey" | "publickey" | "peer" => pub_key = Some(dec_v),
                    "psk" | "preshared_key" | "presharedkey" => psk = Some(dec_v),
                    "address" | "ip" => addr = Some(dec_v),
                    "jc" => jc = dec_v.parse().ok(),
                    "jmin" => jmin = dec_v.parse().ok(),
                    "jmax" => jmax = dec_v.parse().ok(),
                    "s1" => s1 = dec_v.parse().ok(),
                    "s2" => s2 = dec_v.parse().ok(),
                    "s3" => s3 = dec_v.parse().ok(),
                    "s4" => s4 = dec_v.parse().ok(),
                    "h1" => h1 = Some(dec_v),
                    "h2" => h2 = Some(dec_v),
                    "h3" => h3 = Some(dec_v),
                    "h4" => h4 = Some(dec_v),
                    "header_protection_key" | "headerprotectionkey" | "hpk" => header_protection_key = Some(dec_v),
                    "rekey_after_time" | "rekeyaftertime" => rekey_after_time = Some(dec_v),
                    "rekey_timeout" | "rekeytimeout" => rekey_timeout = Some(dec_v),
                    "reject_after_time" | "rejectaftertime" => reject_after_time = Some(dec_v),
                    "keepalive_timeout" | "keepalivetimeout" => keepalive_timeout = Some(dec_v),
                    "max_handshake_attempts" | "maxhandshakeattempts" => max_handshake_attempts = Some(dec_v),
                    "random_trailers" | "randomtrailers" => random_trailers = Some(dec_v.eq_ignore_ascii_case("on") || dec_v == "1" || dec_v.eq_ignore_ascii_case("true")),
                    "disable_cookies" | "disablecookies" => disable_cookies = Some(dec_v.eq_ignore_ascii_case("on") || dec_v == "1" || dec_v.eq_ignore_ascii_case("true")),
                    "persistent_keepalive" | "persistentkeepalive" => persistent_keepalive = Some(dec_v),
                    "version" | "ver" => parsed_version = Some(dec_v),
                    "i1" => i1 = Some(dec_v),
                    "i2" => i2 = Some(dec_v),
                    "i3" => i3 = Some(dec_v),
                    "i4" => i4 = Some(dec_v),
                    "hop_ports" | "mport" | "ports" => hop_ports = Some(dec_v),
                    _ => {}
                }
            }
        }
    }

    let is_v2 = is_explicit_awg2 || parsed_version.as_deref() == Some("2.0") || parsed_version.as_deref() == Some("2");
    let protocol = if is_v2 { "awg2".to_string() } else { "amneziawg".to_string() };
    let version_str = if is_v2 { "2.0".to_string() } else { "3.1".to_string() };

    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol,
        server,
        port,
        sni: String::new(),
        insecure: false,
        password: priv_key.clone(),
        uuid: None,
        obfs_password: None,
        hop_ports: hop_ports.clone(),
        hop_interval: None,
        reality_public_key: None,
        reality_short_id: None,
        reality_flow: None,
        ss_method: None,
        ss_plugin: None,
        ss_plugin_opts: None,
        awg_private_key: Some(priv_key),
        awg_public_key: pub_key,
        awg_preshared_key: psk,
        awg_local_address: addr,
        awg_jc: jc,
        awg_jmin: jmin,
        awg_jmax: jmax,
        awg_s1: s1,
        awg_s2: s2,
        awg_s3: s3,
        awg_s4: s4,
        awg_h1: h1,
        awg_h2: h2,
        awg_h3: h3,
        awg_h4: h4,
        awg_i1: i1,
        awg_i2: i2,
        awg_i3: i3,
        awg_i4: i4,
        awg_hop_ports: hop_ports,
        awg_port_probing: Some(true),
        awg_header_protection_key: header_protection_key,
        awg_rekey_after_time: rekey_after_time,
        awg_rekey_timeout: rekey_timeout,
        awg_reject_after_time: reject_after_time,
        awg_keepalive_timeout: keepalive_timeout,
        awg_max_handshake_attempts: max_handshake_attempts,
        awg_random_trailers: random_trailers,
        awg_disable_cookies: disable_cookies,
        awg_persistent_keepalive: persistent_keepalive,
        awg_version: Some(version_str),
        up_mbps: 100,
        down_mbps: 100,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Ok(prof)
}

pub fn parse_amnezia_conf(text: &str) -> Result<VpnProfile, String> {
    let mut priv_key = String::new();
    let mut local_addr = None;
    let mut pub_key = None;
    let mut psk = None;
    let mut server = String::new();
    let mut port = 51820u16;
    let mut jc = None;
    let mut jmin = None;
    let mut jmax = None;
    let mut s1 = None;
    let mut s2 = None;
    let mut s3 = None;
    let mut s4 = None;
    let mut h1 = None;
    let mut h2 = None;
    let mut h3 = None;
    let mut h4 = None;
    let mut i1 = None;
    let mut i2 = None;
    let mut i3 = None;
    let mut i4 = None;
    let mut header_protection_key = None;
    let mut rekey_after_time = None;
    let mut rekey_timeout = None;
    let mut reject_after_time = None;
    let mut keepalive_timeout = None;
    let mut max_handshake_attempts = None;
    let mut random_trailers = None;
    let mut disable_cookies = None;
    let mut persistent_keepalive = None;
    let mut parsed_name: Option<String> = None;

    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            let comment = l.trim_start_matches('#').trim();
            if let Some((k, v)) = comment.split_once(':').or_else(|| comment.split_once('=')) {
                let k_trim = k.trim();
                if k_trim.eq_ignore_ascii_case("name") || k_trim.eq_ignore_ascii_case("profile") || k_trim.eq_ignore_ascii_case("tag") {
                    let val_name = v.trim().to_string();
                    if !val_name.is_empty() {
                        parsed_name = Some(val_name);
                    }
                }
            }
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            let key = k.trim().to_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "privatekey" => priv_key = val,
                "address" => local_addr = Some(val),
                "publickey" => pub_key = Some(val),
                "presharedkey" => psk = Some(val),
                "endpoint" => {
                    if let Some((h, p)) = val.rsplit_once(':') {
                        server = h.trim().trim_start_matches('[').trim_end_matches(']').to_string();
                        port = p.trim().parse().unwrap_or(51820);
                    } else {
                        server = val;
                    }
                }
                "jc" => jc = val.parse().ok(),
                "jmin" => jmin = val.parse().ok(),
                "jmax" => jmax = val.parse().ok(),
                "s1" => s1 = val.parse().ok(),
                "s2" => s2 = val.parse().ok(),
                "s3" => s3 = val.parse().ok(),
                "s4" => s4 = val.parse().ok(),
                "h1" => h1 = Some(val),
                "h2" => h2 = Some(val),
                "h3" => h3 = Some(val),
                "h4" => h4 = Some(val),
                "headerprotectionkey" | "header_protection_key" | "hpk" => header_protection_key = Some(val),
                "rekeyaftertime" | "rekey_after_time" => rekey_after_time = Some(val),
                "rekeytimeout" | "rekey_timeout" => rekey_timeout = Some(val),
                "rejectaftertime" | "reject_after_time" => reject_after_time = Some(val),
                "keepalivetimeout" | "keepalive_timeout" => keepalive_timeout = Some(val),
                "maxhandshakeattempts" | "max_handshake_attempts" => max_handshake_attempts = Some(val),
                "randomtrailers" | "random_trailers" => random_trailers = Some(val.eq_ignore_ascii_case("on") || val == "1" || val.eq_ignore_ascii_case("true")),
                "disablecookies" | "disable_cookies" => disable_cookies = Some(val.eq_ignore_ascii_case("on") || val == "1" || val.eq_ignore_ascii_case("true")),
                "persistentkeepalive" | "persistent_keepalive" => persistent_keepalive = Some(val),
                "i1" => i1 = Some(val),
                "i2" => i2 = Some(val),
                "i3" => i3 = Some(val),
                "i4" => i4 = Some(val),
                _ => {}
            }
        }
    }

    if server.is_empty() {
        return Err("В конфигурации AmneziaWG не найден Endpoint (адрес сервера)".to_string());
    }
    if priv_key.is_empty() {
        return Err("В конфигурации AmneziaWG не найден PrivateKey".to_string());
    }

    let is_v31 = header_protection_key.is_some()
        || rekey_after_time.is_some()
        || s3.is_some()
        || s4.is_some()
        || random_trailers.is_some()
        || disable_cookies.is_some();

    let protocol = if is_v31 { "amneziawg".to_string() } else { "awg2".to_string() };
    let version_str = if is_v31 { "3.1".to_string() } else { "2.0".to_string() };
    let default_title = if is_v31 { format!("Amnezia 3.1 ({})", server) } else { format!("AmneziaWG 2.0 ({})", server) };
    let name = parsed_name.unwrap_or(default_title);

    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol,
        server,
        port,
        sni: String::new(),
        insecure: false,
        password: priv_key.clone(),
        uuid: None,
        obfs_password: None,
        hop_ports: Some("20000:50000".to_string()),
        hop_interval: None,
        reality_public_key: None,
        reality_short_id: None,
        reality_flow: None,
        ss_method: None,
        ss_plugin: None,
        ss_plugin_opts: None,
        awg_private_key: Some(priv_key),
        awg_public_key: pub_key,
        awg_preshared_key: psk,
        awg_local_address: local_addr.or_else(|| Some("10.8.0.2/32".to_string())),
        awg_jc: jc.or(Some(4)),
        awg_jmin: jmin.or(Some(40)),
        awg_jmax: jmax.or(Some(70)),
        awg_s1: s1.or_else(|| if is_v31 { Some(12) } else { Some(15) }),
        awg_s2: s2.or_else(|| if is_v31 { Some(12) } else { Some(25) }),
        awg_s3: s3.or_else(|| if is_v31 { Some(12) } else { None }),
        awg_s4: s4.or_else(|| if is_v31 { Some(12) } else { None }),
        awg_h1: h1.or_else(|| Some("1".to_string())),
        awg_h2: h2.or_else(|| Some("2".to_string())),
        awg_h3: h3.or_else(|| Some("3".to_string())),
        awg_h4: h4.or_else(|| Some("4".to_string())),
        awg_header_protection_key: header_protection_key,
        awg_rekey_after_time: rekey_after_time,
        awg_rekey_timeout: rekey_timeout,
        awg_reject_after_time: reject_after_time,
        awg_keepalive_timeout: keepalive_timeout,
        awg_max_handshake_attempts: max_handshake_attempts,
        awg_random_trailers: random_trailers,
        awg_disable_cookies: disable_cookies,
        awg_persistent_keepalive: persistent_keepalive,
        awg_version: Some(version_str),
        awg_i1: i1,
        awg_i2: i2,
        awg_i3: i3,
        awg_i4: i4,
        awg_hop_ports: Some("20000:50000".to_string()),
        awg_port_probing: Some(true),
        up_mbps: 100,
        down_mbps: 100,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Ok(prof)
}

mod urlencoding {
    pub fn encode(input: &str) -> String {
        url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
    }
    pub fn decode(input: &str) -> Result<String, ()> {
        let decoded: String = url::form_urlencoded::parse(input.as_bytes())
            .map(|(k, v)| if v.is_empty() { k.to_string() } else { format!("{}={}", k, v) })
            .collect();
        Ok(decoded)
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy)]
struct MibTcpRowOwnerPid {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
    dw_owning_pid: u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct MibTcpRow {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

#[cfg(target_os = "windows")]
extern "system" {
    fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32;
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(
        p_tcp_table: *mut u8,
        pdw_size: *mut u32,
        b_order: i32,
        ul_af: u32,
        table_class: i32,
        reserved: u32,
    ) -> u32;
    fn SetTcpEntry(p_tcprow: *const MibTcpRow) -> u32;
}

/// Immediately drops pre-existing non-TUN TCP sockets (e.g. hung SYN_SENT / old sockets bound to 192.168.x.x)
/// for Telegram, Discord, and other proxy_processes (excluding steam.exe so game downloads aren't interrupted),
/// forcing them to reconnect through neuroveil-tun (172.19.0.1) in 0ms instead of waiting 21s for TCP SYN timeout.
pub fn reset_stale_app_sockets(proxy_processes: &[String]) {
    #[cfg(target_os = "windows")]
    {
        let mut target_names = std::collections::HashSet::new();
        for entry in proxy_processes {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            let lower = trimmed.to_lowercase();
            if lower.contains("telegram") {
                target_names.insert("telegram.exe".to_string());
            }
            if lower.contains("discord") {
                target_names.insert("discord.exe".to_string());
                target_names.insert("discordsystemhelper.exe".to_string());
            }
            if lower.contains("chrome") {
                target_names.insert("chrome.exe".to_string());
            }
            let p = std::path::Path::new(trimmed);
            if p.is_dir() {
                let mut dir_exes = Vec::new();
                collect_exe_in_dir(p, 0, &mut dir_exes);
                for exe in dir_exes {
                    let exe_lower = exe.to_lowercase();
                    if exe_lower != "steam.exe" {
                        target_names.insert(exe_lower);
                    }
                }
            }
            let fname = std::path::Path::new(&lower)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&lower)
                .to_string();
            if !fname.is_empty() && fname != "steam.exe" {
                if fname.ends_with(".exe") {
                    target_names.insert(fname);
                } else {
                    target_names.insert(format!("{}.exe", fname));
                }
            }
        }

        // Fast in-memory process enumeration via Win32 Snapshot API (<1ms instead of 400ms tasklist.exe)
        let mut target_pids = std::collections::HashSet::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(0x00000002, 0); // TH32CS_SNAPPROCESS = 2
            if snap as isize > 0 && snap as isize != -1 {
                let mut entry: ProcessEntry32W = std::mem::zeroed();
                entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
                if Process32FirstW(snap, &mut entry) != 0 {
                    loop {
                        let len = entry
                            .sz_exe_file
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.sz_exe_file.len());
                        let name = String::from_utf16_lossy(&entry.sz_exe_file[..len]).to_lowercase();
                        if target_names.contains(&name) {
                            target_pids.insert(entry.th32_process_id);
                        }
                        if Process32NextW(snap, &mut entry) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snap);
            }
        }
        if target_pids.is_empty() {
            return;
        }

        unsafe {
            let mut size: u32 = 0;
            // AF_INET = 2, TCP_TABLE_OWNER_PID_ALL = 5
            let _ = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, 2, 5, 0);
            if size == 0 {
                return;
            }
            let mut buf = vec![0u8; size as usize];
            if GetExtendedTcpTable(buf.as_mut_ptr(), &mut size, 0, 2, 5, 0) != 0 {
                return;
            }
            if buf.len() < 4 {
                return;
            }
            let num_entries = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            let row_size = std::mem::size_of::<MibTcpRowOwnerPid>();
            let rows_ptr = buf.as_ptr().add(4) as *const MibTcpRowOwnerPid;

            for i in 0..num_entries {
                if 4 + (i + 1) * row_size > buf.len() {
                    break;
                }
                let row = std::ptr::read_unaligned(rows_ptr.add(i));
                if !target_pids.contains(&row.dw_owning_pid) {
                    continue;
                }
                // MIB_TCP_STATE_SYN_SENT = 3, SYN_RCVD = 4, ESTAB = 5
                if row.dw_state < 3 || row.dw_state > 5 {
                    continue;
                }
                let ip_bytes = row.dw_local_addr.to_ne_bytes();
                // Keep loopback (127.x.x.x); reset both physical (192.168.x.x) and TUN (172.19.0.x) sockets for changed processes
                if ip_bytes[0] == 127 {
                    continue;
                }
                let kill_row = MibTcpRow {
                    dw_state: 12, // MIB_TCP_STATE_DELETE_TCB
                    dw_local_addr: row.dw_local_addr,
                    dw_local_port: row.dw_local_port,
                    dw_remote_addr: row.dw_remote_addr,
                    dw_remote_port: row.dw_remote_port,
                };
                let _ = SetTcpEntry(&kill_row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amnezia_31_from_screenshot() {
        let conf = r#"[Interface]
Address = 10.8.1.4/32
DNS = 1.1.1.1, 1.0.0.1
PrivateKey = vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=
Jc = 5
Jmin = 10
Jmax = 50
S1 = 12
S2 = 12
S3 = 12
S4 = 12
H1 = 1
H2 = 2
H3 = 3
H4 = 4
HeaderProtectionKey = zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY=
RekeyAfterTime = 100-120
RekeyTimeout = 3-7
RejectAfterTime = 150-180
KeepaliveTimeout = 5-15
MaxHandshakeAttempts = 15-20
RandomTrailers = on
DisableCookies = on

[Peer]
PublicKey = fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=
PresharedKey = 7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 78.17.216.77:31555
PersistentKeepalive = 25-35
"#;

        let profile = parse_amnezia_conf(conf).expect("Must parse Amnezia 3.1 config");
        assert_eq!(profile.protocol, "amneziawg");
        assert_eq!(profile.awg_version.as_deref(), Some("3.1"));
        assert_eq!(profile.server, "78.17.216.77");
        assert_eq!(profile.port, 31555);
        assert_eq!(profile.password, "vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=");
        assert_eq!(profile.awg_public_key.as_deref(), Some("fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ="));
        assert_eq!(profile.awg_preshared_key.as_deref(), Some("7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE="));
        assert_eq!(profile.awg_header_protection_key.as_deref(), Some("zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY="));
        assert_eq!(profile.awg_local_address.as_deref(), Some("10.8.1.4/32"));
        assert_eq!(profile.awg_jc, Some(5));
        assert_eq!(profile.awg_jmin, Some(10));
        assert_eq!(profile.awg_jmax, Some(50));
        assert_eq!(profile.awg_s1, Some(12));
        assert_eq!(profile.awg_s2, Some(12));
        assert_eq!(profile.awg_s3, Some(12));
        assert_eq!(profile.awg_s4, Some(12));
        assert_eq!(profile.awg_h1.as_deref(), Some("1"));
        assert_eq!(profile.awg_h2.as_deref(), Some("2"));
        assert_eq!(profile.awg_h3.as_deref(), Some("3"));
        assert_eq!(profile.awg_h4.as_deref(), Some("4"));
        assert_eq!(profile.awg_rekey_after_time.as_deref(), Some("100-120"));
        assert_eq!(profile.awg_random_trailers, Some(true));
        assert_eq!(profile.awg_disable_cookies, Some(true));
        assert_eq!(profile.awg_persistent_keepalive.as_deref(), Some("25-35"));

        // Test export roundtrip
        let exported = build_awg_conf(&profile);
        assert!(exported.contains("HeaderProtectionKey = zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY="));
        assert!(exported.contains("RekeyAfterTime = 100-120"));
        assert!(exported.contains("RandomTrailers = on"));
        assert!(exported.contains("DisableCookies = on"));
        assert!(exported.contains("PresharedKey = 7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE="));
        assert!(exported.contains("PersistentKeepalive = 25-35"));
    }

    #[test]
    fn test_parse_awg2_conf() {
        let conf = r#"[Interface]
Address = 10.8.1.2/32
DNS = 1.1.1.1, 1.0.0.1
PrivateKey = vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=
Jc = 4
Jmin = 20
Jmax = 60
S1 = 30
S2 = 40
H1 = 12948291
H2 = 94829104
H3 = 38192847
H4 = 57193821

[Peer]
PublicKey = fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=
PresharedKey = 7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 78.17.216.77:51821
PersistentKeepalive = 25
"#;

        let profile = parse_amnezia_conf(conf).expect("Must parse AWG 2.0 config");
        assert_eq!(profile.protocol, "awg2");
        assert_eq!(profile.awg_version.as_deref(), Some("2.0"));
        assert_eq!(profile.awg_h1.as_deref(), Some("12948291"));
        assert_eq!(profile.awg_header_protection_key, None);

        let exported = build_awg_conf(&profile);
        assert!(!exported.contains("HeaderProtectionKey"));
        assert!(exported.contains("H1 = 12948291"));
    }

    #[test]
    fn test_singbox_client_validation_both_protocols() {
        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");

        if !singbox_bin.exists() {
            println!("sing-box binary not found at {:?}, skipping check", singbox_bin);
            return;
        }

        // Test AWG 3.1
        let conf_31 = r#"[Interface]
Address = 10.8.0.2/32
PrivateKey = vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=
Jc = 5
Jmin = 10
Jmax = 50
S1 = 12
S2 = 12
S3 = 12
S4 = 12
H1 = 1
H2 = 2
H3 = 3
H4 = 4
HeaderProtectionKey = zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY=
RandomTrailers = on
DisableCookies = on

[Peer]
PublicKey = fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=
PresharedKey = 7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE=
AllowedIPs = 0.0.0.0/0
Endpoint = 78.17.216.77:51820
PersistentKeepalive = 25
"#;
        let prof_31 = parse_amnezia_conf(conf_31).unwrap();
        let routing = RoutingConfig::default();
        let cfg_31 = generate_singbox_config(&prof_31, &routing);

        let temp_dir = std::env::temp_dir();
        let temp_31 = temp_dir.join("test_singbox_awg31.json");
        std::fs::write(&temp_31, serde_json::to_string_pretty(&cfg_31).unwrap()).unwrap();

        let out_31 = std::process::Command::new(&singbox_bin)
            .args(["check", "-c", temp_31.to_str().unwrap()])
            .output()
            .expect("Failed to run sing-box check on AWG 3.1");

        assert!(
            out_31.status.success(),
            "sing-box check failed for AWG 3.1:\nSTDOUT: {}\nSTDERR: {}",
            String::from_utf8_lossy(&out_31.stdout),
            String::from_utf8_lossy(&out_31.stderr)
        );

        // Test AWG 2.0
        let conf_20 = r#"[Interface]
Address = 10.8.1.2/32
PrivateKey = vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=
Jc = 4
Jmin = 20
Jmax = 60
S1 = 30
S2 = 40
H1 = 12948291
H2 = 94829104
H3 = 38192847
H4 = 57193821

[Peer]
PublicKey = fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=
PresharedKey = 7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE=
AllowedIPs = 0.0.0.0/0
Endpoint = 78.17.216.77:51821
PersistentKeepalive = 25
"#;
        let prof_20 = parse_amnezia_conf(conf_20).unwrap();
        let cfg_20 = generate_singbox_config(&prof_20, &routing);

        let temp_20 = temp_dir.join("test_singbox_awg20.json");
        std::fs::write(&temp_20, serde_json::to_string_pretty(&cfg_20).unwrap()).unwrap();

        let out_20 = std::process::Command::new(&singbox_bin)
            .args(["check", "-c", temp_20.to_str().unwrap()])
            .output()
            .expect("Failed to run sing-box check on AWG 2.0");

        assert!(
            out_20.status.success(),
            "sing-box check failed for AWG 2.0:\nSTDOUT: {}\nSTDERR: {}",
            String::from_utf8_lossy(&out_20.stdout),
            String::from_utf8_lossy(&out_20.stderr)
        );

        let _ = std::fs::remove_file(temp_31);
        let _ = std::fs::remove_file(temp_20);
    }
}


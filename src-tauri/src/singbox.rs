use crate::models::{is_awg_protocol, RoutingConfig, VpnProfile};
use serde_json::{json, Value};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
#[cfg(not(target_os = "android"))]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use url::Url;
use uuid::Uuid;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static APP_DATA_DIR_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_app_data_dir(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    let _ = APP_DATA_DIR_OVERRIDE.set(dir);
}

pub fn get_app_data_dir() -> PathBuf {
    if let Some(dir) = APP_DATA_DIR_OVERRIDE.get() {
        let _ = fs::create_dir_all(dir);
        return dir.clone();
    }
    if cfg!(test) {
        let dir = std::env::temp_dir().join("neuroveil_test_data");
        let _ = fs::create_dir_all(&dir);
        return dir;
    }
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
    #[cfg(target_os = "android")]
    {
        return (true, "libbox 1.14.4-lx (встроено)".to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
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
}

pub fn download_singbox_core() -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        return Ok("libbox 1.14.4-lx (встроено)".to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
    let bin_path = get_singbox_bin_path();

    let user_agent = concat!("NeuroVeilVPN-Client/", env!("CARGO_PKG_VERSION"));
    // Query latest release from GitHub API (sing-box-lx with native AmneziaWG support)
    let release_resp: Value = ureq::get("https://api.github.com/repos/Leadaxe/sing-box-lx/releases/latest")
        .set("User-Agent", user_agent)
        .call()
        .or_else(|_| {
            ureq::get("https://api.github.com/repos/SagerNet/sing-box/releases/latest")
                .set("User-Agent", user_agent)
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
        .set("User-Agent", user_agent)
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
}

#[allow(unused_variables)]
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
    if depth > 3 || names.len() >= 128 {
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

pub fn get_rules_proxy_proc_path() -> PathBuf {
    get_app_data_dir().join("rules_proxy_proc.json")
}

pub fn get_rules_direct_proc_path() -> PathBuf {
    get_app_data_dir().join("rules_direct_proc.json")
}

pub fn get_rules_proxy_proc_ip_path() -> PathBuf {
    get_app_data_dir().join("rules_proxy_proc_ip.json")
}

pub fn get_rules_proxy_ip_path() -> PathBuf {
    get_app_data_dir().join("rules_proxy_ip.json")
}

pub fn get_rules_direct_ip_path() -> PathBuf {
    get_app_data_dir().join("rules_direct_ip.json")
}

pub fn build_process_ruleset_content(processes: &[String], is_proxy: bool) -> Value {
    let mut rules: Vec<Value> = Vec::new();
    let outbound_tag = if is_proxy { "proxy" } else { "direct" };
    // Use the DNS-safe rule list (process matchers + Telegram/Discord domain_suffix, strictly NO ip_cidr).
    // In sing-box 1.14+, any ip_cidr inside a rule_set referenced by dns.rules activates legacy address
    // filtering, causing every unrelated DNS query to be rejected (`dns: rejected A ...`) and leaked to dns-direct.
    let (_, proc_dns) = build_process_rules(processes, outbound_tag);
    for mut r in proc_dns {
        if let Some(obj) = r.as_object_mut() {
            obj.remove("server");
            rules.push(r);
        }
    }
    json!({
        "version": 1,
        "rules": rules
    })
}

pub fn build_process_ip_ruleset_content(processes: &[String], is_proxy: bool) -> Value {
    let mut rules: Vec<Value> = Vec::new();
    let outbound_tag = if is_proxy { "proxy" } else { "direct" };
    let (proc_routes, _) = build_process_rules(processes, outbound_tag);
    for mut r in proc_routes {
        if let Some(obj) = r.as_object_mut() {
            if obj.contains_key("ip_cidr") {
                obj.remove("outbound");
                rules.push(r);
            }
        }
    }
    json!({
        "version": 1,
        "rules": rules
    })
}

pub fn build_domain_ip_ruleset_content(domains: &[String], ips: &[String]) -> Value {
    let mut rules: Vec<Value> = Vec::new();

    // 1. Custom domains
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

    // 2. Custom IPs / CIDRs
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

#[allow(dead_code)]
pub fn build_ruleset_content(
    processes: &[String],
    domains: &[String],
    ips: &[String],
    is_proxy: bool,
) -> Value {
    let mut rules = build_process_ruleset_content(processes, is_proxy)["rules"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(ip_rules) = build_process_ip_ruleset_content(processes, is_proxy)["rules"].as_array() {
        rules.extend(ip_rules.iter().cloned());
    }
    if let Some(dom_rules) = build_domain_ip_ruleset_content(domains, ips)["rules"].as_array() {
        rules.extend(dom_rules.iter().cloned());
    }
    json!({
        "version": 1,
        "rules": rules
    })
}

pub fn write_routing_rulesets(routing: &RoutingConfig) -> Result<(), String> {
    let proxy_path = get_rules_proxy_path();
    let direct_path = get_rules_direct_path();
    let proxy_proc_path = get_rules_proxy_proc_path();
    let direct_proc_path = get_rules_direct_proc_path();
    let proxy_proc_ip_path = get_rules_proxy_proc_ip_path();
    let proxy_ip_path = get_rules_proxy_ip_path();
    let direct_ip_path = get_rules_direct_ip_path();

    let empty_ruleset = || json!({ "version": 1, "rules": [] });

    let proxy_proc_content = if routing.split_tunneling {
        build_process_ruleset_content(&routing.proxy_processes, true)
    } else {
        empty_ruleset()
    };

    let direct_proc_content = if routing.split_tunneling {
        build_process_ruleset_content(&routing.direct_processes, false)
    } else {
        empty_ruleset()
    };

    let proxy_proc_ip_content = if routing.split_tunneling {
        build_process_ip_ruleset_content(&routing.proxy_processes, true)
    } else {
        empty_ruleset()
    };

    // Keep domain rulesets strictly free of ip_cidr so they can be safely shared with dns.rules
    let proxy_content = if routing.split_tunneling {
        build_domain_ip_ruleset_content(&routing.proxy_domains, &[])
    } else {
        empty_ruleset()
    };

    let direct_content = if routing.split_tunneling {
        build_domain_ip_ruleset_content(&routing.direct_domains, &[])
    } else {
        empty_ruleset()
    };

    let proxy_ip_content = if routing.split_tunneling {
        build_domain_ip_ruleset_content(&[], &routing.proxy_ips)
    } else {
        empty_ruleset()
    };

    let direct_ip_content = if routing.split_tunneling {
        build_domain_ip_ruleset_content(&[], &routing.direct_ips)
    } else {
        empty_ruleset()
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

    write_and_sync(&proxy_proc_path, &proxy_proc_content, "rules_proxy_proc.json")?;
    write_and_sync(&direct_proc_path, &direct_proc_content, "rules_direct_proc.json")?;
    write_and_sync(&proxy_proc_ip_path, &proxy_proc_ip_content, "rules_proxy_proc_ip.json")?;
    write_and_sync(&proxy_path, &proxy_content, "rules_proxy.json")?;
    write_and_sync(&direct_path, &direct_content, "rules_direct.json")?;
    write_and_sync(&proxy_ip_path, &proxy_ip_content, "rules_proxy_ip.json")?;
    write_and_sync(&direct_ip_path, &direct_ip_content, "rules_direct_ip.json")?;

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

fn build_single_outbound(profile: &VpnProfile, tag: &str, bind_ipv4: Option<&str>, override_port: Option<u16>) -> Value {
    let effective_port = override_port.unwrap_or(profile.port);
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
            "server_port": effective_port,
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
    } else if profile.protocol == "vless_ws" {
        let ws_path = profile.ws_path.as_deref().unwrap_or("/neuroveil-ws");
        let cdn_host = profile.cdn_host.as_deref().unwrap_or(&profile.sni);
        let host_header = if !cdn_host.trim().is_empty() { cdn_host.trim() } else { profile.server.as_str() };
        let tls_frag = profile.tls_fragment.unwrap_or(true);

        let mut tls_obj = json!({
            "enabled": true,
            "server_name": host_header,
            "insecure": profile.insecure,
            "alpn": ["http/1.1"],
            "utls": {
                "enabled": true,
                "fingerprint": "chrome"
            }
        });
        if tls_frag {
            tls_obj["fragment"] = json!(true);
            tls_obj["record_fragment"] = json!(true);
        }

        json!({
            "type": "vless",
            "tag": tag,
            "server": profile.server,
            "server_port": effective_port,
            "uuid": profile.uuid.clone().unwrap_or_else(|| profile.password.clone()),
            "packet_encoding": "xudp",
            "transport": {
                "type": "ws",
                "path": ws_path,
                "headers": {
                    "Host": host_header
                }
            },
            "tls": tls_obj
        })
    } else if profile.protocol == "shadowtls" {
        let method = profile.ss_method.as_deref().unwrap_or("2022-blake3-aes-128-gcm");
        let shadowtls_tag = format!("{}-st-detour", tag);

        json!({
            "type": "shadowsocks",
            "tag": tag,
            "method": method,
            "password": profile.password,
            "detour": shadowtls_tag,
            "multiplex": {
                "enabled": true,
                "protocol": "h2mux",
                "max_connections": 4,
                "min_streams": 4,
                "padding": true
            }
        })
    } else if profile.protocol == "webrtc" {
        // High-Stealth UDP WebRTC transport via VLESS gRPC/HTTPUpgrade with audio packet pacing
        let service_name = profile.ws_path.as_deref().unwrap_or("neuroveil-webrtc");
        let sni = if !profile.sni.is_empty() { &profile.sni } else { &profile.server };
        json!({
            "type": "vless",
            "tag": tag,
            "server": profile.server,
            "server_port": effective_port,
            "uuid": profile.uuid.clone().unwrap_or_else(|| profile.password.clone()),
            "packet_encoding": "xudp",
            "transport": {
                "type": "grpc",
                "service_name": service_name,
                "idle_timeout": "15s",
                "ping_timeout": "15s"
            },
            "tls": {
                "enabled": true,
                "server_name": sni,
                "insecure": profile.insecure,
                "alpn": ["h2"],
                "utls": {
                    "enabled": true,
                    "fingerprint": "chrome"
                }
            }
        })
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
            "server_port": effective_port,
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
            "server_port": effective_port,
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
            "server_port": effective_port,
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
            "server_port": effective_port,
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
        if override_port.is_none() {
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
        }
        hy2
    };
    if let Some(ip) = bind_ipv4 {
        out["inet4_bind_address"] = json!(ip);
    }
    out
}

fn random_u16_in_range(min: u16, max: u16) -> u16 {
    if min >= max {
        return min;
    }
    let u = Uuid::new_v4();
    let b = u.as_bytes();
    let n = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
    (min as u32 + (n % ((max - min) as u32 + 1))) as u16
}

pub fn parse_hop_port_ranges(
    hop_a: Option<&str>,
    hop_b: Option<&str>,
) -> (Vec<(u16, u16)>, Vec<u16>) {
    let mut ranges: Vec<(u16, u16)> = Vec::new();
    let mut singles: Vec<u16> = Vec::new();
    for hop_opt in [hop_a, hop_b] {
        if let Some(hop) = hop_opt {
            for part in hop.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                if let Some((start_s, end_s)) = p.split_once(':').or_else(|| p.split_once('-')) {
                    if let (Ok(s), Ok(e)) = (start_s.trim().parse::<u16>(), end_s.trim().parse::<u16>()) {
                        let (min_p, max_p) = if s <= e { (s, e) } else { (e, s) };
                        if min_p > 0 && max_p > 0 && !ranges.contains(&(min_p, max_p)) {
                            ranges.push((min_p, max_p));
                        }
                    }
                } else if let Ok(port_num) = p.parse::<u16>() {
                    if port_num > 0 && !singles.contains(&port_num) {
                        singles.push(port_num);
                    }
                }
            }
        }
    }
    (ranges, singles)
}

pub fn port_matches_hop_spec(port: u16, ranges: &[(u16, u16)], singles: &[u16]) -> bool {
    if port == 0 {
        return false;
    }
    if singles.contains(&port) {
        return true;
    }
    ranges.iter().any(|&(lo, hi)| port >= lo && port <= hi)
}

pub fn get_profile_candidate_ports(
    profile: &VpnProfile,
    custom_ports: Option<Vec<u16>>,
    exclude_ports: Option<Vec<u16>>,
    target_count: usize,
) -> Vec<u16> {
    let mut ports: Vec<u16> = Vec::new();
    let excluded = exclude_ports.unwrap_or_default();

    if let Some(custom) = custom_ports {
        for p in custom {
            if p > 0 && !ports.contains(&p) && !excluded.contains(&p) {
                ports.push(p);
            }
        }
        if !ports.is_empty() {
            return ports;
        }
    }

    let is_udp = is_awg_protocol(&profile.protocol)
        || profile.protocol == "hysteria2"
        || profile.protocol == "tuic";

    // 1. Collect any explicitly specified hop ranges / single hop ports first
    let (mut ranges, explicit_singles) =
        parse_hop_port_ranges(profile.awg_hop_ports.as_deref(), profile.hop_ports.as_deref());
    let has_explicit_hops = !ranges.is_empty() || !explicit_singles.is_empty();

    // 2. If primary/original port or valid profile port is not yet excluded, add it first
    let primary_port = profile.original_port.unwrap_or(profile.port);
    if primary_port > 0 && !excluded.contains(&primary_port) && !ports.contains(&primary_port) {
        ports.push(primary_port);
    }
    let current_port_valid = !has_explicit_hops
        || profile.port == primary_port
        || port_matches_hop_spec(profile.port, &ranges, &explicit_singles);
    if current_port_valid
        && profile.port > 0
        && profile.port != primary_port
        && !excluded.contains(&profile.port)
        && !ports.contains(&profile.port)
    {
        ports.push(profile.port);
    }
    for &port_num in &explicit_singles {
        if port_num > 0 && !ports.contains(&port_num) && !excluded.contains(&port_num) {
            ports.push(port_num);
        }
    }

    // Default range for UDP protocols (AmneziaWG / Hysteria2 NAT range configured on VPS)
    if ranges.is_empty() && is_udp {
        ranges.push((20000, 65000));
    }

    // 3. Pool of well-known standard ports.
    // CRITICAL: When explicit hop ranges are configured (e.g. "20000:65000" for Enigma/AWG/Hy2),
    // ONLY include default ports that actually fall inside those hop ranges — never inject
    // closed low ports like 53/443/500/4500 that are outside server iptables PREROUTING rules!
    let defaults: &[u16] = if is_udp {
        &[
            53, 443, 8443, 1194, 51820, 123, 500, 4500, 5060, 5061, 8080, 993, 995, 3478, 3479,
            20000, 25000, 30000, 35000, 40000, 45000, 50000, 55000, 60000, 65000,
        ]
    } else {
        &[
            443, 80, 8443, 2053, 2083, 2087, 2096, 8080, 8880, 2052, 2082, 2086, 2095, 8444,
            9443, 10443, 4443, 7443, 8000, 8008, 8888, 3000, 5000, 9090, 1080, 3128, 22, 2222,
            2200, 22222, 5222, 5228, 853, 993, 995, 1433, 3306, 5432, 6379, 8081, 8088, 8445,
            9000, 9001, 9999,
        ]
    };

    // Add unvisited standard ports (filtered by explicit hop spec when present)
    for &cp in defaults {
        if ports.len() >= target_count {
            break;
        }
        if has_explicit_hops && !port_matches_hop_spec(cp, &ranges, &explicit_singles) {
            continue;
        }
        if cp > 0 && !ports.contains(&cp) && !excluded.contains(&cp) {
            ports.push(cp);
        }
    }

    // 4. Sample ports systematically across the entire range to guarantee full coverage from min to max
    if !ranges.is_empty() {
        for &(min_r, max_r) in &ranges {
            let span = max_r.saturating_sub(min_r);
            if span > 0 {
                let steps = (target_count / 2).max(10) as u32;
                let step_size = (span as u32 / steps).max(1);
                for i in 0..=steps {
                    let p = (min_r as u32 + i * step_size).min(max_r as u32) as u16;
                    if p > 0 && !ports.contains(&p) && !excluded.contains(&p) {
                        ports.push(p);
                    }
                    if ports.len() >= target_count {
                        break;
                    }
                }
            }
        }
    }

    // 5. Fill remaining slots with random selections across the available ranges
    if !ranges.is_empty() {
        let mut attempts = 0;
        let mut range_idx = 0;
        while ports.len() < target_count && attempts < 2500 {
            attempts += 1;
            let (min_r, max_r) = ranges[range_idx % ranges.len()];
            range_idx += 1;
            let cand = random_u16_in_range(min_r, max_r);
            if cand > 0 && !ports.contains(&cand) && !excluded.contains(&cand) {
                ports.push(cand);
            }
        }
    }

    // 6. Fallback if the range was fully exhausted: allow repeating from range without excluded check
    if ports.len() < target_count && !ranges.is_empty() {
        let mut attempts = 0;
        let mut range_idx = 0;
        while ports.len() < target_count && attempts < 500 {
            attempts += 1;
            let (min_r, max_r) = ranges[range_idx % ranges.len()];
            range_idx += 1;
            let cand = random_u16_in_range(min_r, max_r);
            if cand > 0 && !ports.contains(&cand) {
                ports.push(cand);
            }
        }
    }

    // 7. If TCP and still not enough, generate random high ports in 10000..60000
    if ports.len() < target_count && !is_udp {
        let mut attempts = 0;
        while ports.len() < target_count && attempts < 500 {
            attempts += 1;
            let cand = random_u16_in_range(10000, 60000);
            if cand > 0 && !ports.contains(&cand) && !excluded.contains(&cand) {
                ports.push(cand);
            }
        }
    }

    ports
}

#[allow(dead_code)]
pub fn get_awg_candidate_ports(profile: &VpnProfile) -> Vec<u16> {
    get_profile_candidate_ports(profile, None, None, 40)
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
    generate_singbox_config_with_bridge(profile, None, routing, override_port, None, for_mobile)
}

fn append_bridge_profile_nodes(
    profile: &VpnProfile,
    node_tag: &str,
    outer_detour: Option<&str>,
    override_port: Option<u16>,
    for_mobile: bool,
    nested_wg_mtu: Option<u16>,
    endpoints: &mut Vec<Value>,
    outbounds: &mut Vec<Value>,
) {
    let active_port = override_port.unwrap_or(profile.port);
    let is_chained = outer_detour.is_some();

    if is_awg_protocol(&profile.protocol) {
        let vless_detour_tag = if node_tag == "proxy" {
            "vless-reality-detour".to_string()
        } else {
            format!("{}-vless-detour", node_tag)
        };
        let include_listen = !is_chained && !for_mobile;
        let (mut ep, mut vless_detour_opt) = build_wireguard_endpoint(
            profile,
            node_tag,
            &vless_detour_tag,
            active_port,
            include_listen,
        );
        if profile.protocol == "enigma" && !for_mobile && !is_chained {
            ep["inet4_bind_address"] = json!("127.0.0.1");
        }
        if is_chained {
            if let Some(obj) = ep.as_object_mut() {
                obj.remove("listen_port");
                obj.remove("inet4_bind_address");
            }
            if let Some(mtu) = nested_wg_mtu {
                ep["mtu"] = json!(mtu);
            }
        }
        if let Some(detour_target) = outer_detour {
            if let Some(ref mut vless_ob) = vless_detour_opt {
                vless_ob["detour"] = json!(detour_target);
            } else {
                ep["detour"] = json!(detour_target);
            }
        }
        if let Some(vless_ob) = vless_detour_opt {
            outbounds.push(vless_ob);
        }
        endpoints.push(ep);
    } else {
        let mut ob = build_single_outbound(profile, node_tag, None, Some(active_port));
        if node_tag == "bridge-entry" && profile.protocol == "vless" {
            ob["packet_encoding"] = json!("xudp");
        }
        if profile.protocol == "shadowtls" {
            let st_detour_tag = format!("{}-st-detour", node_tag);
            let st_sni = profile.shadowtls_sni.as_deref().unwrap_or(if !profile.sni.is_empty() {
                &profile.sni
            } else {
                "www.microsoft.com"
            });
            let st_pass = profile.shadowtls_password.as_deref().unwrap_or(&profile.password);
            let mut st_ob = json!({
                "type": "shadowtls",
                "tag": st_detour_tag,
                "server": profile.server,
                "server_port": active_port,
                "version": 3,
                "password": st_pass,
                "tls": {
                    "enabled": true,
                    "server_name": st_sni,
                    "utls": {
                        "enabled": true,
                        "fingerprint": "chrome"
                    }
                }
            });
            if let Some(detour_target) = outer_detour {
                st_ob["detour"] = json!(detour_target);
            }
            outbounds.push(ob);
            outbounds.push(st_ob);
        } else {
            if let Some(detour_target) = outer_detour {
                ob["detour"] = json!(detour_target);
            }
            outbounds.push(ob);
        }
    }
}

/// Builds sing-box `endpoints` and `outbounds` entries for an Exit VPS profile
/// running inside a server-side Bridge Gateway on Entry VPS (`tag = "bridge-exit"`).
pub fn build_bridge_exit_nodes(
    exit_profile: &VpnProfile,
    nested_wg_mtu: Option<u16>,
) -> (Vec<Value>, Vec<Value>) {
    let mut endpoints = Vec::new();
    let mut outbounds = Vec::new();
    append_bridge_profile_nodes(
        exit_profile,
        "bridge-exit",
        None,
        None,
        true,
        nested_wg_mtu,
        &mut endpoints,
        &mut outbounds,
    );
    if let Some(mtu) = nested_wg_mtu {
        for ep in endpoints.iter_mut() {
            if ep.get("tag").and_then(|v| v.as_str()) == Some("bridge-exit") {
                ep["mtu"] = json!(mtu);
            }
        }
    }
    for ep in endpoints.iter_mut() {
        if let Some(obj) = ep.as_object_mut() {
            obj.remove("listen_port");
            obj.remove("inet4_bind_address");
        }
    }
    for ob in outbounds.iter_mut() {
        if let Some(obj) = ob.as_object_mut() {
            obj.remove("inet4_bind_address");
        }
    }
    (endpoints, outbounds)
}

/// Generates a standalone, router-friendly 2-Hop sing-box JSON config (OpenWrt / HomeProxy / Keenetic XKeen / Hiddify / Nekobox)
/// without needing SSH access to Entry VPS.
pub fn generate_router_2hop_singbox_json(
    entry_profile: &VpnProfile,
    exit_profile: &VpnProfile,
    routing: &RoutingConfig,
) -> Value {
    let mut router_routing = routing.clone();
    router_routing.mode = "tun".to_string();
    router_routing.proxy_processes.clear();
    router_routing.direct_processes.clear();
    let mut cfg = generate_singbox_config_with_bridge(
        exit_profile,
        Some(entry_profile),
        &router_routing,
        None,
        None,
        true,
    );
    if let Some(obj) = cfg.as_object_mut() {
        obj.remove("experimental");
        if let Some(inbounds) = obj.get_mut("inbounds").and_then(|v| v.as_array_mut()) {
            for ib in inbounds.iter_mut() {
                if ib.get("type").and_then(|v| v.as_str()) == Some("mixed") {
                    ib["listen"] = json!("0.0.0.0");
                }
            }
        }
    }
    cfg
}

pub fn generate_singbox_config_with_bridge(
    profile: &VpnProfile,
    entry_profile: Option<&VpnProfile>,
    routing: &RoutingConfig,
    override_port: Option<u16>,
    override_entry_port: Option<u16>,
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
    let entry_is_awg = entry_profile
        .map(|e| is_awg_protocol(&e.protocol))
        .unwrap_or(false);

    let mut outbounds = Vec::new();
    let mut endpoints = Vec::new();

    if let Some(entry) = entry_profile {
        // 2-Hop Bridge Mode:
        //   Client -> "bridge-entry" (unblocked Entry Bridge) -> "proxy" (Exit Server via detour)
        append_bridge_profile_nodes(
            entry,
            "bridge-entry",
            None,
            override_entry_port,
            for_mobile,
            None,
            &mut endpoints,
            &mut outbounds,
        );
        let nested_wg_mtu = if is_awg && entry_is_awg {
            Some(1220)
        } else if is_awg {
            Some(1280)
        } else {
            None
        };
        append_bridge_profile_nodes(
            profile,
            "proxy",
            Some("bridge-entry"),
            override_port,
            for_mobile,
            nested_wg_mtu,
            &mut endpoints,
            &mut outbounds,
        );
        outbounds.push(direct_out);
        outbounds.push(block_out);
    } else {
        append_bridge_profile_nodes(
            profile,
            "proxy",
            None,
            override_port,
            for_mobile,
            None,
            &mut endpoints,
            &mut outbounds,
        );
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
        let tun_mtu = if entry_profile.is_some() {
            if is_awg && entry_is_awg {
                1220
            } else if is_awg || entry_is_awg {
                1280
            } else {
                1360
            }
        } else if is_awg {
            1280
        } else {
            1400
        };
        let mut tun_obj = json!({
            "type": "tun",
            "tag": "tun-in",
            "interface_name": "neuroveil-tun",
            "address": ["172.19.0.1/30"],
            "mtu": tun_mtu,
            "auto_route": true,
            "strict_route": routing.kill_switch,
            "stack": "mixed",
            "endpoint_independent_nat": true,
            "udp_mapping": "endpoint_independent",
            "udp_filtering": "endpoint_independent"
        });
        // Exclude the outer hop (Entry Bridge in 2-Hop mode, or Exit profile in direct mode) from TUN
        let outer_profile = entry_profile.unwrap_or(profile);
        if let Ok(ipv4) = outer_profile.server.trim().parse::<std::net::Ipv4Addr>() {
            tun_obj["route_exclude_address"] = json!([format!("{}/32", ipv4)]);
        } else if outer_profile.protocol == "enigma" || entry_profile.is_some() {
            use std::net::ToSocketAddrs;
            if let Ok(addrs) = (outer_profile.server.trim(), outer_profile.port).to_socket_addrs() {
                let v4_cidrs: Vec<String> = addrs
                    .filter_map(|sa| match sa {
                        std::net::SocketAddr::V4(v4) => Some(format!("{}/32", v4.ip())),
                        _ => None,
                    })
                    .collect();
                if !v4_cidrs.is_empty() {
                    tun_obj["route_exclude_address"] = json!(v4_cidrs);
                }
            }
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
            "timeout": "300ms"
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
        // Process rulesets MUST be evaluated before domain/IP rulesets so adding an app (e.g. a game or launcher)
        // to proxy_processes routes 100% of its traffic consistently instead of splitting .ru/.su domains direct
        // and raw server IPs through VPN.
        let _ = write_routing_rulesets(routing);
        let direct_proc_path_str = get_rules_direct_proc_path().to_string_lossy().to_string();
        let proxy_proc_path_str = get_rules_proxy_proc_path().to_string_lossy().to_string();
        let proxy_proc_ip_path_str = get_rules_proxy_proc_ip_path().to_string_lossy().to_string();
        let direct_path_str = get_rules_direct_path().to_string_lossy().to_string();
        let direct_ip_path_str = get_rules_direct_ip_path().to_string_lossy().to_string();
        let proxy_path_str = get_rules_proxy_path().to_string_lossy().to_string();
        let proxy_ip_path_str = get_rules_proxy_ip_path().to_string_lossy().to_string();

        route_rules.push(json!({
            "rule_set": "direct-proc-rules",
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "rule_set": "proxy-proc-rules",
            "outbound": "proxy"
        }));
        route_rules.push(json!({
            "rule_set": "proxy-proc-ip-rules",
            "outbound": "proxy"
        }));
        route_rules.push(json!({
            "rule_set": "direct-rules",
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "rule_set": "direct-ip-rules",
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "rule_set": "proxy-rules",
            "outbound": "proxy"
        }));
        route_rules.push(json!({
            "rule_set": "proxy-ip-rules",
            "outbound": "proxy"
        }));

        // DNS rules reference ONLY the IP-free rulesets (never *-ip-rules), preventing sing-box 1.14
        // from activating legacy address filtering and rejecting unrelated DNS responses.
        dns_rules.push(json!({
            "rule_set": "direct-proc-rules",
            "server": "dns-direct"
        }));
        dns_rules.push(json!({
            "rule_set": "proxy-proc-rules",
            "server": "dns-remote"
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
                    "tag": "direct-proc-rules",
                    "type": "local",
                    "format": "source",
                    "path": direct_proc_path_str
                },
                {
                    "tag": "proxy-proc-rules",
                    "type": "local",
                    "format": "source",
                    "path": proxy_proc_path_str
                },
                {
                    "tag": "proxy-proc-ip-rules",
                    "type": "local",
                    "format": "source",
                    "path": proxy_proc_ip_path_str
                },
                {
                    "tag": "direct-rules",
                    "type": "local",
                    "format": "source",
                    "path": direct_path_str
                },
                {
                    "tag": "direct-ip-rules",
                    "type": "local",
                    "format": "source",
                    "path": direct_ip_path_str
                },
                {
                    "tag": "proxy-rules",
                    "type": "local",
                    "format": "source",
                    "path": proxy_path_str
                },
                {
                    "tag": "proxy-ip-rules",
                    "type": "local",
                    "format": "source",
                    "path": proxy_ip_path_str
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

    let is_tcp_only = profile.protocol == "ssh"
        || profile.protocol == "shadowtls"
        || entry_profile
            .map(|e| e.protocol == "ssh" || e.protocol == "shadowtls")
            .unwrap_or(false);
    let remote_dns_type = if is_tcp_only { "tcp" } else { "udp" };

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
            "strategy": "ipv4_only",
            "reverse_mapping": true
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
            tun_map.insert("endpoint_independent_nat".to_string(), json!(true));
            tun_map.insert("udp_mapping".to_string(), json!("endpoint_independent"));
            tun_map.insert("udp_filtering".to_string(), json!("endpoint_independent"));
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

fn is_android_package_id(entry: &str) -> bool {
    let t = entry.trim();
    !t.is_empty()
        && t.contains('.')
        && !t.contains('\\')
        && !t.contains('/')
        && !t.to_ascii_lowercase().ends_with(".exe")
        && !t.chars().any(char::is_whitespace)
}

pub fn generate_android_runtime_singbox_config(profile: &VpnProfile, routing: &RoutingConfig) -> Value {
    generate_android_runtime_singbox_config_with_bridge(profile, None, routing)
}

pub fn generate_android_runtime_singbox_config_with_bridge(
    profile: &VpnProfile,
    entry_profile: Option<&VpnProfile>,
    routing: &RoutingConfig,
) -> Value {
    let mut mobile_routing = routing.clone();
    // On Android runtime, strip desktop .exe entries while keeping Android package names
    let proxy_pkgs: Vec<String> = routing
        .proxy_processes
        .iter()
        .filter(|s| is_android_package_id(s))
        .map(|s| s.trim().to_string())
        .collect();
    let direct_pkgs: Vec<String> = routing
        .direct_processes
        .iter()
        .filter(|s| is_android_package_id(s))
        .map(|s| s.trim().to_string())
        .collect();

    mobile_routing.proxy_processes.clear();
    mobile_routing.direct_processes.clear();

    let mut cfg = generate_singbox_config_with_bridge(profile, entry_profile, &mobile_routing, None, None, true);
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

            if let Some(rules) = route.get_mut("rules").and_then(|v| v.as_array_mut()) {
                // Dedicated local SOCKS/HTTP mixed inbound always routes via proxy for in-app speedtest/tunnel fallback
                rules.insert(
                    3.min(rules.len()),
                    json!({
                        "inbound": ["vpn-tunnel-in"],
                        "outbound": "proxy"
                    }),
                );

                // In whitelist mode with extra domain/IP rules, also route selected packages by package_name
                if routing.split_tunneling
                    && routing.default_outbound == "direct"
                    && !proxy_pkgs.is_empty()
                    && (!routing.proxy_domains.is_empty() || !routing.proxy_ips.is_empty())
                {
                    rules.push(json!({
                        "package_name": proxy_pkgs,
                        "outbound": "proxy"
                    }));
                }
            }
        }
        if let Some(inbounds) = obj.get_mut("inbounds").and_then(|v| v.as_array_mut()) {
            let mut tun_map = serde_json::Map::new();
            tun_map.insert("type".to_string(), json!("tun"));
            tun_map.insert("tag".to_string(), json!("tun-in"));
            let tun_addrs = if routing.block_ipv6 {
                json!(["172.19.0.1/28"])
            } else {
                json!(["172.19.0.1/28", "fdfe:dcba:9876::1/126"])
            };
            tun_map.insert("address".to_string(), tun_addrs);
            let exit_is_awg = is_awg_protocol(&profile.protocol);
            let entry_is_awg = entry_profile
                .map(|e| is_awg_protocol(&e.protocol))
                .unwrap_or(false);
            let tun_mtu = if entry_profile.is_some() {
                if exit_is_awg && entry_is_awg {
                    1220
                } else if exit_is_awg || entry_is_awg {
                    1280
                } else {
                    1360
                }
            } else if exit_is_awg {
                1280
            } else {
                1400
            };
            tun_map.insert("mtu".to_string(), json!(tun_mtu));
            tun_map.insert("auto_route".to_string(), json!(true));
            tun_map.insert("strict_route".to_string(), json!(routing.kill_switch));
            tun_map.insert("stack".to_string(), json!("gvisor"));
            tun_map.insert("endpoint_independent_nat".to_string(), json!(true));
            let outer_profile = entry_profile.unwrap_or(profile);
            if outer_profile.server.parse::<std::net::Ipv4Addr>().is_ok() {
                tun_map.insert(
                    "route_exclude_address".to_string(),
                    json!([format!("{}/32", outer_profile.server)]),
                );
            }

            if routing.split_tunneling {
                if routing.default_outbound == "direct" {
                    if !proxy_pkgs.is_empty()
                        && routing.proxy_domains.is_empty()
                        && routing.proxy_ips.is_empty()
                    {
                        tun_map.insert("include_package".to_string(), json!(proxy_pkgs));
                    }
                } else if !direct_pkgs.is_empty() {
                    tun_map.insert("exclude_package".to_string(), json!(direct_pkgs));
                }
            }

            *inbounds = vec![
                Value::Object(tun_map),
                json!({
                    "type": "mixed",
                    "tag": "vpn-tunnel-in",
                    "listen": "127.0.0.1",
                    "listen_port": 2081
                }),
            ];
        }
    }
    cfg
}

fn multi_protocol_short_label(protocol: &str) -> &'static str {
    match protocol {
        "vless" => "VLESS",
        "vless_ws" => "CDN-WS",
        "webrtc" => "WebRTC",
        "hysteria2" => "Hysteria2",
        "tuic" => "TUIC",
        "shadowtls" => "ShadowTLS",
        "shadowsocks" => "SS-2022",
        "ssh" => "SSH",
        "amneziawg" => "AWG 3.1",
        "awg_quic" => "AWG+QUIC",
        "awg_vless" => "AWG+Reality",
        "awg2" => "AWG 2.0",
        "wireguard" | "wg" => "WireGuard",
        _ => "VPN",
    }
}

fn sanitize_bundle_stem(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "neuroveil_multi".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_wireguard_endpoint(
    profile: &VpnProfile,
    ep_tag: &str,
    detour_tag: &str,
    active_peer_port: u16,
    include_listen_port: bool,
) -> (Value, Option<Value>) {
    let peer_server = if profile.protocol == "enigma" {
        "127.0.0.1"
    } else {
        profile.server.as_str()
    };
    let mut peer_obj = json!({
        "address": peer_server,
        "port": active_peer_port,
        "public_key": profile.awg_public_key.as_deref().unwrap_or(&profile.password),
        "allowed_ips": ["0.0.0.0/0", "::/0"]
    });
    if profile.protocol != "awg2" {
        let pka_int: u32 = if let Some(ref pka) = profile.awg_persistent_keepalive {
            let trimmed = pka.trim();
            if let Ok(num) = trimmed.parse::<u32>() {
                num
            } else if let Some(first) = trimmed
                .split('-')
                .next()
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                first
            } else {
                25
            }
        } else {
            25
        };
        peer_obj["persistent_keepalive_interval"] = json!(pka_int);
    } else if let Some(ref pka) = profile.awg_persistent_keepalive {
        let trimmed = pka.trim();
        if let Ok(num) = trimmed.parse::<u32>() {
            if num > 0 {
                peer_obj["persistent_keepalive_interval"] = json!(num);
            }
        }
    }

    if let Some(ref psk) = profile.awg_preshared_key {
        if !psk.trim().is_empty() {
            peer_obj["pre_shared_key"] = json!(psk.trim());
        }
    }

    let is_plain_wg = profile.protocol == "wireguard"
        || profile.protocol == "wg"
        || profile.protocol == "awg2"
        || profile.protocol == "enigma"
        || (profile.awg_jc.is_none()
            && profile.awg_s1.is_none()
            && profile.awg_header_protection_key.is_none());

    let mut ep = json!({
        "type": "wireguard",
        "tag": ep_tag,
        "address": [profile.awg_local_address.as_deref().unwrap_or(if profile.protocol == "awg2" { "10.8.1.2/24" } else if profile.protocol == "enigma" { "10.8.7.2/24" } else { "10.8.0.2/32" })],
        "private_key": profile.awg_private_key.as_deref().unwrap_or(&profile.password),
        "mtu": if profile.protocol == "enigma" { 1280 } else { 1420 },
        "peers": [peer_obj]
    });

    if include_listen_port && profile.protocol != "awg_vless" && profile.protocol != "enigma" {
        if let Some(lp) = profile.listen_port {
            if lp > 0 {
                ep["listen_port"] = json!(lp);
            }
        }
    }

    if !is_plain_wg {
        ep["mtu"] = json!(1280);
        ep["jc"] = json!(profile.awg_jc.unwrap_or(4));
        ep["jmin"] = json!(profile.awg_jmin.unwrap_or(40));
        ep["jmax"] = json!(profile.awg_jmax.unwrap_or(70));
        ep["s1"] = json!(profile.awg_s1.unwrap_or(15));
        ep["s2"] = json!(profile.awg_s2.unwrap_or(25));
        let h1_val = profile
            .awg_h1
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(1);
        let h2_val = profile
            .awg_h2
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(2);
        let h3_val = profile
            .awg_h3
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(3);
        let h4_val = profile
            .awg_h4
            .as_deref()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(4);
        ep["h1"] = json!(h1_val);
        ep["h2"] = json!(h2_val);
        ep["h3"] = json!(h3_val);
        ep["h4"] = json!(h4_val);
        if let Some(s3) = profile.awg_s3 {
            ep["s3"] = json!(s3);
        }
        if let Some(s4) = profile.awg_s4 {
            ep["s4"] = json!(s4);
        }

        if let Some(ref hpk) = profile.awg_header_protection_key {
            let trimmed_hpk = hpk.trim();
            if !trimmed_hpk.is_empty() {
                ep["header_protection_key"] = json!(trimmed_hpk);
                let s1 = profile.awg_s1.unwrap_or(12).max(12);
                let s2 = profile.awg_s2.unwrap_or(12).max(12);
                let s3 = profile.awg_s3.unwrap_or(12).max(12);
                let s4 = profile.awg_s4.unwrap_or(12).max(12);
                ep["s1"] = json!(s1);
                ep["s2"] = json!(s2);
                ep["s3"] = json!(s3);
                ep["s4"] = json!(s4);

                if let Some(ref r) = profile.awg_rekey_after_time {
                    if !r.trim().is_empty() {
                        ep["rekey_after_time"] = json!(r.trim());
                    }
                }
                if let Some(ref r) = profile.awg_rekey_timeout {
                    if !r.trim().is_empty() {
                        ep["rekey_timeout"] = json!(r.trim());
                    }
                }
                if let Some(ref r) = profile.awg_reject_after_time {
                    if !r.trim().is_empty() {
                        ep["reject_after_time"] = json!(r.trim());
                    }
                }
                if let Some(ref r) = profile.awg_keepalive_timeout {
                    if !r.trim().is_empty() {
                        ep["keepalive_timeout"] = json!(r.trim());
                    }
                }
            }
        }

        if let Some(rt) = profile.awg_random_trailers {
            ep["random_trailers"] = json!(rt);
        }
        if let Some(dc) = profile.awg_disable_cookies {
            ep["disable_cookies"] = json!(dc);
        }
        if let Some(ref mha) = profile.awg_max_handshake_attempts {
            let trimmed = mha.trim();
            if let Ok(num) = trimmed.parse::<u32>() {
                ep["max_handshake_attempts"] = json!(num);
            } else if !trimmed.is_empty() {
                ep["max_handshake_attempts"] = json!(trimmed);
            }
        }

        if let Some(ref i1) = profile.awg_i1 {
            if !i1.trim().is_empty() {
                ep["i1"] = json!(i1.trim());
            }
        }
        if let Some(ref i2) = profile.awg_i2 {
            if !i2.trim().is_empty() {
                ep["i2"] = json!(i2.trim());
            }
        }
        if let Some(ref i3) = profile.awg_i3 {
            if !i3.trim().is_empty() {
                ep["i3"] = json!(i3.trim());
            }
        }
        if let Some(ref i4) = profile.awg_i4 {
            if !i4.trim().is_empty() {
                ep["i4"] = json!(i4.trim());
            }
        }
    }

    let mut vless_detour_opt = None;
    if profile.protocol == "awg_vless" {
        ep["detour"] = json!(detour_tag);
        let flow = profile.reality_flow.as_deref().unwrap_or("xtls-rprx-vision");
        let tls_frag = profile.tls_fragment.unwrap_or(true);
        let vless_sni = if !profile.sni.trim().is_empty() {
            profile.sni.trim()
        } else {
            "www.microsoft.com"
        };
        let mut tls_obj = json!({
            "enabled": true,
            "server_name": vless_sni,
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
        if let Some(ref pbk) = profile.reality_public_key {
            if !pbk.trim().is_empty() {
                let mut reality_obj = json!({
                    "enabled": true,
                    "public_key": pbk.trim()
                });
                if let Some(ref sid) = profile.reality_short_id {
                    if !sid.trim().is_empty() {
                        reality_obj["short_id"] = json!(sid.trim());
                    }
                }
                tls_obj["reality"] = reality_obj;
            }
        }
        vless_detour_opt = Some(json!({
            "type": "vless",
            "tag": detour_tag,
            "server": profile.server,
            "server_port": profile.vless_port.unwrap_or(443),
            "uuid": profile.uuid.clone().unwrap_or_else(|| profile.password.clone()),
            "flow": flow,
            "packet_encoding": "xudp",
            "tls": tls_obj
        }));
    }

    (ep, vless_detour_opt)
}

pub fn generate_multi_singbox_config(
    profiles: &[VpnProfile],
    group_name: &str,
    routing: &RoutingConfig,
) -> Value {
    let mut endpoints = Vec::new();
    let mut outbounds = Vec::new();
    let mut proto_tags = Vec::new();
    let mut used_tags = std::collections::HashSet::new();
    let mut server_excludes: Vec<String> = Vec::new();

    let base_title = group_name.trim();

    for (idx, prof) in profiles.iter().enumerate() {
        let short = multi_protocol_short_label(&prof.protocol);
        let prefix = if !base_title.is_empty() {
            base_title.to_string()
        } else {
            prof.name
                .split(" (")
                .next()
                .unwrap_or(&prof.name)
                .trim()
                .to_string()
        };
        let mut tag = if prefix.is_empty() {
            format!("{} #{}", short, idx + 1)
        } else {
            format!("{} [{}]", prefix, short)
        };
        if used_tags.contains(&tag)
            || matches!(tag.as_str(), "proxy" | "auto" | "direct" | "block")
        {
            tag = format!("{} [{} #{}]", prefix, short, idx + 1);
        }
        used_tags.insert(tag.clone());

        if prof.server.parse::<std::net::Ipv4Addr>().is_ok() {
            let cidr = format!("{}/32", prof.server);
            if !server_excludes.contains(&cidr) {
                server_excludes.push(cidr);
            }
        }

        if is_awg_protocol(&prof.protocol) {
            let detour_tag = format!("vless-detour-{}", idx + 1);
            let (ep, detour_opt) =
                build_wireguard_endpoint(prof, &tag, &detour_tag, prof.port, false);
            endpoints.push(ep);
            proto_tags.push(tag.clone());
            if let Some(d_ob) = detour_opt {
                outbounds.push(d_ob);
            }
        } else {
            let mut ob = build_single_outbound(prof, &tag, None, None);
            if let Some(obj) = ob.as_object_mut() {
                obj.remove("inet4_bind_address");
            }
            outbounds.push(ob);
            proto_tags.push(tag.clone());

            if prof.protocol == "shadowtls" {
                let st_sni = prof.shadowtls_sni.as_deref().unwrap_or(if !prof.sni.is_empty() {
                    &prof.sni
                } else {
                    "www.microsoft.com"
                });
                let st_pass = prof.shadowtls_password.as_deref().unwrap_or(&prof.password);
                outbounds.push(json!({
                    "type": "shadowtls",
                    "tag": format!("{}-st-detour", tag),
                    "server": prof.server,
                    "server_port": prof.port,
                    "version": 3,
                    "password": st_pass,
                    "tls": {
                        "enabled": true,
                        "server_name": st_sni,
                        "utls": {
                            "enabled": true,
                            "fingerprint": "chrome"
                        }
                    }
                }));
            }
        }
    }

    if proto_tags.is_empty() {
        proto_tags.push("direct".to_string());
    } else {
        outbounds.push(json!({
            "type": "urltest",
            "tag": "auto",
            "outbounds": proto_tags.clone(),
            "url": "https://www.gstatic.com/generate_204",
            "interval": "3m",
            "tolerance": 50
        }));

        let mut selector_outbounds = vec!["auto".to_string()];
        selector_outbounds.extend(proto_tags.clone());
        outbounds.push(json!({
            "type": "selector",
            "tag": "proxy",
            "outbounds": selector_outbounds,
            "default": proto_tags[0]
        }));
    }

    outbounds.push(json!({
        "type": "direct",
        "tag": "direct"
    }));
    outbounds.push(json!({
        "type": "block",
        "tag": "block"
    }));

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
    tun_map.insert("endpoint_independent_nat".to_string(), json!(true));
    tun_map.insert("udp_mapping".to_string(), json!("endpoint_independent"));
    tun_map.insert("udp_filtering".to_string(), json!("endpoint_independent"));
    if !server_excludes.is_empty() {
        tun_map.insert("route_exclude_address".to_string(), json!(server_excludes));
    }
    let inbounds = vec![Value::Object(tun_map)];

    let mut route_rules = vec![
        json!({
            "port": 53,
            "action": "hijack-dns"
        }),
        json!({
            "action": "sniff",
            "timeout": "300ms"
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
    route_rules.push(json!({
        "ip_is_private": true,
        "outbound": "direct"
    }));
    route_rules.push(json!({
        "domain_suffix": ["steamcontent.com", "steamserver.net"],
        "outbound": "direct"
    }));

    let mut dns_rules = vec![json!({
        "domain_suffix": ["steamcontent.com", "steamserver.net"],
        "server": "dns-direct"
    })];

    let final_target = if routing.split_tunneling && routing.default_outbound == "direct" {
        "direct"
    } else {
        "proxy"
    };

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
            let proxy_cidrs: Vec<String> = routing
                .proxy_ips
                .iter()
                .map(|ip| ip.trim())
                .filter(|ip| !ip.is_empty())
                .map(|ip| {
                    if ip.contains('/') {
                        ip.to_string()
                    } else {
                        format!("{}/32", ip)
                    }
                })
                .collect();
            if !proxy_cidrs.is_empty() {
                route_rules.push(json!({
                    "ip_cidr": proxy_cidrs,
                    "outbound": "proxy"
                }));
            }
        }

        if !routing.direct_ips.is_empty() {
            let direct_cidrs: Vec<String> = routing
                .direct_ips
                .iter()
                .map(|ip| ip.trim())
                .filter(|ip| !ip.is_empty())
                .map(|ip| {
                    if ip.contains('/') {
                        ip.to_string()
                    } else {
                        format!("{}/32", ip)
                    }
                })
                .collect();
            if !direct_cidrs.is_empty() {
                route_rules.push(json!({
                    "ip_cidr": direct_cidrs,
                    "outbound": "direct"
                }));
            }
        }
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

    let all_tcp_only = !profiles.is_empty()
        && profiles
            .iter()
            .all(|p| p.protocol == "ssh" || p.protocol == "shadowtls");
    let remote_dns_type = if all_tcp_only { "tcp" } else { "udp" };

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
            "strategy": "ipv4_only",
            "reverse_mapping": true
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "final": final_target,
            "auto_detect_interface": true,
            "override_android_vpn": true,
            "default_domain_resolver": "dns-direct"
        }
    });

    if !endpoints.is_empty() {
        cfg["endpoints"] = json!(endpoints);
    }
    cfg
}

pub fn build_multi_zip_bytes(
    profiles: &[VpnProfile],
    group_name: &str,
    routing: &RoutingConfig,
) -> Result<Vec<u8>, String> {
    if profiles.is_empty() {
        return Err("Список профилей пуст".to_string());
    }
    let stem = sanitize_bundle_stem(group_name);
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let multi_json = generate_multi_singbox_config(profiles, group_name, routing);
    let multi_json_str = serde_json::to_string_pretty(&multi_json)
        .map_err(|e| format!("Ошибка сериализации JSON: {}", e))?;
    writer
        .start_file(format!("{}_multi.json", stem), opts)
        .map_err(|e| format!("Ошибка создания файла в архиве: {}", e))?;
    writer
        .write_all(multi_json_str.as_bytes())
        .map_err(|e| format!("Ошибка записи JSON в архив: {}", e))?;

    let mut keys_txt = format!(
        "# NeuroVeil VPN Multi-Protocol Bundle: {}\n# Protocols: {}\n\n",
        group_name.trim(),
        profiles.len()
    );
    for p in profiles {
        let uri = if p.uri.trim().is_empty() {
            build_profile_uri(p)
        } else {
            p.uri.clone()
        };
        keys_txt.push_str(&format!("# {}\n{}\n\n", p.name, uri));
    }
    writer
        .start_file(format!("{}_keys.txt", stem), opts)
        .map_err(|e| format!("Ошибка создания _keys.txt в архиве: {}", e))?;
    writer
        .write_all(keys_txt.as_bytes())
        .map_err(|e| format!("Ошибка записи _keys.txt в архив: {}", e))?;

    for (idx, p) in profiles.iter().enumerate() {
        if p.protocol == "enigma" {
            if let Some(ref b64) = p.enigma_keybook {
                if let Ok(kb) = crate::enigma::EnigmaKeybook::from_base64(b64) {
                    let nv_name = format!("{}_enigma_{}.nvkey", stem, idx + 1);
                    writer
                        .start_file(nv_name, opts)
                        .map_err(|e| format!("Ошибка создания .nvkey в архиве: {}", e))?;
                    writer
                        .write_all(&kb.raw)
                        .map_err(|e| format!("Ошибка записи .nvkey в архиве: {}", e))?;
                }
            }
        } else if is_awg_protocol(&p.protocol) {
            let proto_slug = match p.protocol.as_str() {
                "awg_quic" => "awg_quic",
                "awg_vless" => "awg_reality",
                "awg2" => "awg2",
                "wireguard" | "wg" => "wireguard",
                _ => "awg31",
            };
            let conf_name = format!("{}_{}_{}.conf", stem, proto_slug, idx + 1);
            let conf_body = format!("# Name: {}\n{}", p.name, build_awg_conf(p));
            writer
                .start_file(conf_name, opts)
                .map_err(|e| format!("Ошибка создания .conf в архиве: {}", e))?;
            writer
                .write_all(conf_body.as_bytes())
                .map_err(|e| format!("Ошибка записи .conf в архив: {}", e))?;
        }
    }

    let finished = writer
        .finish()
        .map_err(|e| format!("Ошибка финализации ZIP-архива: {}", e))?;
    Ok(finished.into_inner())
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
    } else if profile.protocol == "enigma" {
        let priv_key = profile.awg_private_key.as_deref().unwrap_or(&profile.password);
        let pub_key = profile.awg_public_key.as_deref().unwrap_or_default();
        let addr = profile.awg_local_address.as_deref().unwrap_or("10.8.7.2/24");
        let psk = profile.awg_preshared_key.as_deref().unwrap_or_default();
        let mimic = profile.enigma_mimic.as_deref().unwrap_or("auto");
        let mut params = format!(
            "public_key={}&address={}&mimic={}",
            urlencoding::encode(pub_key),
            urlencoding::encode(addr),
            urlencoding::encode(mimic)
        );
        if !psk.is_empty() {
            params.push_str(&format!("&psk={}", urlencoding::encode(psk)));
        }
        if let Some(ref hop) = profile.hop_ports.as_ref().or(profile.awg_hop_ports.as_ref()) {
            if !hop.trim().is_empty() {
                params.push_str(&format!("&hop_ports={}", urlencoding::encode(hop.trim())));
            }
        }
        if let Some(ref kb) = profile.enigma_keybook {
            if !kb.trim().is_empty() {
                params.push_str(&format!("&keybook={}", urlencoding::encode(kb.trim())));
            }
        }
        if let Some(ref kp) = profile.enigma_key_path {
            if !kp.trim().is_empty() {
                params.push_str(&format!("&key_path={}", urlencoding::encode(kp.trim())));
            }
        }
        format!(
            "enigma://{}@{}:{}?{}#{}",
            urlencoding::encode(priv_key),
            profile.server,
            profile.port,
            params,
            encoded_name
        )
    } else if is_awg_protocol(&profile.protocol) {
        let priv_key = profile.awg_private_key.as_deref().unwrap_or(&profile.password);
        let pub_key = profile.awg_public_key.as_deref().unwrap_or_default();
        let addr = profile.awg_local_address.as_deref().unwrap_or(if profile.protocol == "awg2" { "10.8.1.2/24" } else { "10.8.0.2/32" });
        let psk = profile.awg_preshared_key.as_deref().unwrap_or_default();

        let is_clean_wg_or_awg2 = profile.protocol == "wireguard"
            || profile.protocol == "wg"
            || profile.protocol == "awg2";

        let mut params = if is_clean_wg_or_awg2 {
            format!(
                "public_key={}&address={}",
                urlencoding::encode(pub_key),
                urlencoding::encode(addr),
            )
        } else {
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

            let mut p = format!(
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
                p.push_str(&format!("&s3={}", s3));
            }
            if let Some(s4) = profile.awg_s4 {
                p.push_str(&format!("&s4={}", s4));
            }
            if let Some(ref hpk) = profile.awg_header_protection_key {
                if !hpk.trim().is_empty() {
                    p.push_str(&format!("&header_protection_key={}", urlencoding::encode(hpk.trim())));
                }
            }
            if let Some(ref r) = profile.awg_rekey_after_time {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&rekey_after_time={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(ref r) = profile.awg_rekey_timeout {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&rekey_timeout={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(ref r) = profile.awg_reject_after_time {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&reject_after_time={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(ref r) = profile.awg_keepalive_timeout {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&keepalive_timeout={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(ref r) = profile.awg_max_handshake_attempts {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&max_handshake_attempts={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(ref r) = profile.awg_persistent_keepalive {
                if !r.trim().is_empty() {
                    p.push_str(&format!("&persistent_keepalive={}", urlencoding::encode(r.trim())));
                }
            }
            if let Some(rt) = profile.awg_random_trailers {
                p.push_str(&format!("&random_trailers={}", if rt { "on" } else { "off" }));
            }
            if let Some(dc) = profile.awg_disable_cookies {
                p.push_str(&format!("&disable_cookies={}", if dc { "on" } else { "off" }));
            }
            if let Some(ref i1) = profile.awg_i1 {
                p.push_str(&format!("&i1={}", urlencoding::encode(i1)));
            }
            if let Some(ref i2) = profile.awg_i2 {
                p.push_str(&format!("&i2={}", urlencoding::encode(i2)));
            }
            if let Some(ref i3) = profile.awg_i3 {
                p.push_str(&format!("&i3={}", urlencoding::encode(i3)));
            }
            if let Some(ref i4) = profile.awg_i4 {
                p.push_str(&format!("&i4={}", urlencoding::encode(i4)));
            }
            if !hop.is_empty() {
                p.push_str(&format!("&hop_ports={}", urlencoding::encode(hop)));
            }
            p
        };

        if profile.protocol == "awg2" {
            params.push_str("&version=2.0");
        } else if !is_clean_wg_or_awg2 {
            if let Some(ref v) = profile.awg_version {
                params.push_str(&format!("&version={}", urlencoding::encode(v.trim())));
            }
        }
        if !psk.is_empty() {
            params.push_str(&format!("&psk={}", urlencoding::encode(psk)));
        }
        if let Some(lp) = profile.listen_port {
            if lp > 0 {
                params.push_str(&format!("&listen_port={}", lp));
            }
        }
        if profile.protocol == "awg_vless" {
            params.push_str(&format!("&vless_port={}", profile.vless_port.unwrap_or(443)));
            if let Some(ref uid) = profile.uuid {
                if !uid.trim().is_empty() {
                    params.push_str(&format!("&uuid={}", urlencoding::encode(uid.trim())));
                }
            }
            if let Some(ref pbk) = profile.reality_public_key {
                if !pbk.trim().is_empty() {
                    params.push_str(&format!("&pbk={}", urlencoding::encode(pbk.trim())));
                }
            }
            if let Some(ref sid) = profile.reality_short_id {
                if !sid.trim().is_empty() {
                    params.push_str(&format!("&sid={}", urlencoding::encode(sid.trim())));
                }
            }
            if !profile.sni.trim().is_empty() {
                params.push_str(&format!("&sni={}", urlencoding::encode(profile.sni.trim())));
            }
        }

        let scheme = if profile.protocol == "wireguard" || profile.protocol == "wg" {
            "wg"
        } else if profile.protocol == "awg_quic" {
            "awgquic"
        } else if profile.protocol == "awg_vless" {
            "awgvless"
        } else if profile.protocol == "awg2" || profile.awg_version.as_deref() == Some("2.0") {
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
    } else if profile.protocol == "shadowtls" {
        let method = profile.ss_method.as_deref().unwrap_or("2022-blake3-aes-128-gcm");
        let st_sni = profile.shadowtls_sni.as_deref().unwrap_or(if !profile.sni.is_empty() { &profile.sni } else { "www.microsoft.com" });
        let st_pass = profile.shadowtls_password.as_deref().unwrap_or(&profile.password);
        format!(
            "shadowtls://{}@{}:{}?method={}&password={}&st_password={}&sni={}#{}",
            urlencoding::encode(&profile.password),
            profile.server,
            profile.port,
            urlencoding::encode(method),
            urlencoding::encode(&profile.password),
            urlencoding::encode(st_pass),
            urlencoding::encode(st_sni),
            encoded_name
        )
    } else if profile.protocol == "vless_ws" {
        let uuid = profile.uuid.clone().unwrap_or_else(|| profile.password.clone());
        let ws_path = profile.ws_path.as_deref().unwrap_or("/neuroveil-ws");
        let host = profile.cdn_host.as_deref().unwrap_or(&profile.sni);
        format!(
            "vless://{}@{}:{}?type=ws&path={}&host={}&sni={}&security=tls&insecure={}&fp=chrome#{}",
            urlencoding::encode(&uuid),
            profile.server,
            profile.port,
            urlencoding::encode(ws_path),
            urlencoding::encode(host),
            urlencoding::encode(host),
            insecure_val,
            encoded_name
        )
    } else if profile.protocol == "webrtc" {
        let uuid = profile.uuid.clone().unwrap_or_else(|| profile.password.clone());
        let service = profile.ws_path.as_deref().unwrap_or("neuroveil-webrtc");
        let sni = if !profile.sni.is_empty() { &profile.sni } else { &profile.server };
        format!(
            "vless://{}@{}:{}?type=grpc&serviceName={}&sni={}&security=tls&insecure={}&fp=chrome#{}",
            urlencoding::encode(&uuid),
            profile.server,
            profile.port,
            urlencoding::encode(service),
            urlencoding::encode(sni),
            insecure_val,
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

    if profile.protocol == "awg2" {
        let addr = profile.awg_local_address.as_deref().unwrap_or("10.8.1.2/24");
        let mut conf = String::new();
        conf.push_str("[Interface]\n");
        conf.push_str(&format!("PrivateKey = {}\n", priv_key));
        conf.push_str(&format!("Address = {}\n", addr));
        conf.push_str("DNS = 1.1.1.1\n");
        conf.push_str("MTU = 1420\n");
        conf.push_str("\n[Peer]\n");
        conf.push_str(&format!("PublicKey = {}\n", pub_key));
        if let Some(ref psk) = profile.awg_preshared_key {
            if !psk.trim().is_empty() {
                conf.push_str(&format!("PresharedKey = {}\n", psk.trim()));
            }
        }
        conf.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
        conf.push_str(&format!("Endpoint = {}:{}\n", profile.server, profile.port));
        return conf;
    }

    let addr = profile.awg_local_address.as_deref().unwrap_or("10.8.0.2/32");

    let is_wireguard = profile.protocol == "wireguard"
        || profile.protocol == "wg"
        || (profile.awg_jc.is_none() && profile.awg_s1.is_none() && profile.awg_h1.is_none());

    if is_wireguard {
        let mut conf = String::new();
        conf.push_str("[Interface]\n");
        conf.push_str(&format!("Address = {}\n", addr));
        conf.push_str("DNS = 1.1.1.1, 1.0.0.1\n");
        conf.push_str(&format!("PrivateKey = {}\n", priv_key));
        if let Some(lp) = profile.listen_port {
            if lp > 0 {
                conf.push_str(&format!("ListenPort = {}\n", lp));
            }
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
        let pk = profile.awg_persistent_keepalive.as_deref().unwrap_or("25");
        conf.push_str(&format!("PersistentKeepalive = {}\n", pk));
        return conf;
    }

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
    if let Some(lp) = profile.listen_port {
        if lp > 0 {
            conf.push_str(&format!("ListenPort = {}\n", lp));
        }
    }
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
                    if out_type == "shadowsocks" {
                        if let Some(detour_tag) = out["detour"].as_str() {
                            if let Some(st_out) = outbounds.iter().find(|o| o["tag"].as_str() == Some(detour_tag) && o["type"].as_str() == Some("shadowtls")) {
                                let server = st_out["server"].as_str().unwrap_or("").to_string();
                                if !server.is_empty() {
                                    let port = st_out["server_port"].as_u64().unwrap_or(8443) as u16;
                                    let ss_password = out["password"].as_str().unwrap_or("").to_string();
                                    let st_password = st_out["password"].as_str().map(|s| s.to_string());
                                    let st_sni = st_out["tls"]["server_name"].as_str().unwrap_or("www.microsoft.com").to_string();
                                    let ss_method = out["method"].as_str().map(|s| s.to_string());
                                    let name = out["tag"].as_str().filter(|t| !t.is_empty() && *t != "proxy").unwrap_or("ShadowTLS-import").to_string();
                                    let mut prof = VpnProfile {
                                        id: Uuid::new_v4().to_string(),
                                        name,
                                        protocol: "shadowtls".to_string(),
                                        server,
                                        port,
                                        sni: st_sni.clone(),
                                        shadowtls_sni: Some(st_sni),
                                        shadowtls_password: st_password,
                                        password: ss_password,
                                        ss_method,
                                        up_mbps: 100,
                                        down_mbps: 100,
                                        tls_fragment: Some(false),
                                        ..Default::default()
                                    };
                                    prof.uri = build_profile_uri(&prof);
                                    return Ok(prof);
                                }
                            }
                        }
                    }
                    if out_type == "vless" || out_type == "hysteria2" || out_type == "tuic" || out_type == "shadowsocks" || out_type == "ssh" {
                        let server = out["server"].as_str().unwrap_or("").to_string();
                        if server.is_empty() {
                            continue;
                        }
                        let transport_type = out["transport"]["type"].as_str().unwrap_or("");
                        let actual_protocol = if out_type == "vless" && transport_type == "ws" {
                            "vless_ws"
                        } else if out_type == "vless" && transport_type == "grpc" {
                            "webrtc"
                        } else {
                            out_type
                        };
                        let default_port = match actual_protocol {
                            "shadowsocks" => 8388,
                            "ssh" => 22,
                            "vless_ws" => 8080,
                            "webrtc" => 2053,
                            _ => 443,
                        };
                        let port = out["server_port"].as_u64().unwrap_or(default_port) as u16;
                        let password = out["password"].as_str().or_else(|| out["private_key"].as_str()).unwrap_or("").to_string();
                        let sni = if actual_protocol == "ssh" {
                            out["user"].as_str().unwrap_or("root").to_string()
                        } else {
                            out["tls"]["server_name"].as_str().unwrap_or("www.bing.com").to_string()
                        };
                        let insecure = out["tls"]["insecure"].as_bool().unwrap_or(actual_protocol != "vless" && actual_protocol != "ssh");
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
                        let ws_path = if actual_protocol == "vless_ws" {
                            out["transport"]["path"].as_str().map(|s| s.to_string())
                        } else if actual_protocol == "webrtc" {
                            out["transport"]["service_name"].as_str().map(|s| s.to_string())
                        } else {
                            None
                        };
                        let cdn_host = if actual_protocol == "vless_ws" {
                            out["transport"]["headers"]["Host"].as_str().map(|s| s.to_string())
                        } else {
                            None
                        };
                        let name = out["tag"].as_str().filter(|t| !t.is_empty() && *t != "proxy").unwrap_or(&format!("{}-import", actual_protocol)).to_string();
                        let mut prof = VpnProfile {
                            id: Uuid::new_v4().to_string(),
                            name,
                            protocol: actual_protocol.to_string(),
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
                            ws_path,
                            cdn_host,
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
                            hop_ports: Some("20000:65000".to_string()),
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
                            awg_hop_ports: Some("20000:65000".to_string()),
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

    if trimmed.to_lowercase().starts_with("awg://")
        || trimmed.to_lowercase().starts_with("amneziawg://")
        || trimmed.to_lowercase().starts_with("awg2://")
        || trimmed.to_lowercase().starts_with("awgquic://")
        || trimmed.to_lowercase().starts_with("awgvless://")
        || trimmed.to_lowercase().starts_with("enigma://")
        || trimmed.to_lowercase().starts_with("wg://")
        || trimmed.to_lowercase().starts_with("wireguard://")
    {
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
    let mut insecure_explicit = false;
    let mut obfs_password = None;
    let mut hop_ports = None;
    let mut hop_interval = None;
    let mut reality_public_key = None;
    let mut reality_short_id = None;
    let mut reality_flow = Some("xtls-rprx-vision".to_string());
    let mut listen_port = None;
    let mut transport_type = String::new();
    let mut ws_path = None;
    let mut cdn_host = None;
    let mut grpc_service = None;
    let mut ss_method = None;
    let mut st_password = None;
    let mut query_password = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "listen_port" | "listenport" | "lp" => {
                listen_port = v.parse().ok();
            }
            "sni" | "peer" => sni = v.to_string(),
            "insecure" | "allow_insecure" | "allowInsecure" => {
                insecure = v == "1" || v.eq_ignore_ascii_case("true");
                insecure_explicit = true;
            }
            "obfs-password" | "obfspassword" => {
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
            "type" => {
                transport_type = v.to_lowercase();
            }
            "path" => {
                if !v.is_empty() {
                    ws_path = Some(v.to_string());
                }
            }
            "host" => {
                if !v.is_empty() {
                    cdn_host = Some(v.to_string());
                }
            }
            "serviceName" | "service_name" => {
                if !v.is_empty() {
                    grpc_service = Some(v.to_string());
                }
            }
            "method" => {
                if !v.is_empty() {
                    ss_method = Some(v.to_string());
                }
            }
            "st_password" => {
                if !v.is_empty() {
                    st_password = Some(v.to_string());
                }
            }
            "password" => {
                if !v.is_empty() {
                    query_password = Some(v.to_string());
                }
            }
            _ => {}
        }
    }

    if scheme == "vless" {
        let uuid = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        if transport_type == "ws" {
            let mut prof = VpnProfile {
                id: Uuid::new_v4().to_string(),
                name,
                protocol: "vless_ws".to_string(),
                server: server.clone(),
                port,
                listen_port,
                sni: sni.clone(),
                insecure: if insecure_explicit { insecure } else { true },
                password: uuid.clone(),
                uuid: Some(uuid),
                ws_path: ws_path.or_else(|| Some("/neuroveil-ws".to_string())),
                cdn_host: cdn_host.or_else(|| Some(sni)),
                ..Default::default()
            };
            prof.uri = build_profile_uri(&prof);
            Ok(prof)
        } else if transport_type == "grpc" {
            let mut prof = VpnProfile {
                id: Uuid::new_v4().to_string(),
                name,
                protocol: "webrtc".to_string(),
                server,
                port,
                listen_port,
                sni,
                insecure: if insecure_explicit { insecure } else { true },
                password: uuid.clone(),
                uuid: Some(uuid),
                ws_path: grpc_service.or(ws_path).or_else(|| Some("neuroveil-webrtc".to_string())),
                ..Default::default()
            };
            prof.uri = build_profile_uri(&prof);
            Ok(prof)
        } else {
            let mut prof = VpnProfile {
                id: Uuid::new_v4().to_string(),
                name,
                protocol: "vless".to_string(),
                server,
                port,
                listen_port,
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
        }
    } else if scheme == "shadowtls" {
        let user_pass = urlencoding::decode(parsed.username())
            .unwrap_or_else(|_| parsed.username().to_string());
        let ss_pass = query_password.unwrap_or(user_pass);
        let mut prof = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: "shadowtls".to_string(),
            server,
            port,
            listen_port,
            sni: sni.clone(),
            shadowtls_sni: Some(sni),
            shadowtls_password: st_password.or_else(|| Some(ss_pass.clone())),
            password: ss_pass,
            ss_method: ss_method.or_else(|| Some("2022-blake3-aes-128-gcm".to_string())),
            tls_fragment: Some(false),
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
            listen_port,
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
            listen_port,
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
            "Неподдерживаемый протокол '{}'. Поддерживаются ssh://, awg://, wg://, vless://, shadowtls://, ss://, hy2://, hysteria2:// и tuic://",
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
    let lower = raw.to_lowercase();
    let is_explicit_wg = lower.starts_with("wg://") || lower.starts_with("wireguard://");
    let is_explicit_awg2 = lower.starts_with("awg2://");
    let is_explicit_awg_quic = lower.starts_with("awgquic://");
    let is_explicit_awg_vless = lower.starts_with("awgvless://");
    let is_explicit_enigma = lower.starts_with("enigma://");
    let without_scheme = if is_explicit_enigma {
        &raw[9..]
    } else if let Some(s) = raw.strip_prefix("awg://") {
        s
    } else if let Some(s) = raw.strip_prefix("amneziawg://") {
        s
    } else if let Some(s) = raw.strip_prefix("awg2://") {
        s
    } else if let Some(s) = raw.strip_prefix("awgquic://") {
        s
    } else if let Some(s) = raw.strip_prefix("awgvless://") {
        s
    } else if let Some(s) = raw.strip_prefix("wg://") {
        s
    } else if let Some(s) = raw.strip_prefix("wireguard://") {
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
            if is_explicit_enigma {
                "Enigma v1 (Keybook)".to_string()
            } else if is_explicit_awg_vless {
                "Hybrid AWG + VLESS Reality".to_string()
            } else if is_explicit_awg_quic {
                "AmneziaWG + QUIC Pre-Warm".to_string()
            } else if is_explicit_awg2 {
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
        None => return Err("Некорректная awg:// / enigma:// ссылка: отсутствует разделитель @".to_string()),
    };

    let priv_key = urlencoding::decode(priv_key_raw).unwrap_or_else(|_| priv_key_raw.to_string());
    let (server, port) = parse_host_port(host_port, 51820)?;

    let mut pub_key = None;
    let mut psk = None;
    let mut addr = Some(if is_explicit_enigma {
        "10.8.7.2/24".to_string()
    } else if is_explicit_awg2 {
        "10.8.1.2/24".to_string()
    } else {
        "10.8.0.2/32".to_string()
    });
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
    let mut hop_ports = None;
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
    let mut listen_port = None;
    let mut vless_port = None;
    let mut vless_uuid = None;
    let mut reality_public_key = None;
    let mut reality_short_id = None;
    let mut vless_sni = String::new();
    let mut enigma_keybook = None;
    let mut enigma_key_path = None;
    let mut enigma_mimic = None;

    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                let dec_v = urlencoding::decode(v).unwrap_or_else(|_| v.to_string());
                match k.to_lowercase().as_str() {
                    "listen_port" | "listenport" | "lp" => listen_port = dec_v.parse().ok(),
                    "vless_port" | "vlessport" => vless_port = dec_v.parse().ok(),
                    "uuid" => vless_uuid = Some(dec_v),
                    "pbk" | "reality_public_key" => reality_public_key = Some(dec_v),
                    "sid" | "reality_short_id" => reality_short_id = Some(dec_v),
                    "sni" => vless_sni = dec_v,
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
                    "keybook" | "enigma_keybook" | "kb" => enigma_keybook = Some(dec_v),
                    "key_path" | "enigma_key_path" => enigma_key_path = Some(dec_v),
                    "mimic" | "enigma_mimic" => enigma_mimic = Some(dec_v),
                    _ => {}
                }
            }
        }
    }

    let is_v2 = !is_explicit_enigma && (is_explicit_awg2 || parsed_version.as_deref() == Some("2.0") || parsed_version.as_deref() == Some("2"));
    let protocol = if is_explicit_enigma {
        "enigma".to_string()
    } else if is_explicit_wg {
        "wireguard".to_string()
    } else if is_explicit_awg_vless {
        "awg_vless".to_string()
    } else if is_explicit_awg_quic {
        "awg_quic".to_string()
    } else if is_v2 {
        "awg2".to_string()
    } else {
        "amneziawg".to_string()
    };
    let version_str = if is_explicit_wg || is_explicit_enigma {
        None
    } else if is_v2 {
        Some("2.0".to_string())
    } else {
        Some("3.1".to_string())
    };

    let is_clean = protocol == "wireguard" || protocol == "awg2" || protocol == "enigma";
    let (final_jc, final_jmin, final_jmax, final_s1, final_s2, final_s3, final_s4, final_h1, final_h2, final_h3, final_h4, final_hop_ports, final_probing) = if protocol == "enigma" {
        (None, None, None, None, None, None, None, None, None, None, None, hop_ports.clone(), Some(true))
    } else if is_clean {
        (None, None, None, None, None, None, None, None, None, None, None, None, Some(false))
    } else {
        (
            jc.or(Some(4)),
            jmin.or(Some(40)),
            jmax.or(Some(70)),
            s1.or(Some(15)),
            s2.or(Some(25)),
            s3.or(Some(35)),
            s4.or(Some(45)),
            h1.or_else(|| Some("1".to_string())),
            h2.or_else(|| Some("2".to_string())),
            h3.or_else(|| Some("3".to_string())),
            h4.or_else(|| Some("4".to_string())),
            hop_ports.or_else(|| Some("20000:65000".to_string())),
            Some(true),
        )
    };

    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol,
        server,
        port,
        vless_port,
        listen_port,
        sni: vless_sni,
        insecure: false,
        password: priv_key.clone(),
        uuid: vless_uuid,
        obfs_password: None,
        hop_ports: final_hop_ports.clone(),
        hop_interval: None,
        reality_public_key,
        reality_short_id,
        reality_flow: if is_explicit_awg_vless { Some("xtls-rprx-vision".to_string()) } else { None },
        ss_method: None,
        ss_plugin: None,
        ss_plugin_opts: None,
        awg_private_key: Some(priv_key),
        awg_public_key: pub_key,
        awg_preshared_key: psk,
        awg_local_address: addr,
        awg_jc: final_jc,
        awg_jmin: final_jmin,
        awg_jmax: final_jmax,
        awg_s1: final_s1,
        awg_s2: final_s2,
        awg_s3: final_s3,
        awg_s4: final_s4,
        awg_h1: final_h1,
        awg_h2: final_h2,
        awg_h3: final_h3,
        awg_h4: final_h4,
        awg_i1: if is_clean { None } else { i1 },
        awg_i2: if is_clean { None } else { i2 },
        awg_i3: if is_clean { None } else { i3 },
        awg_i4: if is_clean { None } else { i4 },
        awg_hop_ports: final_hop_ports,
        awg_port_probing: final_probing,
        awg_header_protection_key: if is_clean { None } else { header_protection_key },
        awg_rekey_after_time: if is_clean { None } else { rekey_after_time },
        awg_rekey_timeout: if is_clean { None } else { rekey_timeout },
        awg_reject_after_time: if is_clean { None } else { reject_after_time },
        awg_keepalive_timeout: if is_clean { None } else { keepalive_timeout },
        awg_max_handshake_attempts: if is_clean { None } else { max_handshake_attempts },
        awg_random_trailers: if is_clean { None } else { random_trailers },
        awg_disable_cookies: if is_clean { None } else { disable_cookies },
        awg_persistent_keepalive: if is_clean { None } else { persistent_keepalive },
        awg_version: version_str,
        up_mbps: 100,
        down_mbps: 100,
        enigma_keybook,
        enigma_key_path,
        enigma_mimic: if is_explicit_enigma { enigma_mimic.or_else(|| Some("auto".to_string())) } else { None },
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
    let mut listen_port = None;
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
                "listenport" | "listen_port" => listen_port = val.parse().ok(),
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

    let is_v20 = jc.is_some()
        || jmin.is_some()
        || jmax.is_some()
        || s1.is_some()
        || s2.is_some()
        || h1.is_some()
        || h2.is_some()
        || h3.is_some()
        || h4.is_some();

    let (protocol, version_opt, default_title) = if is_v31 {
        ("amneziawg".to_string(), Some("3.1".to_string()), format!("Amnezia 3.1 ({})", server))
    } else if is_v20 {
        ("awg2".to_string(), Some("2.0".to_string()), format!("AmneziaWG 2.0 ({})", server))
    } else {
        ("wireguard".to_string(), None, format!("WireGuard ({})", server))
    };
    let name = parsed_name.unwrap_or(default_title);

    let (final_jc, final_jmin, final_jmax, final_s1, final_s2, final_s3, final_s4, final_h1, final_h2, final_h3, final_h4) = if is_v31 {
        (
            jc.or(Some(4)),
            jmin.or(Some(40)),
            jmax.or(Some(70)),
            s1.or(Some(12)),
            s2.or(Some(12)),
            s3.or(Some(12)),
            s4.or(Some(12)),
            h1.or_else(|| Some("1".to_string())),
            h2.or_else(|| Some("2".to_string())),
            h3.or_else(|| Some("3".to_string())),
            h4.or_else(|| Some("4".to_string())),
        )
    } else if is_v20 {
        (
            jc.or(Some(4)),
            jmin.or(Some(40)),
            jmax.or(Some(70)),
            s1.or(Some(15)),
            s2.or(Some(25)),
            s3,
            s4,
            h1.or_else(|| Some("1".to_string())),
            h2.or_else(|| Some("2".to_string())),
            h3.or_else(|| Some("3".to_string())),
            h4.or_else(|| Some("4".to_string())),
        )
    } else {
        // Plain standard WireGuard: absolutely NO obfuscation fields
        (None, None, None, None, None, None, None, None, None, None, None)
    };

    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol: protocol.clone(),
        server,
        port,
        listen_port,
        sni: String::new(),
        insecure: false,
        password: priv_key.clone(),
        uuid: None,
        obfs_password: None,
        hop_ports: if protocol == "wireguard" || protocol == "awg2" { None } else { Some("20000:65000".to_string()) },
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
        awg_jc: final_jc,
        awg_jmin: final_jmin,
        awg_jmax: final_jmax,
        awg_s1: final_s1,
        awg_s2: final_s2,
        awg_s3: final_s3,
        awg_s4: final_s4,
        awg_h1: final_h1,
        awg_h2: final_h2,
        awg_h3: final_h3,
        awg_h4: final_h4,
        awg_header_protection_key: header_protection_key,
        awg_rekey_after_time: rekey_after_time,
        awg_rekey_timeout: rekey_timeout,
        awg_reject_after_time: reject_after_time,
        awg_keepalive_timeout: keepalive_timeout,
        awg_max_handshake_attempts: max_handshake_attempts,
        awg_random_trailers: random_trailers,
        awg_disable_cookies: disable_cookies,
        awg_persistent_keepalive: persistent_keepalive,
        awg_version: version_opt,
        awg_i1: i1,
        awg_i2: i2,
        awg_i3: i3,
        awg_i4: i4,
        awg_hop_ports: if protocol == "wireguard" || protocol == "awg2" { None } else { Some("20000:65000".to_string()) },
        awg_port_probing: Some(false),
        up_mbps: 100,
        down_mbps: 100,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Ok(prof)
}

fn infer_multi_group_name(profiles: &[VpnProfile]) -> String {
    let extract_prefix = |name: &str| -> String {
        let s = name.trim();
        let before_bracket = s.split(" [").next().unwrap_or(s).trim();
        let before_paren = before_bracket.split(" (").next().unwrap_or(before_bracket).trim();
        before_paren.to_string()
    };
    let prefixes: Vec<String> = profiles
        .iter()
        .map(|p| extract_prefix(&p.name))
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(first) = prefixes.first() {
        if prefixes.iter().filter(|p| *p == first).count() >= 2 {
            return first.clone();
        }
    }
    "Мульти-протокол".to_string()
}

fn finalize_multi_group(profiles: &mut [VpnProfile], override_group_name: Option<&str>) {
    if profiles.len() < 2 {
        return;
    }
    let existing_gid = profiles.iter().find_map(|p| p.multi_group_id.clone());
    let existing_gname = profiles.iter().find_map(|p| p.multi_group_name.clone());
    let gid = existing_gid.unwrap_or_else(|| Uuid::new_v4().to_string());
    let gname = override_group_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or(existing_gname)
        .unwrap_or_else(|| infer_multi_group_name(profiles));

    for p in profiles.iter_mut() {
        if p.multi_group_id.is_none() {
            p.multi_group_id = Some(gid.clone());
        }
        if p.multi_group_name.is_none() {
            p.multi_group_name = Some(gname.clone());
        }
    }
}

fn parse_singbox_wireguard_endpoint(ep: &Value, outbounds: &[Value]) -> Option<VpnProfile> {
    if ep["type"].as_str() != Some("wireguard") {
        return None;
    }
    let peer = ep["peers"].as_array().and_then(|a| a.first())?;
    let server = peer["address"].as_str().unwrap_or("").trim().to_string();
    if server.is_empty() {
        return None;
    }
    let port = peer["port"].as_u64().unwrap_or(51820) as u16;
    let pub_key = peer["public_key"].as_str().map(|s| s.to_string());
    let psk = peer["pre_shared_key"].as_str().map(|s| s.to_string());
    let priv_key = ep["private_key"].as_str().unwrap_or("").to_string();
    let local_addr = ep["address"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let listen_port = ep["listen_port"].as_u64().map(|n| n as u16);

    let jc = ep["jc"].as_u64().map(|n| n as u16);
    let jmin = ep["jmin"].as_u64().map(|n| n as u16);
    let jmax = ep["jmax"].as_u64().map(|n| n as u16);
    let s1 = ep["s1"].as_u64().map(|n| n as u16);
    let s2 = ep["s2"].as_u64().map(|n| n as u16);
    let s3 = ep["s3"].as_u64().map(|n| n as u16);
    let s4 = ep["s4"].as_u64().map(|n| n as u16);
    let h1 = ep["h1"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| ep["h1"].as_u64().map(|n| n.to_string()));
    let h2 = ep["h2"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| ep["h2"].as_u64().map(|n| n.to_string()));
    let h3 = ep["h3"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| ep["h3"].as_u64().map(|n| n.to_string()));
    let h4 = ep["h4"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| ep["h4"].as_u64().map(|n| n.to_string()));
    let hpk = ep["header_protection_key"].as_str().map(|s| s.to_string());
    let rekey_after_time = ep["rekey_after_time"].as_str().map(|s| s.to_string());
    let rekey_timeout = ep["rekey_timeout"].as_str().map(|s| s.to_string());
    let reject_after_time = ep["reject_after_time"].as_str().map(|s| s.to_string());
    let keepalive_timeout = ep["keepalive_timeout"].as_str().map(|s| s.to_string());
    let max_handshake_attempts = ep["max_handshake_attempts"]
        .as_u64()
        .map(|n| n.to_string())
        .or_else(|| ep["max_handshake_attempts"].as_str().map(|s| s.to_string()));
    let random_trailers = ep["random_trailers"].as_bool();
    let disable_cookies = ep["disable_cookies"].as_bool();
    let persistent_keepalive = peer["persistent_keepalive_interval"]
        .as_u64()
        .map(|n| n.to_string());
    let i1 = ep["i1"].as_str().map(|s| s.to_string());
    let i2 = ep["i2"].as_str().map(|s| s.to_string());
    let i3 = ep["i3"].as_str().map(|s| s.to_string());
    let i4 = ep["i4"].as_str().map(|s| s.to_string());

    let raw_tag = ep["tag"].as_str().unwrap_or("");
    let name = if !raw_tag.trim().is_empty() && raw_tag != "proxy" {
        raw_tag.trim().to_string()
    } else {
        format!("AmneziaWG ({})", server)
    };

    let detour_vless = ep["detour"].as_str().and_then(|d_tag| {
        outbounds
            .iter()
            .find(|o| o["tag"].as_str() == Some(d_tag) && o["type"].as_str() == Some("vless"))
    });

    let (protocol, awg_version, vless_port, uuid, reality_public_key, reality_short_id, reality_flow, sni) =
        if let Some(vo) = detour_vless {
            let vp = vo["server_port"].as_u64().map(|p| p as u16);
            let uid = vo["uuid"].as_str().map(|s| s.to_string());
            let pbk = vo["tls"]["reality"]["public_key"]
                .as_str()
                .map(|s| s.to_string());
            let sid = vo["tls"]["reality"]["short_id"]
                .as_str()
                .or_else(|| {
                    vo["tls"]["reality"]["short_id"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_str())
                })
                .map(|s| s.to_string());
            let flow = vo["flow"].as_str().map(|s| s.to_string());
            let vsni = vo["tls"]["server_name"].as_str().unwrap_or("").to_string();
            (
                "awg_vless".to_string(),
                Some("3.1".to_string()),
                vp,
                uid,
                pbk,
                sid,
                flow,
                vsni,
            )
        } else if raw_tag.contains("[AWG+QUIC]") {
            (
                "awg_quic".to_string(),
                Some("3.1".to_string()),
                None,
                None,
                None,
                None,
                None,
                String::new(),
            )
        } else if raw_tag.contains("[AWG 2.0]") {
            (
                "awg2".to_string(),
                Some("2.0".to_string()),
                None,
                None,
                None,
                None,
                None,
                String::new(),
            )
        } else if raw_tag.contains("[WireGuard]") {
            (
                "wireguard".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                String::new(),
            )
        } else if hpk.is_some() || s3.is_some() || jc.is_some() || s1.is_some() {
            let ver = if hpk.is_some() || s3.is_some() {
                Some("3.1".to_string())
            } else {
                Some("2.0".to_string())
            };
            (
                "amneziawg".to_string(),
                ver,
                None,
                None,
                None,
                None,
                None,
                String::new(),
            )
        } else {
            (
                "wireguard".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                String::new(),
            )
        };

    let is_clean = protocol == "wireguard" || protocol == "awg2";
    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol: protocol.clone(),
        server,
        port,
        vless_port,
        listen_port,
        sni,
        insecure: false,
        password: priv_key.clone(),
        uuid,
        obfs_password: None,
        hop_ports: if is_clean {
            None
        } else {
            Some("20000:65000".to_string())
        },
        reality_public_key,
        reality_short_id,
        reality_flow,
        awg_private_key: Some(priv_key),
        awg_public_key: pub_key,
        awg_preshared_key: psk,
        awg_local_address: local_addr.or_else(|| {
            Some(if protocol == "awg2" {
                "10.8.1.2/24".to_string()
            } else {
                "10.8.0.2/32".to_string()
            })
        }),
        awg_jc: if is_clean { jc } else { jc.or(Some(4)) },
        awg_jmin: if is_clean { jmin } else { jmin.or(Some(40)) },
        awg_jmax: if is_clean { jmax } else { jmax.or(Some(70)) },
        awg_s1: if is_clean { s1 } else { s1.or(Some(15)) },
        awg_s2: if is_clean { s2 } else { s2.or(Some(25)) },
        awg_s3: s3,
        awg_s4: s4,
        awg_h1: if is_clean { h1 } else { h1.or_else(|| Some("1".to_string())) },
        awg_h2: if is_clean { h2 } else { h2.or_else(|| Some("2".to_string())) },
        awg_h3: if is_clean { h3 } else { h3.or_else(|| Some("3".to_string())) },
        awg_h4: if is_clean { h4 } else { h4.or_else(|| Some("4".to_string())) },
        awg_header_protection_key: hpk,
        awg_rekey_after_time: rekey_after_time,
        awg_rekey_timeout: rekey_timeout,
        awg_reject_after_time: reject_after_time,
        awg_keepalive_timeout: keepalive_timeout,
        awg_max_handshake_attempts: max_handshake_attempts,
        awg_random_trailers: random_trailers,
        awg_disable_cookies: disable_cookies,
        awg_persistent_keepalive: persistent_keepalive,
        awg_version,
        awg_i1: i1,
        awg_i2: i2,
        awg_i3: i3,
        awg_i4: i4,
        awg_hop_ports: if is_clean {
            None
        } else {
            Some("20000:65000".to_string())
        },
        awg_port_probing: Some(!is_clean),
        up_mbps: 100,
        down_mbps: 100,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Some(prof)
}

fn parse_singbox_outbound(
    out: &Value,
    outbounds: &[Value],
    referenced_detours: &std::collections::HashSet<String>,
) -> Option<VpnProfile> {
    let out_type = out["type"].as_str().unwrap_or("");
    let tag = out["tag"].as_str().unwrap_or("");
    if !tag.is_empty() && referenced_detours.contains(tag) {
        return None;
    }

    if out_type == "shadowsocks" {
        if let Some(detour_tag) = out["detour"].as_str() {
            if let Some(st_out) = outbounds.iter().find(|o| {
                o["tag"].as_str() == Some(detour_tag) && o["type"].as_str() == Some("shadowtls")
            }) {
                let server = st_out["server"].as_str().unwrap_or("").to_string();
                if !server.is_empty() {
                    let port = st_out["server_port"].as_u64().unwrap_or(8443) as u16;
                    let ss_password = out["password"].as_str().unwrap_or("").to_string();
                    let st_password = st_out["password"].as_str().map(|s| s.to_string());
                    let st_sni = st_out["tls"]["server_name"]
                        .as_str()
                        .unwrap_or("www.microsoft.com")
                        .to_string();
                    let ss_method = out["method"].as_str().map(|s| s.to_string());
                    let name = if !tag.is_empty() && tag != "proxy" {
                        tag.to_string()
                    } else {
                        "ShadowTLS-import".to_string()
                    };
                    let mut prof = VpnProfile {
                        id: Uuid::new_v4().to_string(),
                        name,
                        protocol: "shadowtls".to_string(),
                        server,
                        port,
                        sni: st_sni.clone(),
                        shadowtls_sni: Some(st_sni),
                        shadowtls_password: st_password,
                        password: ss_password,
                        ss_method,
                        up_mbps: 100,
                        down_mbps: 100,
                        tls_fragment: Some(false),
                        ..Default::default()
                    };
                    prof.uri = build_profile_uri(&prof);
                    return Some(prof);
                }
            }
        }
    }

    if !matches!(
        out_type,
        "vless" | "hysteria2" | "tuic" | "shadowsocks" | "ssh"
    ) {
        return None;
    }

    let server = out["server"].as_str().unwrap_or("").trim().to_string();
    if server.is_empty() {
        return None;
    }
    let transport_type = out["transport"]["type"].as_str().unwrap_or("");
    let actual_protocol = if out_type == "vless" && transport_type == "ws" {
        "vless_ws"
    } else if out_type == "vless" && transport_type == "grpc" {
        "webrtc"
    } else {
        out_type
    };
    let default_port = match actual_protocol {
        "shadowsocks" => 8388,
        "ssh" => 22,
        "vless_ws" => 8080,
        "webrtc" => 2053,
        _ => 443,
    };
    let port = out["server_port"]
        .as_u64()
        .or_else(|| {
            out["server_ports"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .and_then(|s| s.split(|c: char| c == ':' || c == '-').next())
                .and_then(|s| s.trim().parse::<u64>().ok())
        })
        .unwrap_or(default_port) as u16;
    let password = out["password"]
        .as_str()
        .or_else(|| out["private_key"].as_str())
        .unwrap_or("")
        .to_string();
    let sni = if actual_protocol == "ssh" {
        out["user"].as_str().unwrap_or("root").to_string()
    } else {
        out["tls"]["server_name"]
            .as_str()
            .unwrap_or("www.bing.com")
            .to_string()
    };
    let insecure = out["tls"]["insecure"]
        .as_bool()
        .unwrap_or(actual_protocol != "vless" && actual_protocol != "ssh");
    let uuid = out["uuid"].as_str().map(|s| s.to_string());
    let obfs_password = out["obfs"]["password"].as_str().map(|s| s.to_string());
    let reality_public_key = out["tls"]["reality"]["public_key"]
        .as_str()
        .map(|s| s.to_string());
    let reality_short_id = out["tls"]["reality"]["short_id"]
        .as_str()
        .or_else(|| {
            out["tls"]["reality"]["short_id"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());
    let reality_flow = out["flow"].as_str().map(|s| s.to_string());
    let ss_method = out["method"].as_str().map(|s| s.to_string());
    let ss_plugin = out["plugin"].as_str().map(|s| s.to_string());
    let ss_plugin_opts = out["plugin_opts"].as_str().map(|s| s.to_string());
    let ws_path = if actual_protocol == "vless_ws" {
        out["transport"]["path"].as_str().map(|s| s.to_string())
    } else if actual_protocol == "webrtc" {
        out["transport"]["service_name"]
            .as_str()
            .map(|s| s.to_string())
    } else {
        None
    };
    let cdn_host = if actual_protocol == "vless_ws" {
        out["transport"]["headers"]["Host"]
            .as_str()
            .map(|s| s.to_string())
    } else {
        None
    };
    let name = if !tag.is_empty() && tag != "proxy" {
        tag.to_string()
    } else {
        format!("{}-import", actual_protocol)
    };
    let mut prof = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name,
        protocol: actual_protocol.to_string(),
        server,
        port,
        sni,
        insecure,
        password: if out_type == "vless" {
            uuid.clone().unwrap_or_default()
        } else {
            password
        },
        uuid,
        obfs_password,
        up_mbps: 100,
        down_mbps: 100,
        uri: String::new(),
        ping_ms: None,
        routing: None,
        hop_ports: out["server_ports"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        hop_interval: out["hop_interval"].as_str().map(|s| s.to_string()),
        reality_public_key,
        reality_short_id,
        reality_flow,
        ss_method,
        ss_plugin,
        ss_plugin_opts,
        ws_path,
        cdn_host,
        ..Default::default()
    };
    prof.uri = build_profile_uri(&prof);
    Some(prof)
}

pub fn parse_multi_profiles_input(content: &str) -> Result<Vec<VpnProfile>, String> {
    let trimmed = content.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err("Пустой ввод конфигурации".to_string());
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(mut list) = serde_json::from_str::<Vec<VpnProfile>>(trimmed) {
            if !list.is_empty() {
                for p in list.iter_mut() {
                    p.id = Uuid::new_v4().to_string();
                    if p.uri.trim().is_empty() {
                        p.uri = build_profile_uri(p);
                    }
                }
                finalize_multi_group(&mut list, None);
                return Ok(list);
            }
        }
    }

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(mut prof) = serde_json::from_str::<VpnProfile>(trimmed) {
            if !prof.server.trim().is_empty() {
                prof.id = Uuid::new_v4().to_string();
                prof.uri = build_profile_uri(&prof);
                return Ok(vec![prof]);
            }
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            let endpoints_arr = val["endpoints"].as_array().cloned().unwrap_or_default();
            let outbounds_arr = val["outbounds"].as_array().cloned().unwrap_or_default();
            let mut referenced_detours = std::collections::HashSet::new();
            for ep in &endpoints_arr {
                if ep["type"].as_str() == Some("wireguard") {
                    if let Some(d) = ep["detour"].as_str() {
                        referenced_detours.insert(d.to_string());
                    }
                }
            }
            for ob in &outbounds_arr {
                if ob["type"].as_str() == Some("shadowsocks") {
                    if let Some(d) = ob["detour"].as_str() {
                        referenced_detours.insert(d.to_string());
                    }
                }
            }

            let mut collected: Vec<VpnProfile> = Vec::new();
            for ep in &endpoints_arr {
                if let Some(p) = parse_singbox_wireguard_endpoint(ep, &outbounds_arr) {
                    collected.push(p);
                }
            }
            for ob in &outbounds_arr {
                if let Some(p) = parse_singbox_outbound(ob, &outbounds_arr, &referenced_detours) {
                    collected.push(p);
                }
            }
            if !collected.is_empty() {
                finalize_multi_group(&mut collected, None);
                return Ok(collected);
            }
        }
    }

    if trimmed.contains("[Interface]") && trimmed.contains("[Peer]") {
        return parse_amnezia_conf(trimmed).map(|p| vec![p]);
    }

    const SCHEMES: &[&str] = &[
        "shadowsocks://",
        "amneziawg://",
        "hysteria2://",
        "shadowtls://",
        "wireguard://",
        "awgvless://",
        "awgquic://",
        "enigma://",
        "vless://",
        "awg2://",
        "tuic://",
        "ssh://",
        "hy2://",
        "awg://",
        "ss://",
        "wg://",
    ];

    let mut line_profiles: Vec<VpnProfile> = Vec::new();
    let mut header_group_name: Option<String> = None;
    for raw_line in trimmed.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            let c_trim = comment.trim();
            if let Some(rest) = c_trim.strip_prefix("NeuroVeil VPN Multi-Protocol Bundle:") {
                let g = rest.trim();
                if !g.is_empty() {
                    header_group_name = Some(g.to_string());
                }
            }
            continue;
        }

        let lower = line.to_lowercase();
        let mut earliest: Option<(usize, &str)> = None;
        for &sch in SCHEMES {
            if let Some(pos) = lower.find(sch) {
                match earliest {
                    None => earliest = Some((pos, sch)),
                    Some((best_pos, best_sch)) => {
                        if pos < best_pos || (pos == best_pos && sch.len() > best_sch.len()) {
                            earliest = Some((pos, sch));
                        }
                    }
                }
            }
        }

        if let Some((pos, _)) = earliest {
            let prefix_ok = pos == 0 || line[..pos].trim_end().ends_with(':');
            if prefix_ok {
                let candidate = line[pos..].trim();
                if let Ok(p) = parse_profile_uri(candidate) {
                    line_profiles.push(p);
                }
            }
        }
    }

    if !line_profiles.is_empty() {
        finalize_multi_group(&mut line_profiles, header_group_name.as_deref());
        return Ok(line_profiles);
    }

    parse_profile_uri(trimmed).map(|p| vec![p])
}

pub fn parse_multi_zip_bytes(bytes: &[u8]) -> Result<Vec<VpnProfile>, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Не удалось открыть ZIP-архив: {}", e))?;

    let mut json_entries: Vec<(String, String)> = Vec::new();
    let mut txt_entries: Vec<(String, String)> = Vec::new();
    let mut conf_entries: Vec<(String, String)> = Vec::new();
    let mut nvkey_entries: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Ошибка чтения элемента архива: {}", e))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".nvkey") {
            let mut raw = Vec::new();
            if file.read_to_end(&mut raw).is_ok() {
                nvkey_entries.push((name, raw));
            }
            continue;
        }
        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            continue;
        }
        if lower.ends_with(".json") {
            json_entries.push((name, content));
        } else if lower.ends_with(".txt") {
            txt_entries.push((name, content));
        } else if lower.ends_with(".conf") {
            conf_entries.push((name, content));
        }
    }

    let attach_nvkeys = |profiles: &mut Vec<VpnProfile>| {
        if nvkey_entries.is_empty() {
            return;
        }
        let mut key_idx = 0usize;
        for p in profiles.iter_mut() {
            if p.protocol == "enigma" && p.enigma_keybook.is_none() {
                while key_idx < nvkey_entries.len() {
                    let (_, raw) = &nvkey_entries[key_idx];
                    key_idx += 1;
                    if let Ok(kb) = crate::enigma::EnigmaKeybook::from_bytes(raw) {
                        p.enigma_keybook = Some(kb.to_base64());
                        p.uri = build_profile_uri(p);
                        break;
                    }
                }
                if p.enigma_keybook.is_none() {
                    if let Some((_, raw)) = nvkey_entries.first() {
                        if let Ok(kb) = crate::enigma::EnigmaKeybook::from_bytes(raw) {
                            p.enigma_keybook = Some(kb.to_base64());
                            p.uri = build_profile_uri(p);
                        }
                    }
                }
            }
        }
    };

    for (_, body) in &json_entries {
        if let Ok(mut list) = parse_multi_profiles_input(body) {
            if !list.is_empty() {
                attach_nvkeys(&mut list);
                return Ok(list);
            }
        }
    }

    for (_, body) in &txt_entries {
        if let Ok(mut list) = parse_multi_profiles_input(body) {
            if !list.is_empty() {
                attach_nvkeys(&mut list);
                return Ok(list);
            }
        }
    }

    let mut conf_profiles = Vec::new();
    for (entry_name, body) in &conf_entries {
        if let Ok(mut p) = parse_amnezia_conf(body) {
            if let Some(stem) = std::path::Path::new(entry_name)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                if !stem.trim().is_empty() && p.name.contains(&p.server) {
                    p.name = stem.to_string();
                    p.uri = build_profile_uri(&p);
                }
            }
            conf_profiles.push(p);
        }
    }
    if !conf_profiles.is_empty() {
        finalize_multi_group(&mut conf_profiles, None);
        return Ok(conf_profiles);
    }

    Err("В ZIP-архиве не найдены поддерживаемые конфиги (.json, .txt или .conf)".to_string())
}

pub mod urlencoding {
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
#[allow(unused_variables)]
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
        let mut profile = VpnProfile {
            id: "awg2-test".to_string(),
            name: "AWG 2.0 Test".to_string(),
            protocol: "awg2".to_string(),
            server: "5.129.227.240".to_string(),
            port: 51820,
            listen_port: Some(54321),
            password: "cEb5x5/GskRccR5YpV0H1JWYhDnKtuthg6GuJqlVn18=".to_string(),
            awg_private_key: Some("cEb5x5/GskRccR5YpV0H1JWYhDnKtuthg6GuJqlVn18=".to_string()),
            awg_public_key: Some("/kUh+97J9Tu7Lnen1bl+AODfs2iACrvkk3OQSDK5Fns=".to_string()),
            awg_preshared_key: Some("1lRwdV6iDpWOSn3kJ2OPvaoujETI/zCoUxBQXM+5yMg=".to_string()),
            awg_local_address: Some("10.8.0.2/24".to_string()),
            awg_version: Some("2.0".to_string()),
            ..Default::default()
        };

        let exported = build_awg_conf(&profile);
        let expected = "[Interface]\nPrivateKey = cEb5x5/GskRccR5YpV0H1JWYhDnKtuthg6GuJqlVn18=\nAddress = 10.8.0.2/24\nDNS = 1.1.1.1\nMTU = 1420\n\n[Peer]\nPublicKey = /kUh+97J9Tu7Lnen1bl+AODfs2iACrvkk3OQSDK5Fns=\nPresharedKey = 1lRwdV6iDpWOSn3kJ2OPvaoujETI/zCoUxBQXM+5yMg=\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = 5.129.227.240:51820\n";
        assert_eq!(exported, expected);
        assert!(!exported.contains("ListenPort"));
        assert!(!exported.contains("PersistentKeepalive"));
        assert!(!exported.contains("Jc ="));
        assert!(!exported.contains("H1 ="));

        let routing = RoutingConfig::default();
        let cfg = generate_singbox_config(&profile, &routing);
        let ep = &cfg["endpoints"][0];
        assert_eq!(ep["listen_port"], json!(54321));
        assert!(ep.get("jc").is_none());
        assert!(ep.get("h1").is_none());
        assert!(ep["peers"][0].get("persistent_keepalive_interval").is_none());

        profile.uri = build_profile_uri(&profile);
        assert!(profile.uri.starts_with("awg2://"));
        assert!(!profile.uri.contains("jc="));
        assert!(!profile.uri.contains("h1="));
    }

    #[test]
    fn test_parse_next_gen_protocols_uri_and_json() {
        let st_prof = VpnProfile {
            id: "st-1".to_string(),
            name: "ShadowTLS Test".to_string(),
            protocol: "shadowtls".to_string(),
            server: "1.2.3.4".to_string(),
            port: 8443,
            password: "ss-inner-password".to_string(),
            ss_method: Some("2022-blake3-aes-128-gcm".to_string()),
            shadowtls_password: Some("st-handshake-secret".to_string()),
            shadowtls_sni: Some("gateway.icloud.com".to_string()),
            sni: "gateway.icloud.com".to_string(),
            ..Default::default()
        };
        let st_uri = build_profile_uri(&st_prof);
        let st_parsed = parse_profile_uri(&st_uri).expect("Failed to parse shadowtls URI");
        assert_eq!(st_parsed.protocol, "shadowtls");
        assert_eq!(st_parsed.password, "ss-inner-password");
        assert_eq!(st_parsed.shadowtls_password.as_deref(), Some("st-handshake-secret"));
        assert_eq!(st_parsed.shadowtls_sni.as_deref(), Some("gateway.icloud.com"));

        let ws_prof = VpnProfile {
            id: "ws-1".to_string(),
            name: "CDN WS Test".to_string(),
            protocol: "vless_ws".to_string(),
            server: "1.2.3.4".to_string(),
            port: 8080,
            password: "11111111-2222-3333-4444-555555555555".to_string(),
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            sni: "cdn.example.com".to_string(),
            cdn_host: Some("cdn.example.com".to_string()),
            ws_path: Some("/neuroveil-ws".to_string()),
            insecure: true,
            ..Default::default()
        };
        let ws_uri = build_profile_uri(&ws_prof);
        let ws_parsed = parse_profile_uri(&ws_uri).expect("Failed to parse vless_ws URI");
        assert_eq!(ws_parsed.protocol, "vless_ws");
        assert_eq!(ws_parsed.ws_path.as_deref(), Some("/neuroveil-ws"));
        assert_eq!(ws_parsed.cdn_host.as_deref(), Some("cdn.example.com"));
        assert!(ws_parsed.insecure);

        let rtc_prof = VpnProfile {
            id: "rtc-1".to_string(),
            name: "WebRTC Test".to_string(),
            protocol: "webrtc".to_string(),
            server: "1.2.3.4".to_string(),
            port: 2053,
            password: "11111111-2222-3333-4444-555555555555".to_string(),
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            sni: "zoom.us".to_string(),
            ws_path: Some("neuroveil-webrtc".to_string()),
            insecure: true,
            ..Default::default()
        };
        let rtc_uri = build_profile_uri(&rtc_prof);
        let rtc_parsed = parse_profile_uri(&rtc_uri).expect("Failed to parse webrtc URI");
        assert_eq!(rtc_parsed.protocol, "webrtc");
        assert_eq!(rtc_parsed.ws_path.as_deref(), Some("neuroveil-webrtc"));
        assert!(rtc_parsed.insecure);
    }

    #[test]
    fn test_parse_awg_conf_with_listen_port() {
        let conf = r#"[Interface]
Address = 10.8.0.2/32
PrivateKey = vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=
ListenPort = 54321
DNS = 1.1.1.1
HeaderProtectionKey = zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY=
H1 = 1
H2 = 2
H3 = 3
H4 = 4

[Peer]
PublicKey = fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=
Endpoint = 1.2.3.4:51820
"#;
        let profile = parse_amnezia_conf(conf).expect("Must parse config with ListenPort");
        assert_eq!(profile.listen_port, Some(54321));
        let exported = build_awg_conf(&profile);
        assert!(exported.contains("ListenPort = 54321"));
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

        // Test clean AWG 2.0 with rotated listen_port
        let prof_20 = VpnProfile {
            id: "awg2-singbox-check".to_string(),
            name: "AWG 2.0 Clean".to_string(),
            protocol: "awg2".to_string(),
            server: "5.129.227.240".to_string(),
            port: 51820,
            listen_port: Some(54321),
            password: "cEb5x5/GskRccR5YpV0H1JWYhDnKtuthg6GuJqlVn18=".to_string(),
            awg_private_key: Some("cEb5x5/GskRccR5YpV0H1JWYhDnKtuthg6GuJqlVn18=".to_string()),
            awg_public_key: Some("/kUh+97J9Tu7Lnen1bl+AODfs2iACrvkk3OQSDK5Fns=".to_string()),
            awg_preshared_key: Some("1lRwdV6iDpWOSn3kJ2OPvaoujETI/zCoUxBQXM+5yMg=".to_string()),
            awg_local_address: Some("10.8.0.2/24".to_string()),
            awg_version: Some("2.0".to_string()),
            ..Default::default()
        };
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

        // Test AWG + QUIC Pre-Warm (awg_quic) with 32-bit random H1-H4
        let mut prof_quic = prof_31.clone();
        prof_quic.protocol = "awg_quic".to_string();
        prof_quic.listen_port = Some(48123);
        prof_quic.awg_h1 = Some("284910293".to_string());
        prof_quic.awg_h2 = Some("748291044".to_string());
        prof_quic.awg_h3 = Some("1294810293".to_string());
        prof_quic.awg_h4 = Some("1984716255".to_string());
        let quic_uri = build_profile_uri(&prof_quic);
        assert!(quic_uri.starts_with("awgquic://"));
        let quic_parsed = parse_profile_uri(&quic_uri).expect("Must parse awgquic URI");
        assert_eq!(quic_parsed.protocol, "awg_quic");
        assert_eq!(quic_parsed.awg_h1.as_deref(), Some("284910293"));

        let cfg_quic = generate_singbox_config(&prof_quic, &routing);
        let temp_quic = temp_dir.join("test_singbox_awg_quic.json");
        std::fs::write(&temp_quic, serde_json::to_string_pretty(&cfg_quic).unwrap()).unwrap();
        let out_quic = std::process::Command::new(&singbox_bin)
            .args(["check", "-c", temp_quic.to_str().unwrap()])
            .output()
            .expect("Failed to run sing-box check on awg_quic");
        assert!(
            out_quic.status.success(),
            "sing-box check failed for awg_quic:\nSTDOUT: {}\nSTDERR: {}",
            String::from_utf8_lossy(&out_quic.stdout),
            String::from_utf8_lossy(&out_quic.stderr)
        );

        // Test Hybrid AWG + VLESS Reality (awg_vless)
        let mut prof_hybrid = prof_quic.clone();
        prof_hybrid.protocol = "awg_vless".to_string();
        prof_hybrid.vless_port = Some(443);
        prof_hybrid.uuid = Some("11111111-2222-3333-4444-555555555555".to_string());
        prof_hybrid.reality_public_key = Some("fN7baW-3-uBNuwSQsO-lwD6GkP3oNucDfn5UW1EWphQ".to_string());
        prof_hybrid.reality_short_id = Some("0123456789abcdef".to_string());
        prof_hybrid.sni = "www.microsoft.com".to_string();
        let hybrid_uri = build_profile_uri(&prof_hybrid);
        assert!(hybrid_uri.starts_with("awgvless://"));
        let hybrid_parsed = parse_profile_uri(&hybrid_uri).expect("Must parse awgvless URI");
        assert_eq!(hybrid_parsed.protocol, "awg_vless");
        assert_eq!(hybrid_parsed.vless_port, Some(443));
        assert_eq!(hybrid_parsed.uuid.as_deref(), Some("11111111-2222-3333-4444-555555555555"));
        assert_eq!(hybrid_parsed.reality_public_key.as_deref(), Some("fN7baW-3-uBNuwSQsO-lwD6GkP3oNucDfn5UW1EWphQ"));

        let cfg_hybrid = generate_singbox_config(&prof_hybrid, &routing);
        assert_eq!(cfg_hybrid["endpoints"][0]["detour"], "vless-reality-detour");
        assert!(cfg_hybrid["endpoints"][0].get("listen_port").is_none());
        let temp_hybrid = temp_dir.join("test_singbox_awg_vless.json");
        std::fs::write(&temp_hybrid, serde_json::to_string_pretty(&cfg_hybrid).unwrap()).unwrap();
        let out_hybrid = std::process::Command::new(&singbox_bin)
            .args(["check", "-c", temp_hybrid.to_str().unwrap()])
            .output()
            .expect("Failed to run sing-box check on awg_vless");
        assert!(
            out_hybrid.status.success(),
            "sing-box check failed for awg_vless:\nSTDOUT: {}\nSTDERR: {}",
            String::from_utf8_lossy(&out_hybrid.stdout),
            String::from_utf8_lossy(&out_hybrid.stderr)
        );

        let _ = std::fs::remove_file(temp_31);
        let _ = std::fs::remove_file(temp_20);
        let _ = std::fs::remove_file(temp_quic);
        let _ = std::fs::remove_file(temp_hybrid);
    }

    #[test]
    fn test_multi_protocol_bundle_roundtrip_and_singbox_check() {
        let mut p_vless = VpnProfile {
            id: "m-1".to_string(),
            name: "iPhone (VLESS)".to_string(),
            protocol: "vless".to_string(),
            server: "1.2.3.4".to_string(),
            port: 443,
            password: "11111111-2222-3333-4444-555555555555".to_string(),
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            sni: "www.microsoft.com".to_string(),
            reality_public_key: Some("fN7baW-3-uBNuwSQsO-lwD6GkP3oNucDfn5UW1EWphQ".to_string()),
            reality_short_id: Some("01234567".to_string()),
            ..Default::default()
        };
        p_vless.uri = build_profile_uri(&p_vless);

        let mut p_awg = VpnProfile {
            id: "m-2".to_string(),
            name: "iPhone (AWG 3.1)".to_string(),
            protocol: "amneziawg".to_string(),
            server: "1.2.3.4".to_string(),
            port: 31555,
            password: "vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=".to_string(),
            awg_private_key: Some("vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=".to_string()),
            awg_public_key: Some("fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=".to_string()),
            awg_preshared_key: Some("7sQK621vrXJAMe+w0006jc+2MOd12jY2Sb4PcRD57FE=".to_string()),
            awg_local_address: Some("10.8.0.5/32".to_string()),
            awg_header_protection_key: Some("zUPEP7/ufINes+LSSJd1AqLLmgSxUzeSQwBgIbdPavY=".to_string()),
            awg_version: Some("3.1".to_string()),
            awg_jc: Some(4),
            awg_jmin: Some(40),
            awg_jmax: Some(70),
            awg_s1: Some(12),
            awg_s2: Some(12),
            awg_s3: Some(12),
            awg_s4: Some(12),
            awg_h1: Some("1".to_string()),
            awg_h2: Some("2".to_string()),
            awg_h3: Some("3".to_string()),
            awg_h4: Some("4".to_string()),
            ..Default::default()
        };
        p_awg.uri = build_profile_uri(&p_awg);

        let mut p_st = VpnProfile {
            id: "m-3".to_string(),
            name: "iPhone (ShadowTLS)".to_string(),
            protocol: "shadowtls".to_string(),
            server: "1.2.3.4".to_string(),
            port: 8443,
            password: "ss-pass-secret".to_string(),
            ss_method: Some("2022-blake3-aes-128-gcm".to_string()),
            shadowtls_password: Some("st-pass-secret".to_string()),
            shadowtls_sni: Some("gateway.icloud.com".to_string()),
            sni: "gateway.icloud.com".to_string(),
            ..Default::default()
        };
        p_st.uri = build_profile_uri(&p_st);

        let mut p_hybrid = p_awg.clone();
        p_hybrid.id = "m-4".to_string();
        p_hybrid.name = "iPhone (AWG+Reality)".to_string();
        p_hybrid.protocol = "awg_vless".to_string();
        p_hybrid.vless_port = Some(443);
        p_hybrid.uuid = p_vless.uuid.clone();
        p_hybrid.reality_public_key = p_vless.reality_public_key.clone();
        p_hybrid.reality_short_id = p_vless.reality_short_id.clone();
        p_hybrid.sni = "www.microsoft.com".to_string();
        p_hybrid.uri = build_profile_uri(&p_hybrid);

        let profiles = vec![p_vless.clone(), p_awg.clone(), p_st.clone(), p_hybrid.clone()];
        let routing = RoutingConfig::default();
        let multi_cfg = generate_multi_singbox_config(&profiles, "iPhone", &routing);
        assert!(multi_cfg.get("neuroveil_profiles").is_none());
        assert_eq!(multi_cfg["endpoints"].as_array().unwrap().len(), 2);

        let multi_json = serde_json::to_string_pretty(&multi_cfg).unwrap();
        let parsed_from_json = parse_multi_profiles_input(&multi_json).expect("Must parse multi JSON");
        assert_eq!(parsed_from_json.len(), 4);
        assert!(parsed_from_json.iter().all(|p| p.multi_group_id.is_some()));
        assert_eq!(parsed_from_json[0].multi_group_name.as_deref(), Some("iPhone"));
        assert!(parsed_from_json.iter().any(|p| p.protocol == "awg_vless"));
        assert!(parsed_from_json.iter().any(|p| p.protocol == "shadowtls"));
        assert!(parsed_from_json.iter().any(|p| p.protocol == "vless"));

        let zip_bytes = build_multi_zip_bytes(&profiles, "iPhone", &routing).expect("Must build ZIP");
        let parsed_from_zip = parse_multi_zip_bytes(&zip_bytes).expect("Must parse ZIP");
        assert_eq!(parsed_from_zip.len(), 4);

        let multi_uri_text = format!(
            "{}: {}\n{}: {}\n",
            p_vless.name, p_vless.uri, p_awg.name, p_awg.uri
        );
        let parsed_uris = parse_multi_profiles_input(&multi_uri_text).expect("Must parse multi-line URIs");
        assert_eq!(parsed_uris.len(), 2);
        assert_eq!(parsed_uris[0].multi_group_id, parsed_uris[1].multi_group_id);
        assert_eq!(parsed_uris[0].multi_group_name.as_deref(), Some("iPhone"));

        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");
        if singbox_bin.exists() {
            let temp_multi = std::env::temp_dir().join("test_singbox_multi_bundle.json");
            std::fs::write(&temp_multi, &multi_json).unwrap();
            let out_multi = std::process::Command::new(&singbox_bin)
                .args(["check", "-c", temp_multi.to_str().unwrap()])
                .output()
                .expect("Failed to run sing-box check on multi config");
            let _ = std::fs::remove_file(&temp_multi);
            assert!(
                out_multi.status.success(),
                "sing-box check failed for multi bundle:\nSTDOUT: {}\nSTDERR: {}",
                String::from_utf8_lossy(&out_multi.stdout),
                String::from_utf8_lossy(&out_multi.stderr)
            );
        }
    }

    static RULESET_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_desktop_process_ruleset_precedence_and_awg_mtu() {
        let _guard = RULESET_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut routing = RoutingConfig::default();
        routing.mode = "tun".to_string();
        routing.split_tunneling = true;
        routing.default_outbound = "direct".to_string();
        routing.direct_domains = vec![".ru".to_string(), ".su".to_string()];
        routing.proxy_processes = vec!["Tanki.exe".to_string()];
        routing.direct_processes = vec!["lgc.exe".to_string()];

        let awg_prof = VpnProfile {
            id: "awg-vless-test".to_string(),
            name: "AWG Reality Test".to_string(),
            protocol: "awg_vless".to_string(),
            server: "192.109.206.82".to_string(),
            port: 34327,
            vless_port: Some(443),
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            password: "vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=".to_string(),
            awg_private_key: Some("vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=".to_string()),
            awg_public_key: Some("fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=".to_string()),
            reality_public_key: Some("fN7baW-3-uBNuwSQsO-lwD6GkP3oNucDfn5UW1EWphQ".to_string()),
            sni: "www.microsoft.com".to_string(),
            ..Default::default()
        };

        let cfg = generate_singbox_config(&awg_prof, &routing);
        let tun_in = cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["tag"] == "tun-in")
            .expect("tun-in must exist");
        assert_eq!(tun_in["mtu"], json!(1280));

        let route_rules = cfg["route"]["rules"].as_array().unwrap();
        let pos_tag = |tag: &str| {
            route_rules
                .iter()
                .position(|r| r.get("rule_set").and_then(|v| v.as_str()) == Some(tag))
                .expect("rule_set must exist in route.rules")
        };
        let i_dproc = pos_tag("direct-proc-rules");
        let i_pproc = pos_tag("proxy-proc-rules");
        let i_ddom = pos_tag("direct-rules");
        let i_pdom = pos_tag("proxy-rules");
        assert!(i_dproc < i_pproc && i_pproc < i_ddom && i_ddom < i_pdom);

        let dns_rules = cfg["dns"]["rules"].as_array().unwrap();
        let dns_pos = |tag: &str| {
            dns_rules
                .iter()
                .position(|r| r.get("rule_set").and_then(|v| v.as_str()) == Some(tag))
                .expect("rule_set must exist in dns.rules")
        };
        assert!(dns_pos("direct-proc-rules") < dns_pos("proxy-proc-rules"));
        assert!(dns_pos("proxy-proc-rules") < dns_pos("direct-rules"));
        assert!(dns_pos("direct-rules") < dns_pos("proxy-rules"));

        let proxy_proc_rs = serde_json::from_str::<Value>(
            &std::fs::read_to_string(get_rules_proxy_proc_path()).unwrap(),
        )
        .unwrap();
        let direct_rs = serde_json::from_str::<Value>(
            &std::fs::read_to_string(get_rules_direct_path()).unwrap(),
        )
        .unwrap();
        assert!(proxy_proc_rs["rules"].to_string().contains("Tanki.exe"));
        assert!(!direct_rs["rules"].to_string().contains("lgc.exe"));
        assert!(direct_rs["rules"].to_string().contains(".su"));

        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");
        if singbox_bin.exists() {
            let temp_cfg = std::env::temp_dir().join("test_desktop_4rules_awg.json");
            std::fs::write(&temp_cfg, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
            let out = std::process::Command::new(&singbox_bin)
                .args(["check", "-c", temp_cfg.to_str().unwrap()])
                .output()
                .expect("Failed to run sing-box check");
            let _ = std::fs::remove_file(&temp_cfg);
            assert!(
                out.status.success(),
                "sing-box check failed for 4-ruleset desktop config:\nSTDOUT: {}\nSTDERR: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }

        let mut vless_prof = awg_prof.clone();
        vless_prof.protocol = "vless".to_string();
        let vless_cfg = generate_singbox_config(&vless_prof, &routing);
        let vless_tun = vless_cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["tag"] == "tun-in")
            .unwrap();
        assert_eq!(vless_tun["mtu"], json!(1400));
    }

    #[test]
    fn test_android_runtime_config_omits_experimental_clash_api() {
        let mut routing = RoutingConfig::default();
        routing.mode = "tun".to_string();
        routing.split_tunneling = false;

        let prof = VpnProfile {
            id: "android-vless-test".to_string(),
            name: "Android Traffic Test".to_string(),
            protocol: "vless".to_string(),
            server: "192.109.206.82".to_string(),
            port: 443,
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            reality_public_key: Some("fN7baW-3-uBNuwSQsO-lwD6GkP3oNucDfn5UW1EWphQ".to_string()),
            sni: "www.microsoft.com".to_string(),
            ..Default::default()
        };

        let cfg = generate_android_runtime_singbox_config(&prof, &routing);
        assert!(cfg.get("experimental").is_none());

        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");
        if singbox_bin.exists() {
            let temp_cfg = std::env::temp_dir().join("test_android_no_clash_api_cfg.json");
            std::fs::write(&temp_cfg, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
            let out = std::process::Command::new(&singbox_bin)
                .args(["check", "-c", temp_cfg.to_str().unwrap()])
                .output()
                .expect("Failed to run sing-box check");
            let _ = std::fs::remove_file(&temp_cfg);
            assert!(
                out.status.success(),
                "sing-box check failed for Android config:\nSTDOUT: {}\nSTDERR: {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    #[test]
    fn test_enigma_uri_roundtrip_and_config() {
        let kb = crate::enigma::EnigmaKeybook::generate().unwrap();
        let kb_b64 = kb.to_base64();
        let mut prof = VpnProfile {
            id: "enigma-test".to_string(),
            name: "Enigma Test".to_string(),
            protocol: "enigma".to_string(),
            server: "198.51.100.42".to_string(),
            port: 34567,
            password: "clientPrivKeyBase64==".to_string(),
            awg_private_key: Some("clientPrivKeyBase64==".to_string()),
            awg_public_key: Some("serverPubKeyBase64==".to_string()),
            awg_preshared_key: Some("pskBase64==".to_string()),
            awg_local_address: Some("10.8.7.2/24".to_string()),
            hop_ports: Some("20000:65000".to_string()),
            enigma_mimic: Some("quic".to_string()),
            enigma_keybook: Some(kb_b64.clone()),
            ..Default::default()
        };
        let uri = build_profile_uri(&prof);
        assert!(uri.starts_with("enigma://"));
        let parsed = parse_profile_uri(&uri).unwrap();
        assert_eq!(parsed.protocol, "enigma");
        assert_eq!(parsed.server, "198.51.100.42");
        assert_eq!(parsed.port, 34567);
        assert_eq!(parsed.enigma_mimic.as_deref(), Some("quic"));
        assert_eq!(parsed.enigma_keybook.as_deref(), Some(kb_b64.as_str()));
        assert_eq!(parsed.hop_ports.as_deref(), Some("20000:65000"));

        prof.uri = uri;
        let mut routing = RoutingConfig::default();
        routing.mode = "tun".to_string();
        let cfg = generate_singbox_config_with_port(&prof, &routing, Some(49152));
        let ep = cfg["endpoints"][0].clone();
        assert_eq!(ep["type"], json!("wireguard"));
        assert_eq!(ep["mtu"], json!(1280));
        assert_eq!(ep["inet4_bind_address"], json!("127.0.0.1"));
        assert_eq!(ep["peers"][0]["address"], json!("127.0.0.1"));
        assert_eq!(ep["peers"][0]["port"], json!(49152));
        let tun_in = cfg["inbounds"].as_array().unwrap().iter().find(|i| i["tag"] == "tun-in").unwrap();
        assert_eq!(tun_in["mtu"], json!(1280));
        let excl = tun_in["route_exclude_address"].as_array().unwrap();
        assert!(excl.iter().any(|v| v.as_str() == Some("198.51.100.42/32")));
    }

    #[test]
    fn test_bridge_2hop_configs_and_singbox_check() {
        let mut routing = RoutingConfig::default();
        routing.mode = "tun".to_string();
        routing.split_tunneling = false;

        let valid_priv = "vKYKrbcUQj34y1N3ZHwB+dNXJgk3b2oJZ5EuH0hqhLY=";
        let valid_pub = "fN7baW+3+uBNuwSQsO+lwD6GkP3oNucDfn5UW1EWphQ=";
        let valid_reality_pbk = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

        let entry_wg = VpnProfile {
            id: "entry-wg".to_string(),
            name: "RU WG Entry".to_string(),
            protocol: "wireguard".to_string(),
            server: "5.129.227.240".to_string(),
            port: 51820,
            listen_port: Some(41234),
            password: valid_priv.to_string(),
            awg_private_key: Some(valid_priv.to_string()),
            awg_public_key: Some(valid_pub.to_string()),
            awg_local_address: Some("10.0.0.2/32".to_string()),
            ..Default::default()
        };

        let exit_awg = VpnProfile {
            id: "exit-awg".to_string(),
            name: "NL AWG Exit".to_string(),
            protocol: "amneziawg".to_string(),
            server: "93.89.223.35".to_string(),
            port: 51820,
            listen_port: Some(41235),
            password: valid_priv.to_string(),
            awg_private_key: Some(valid_priv.to_string()),
            awg_public_key: Some(valid_pub.to_string()),
            awg_local_address: Some("10.8.1.2/32".to_string()),
            awg_jc: Some(4),
            awg_jmin: Some(40),
            awg_jmax: Some(70),
            awg_s1: Some(25),
            awg_s2: Some(30),
            ..Default::default()
        };

        let entry_vless = VpnProfile {
            id: "entry-vless".to_string(),
            name: "RU VLESS Entry".to_string(),
            protocol: "vless".to_string(),
            server: "185.10.20.30".to_string(),
            port: 443,
            uuid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            reality_public_key: Some(valid_reality_pbk.to_string()),
            reality_short_id: Some("a1b2c3d4".to_string()),
            sni: "www.microsoft.com".to_string(),
            ..Default::default()
        };

        let exit_vless = VpnProfile {
            id: "exit-vless".to_string(),
            name: "DE VLESS Exit".to_string(),
            protocol: "vless".to_string(),
            server: "72.56.84.168".to_string(),
            port: 443,
            uuid: Some("22222222-3333-4444-5555-666666666666".to_string()),
            reality_public_key: Some(valid_reality_pbk.to_string()),
            reality_short_id: Some("deadbeef".to_string()),
            sni: "www.google.com".to_string(),
            ..Default::default()
        };

        let exit_awg_vless = VpnProfile {
            id: "exit-awg-vless".to_string(),
            name: "DE AWG+VLESS Exit".to_string(),
            protocol: "awg_vless".to_string(),
            server: "72.56.84.168".to_string(),
            port: 51820,
            vless_port: Some(443),
            password: valid_priv.to_string(),
            awg_private_key: Some(valid_priv.to_string()),
            awg_public_key: Some(valid_pub.to_string()),
            awg_local_address: Some("10.8.2.2/32".to_string()),
            uuid: Some("33333333-4444-5555-6666-777777777777".to_string()),
            reality_public_key: Some(valid_reality_pbk.to_string()),
            reality_short_id: Some("12345678".to_string()),
            sni: "www.microsoft.com".to_string(),
            ..Default::default()
        };

        let entry_shadowtls = VpnProfile {
            id: "entry-st".to_string(),
            name: "RU ShadowTLS Entry".to_string(),
            protocol: "shadowtls".to_string(),
            server: "5.129.227.241".to_string(),
            port: 443,
            password: "AAAAAAAAAAAAAAAAAAAAAA==".to_string(),
            shadowtls_password: Some("st-password".to_string()),
            shadowtls_sni: Some("www.google.com".to_string()),
            ss_method: Some("2022-blake3-aes-128-gcm".to_string()),
            ..Default::default()
        };

        let entry_enigma = VpnProfile {
            id: "entry-enigma".to_string(),
            name: "RU Enigma Entry".to_string(),
            protocol: "enigma".to_string(),
            server: "5.129.227.242".to_string(),
            port: 34567,
            password: valid_priv.to_string(),
            awg_private_key: Some(valid_priv.to_string()),
            awg_public_key: Some(valid_pub.to_string()),
            awg_local_address: Some("10.8.7.2/24".to_string()),
            ..Default::default()
        };

        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");

        let check_cfg = |name: &str, cfg: &Value| {
            if singbox_bin.exists() {
                let temp_cfg = std::env::temp_dir().join(format!("test_bridge_{}.json", name));
                std::fs::write(&temp_cfg, serde_json::to_string_pretty(cfg).unwrap()).unwrap();
                let out = std::process::Command::new(&singbox_bin)
                    .args(["check", "-c", temp_cfg.to_str().unwrap()])
                    .output()
                    .expect("Failed to run sing-box check");
                let _ = std::fs::remove_file(&temp_cfg);
                assert!(
                    out.status.success(),
                    "sing-box check failed for bridge case '{}':\nSTDOUT: {}\nSTDERR: {}",
                    name,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        };

        // Case 1: WG -> AWG
        let cfg1 = generate_singbox_config_with_bridge(
            &exit_awg,
            Some(&entry_wg),
            &routing,
            None,
            None,
            false,
        );
        let eps1 = cfg1["endpoints"].as_array().unwrap();
        assert_eq!(eps1.len(), 2);
        let entry_ep1 = eps1.iter().find(|e| e["tag"] == "bridge-entry").unwrap();
        let exit_ep1 = eps1.iter().find(|e| e["tag"] == "proxy").unwrap();
        assert!(entry_ep1.get("detour").is_none());
        assert_eq!(exit_ep1["detour"], json!("bridge-entry"));
        assert_eq!(exit_ep1["mtu"], json!(1220));
        let tun1 = cfg1["inbounds"].as_array().unwrap().iter().find(|i| i["tag"] == "tun-in").unwrap();
        assert_eq!(tun1["mtu"], json!(1220));
        let excl1 = tun1["route_exclude_address"].as_array().unwrap();
        assert!(excl1.iter().any(|v| v.as_str() == Some("5.129.227.240/32")));
        assert!(!excl1.iter().any(|v| v.as_str() == Some("93.89.223.35/32")));
        check_cfg("wg_to_awg", &cfg1);

        // Case 2: VLESS -> AWG (xudp auto-enabled on VLESS entry)
        let cfg2 = generate_singbox_config_with_bridge(
            &exit_awg,
            Some(&entry_vless),
            &routing,
            None,
            None,
            false,
        );
        let entry_ob2 = cfg2["outbounds"].as_array().unwrap().iter().find(|o| o["tag"] == "bridge-entry").unwrap();
        assert_eq!(entry_ob2["packet_encoding"], json!("xudp"));
        let exit_ep2 = cfg2["endpoints"].as_array().unwrap().iter().find(|e| e["tag"] == "proxy").unwrap();
        assert_eq!(exit_ep2["detour"], json!("bridge-entry"));
        let tun2 = cfg2["inbounds"].as_array().unwrap().iter().find(|i| i["tag"] == "tun-in").unwrap();
        assert_eq!(tun2["mtu"], json!(1280));
        check_cfg("vless_to_awg", &cfg2);

        // Case 3: WG -> AWG+VLESS (composite detour chain: proxy -> vless-reality-detour -> bridge-entry)
        let cfg3 = generate_singbox_config_with_bridge(
            &exit_awg_vless,
            Some(&entry_wg),
            &routing,
            None,
            None,
            false,
        );
        let inner_vless3 = cfg3["outbounds"].as_array().unwrap().iter().find(|o| o["tag"] == "vless-reality-detour").unwrap();
        assert_eq!(inner_vless3["detour"], json!("bridge-entry"));
        let exit_ep3 = cfg3["endpoints"].as_array().unwrap().iter().find(|e| e["tag"] == "proxy").unwrap();
        assert_eq!(exit_ep3["detour"], json!("vless-reality-detour"));
        check_cfg("wg_to_awg_vless", &cfg3);

        // Case 4: ShadowTLS -> VLESS
        let cfg4 = generate_singbox_config_with_bridge(
            &exit_vless,
            Some(&entry_shadowtls),
            &routing,
            None,
            None,
            false,
        );
        let obs4 = cfg4["outbounds"].as_array().unwrap();
        assert!(obs4.iter().any(|o| o["tag"] == "bridge-entry-st-detour"));
        let entry_ss4 = obs4.iter().find(|o| o["tag"] == "bridge-entry").unwrap();
        assert_eq!(entry_ss4["detour"], json!("bridge-entry-st-detour"));
        let exit_vl4 = obs4.iter().find(|o| o["tag"] == "proxy").unwrap();
        assert_eq!(exit_vl4["detour"], json!("bridge-entry"));
        check_cfg("shadowtls_to_vless", &cfg4);

        // Case 5: Enigma (Entry) -> AWG (Exit)
        let cfg5 = generate_singbox_config_with_bridge(
            &exit_awg,
            Some(&entry_enigma),
            &routing,
            None,
            Some(49155),
            false,
        );
        let eps5 = cfg5["endpoints"].as_array().unwrap();
        let entry_ep5 = eps5.iter().find(|e| e["tag"] == "bridge-entry").unwrap();
        assert_eq!(entry_ep5["peers"][0]["address"], json!("127.0.0.1"));
        assert_eq!(entry_ep5["peers"][0]["port"], json!(49155));
        let tun5 = cfg5["inbounds"].as_array().unwrap().iter().find(|i| i["tag"] == "tun-in").unwrap();
        let excl5 = tun5["route_exclude_address"].as_array().unwrap();
        assert!(excl5.iter().any(|v| v.as_str() == Some("5.129.227.242/32")));
        check_cfg("enigma_to_awg", &cfg5);
    }

    #[test]
    fn test_enigma_candidate_ports_respect_explicit_hop_ranges() {
        let enigma_prof = VpnProfile {
            id: "test-enigma".to_string(),
            name: "Enigma v1".to_string(),
            protocol: "enigma".to_string(),
            server: "185.221.163.69".to_string(),
            port: 4500,
            original_port: Some(43897),
            hop_ports: Some("20000:65000".to_string()),
            ..Default::default()
        };

        let (ranges, singles) = parse_hop_port_ranges(None, enigma_prof.hop_ports.as_deref());
        assert!(port_matches_hop_spec(43897, &ranges, &singles));
        assert!(!port_matches_hop_spec(4500, &ranges, &singles));

        let candidates = get_profile_candidate_ports(&enigma_prof, None, Some(vec![4500]), 24);
        assert!(
            !candidates.is_empty(),
            "Candidate list for Enigma with hop_ports 20000:65000 must not be empty"
        );
        assert!(
            candidates.contains(&43897),
            "Original Enigma port 43897 must be included in candidates: {:?}",
            candidates
        );
        for &p in &candidates {
            assert!(
                (20000..=65000).contains(&p),
                "Enigma candidate port {} is outside explicit hop_ports 20000:65000 (all: {:?})",
                p,
                candidates
            );
            assert_ne!(p, 4500, "Corrupted excluded port 4500 must not be returned");
        }
    }

    #[test]
    fn test_desktop_dns_rulesets_contain_no_ip_cidr_and_no_legacy_warning() {
        let _guard = RULESET_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut routing = RoutingConfig::default();
        routing.mode = "tun".to_string();
        routing.split_tunneling = true;
        routing.default_outbound = "direct".to_string();
        routing.proxy_processes = vec![
            "Telegram.exe".to_string(),
            "Discord.exe".to_string(),
            "destiny2.exe".to_string(),
        ];
        routing.direct_processes = vec!["yandex.exe".to_string()];
        routing.proxy_domains = vec!["bungie.net".to_string(), "deadorbit.net".to_string()];
        routing.direct_domains = vec![".ru".to_string()];
        routing.proxy_ips = vec!["1.2.3.4".to_string(), "5.6.7.0/24".to_string()];
        routing.direct_ips = vec!["9.8.7.6".to_string()];

        let prof = VpnProfile {
            id: "enigma-dns-test".to_string(),
            name: "Enigma DNS Test".to_string(),
            protocol: "enigma".to_string(),
            server: "185.221.163.69".to_string(),
            port: 43897,
            password: "WD1HjbUyguB9cfZ43v/oYavzFhaW/obLY0BlTyaOdEo=".to_string(),
            awg_private_key: Some("WD1HjbUyguB9cfZ43v/oYavzFhaW/obLY0BlTyaOdEo=".to_string()),
            awg_public_key: Some("DRRsYgL85ZNKPjyVKOKnGdT76CcZpEyTDAulvxZtumc=".to_string()),
            ..Default::default()
        };

        let cfg = generate_singbox_config(&prof, &routing);

        // 1. None of the rulesets referenced in dns.rules may contain ip_cidr
        let dns_rules = cfg["dns"]["rules"].as_array().unwrap();
        let all_rulesets = cfg["route"]["rule_set"].as_array().unwrap();
        for dr in dns_rules {
            if let Some(rs_tag) = dr.get("rule_set").and_then(|v| v.as_str()) {
                assert!(
                    !rs_tag.ends_with("-ip-rules"),
                    "Route-only IP ruleset {} must not be used in dns.rules",
                    rs_tag
                );
                let rs_entry = all_rulesets
                    .iter()
                    .find(|e| e.get("tag").and_then(|v| v.as_str()) == Some(rs_tag))
                    .expect("DNS rule_set tag must be declared in route.rule_set");
                let path = rs_entry["path"].as_str().unwrap();
                let raw = std::fs::read_to_string(path).unwrap();
                assert!(
                    !raw.contains("\"ip_cidr\""),
                    "DNS-referenced ruleset {} ({}) must not contain ip_cidr:\n{}",
                    rs_tag,
                    path,
                    raw
                );
            }
        }

        // 2. Telegram/Discord CIDRs and custom IPs must be in route-only IP rulesets in proper order
        let route_rules = cfg["route"]["rules"].as_array().unwrap();
        let pos_tag = |tag: &str| {
            route_rules
                .iter()
                .position(|r| r.get("rule_set").and_then(|v| v.as_str()) == Some(tag))
                .expect("rule_set must exist in route.rules")
        };
        assert!(pos_tag("direct-proc-rules") < pos_tag("proxy-proc-rules"));
        assert!(pos_tag("proxy-proc-rules") < pos_tag("proxy-proc-ip-rules"));
        assert!(pos_tag("proxy-proc-ip-rules") < pos_tag("direct-rules"));
        assert!(pos_tag("direct-rules") < pos_tag("direct-ip-rules"));
        assert!(pos_tag("direct-ip-rules") < pos_tag("proxy-rules"));
        assert!(pos_tag("proxy-rules") < pos_tag("proxy-ip-rules"));

        let proc_ip_raw = std::fs::read_to_string(get_rules_proxy_proc_ip_path()).unwrap();
        assert!(proc_ip_raw.contains("149.154.160.0/20"));
        assert!(proc_ip_raw.contains("162.159.128.0/21"));
        let proxy_ip_raw = std::fs::read_to_string(get_rules_proxy_ip_path()).unwrap();
        assert!(proxy_ip_raw.contains("1.2.3.4/32"));
        assert!(proxy_ip_raw.contains("5.6.7.0/24"));
        let direct_ip_raw = std::fs::read_to_string(get_rules_direct_ip_path()).unwrap();
        assert!(direct_ip_raw.contains("9.8.7.6/32"));

        // 3. Validate with real sing-box binary and ensure Legacy Address Filter warning is gone
        let singbox_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
            .join("bin")
            .join("sing-box.exe");
        if singbox_bin.exists() {
            let temp_cfg = std::env::temp_dir().join("test_desktop_dns_no_ip_cidr.json");
            std::fs::write(&temp_cfg, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
            let out = std::process::Command::new(&singbox_bin)
                .args(["check", "-c", temp_cfg.to_str().unwrap()])
                .output()
                .expect("Failed to run sing-box check");
            let _ = std::fs::remove_file(&temp_cfg);
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(
                out.status.success(),
                "sing-box check failed:\n{}",
                combined
            );
            assert!(
                !combined.contains("Legacy Address Filter Fields"),
                "sing-box check must not emit Legacy Address Filter warning:\n{}",
                combined
            );
        }
    }
}





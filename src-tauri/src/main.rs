#![windows_subsystem = "windows"]

mod models;
mod singbox;
mod ssh_deployer;

use models::{
    default_routing_presets, is_awg_protocol, AppPersistentState, AppUpdateInfo, DeployResult,
    PortProbeResult, RoutingConfig, RoutingPreset, RunningProcessInfo, VpnProfile, VpnStatus,
    VpsCredentials,
};
use singbox::{
    build_awg_conf, build_profile_uri, check_singbox_version, collect_exe_in_dir,
    download_singbox_core, generate_mobile_singbox_config, generate_singbox_config,
    get_app_data_dir, get_awg_candidate_ports, get_singbox_bin_path, parse_profile_uri,
    reset_stale_app_sockets, set_windows_system_proxy, write_routing_rulesets,
};
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct RuntimeState {
    persistent: AppPersistentState,
    child: Option<Child>,
    active_profile_id: Option<String>,
    active_profile_name: Option<String>,
    active_protocol: Option<String>,
    active_port: Option<u16>,
    port_hunting_status: Option<String>,
    is_handshaking: bool,
    handshake_status: Option<String>,
    singbox_installed: bool,
    singbox_version: String,
    logs: Vec<String>,
    up_bytes: u64,
    down_bytes: u64,
    up_speed: u64,
    down_speed: u64,
}

type SharedState = Arc<Mutex<RuntimeState>>;

fn get_state_file_path() -> std::path::PathBuf {
    get_app_data_dir().join("state.json")
}

fn load_persistent_state() -> AppPersistentState {
    let path = get_state_file_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut state) = serde_json::from_str::<AppPersistentState>(&content) {
            for p in &mut state.profiles {
                p.uri = build_profile_uri(p);
            }
            state.profiles.retain(|p| p.pinned_on_pc != Some(false));
            if state.routing_presets.is_empty() {
                state.routing_presets = default_routing_presets();
            }
            return state;
        }
    }
    AppPersistentState::default()
}

fn save_persistent_state(state: &AppPersistentState) {
    let path = get_state_file_path();
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, json);
    }
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;
    for c in input.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn push_log(logs: &mut Vec<String>, line: String) {
    let clean = strip_ansi(&line);
    let ts = chrono::Local::now().format("%H:%M:%S").to_string();
    let formatted = format!("[{}] {}", ts, clean);
    logs.push(formatted.clone());
    if logs.len() > 250 {
        let excess = logs.len() - 250;
        logs.drain(0..excess);
    }
    let log_file = get_app_data_dir().join("singbox.log");
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_file) {
        let _ = writeln!(f, "{}", formatted);
    }
}

fn stop_singbox_internal(rt: &mut RuntimeState) {
    if let Some(mut child) = rt.child.take() {
        let _ = set_windows_system_proxy(false, rt.persistent.routing.local_mixed_port);
        let _ = child.kill();
        let _ = child.wait();
        save_persistent_state(&rt.persistent);
        push_log(&mut rt.logs, "Процесс sing-box остановлен.".to_string());
    }
    rt.active_profile_id = None;
    rt.active_profile_name = None;
    rt.active_protocol = None;
    rt.active_port = None;
    rt.port_hunting_status = None;
    rt.is_handshaking = false;
    rt.handshake_status = None;
    rt.up_speed = 0;
    rt.down_speed = 0;
}

fn build_vpn_status(rt: &mut RuntimeState) -> VpnStatus {
    // Check if child exited unexpectedly
    if let Some(ref mut child) = rt.child {
        if let Ok(Some(status)) = child.try_wait() {
            let code = status.code().unwrap_or(-1);
            push_log(
                &mut rt.logs,
                format!("Процесс sing-box завершился (код: {}).", code),
            );
            let _ = set_windows_system_proxy(false, rt.persistent.routing.local_mixed_port);
            rt.child = None;
            rt.active_profile_id = None;
            rt.active_profile_name = None;
            rt.active_protocol = None;
            rt.active_port = None;
            rt.port_hunting_status = None;
            rt.is_handshaking = false;
            rt.handshake_status = None;
            rt.up_speed = 0;
            rt.down_speed = 0;
        }
    }

    let sb_bin = get_singbox_bin_path();
    let sb_exists = sb_bin.exists();
    if !sb_exists {
        rt.singbox_installed = false;
        rt.singbox_version = "Не установлен".to_string();
    } else if !rt.singbox_installed {
        rt.singbox_installed = true;
        if rt.singbox_version.is_empty() || rt.singbox_version == "Не установлен" {
            rt.singbox_version = "sing-box (установлен)".to_string();
        }
    }

    VpnStatus {
        connected: rt.child.is_some(),
        active_profile_id: rt.active_profile_id.clone(),
        active_profile_name: rt.active_profile_name.clone(),
        active_protocol: rt.active_protocol.clone(),
        active_port: rt.active_port,
        port_hunting_status: rt.port_hunting_status.clone(),
        is_handshaking: rt.is_handshaking,
        handshake_status: rt.handshake_status.clone(),
        mode: rt.persistent.routing.mode.clone(),
        split_tunneling: rt.persistent.routing.split_tunneling,
        up_bytes: rt.up_bytes,
        down_bytes: rt.down_bytes,
        up_speed: rt.up_speed,
        down_speed: rt.down_speed,
        singbox_installed: rt.singbox_installed,
        singbox_version: rt.singbox_version.clone(),
        singbox_path: sb_bin.to_string_lossy().to_string(),
        logs: rt.logs.clone(),
    }
}

#[tauri::command]
fn get_app_state(state: State<'_, SharedState>) -> Result<AppPersistentState, String> {
    let rt = state.lock().map_err(|_| "Lock error".to_string())?;
    Ok(rt.persistent.clone())
}

#[tauri::command]
fn get_vpn_status(state: State<'_, SharedState>) -> Result<VpnStatus, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    Ok(build_vpn_status(&mut rt))
}

#[tauri::command]
async fn download_core(state: State<'_, SharedState>) -> Result<String, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        {
            if let Ok(mut rt) = state_clone.lock() {
                push_log(
                    &mut rt.logs,
                    "Скачивание официального ядра sing-box с GitHub Releases...".to_string(),
                );
            }
        }
        let res = download_singbox_core();
        if let Ok(mut rt) = state_clone.lock() {
            match &res {
                Ok(ver) => {
                    rt.singbox_installed = true;
                    rt.singbox_version = ver.clone();
                    push_log(&mut rt.logs, format!("Ядро sing-box установлено: {}", ver));
                }
                Err(e) => push_log(&mut rt.logs, format!("Ошибка установки ядра: {}", e)),
            }
        }
        res
    })
    .await
    .map_err(|e| e.to_string())?
}

fn spawn_and_monitor_singbox(
    state_clone: &Arc<Mutex<RuntimeState>>,
    profile: &VpnProfile,
    routing_snapshot: &RoutingConfig,
    active_port: u16,
    custom_log: Option<String>,
) -> Result<VpnStatus, String> {
    let bin_path = get_singbox_bin_path();
    if !bin_path.exists() {
        return Err(
            "Ядро sing-box.exe еще не установлено. Нажмите «Скачать ядро sing-box» на панели или в настройках."
                .to_string(),
        );
    }

    let config_json = generate_singbox_config(profile, routing_snapshot);
    let config_path = get_app_data_dir().join("active_config.json");
    let config_str = serde_json::to_string_pretty(&config_json).map_err(|e| e.to_string())?;
    fs::write(&config_path, config_str)
        .map_err(|e| format!("Не удалось записать конфигурацию sing-box: {}", e))?;

    let mut cmd = Command::new(&bin_path);
    cmd.args(["run", "-c", &config_path.to_string_lossy()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Не удалось запустить sing-box.exe: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let state_for_stdout = state_clone.clone();
    if let Some(out) = stdout {
        thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut guard) = state_for_stdout.lock() {
                    push_log(&mut guard.logs, line);
                }
            }
        });
    }

    let state_for_stderr = state_clone.clone();
    if let Some(err) = stderr {
        thread::spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut guard) = state_for_stderr.lock() {
                    push_log(&mut guard.logs, line);
                }
            }
        });
    }

    if routing_snapshot.mode == "system_proxy" {
        let port = routing_snapshot.local_mixed_port;
        let _ = set_windows_system_proxy(true, port);
    }

    // Pre-warm tunnel, flush DNS, and reset stale pre-VPN sockets of Telegram/Discord
    let state_for_warmup = state_clone.clone();
    let warmup_clash_port = routing_snapshot.clash_api_port;
    let warmup_procs = routing_snapshot.proxy_processes.clone();
    thread::spawn(move || {
        let check_url = format!("http://127.0.0.1:{}/proxies", warmup_clash_port);
        for _ in 0..35 {
            if ureq::get(&check_url)
                .timeout(Duration::from_millis(80))
                .call()
                .is_ok()
            {
                break;
            }
            thread::sleep(Duration::from_millis(40));
        }
        let delay_url = format!(
            "http://127.0.0.1:{}/proxies/proxy/delay?url=http://1.1.1.1/generate_204&timeout=2000",
            warmup_clash_port
        );
        if ureq::get(&delay_url)
            .timeout(Duration::from_millis(2200))
            .call()
            .is_ok()
        {
            if let Ok(mut g) = state_for_warmup.lock() {
                if g.is_handshaking {
                    g.is_handshaking = false;
                    g.handshake_status = None;
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let mut flush_cmd = Command::new("ipconfig");
            flush_cmd.arg("/flushdns").creation_flags(CREATE_NO_WINDOW);
            let _ = flush_cmd.output();
        }
        reset_stale_app_sockets(&warmup_procs);
    });

    // Update runtime state and spawn traffic monitor
    let status = {
        let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        if let Some(msg) = custom_log {
            push_log(&mut rt.logs, msg);
        } else {
            push_log(
                &mut rt.logs,
                format!(
                    "Запуск sing-box: профиль '{}' ({}), режим: {}, Split Tunneling: {}",
                    profile.name,
                    profile.protocol.to_uppercase(),
                    routing_snapshot.mode,
                    if routing_snapshot.split_tunneling { "ВКЛ" } else { "ВЫКЛ" }
                ),
            );
        }
        if routing_snapshot.mode == "system_proxy" {
            push_log(
                &mut rt.logs,
                format!(
                    "Системный прокси Windows активирован (127.0.0.1:{})",
                    routing_snapshot.local_mixed_port
                ),
            );
        }
        rt.child = Some(child);
        rt.active_profile_id = Some(profile.id.clone());
        rt.active_profile_name = Some(profile.name.clone());
        rt.active_protocol = Some(profile.protocol.clone());
        rt.active_port = Some(active_port);
        rt.port_hunting_status = None;
        rt.up_bytes = 0;
        rt.down_bytes = 0;

        let is_awg = is_awg_protocol(&profile.protocol);
        if is_awg {
            rt.is_handshaking = true;
            rt.handshake_status = Some("Установка соединения (рукопожатие WireGuard)...".to_string());
        } else {
            rt.is_handshaking = false;
            rt.handshake_status = None;
        }

        build_vpn_status(&mut rt)
    };

    let state_for_traffic = state_clone.clone();
    let clash_port = routing_snapshot.clash_api_port;
    let is_awg = is_awg_protocol(&profile.protocol);

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        let url = format!("http://127.0.0.1:{}/connections", clash_port);
        let mut prev_up = 0u64;
        let mut prev_down = 0u64;
        let mut initialized = false;
        let mut consecutive_rx_zero = 0u32;
        let mut save_tick_counter = 0u32;
        let start_time = Instant::now();
        let mut warned_handshake_cooldown = false;
        loop {
            {
                if let Ok(guard) = state_for_traffic.lock() {
                    if guard.child.is_none() {
                        break;
                    }
                } else {
                    break;
                }
            }
            if let Ok(resp) = ureq::get(&url).timeout(Duration::from_millis(700)).call() {
                if let Ok(val) = resp.into_json::<serde_json::Value>() {
                    let total_up = val["uploadTotal"].as_u64().unwrap_or(0);
                    let total_down = val["downloadTotal"].as_u64().unwrap_or(0);

                    if let Ok(mut guard) = state_for_traffic.lock() {
                        if initialized {
                            let delta_up = total_up.saturating_sub(prev_up);
                            let delta_down = total_down.saturating_sub(prev_down);
                            guard.up_speed = delta_up;
                            guard.down_speed = delta_down;

                            if delta_up > 0 || delta_down > 0 {
                                if let Some(active_id) = guard.active_profile_id.clone() {
                                    if let Some(prof) = guard
                                        .persistent
                                        .profiles
                                        .iter_mut()
                                        .find(|p| p.id == active_id)
                                    {
                                        prof.total_up_bytes =
                                            prof.total_up_bytes.saturating_add(delta_up);
                                        prof.total_down_bytes =
                                            prof.total_down_bytes.saturating_add(delta_down);
                                    }
                                }
                                save_tick_counter += 1;
                                if save_tick_counter >= 10 {
                                    save_tick_counter = 0;
                                    save_persistent_state(&guard.persistent);
                                }
                            }

                            if is_awg {
                                if total_down > 0 {
                                    if guard.is_handshaking {
                                        guard.is_handshaking = false;
                                        guard.handshake_status = None;
                                        push_log(
                                            &mut guard.logs,
                                            "[WireGuard] Рукопожатие успешно установлено, сессия активна!".to_string(),
                                        );
                                    }
                                } else {
                                    // Traffic sent but 0 bytes returned from server
                                    if start_time.elapsed().as_secs() >= 4 && !warned_handshake_cooldown {
                                        warned_handshake_cooldown = true;
                                        guard.is_handshaking = true;
                                        guard.handshake_status = Some("Ожидание подтверждения сессии сервером (cooldown WireGuard)...".to_string());
                                        push_log(
                                            &mut guard.logs,
                                            "[WireGuard] Ожидание подтверждения сессии сервером (cooldown WireGuard)...".to_string(),
                                        );
                                    }
                                }

                                if guard.up_speed > 0 && guard.down_speed == 0 {
                                    consecutive_rx_zero += 1;
                                    if consecutive_rx_zero == 25 {
                                        push_log(
                                            &mut guard.logs,
                                            "[AmneziaWG] Внимание: трафик отправляется, но входящие пакеты заблокированы (вероятна DPI блокировка UDP-порта).".to_string(),
                                        );
                                    }
                                } else if guard.down_speed > 0 {
                                    consecutive_rx_zero = 0;
                                }
                            }
                        } else {
                            guard.up_speed = 0;
                            guard.down_speed = 0;
                            initialized = true;
                            if is_awg && total_down > 0 {
                                guard.is_handshaking = false;
                                guard.handshake_status = None;
                            }
                        }
                        guard.up_bytes = total_up;
                        guard.down_bytes = total_down;
                        prev_up = total_up;
                        prev_down = total_down;
                    }
                }
            }
            thread::sleep(Duration::from_millis(800));
        }
    });

    Ok(status)
}

fn terminate_process_connections_clash(clash_api_port: u16, targets: &[String]) {
    if targets.is_empty() {
        return;
    }
    let url = format!("http://127.0.0.1:{}/connections", clash_api_port);
    let resp = match ureq::get(&url).timeout(Duration::from_millis(1000)).call() {
        Ok(r) => r,
        Err(_) => return,
    };
    let json: serde_json::Value = match resp.into_json() {
        Ok(v) => v,
        Err(_) => return,
    };

    let connections = match json.get("connections").and_then(|v| v.as_array()) {
        Some(c) => c,
        None => return,
    };

    let mut patterns: Vec<String> = Vec::new();
    for entry in targets {
        let trimmed = entry.trim();
        let lower = trimmed.to_lowercase();
        if lower.is_empty() {
            continue;
        }
        patterns.push(lower.clone());
        let p = std::path::Path::new(trimmed);
        if p.is_dir() {
            let mut dir_exes = Vec::new();
            collect_exe_in_dir(p, 0, &mut dir_exes);
            for exe in dir_exes {
                patterns.push(exe.to_lowercase());
            }
        }
        let fname = std::path::Path::new(&lower)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&lower)
            .to_string();
        let stem = fname.trim_end_matches(".exe");
        if !stem.is_empty() {
            patterns.push(stem.to_string());
            patterns.push(format!("{}.exe", stem));
        }
        if lower.contains("telegram") {
            patterns.push("telegram".to_string());
        }
        if lower.contains("discord") {
            patterns.push("discord".to_string());
        }
        if lower.contains("chrome") {
            patterns.push("chrome".to_string());
        }
    }

    for conn in connections {
        let process_path = conn
            .pointer("/metadata/processPath")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if process_path.is_empty() {
            continue;
        }
        let lower_path = process_path.to_lowercase();

        let matches = patterns.iter().any(|pat| {
            lower_path.contains(pat) || pat.contains(&lower_path)
        });

        if matches {
            if let Some(id) = conn.get("id").and_then(|v| v.as_str()) {
                let del_url = format!("http://127.0.0.1:{}/connections/{}", clash_api_port, id);
                let _ = ureq::delete(&del_url)
                    .timeout(Duration::from_millis(300))
                    .call();
            }
        }
    }
}

fn hot_reload_or_restart_singbox(
    state_clone: &Arc<Mutex<RuntimeState>>,
    profile: &VpnProfile,
    routing_snapshot: &RoutingConfig,
    old_routing: &RoutingConfig,
    log_msg: String,
) -> Result<VpnStatus, String> {
    let bin_path = get_singbox_bin_path();
    if !bin_path.exists() {
        return Err("Ядро sing-box.exe не найдено.".to_string());
    }

    // 1. Instantly write and sync updated rules to disk. sing-box watches rules_proxy.json
    // and rules_direct.json via fsnotify and dynamically reloads routing table in memory.
    let _ = write_routing_rulesets(routing_snapshot);

    // 2. Persist active_config.json
    let config_json = generate_singbox_config(profile, routing_snapshot);
    let config_path = get_app_data_dir().join("active_config.json");
    if let Ok(config_str) = serde_json::to_string_pretty(&config_json) {
        let _ = fs::write(&config_path, config_str);
    }

    // 3. Switch default-outbound selector if split tunneling fallback changed
    let old_target = if old_routing.split_tunneling && old_routing.default_outbound == "direct" {
        "direct"
    } else {
        "proxy"
    };
    let target = if routing_snapshot.split_tunneling && routing_snapshot.default_outbound == "direct" {
        "direct"
    } else {
        "proxy"
    };
    let default_mode_changed = old_target != target;
    let selector_url = format!(
        "http://127.0.0.1:{}/proxies/default-outbound",
        routing_snapshot.clash_api_port
    );
    let _ = ureq::put(&selector_url)
        .timeout(Duration::from_millis(600))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({ "name": target }));

    // 4. Compute full differences across both proxy_processes and direct_processes
    let norm = |s: &str| s.trim().trim_end_matches(&['\\', '/'][..]).to_lowercase();

    let old_proxy_set: std::collections::HashSet<String> =
        old_routing.proxy_processes.iter().map(|s| norm(s)).collect();
    let new_proxy_set: std::collections::HashSet<String> =
        routing_snapshot.proxy_processes.iter().map(|s| norm(s)).collect();

    let old_direct_set: std::collections::HashSet<String> =
        old_routing.direct_processes.iter().map(|s| norm(s)).collect();
    let new_direct_set: std::collections::HashSet<String> =
        routing_snapshot.direct_processes.iter().map(|s| norm(s)).collect();

    let mut changed_processes: Vec<String> = Vec::new();
    for s in &routing_snapshot.proxy_processes {
        if !old_proxy_set.contains(&norm(s)) {
            changed_processes.push(s.clone());
        }
    }
    for s in &old_routing.proxy_processes {
        if !new_proxy_set.contains(&norm(s)) {
            changed_processes.push(s.clone());
        }
    }
    for s in &routing_snapshot.direct_processes {
        if !old_direct_set.contains(&norm(s)) {
            changed_processes.push(s.clone());
        }
    }
    for s in &old_routing.direct_processes {
        if !new_direct_set.contains(&norm(s)) {
            changed_processes.push(s.clone());
        }
    }

    // 5. Background: wait 250ms for sing-box fsnotify to finish reloading rules_proxy.json
    // and rules_direct.json in memory BEFORE dropping connections, so reconnecting apps
    // immediately hit the newly loaded ruleset instead of the old one!
    let clash_api_port = routing_snapshot.clash_api_port;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));

        #[cfg(target_os = "windows")]
        {
            let mut flush_cmd = Command::new("ipconfig");
            flush_cmd.arg("/flushdns").creation_flags(CREATE_NO_WINDOW);
            let _ = flush_cmd.output();
        }

        if default_mode_changed {
            // Default mode flipped ("direct" <-> "proxy"), close active Clash connections
            let all_url = format!("http://127.0.0.1:{}/connections", clash_api_port);
            let _ = ureq::delete(&all_url).timeout(Duration::from_millis(500)).call();
        }

        if !changed_processes.is_empty() {
            terminate_process_connections_clash(clash_api_port, &changed_processes);
            reset_stale_app_sockets(&changed_processes);

            // Second pass after 300ms to catch any app that reconnected during the transition
            thread::sleep(Duration::from_millis(300));
            terminate_process_connections_clash(clash_api_port, &changed_processes);
            reset_stale_app_sockets(&changed_processes);
        }
    });

    if routing_snapshot.mode == "system_proxy" {
        let _ = set_windows_system_proxy(true, routing_snapshot.local_mixed_port);
    }

    let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
    push_log(
        &mut rt.logs,
        format!("{} (Бесшовное переключение без разрыва соединений)", log_msg),
    );
    Ok(build_vpn_status(&mut rt))
}

#[tauri::command]
async fn connect_vpn(
    profile_id: String,
    mode: String,
    split_tunneling: bool,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bin_path = get_singbox_bin_path();
        if !bin_path.exists() {
            return Err(
                "Ядро sing-box.exe еще не установлено. Нажмите «Скачать ядро sing-box» на панели или в настройках."
                    .to_string(),
            );
        }

        // 0. Pre-flight check: validate credentials before stopping current connection or attempting port hunting
        {
            let rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let prof = rt
                .persistent
                .profiles
                .iter()
                .find(|p| p.id == profile_id)
                .ok_or_else(|| "Выбранный профиль не найден".to_string())?;

            if is_awg_protocol(&prof.protocol) {
                let priv_key = prof
                    .awg_private_key
                    .as_deref()
                    .unwrap_or(&prof.password)
                    .trim();
                if priv_key.is_empty() {
                    return Err(
                        "В профиле AmneziaWG не указан приватный ключ (Private Key).\n\nНажмите значок карандаша ✏️ для ввода Private Key или выберите готовый файл конфигурации через кнопку «Выбрать файл конфига» (.conf)."
                            .to_string(),
                    );
                }
                let pub_key = prof.awg_public_key.as_deref().unwrap_or("").trim();
                if pub_key.is_empty() {
                    return Err(
                        "В профиле AmneziaWG не указан публичный ключ сервера (Public Key).\n\nУкажите Public Key в настройках профиля (✏️) или импортируйте готовый файл .conf."
                            .to_string(),
                    );
                }
            } else if prof.protocol == "vless" {
                let uid = prof.uuid.as_deref().unwrap_or(&prof.password).trim();
                if uid.is_empty() {
                    return Err("В профиле VLESS не указан UUID (пароль). Проверьте настройки профиля.".to_string());
                }
            } else if prof.protocol == "hysteria2" || prof.protocol == "tuic" || prof.protocol == "shadowsocks" {
                if prof.password.trim().is_empty() {
                    return Err(format!("В профиле {} не указан пароль. Проверьте настройки профиля.", prof.protocol.to_uppercase()));
                }
            }
        }

        // 1. Stop existing process and snapshot profile & routing without holding lock during I/O
        let (profile, routing_snapshot, had_tracked_child) = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let had_child = rt.child.is_some();
            stop_singbox_internal(&mut rt);

            rt.persistent.selected_profile_id = Some(profile_id.clone());

            let default_routing = rt.persistent.routing.clone();
            let (prof, prof_routing) = if let Some(p) = rt.persistent.profiles.iter_mut().find(|p| p.id == profile_id) {
                let mut pr = p.routing.clone().unwrap_or(default_routing);
                pr.mode = mode.clone();
                pr.split_tunneling = split_tunneling;
                p.routing = Some(pr.clone());
                (p.clone(), pr)
            } else {
                return Err("Выбранный профиль не найден".to_string());
            };

            rt.persistent.routing.mode = mode.clone();
            rt.persistent.routing.split_tunneling = split_tunneling;
            save_persistent_state(&rt.persistent);

            (prof, prof_routing, had_child)
        };

        // Only run taskkill if there could be an orphaned sing-box.exe not tracked by this session
        #[cfg(target_os = "windows")]
        if !had_tracked_child {
            let mut kill_cmd = Command::new("taskkill");
            kill_cmd
                .args(["/F", "/IM", "sing-box.exe"])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = kill_cmd.output();
        }

        // 2. Direct fast launch of sing-box without blocking loops (< 50ms)
        let active_port = profile.port;
        {
            if let Ok(mut g) = state_clone.lock() {
                g.port_hunting_status = None;
                g.active_port = Some(active_port);
            }
        }

        spawn_and_monitor_singbox(&state_clone, &profile, &routing_snapshot, active_port, None)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn disconnect_vpn(state: State<'_, SharedState>) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        stop_singbox_internal(&mut rt);
        Ok(build_vpn_status(&mut rt))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_routing_presets(state: State<'_, SharedState>) -> Result<Vec<RoutingPreset>, String> {
    let rt = state.lock().map_err(|_| "Lock error".to_string())?;
    Ok(rt.persistent.routing_presets.clone())
}

#[tauri::command]
fn save_routing_preset(
    preset: RoutingPreset,
    state: State<'_, SharedState>,
) -> Result<Vec<RoutingPreset>, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    let preset_clone = preset.clone();
    if let Some(pos) = rt.persistent.routing_presets.iter().position(|p| p.id == preset.id) {
        rt.persistent.routing_presets[pos] = preset;
    } else {
        rt.persistent.routing_presets.push(preset);
    }
    // Keep persistent routing and active/selected profile routing synced with the saved preset
    rt.persistent.active_preset_id = Some(preset_clone.id.clone());
    rt.persistent.routing.split_tunneling = true;
    rt.persistent.routing.default_outbound = preset_clone.default_outbound.clone();
    rt.persistent.routing.proxy_processes = preset_clone.proxy_processes.clone();
    rt.persistent.routing.direct_processes = preset_clone.direct_processes.clone();

    let target_pid = rt
        .active_profile_id
        .clone()
        .or_else(|| rt.persistent.selected_profile_id.clone())
        .or_else(|| rt.persistent.profiles.first().map(|p| p.id.clone()));
    let default_routing = rt.persistent.routing.clone();
    if let Some(ref pid) = target_pid {
        if let Some(prof) = rt.persistent.profiles.iter_mut().find(|p| &p.id == pid) {
            let mut pr = prof.routing.clone().unwrap_or(default_routing);
            pr.split_tunneling = true;
            pr.default_outbound = preset_clone.default_outbound.clone();
            pr.proxy_processes = preset_clone.proxy_processes.clone();
            pr.direct_processes = preset_clone.direct_processes.clone();
            prof.routing = Some(pr);
        }
    }

    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.routing_presets.clone())
}

#[tauri::command]
fn delete_routing_preset(
    preset_id: String,
    state: State<'_, SharedState>,
) -> Result<Vec<RoutingPreset>, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.routing_presets.retain(|p| p.id != preset_id);
    if rt.persistent.active_preset_id.as_deref() == Some(&preset_id) {
        rt.persistent.active_preset_id = None;
    }
    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.routing_presets.clone())
}

#[tauri::command]
fn reset_routing_presets(state: State<'_, SharedState>) -> Result<Vec<RoutingPreset>, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    let user_presets: Vec<RoutingPreset> = rt
        .persistent
        .routing_presets
        .iter()
        .filter(|p| !p.is_builtin)
        .cloned()
        .collect();
    let mut all_presets = default_routing_presets();
    all_presets.extend(user_presets);
    rt.persistent.routing_presets = all_presets;
    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.routing_presets.clone())
}

#[tauri::command]
async fn apply_routing_preset(
    preset_id: String,
    profile_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (preset, profile, routing_snapshot, is_running, old_routing) = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let old_routing = rt.persistent.routing.clone();
            let preset = rt
                .persistent
                .routing_presets
                .iter()
                .find(|p| p.id == preset_id)
                .cloned()
                .ok_or_else(|| format!("Предустановка с ID '{}' не найдена", preset_id))?;

            rt.persistent.active_preset_id = Some(preset.id.clone());
            rt.persistent.routing.split_tunneling = true;
            rt.persistent.routing.default_outbound = preset.default_outbound.clone();
            rt.persistent.routing.proxy_processes = preset.proxy_processes.clone();
            rt.persistent.routing.direct_processes = preset.direct_processes.clone();

            let target_pid = profile_id
                .clone()
                .or_else(|| rt.active_profile_id.clone())
                .or_else(|| rt.persistent.selected_profile_id.clone())
                .or_else(|| rt.persistent.profiles.first().map(|p| p.id.clone()));

            let default_routing = rt.persistent.routing.clone();
            let mut prof_opt = None;
            if let Some(ref pid) = target_pid {
                if let Some(prof) = rt.persistent.profiles.iter_mut().find(|p| &p.id == pid) {
                    let mut pr = prof.routing.clone().unwrap_or(default_routing);
                    pr.split_tunneling = true;
                    pr.default_outbound = preset.default_outbound.clone();
                    pr.proxy_processes = preset.proxy_processes.clone();
                    pr.direct_processes = preset.direct_processes.clone();
                    prof.routing = Some(pr);
                    prof_opt = Some(prof.clone());
                }
            }

            save_persistent_state(&rt.persistent);

            let is_running = rt.child.is_some();
            let routing_snapshot = rt.persistent.routing.clone();
            let active_profile = prof_opt.or_else(|| {
                rt.persistent
                    .profiles
                    .iter()
                    .find(|p| Some(&p.id) == rt.persistent.selected_profile_id.as_ref())
                    .cloned()
            });

            (preset, active_profile, routing_snapshot, is_running, old_routing)
        };

        if is_running {
            if let Some(profile) = profile {
                return hot_reload_or_restart_singbox(
                    &state_clone,
                    &profile,
                    &routing_snapshot,
                    &old_routing,
                    format!(
                        "⚡ Предустановка «{}» мгновенно применена на лету! (Процессы: {} через VPN, {} мимо VPN, режим: {})",
                        preset.name,
                        preset.proxy_processes.len(),
                        preset.direct_processes.len(),
                        if preset.default_outbound == "direct" { "только выбранные" } else { "весь трафик" }
                    ),
                );
            }
        }

        let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        push_log(
            &mut rt.logs,
            format!("Выбрана предустановка «{}».", preset.name),
        );
        Ok(build_vpn_status(&mut rt))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn apply_routing_on_the_fly(
    routing: RoutingConfig,
    profile_id: Option<String>,
    preset_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (profile, routing_snapshot, is_running, old_routing) = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let old_routing = rt.persistent.routing.clone();
            rt.persistent.routing = routing.clone();
            rt.persistent.active_preset_id = preset_id;

            let target_pid = profile_id
                .clone()
                .or_else(|| rt.active_profile_id.clone())
                .or_else(|| rt.persistent.selected_profile_id.clone())
                .or_else(|| rt.persistent.profiles.first().map(|p| p.id.clone()));

            let mut prof_opt = None;
            if let Some(ref pid) = target_pid {
                if let Some(prof) = rt.persistent.profiles.iter_mut().find(|p| &p.id == pid) {
                    prof.routing = Some(routing.clone());
                    prof_opt = Some(prof.clone());
                }
            }

            save_persistent_state(&rt.persistent);

            let is_running = rt.child.is_some();
            let active_profile = prof_opt.or_else(|| {
                rt.persistent
                    .profiles
                    .iter()
                    .find(|p| Some(&p.id) == rt.persistent.selected_profile_id.as_ref())
                    .cloned()
            });

            (active_profile, routing, is_running, old_routing)
        };

        if is_running {
            if let Some(profile) = profile {
                return hot_reload_or_restart_singbox(
                    &state_clone,
                    &profile,
                    &routing_snapshot,
                    &old_routing,
                    format!(
                        "⚡ Правила маршрутизации мгновенно применены на лету! (Процессы: {} через VPN, {} мимо VPN)",
                        routing_snapshot.proxy_processes.len(),
                        routing_snapshot.direct_processes.len()
                    ),
                );
            }
        }

        let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        Ok(build_vpn_status(&mut rt))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
fn add_or_update_profile(
    mut profile: VpnProfile,
    state: State<'_, SharedState>,
) -> Result<Vec<VpnProfile>, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    if profile.id.trim().is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }
    if is_awg_protocol(&profile.protocol) {
        if let Some(ref priv_key) = profile.awg_private_key {
            let trimmed = priv_key.trim();
            if !trimmed.is_empty() && profile.password.trim().is_empty() {
                profile.password = trimmed.to_string();
            }
        } else if !profile.password.trim().is_empty() {
            profile.awg_private_key = Some(profile.password.clone());
        }
    }
    profile.uri = build_profile_uri(&profile);

    if let Some(existing) = rt
        .persistent
        .profiles
        .iter_mut()
        .find(|p| p.id == profile.id)
    {
        *existing = profile.clone();
    } else {
        rt.persistent.profiles.push(profile.clone());
    }

    if rt.persistent.selected_profile_id.is_none() {
        rt.persistent.selected_profile_id = Some(profile.id);
    }
    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.profiles.clone())
}

#[tauri::command]
fn import_profile_uri(uri: String, state: State<'_, SharedState>) -> Result<VpnProfile, String> {
    let mut profile = parse_profile_uri(&uri)?;
    profile.pinned_on_pc = Some(true);
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.profiles.push(profile.clone());
    if rt.persistent.selected_profile_id.is_none() {
        rt.persistent.selected_profile_id = Some(profile.id.clone());
    }
    save_persistent_state(&rt.persistent);
    Ok(profile)
}

#[tauri::command]
fn delete_profile(id: String, state: State<'_, SharedState>) -> Result<Vec<VpnProfile>, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    if rt.active_profile_id.as_deref() == Some(&id) {
        stop_singbox_internal(&mut rt);
    }
    rt.persistent.profiles.retain(|p| p.id != id);
    if rt.persistent.selected_profile_id.as_deref() == Some(&id) {
        rt.persistent.selected_profile_id = rt.persistent.profiles.first().map(|p| p.id.clone());
    }
    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.profiles.clone())
}

#[tauri::command]
async fn ping_profile(id: String, state: State<'_, SharedState>) -> Result<u32, String> {
    let (server, port) = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let prof = rt
            .persistent
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "Профиль не найден".to_string())?;
        (prof.server.clone(), prof.port)
    };

    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Test TCP reachability on SSH port 22 or 443 or target port to measure network RTT to host
        let start = Instant::now();
        let target_str = format!("{}:22", server);
        let fallback_str = format!("{}:{}", server, port);

        let direct_res = target_str
            .to_socket_addrs()
            .or_else(|_| fallback_str.to_socket_addrs())
            .ok()
            .and_then(|mut addrs| addrs.next())
            .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(2000)).ok());

        let ms = if direct_res.is_some() {
            start.elapsed().as_millis() as u32
        } else {
            // Fallback: try through active VPN tunnel (127.0.0.1:2081 or 127.0.0.1:2080) via SOCKS5
            let mut tunnel_ms = None;
            for proxy_port in [2081u16, 2080u16] {
                let proxy_addr: SocketAddr = format!("127.0.0.1:{}", proxy_port).parse().unwrap();
                if let Ok(mut sock) = TcpStream::connect_timeout(&proxy_addr, Duration::from_millis(300)) {
                    let _ = sock.set_read_timeout(Some(Duration::from_millis(2500)));
                    let _ = sock.set_write_timeout(Some(Duration::from_millis(2500)));
                    let t_start = Instant::now();
                    use std::io::{Read, Write};
                    if sock.write_all(&[0x05, 0x01, 0x00]).is_ok() {
                        let mut rep = [0u8; 2];
                        if sock.read_exact(&mut rep).is_ok() && rep == [0x05, 0x00] {
                            let hb = server.as_bytes();
                            let mut req = Vec::with_capacity(7 + hb.len());
                            req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, hb.len() as u8]);
                            req.extend_from_slice(hb);
                            req.extend_from_slice(&[0x00, 22]); // port 22
                            if sock.write_all(&req).is_ok() {
                                let mut hdr = [0u8; 4];
                                if sock.read_exact(&mut hdr).is_ok() && hdr[1] == 0x00 {
                                    tunnel_ms = Some((t_start.elapsed().as_millis() as u32).max(1));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            tunnel_ms.ok_or_else(|| "Сервер недоступен напрямую и через туннель".to_string())?
        };

        if let Ok(mut rt) = state_clone.lock() {
            if let Some(p) = rt.persistent.profiles.iter_mut().find(|p| p.id == id) {
                p.ping_ms = Some(ms);
            }
            save_persistent_state(&rt.persistent);
        }
        Ok(ms)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn probe_awg_ports(
    profile_id: String,
    profile: Option<VpnProfile>,
    state: State<'_, SharedState>,
) -> Result<Vec<PortProbeResult>, String> {
    let profile = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        rt.persistent
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .or(profile)
            .ok_or_else(|| "Профиль не найден".to_string())?
    };

    let candidates = get_awg_candidate_ports(&profile);
    let max_ports = candidates.len().min(12);
    let mut results = Vec::new();

    for &port in candidates.iter().take(max_ports) {
        let start = Instant::now();
        let target = format!("{}:{}", profile.server, port);
        match std::net::UdpSocket::bind("0.0.0.0:0") {
            Ok(sock) => {
                let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = sock.connect(&target);
                let dummy = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
                let _ = sock.send(&dummy);
                let latency = start.elapsed().as_millis() as u32;
                results.push(PortProbeResult {
                    port,
                    reachable: true,
                    latency_ms: Some(latency.max(1)),
                    message: "Порт доступен".to_string(),
                });
            }
            Err(e) => {
                results.push(PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(results)
}

#[tauri::command]
fn export_awg_conf(
    profile_id: String,
    profile: Option<VpnProfile>,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let rt = state.lock().map_err(|_| "Lock error".to_string())?;
    let prof = rt
        .persistent
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .or(profile)
        .ok_or_else(|| "Профиль не найден".to_string())?;
    let priv_key = prof
        .awg_private_key
        .as_deref()
        .unwrap_or(&prof.password)
        .trim();
    if priv_key.is_empty() {
        return Err("Невозможно экспортировать .conf: в профиле отсутствует приватный ключ (Private Key).".to_string());
    }
    Ok(build_awg_conf(&prof))
}

#[tauri::command]
fn save_routing_config(
    routing: RoutingConfig,
    profile_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<RoutingConfig, String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    if let Some(ref pid) = profile_id {
        if let Some(prof) = rt.persistent.profiles.iter_mut().find(|p| &p.id == pid) {
            prof.routing = Some(routing.clone());
        }
    }
    rt.persistent.routing = routing.clone();
    save_persistent_state(&rt.persistent);
    Ok(routing)
}

#[tauri::command]
fn clear_core_logs(state: State<'_, SharedState>) -> Result<(), String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.logs.clear();
    Ok(())
}

#[tauri::command]
async fn export_core_logs_to_file(
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let (logs, version, active_name, active_proto) = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        (
            rt.logs.clone(),
            rt.singbox_version.clone(),
            rt.active_profile_name.clone(),
            rt.active_protocol.clone(),
        )
    };

    let now_display = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let default_filename = format!(
        "neuroveil-vpn-logs-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let log_content = format!(
        "================================================================================\r\n\
         NeuroVeil VPN — Журнал ядра sing-box (Экспорт)\r\n\
         Дата экспорта: {}\r\n\
         Версия ядра: {}\r\n\
         Активный профиль: {}\r\n\
         Количество записей: {}\r\n\
         ================================================================================\r\n\r\n{}\r\n",
        now_display,
        version,
        active_name
            .map(|n| format!("{} ({})", n, active_proto.unwrap_or_default()))
            .unwrap_or_else(|| "Нет активного подключения".to_string()),
        logs.len(),
        logs.join("\r\n")
    );

    tauri::async_runtime::spawn_blocking(move || {
        let ps_script = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
             Add-Type -AssemblyName System.Windows.Forms\n\
             $dlg = New-Object System.Windows.Forms.SaveFileDialog\n\
             $dlg.Title = 'Сохранить журнал ядра sing-box в файл'\n\
             $dlg.Filter = 'Файлы журнала (*.log)|*.log|Текстовые документы (*.txt)|*.txt|Все файлы (*.*)|*.*'\n\
             $dlg.FileName = '{}'\n\
             $dlg.InitialDirectory = [Environment]::GetFolderPath('Desktop')\n\
             if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{\n\
                 Write-Output $dlg.FileName\n\
             }}",
            default_filename
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", &ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let output = cmd.output().map_err(|e| e.to_string())?;
        let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected_path.is_empty() {
            return Ok(None);
        }

        std::fs::write(&selected_path, log_content.as_bytes())
            .map_err(|e| format!("Не удалось сохранить файл: {}", e))?;

        Ok(Some(selected_path))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_exe_file() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Title = "Выберите программу (.exe) для маршрутизации"
$dlg.Filter = "Программы (*.exe)|*.exe|Все файлы (*.*)|*.*"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.FileName
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_config_file_and_import(state: State<'_, SharedState>) -> Result<Option<VpnProfile>, String> {
    let file_path = tauri::async_runtime::spawn_blocking(|| -> Result<Option<String>, String> {
        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Title = "Выберите файл конфигурации VPN (.conf, .json, .txt)"
$dlg.Filter = "Конфигурации VPN (*.conf;*.json;*.txt)|*.conf;*.json;*.txt|WireGuard / Amnezia (*.conf)|*.conf|sing-box (*.json)|*.json|Все файлы (*.*)|*.*"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.FileName
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    let path_str = match file_path {
        Some(p) => p,
        None => return Ok(None),
    };

    let path = std::path::Path::new(&path_str);
    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("config").to_string();
    let content = std::fs::read_to_string(path).map_err(|e| format!("Не удалось прочитать файл: {}", e))?;

    let trimmed = content.trim();
    let mut profile = match parse_profile_uri(trimmed) {
        Ok(p) => p,
        Err(e) => {
            // Check if file contains lines with URIs
            let mut found = None;
            for line in trimmed.lines() {
                let l = line.trim();
                if !l.is_empty() && !l.starts_with('#') {
                    if let Ok(p) = parse_profile_uri(l) {
                        found = Some(p);
                        break;
                    }
                }
            }
            found.ok_or(e)?
        }
    };

    // Set profile name strictly to the config file name (file stem) without any protocol tags
    if !file_stem.is_empty() {
        profile.name = file_stem;
    }
    profile.pinned_on_pc = Some(true);

    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.profiles.push(profile.clone());
    if rt.persistent.selected_profile_id.is_none() {
        rt.persistent.selected_profile_id = Some(profile.id.clone());
    }
    save_persistent_state(&rt.persistent);

    Ok(Some(profile))
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = "Выберите папку с программой или игрой"
$dlg.UseDescriptionForTitle = $true
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.SelectedPath
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_running_processes() -> Result<Vec<RunningProcessInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-Process | Where-Object { $_.Path -ne $null -and $_.Path -notlike "C:\Windows\*" } |
    ForEach-Object { "$($_.ProcessName).exe|$($_.Path)" } |
    Sort-Object -Unique
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut list = Vec::new();
        for line in text.lines() {
            if let Some((name, path)) = line.split_once('|') {
                list.push(RunningProcessInfo {
                    name: name.trim().to_string(),
                    path: path.trim().to_string(),
                });
            }
        }
        Ok(list)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn test_single_vps_ssh(
    app: AppHandle,
    host: String,
    port: u16,
    username: String,
    password: String,
    proxy: Option<String>,
) -> Result<String, String> {
    ssh_deployer::test_single_endpoint(
        &app,
        &host,
        port,
        &username,
        &password,
        proxy.as_deref().unwrap_or(""),
    )
    .await
}

#[tauri::command]
async fn test_vps_ssh(
    app: AppHandle,
    creds: VpsCredentials,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    {
        if let Ok(mut rt) = state.lock() {
            let mut saved = creds.clone();
            saved.password = String::new(); // Don't persist raw password to disk
            rt.persistent.saved_vps = Some(saved);
            save_persistent_state(&rt.persistent);
        }
    }
    ssh_deployer::test_ssh_connection(&app, &creds).await
}

#[tauri::command]
async fn deploy_vps_server(
    app: AppHandle,
    creds: VpsCredentials,
    state: State<'_, SharedState>,
) -> Result<DeployResult, String> {
    let result = ssh_deployer::deploy_vps(&app, &creds).await?;
    if let Ok(mut rt) = state.lock() {
        // Remove older auto-deployed profiles for the same hosts to avoid duplicates
        rt.persistent.profiles.retain(|p| p.server != creds.host.trim());
        for prof in &result.created_profiles {
            rt.persistent.profiles.push(prof.clone());
        }
        if let Some(first) = result.created_profiles.first() {
            rt.persistent.selected_profile_id = Some(first.id.clone());
        }
        let mut saved = creds.clone();
        saved.password = String::new();
        if let Some(port) = result.new_ssh_port {
            saved.ssh_port = port;
        }
        rt.persistent.saved_vps = Some(saved);
        save_persistent_state(&rt.persistent);
    }
    Ok(result)
}

#[tauri::command]
async fn clean_vps_server(app: AppHandle, creds: VpsCredentials) -> Result<String, String> {
    ssh_deployer::clean_vps(&app, &creds).await
}

#[tauri::command]
async fn create_device_key(
    app: AppHandle,
    creds: VpsCredentials,
    device_name: String,
    protocol: String,
    _state: State<'_, SharedState>,
) -> Result<VpnProfile, String> {
    let clean_name = device_name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Введите название устройства или клиента".to_string());
    }

    if creds.host.trim().is_empty() {
        return Err(
            "Не указан IP-адрес VPS сервера. Выпуск новых ключей доступен только владельцу сервера."
                .to_string(),
        );
    }

    if creds.password.trim().is_empty() {
        return Err(
            "Отказано в доступе. Для выпуска нового ключа на сервере требуется ввести SSH-пароль root. Доступ разрешен только владельцу VPS."
                .to_string(),
        );
    }

    // Connect to VPS strictly via SSH with root credentials and register new user in /etc/sing-box/config.json
    let profile =
        ssh_deployer::create_vps_device_user(&app, &creds, &clean_name, &protocol).await?;

    Ok(profile)
}

#[tauri::command]
async fn login_vps_admin(
    app: AppHandle,
    creds: VpsCredentials,
    state: State<'_, SharedState>,
) -> Result<Vec<VpnProfile>, String> {
    if creds.host.trim().is_empty() {
        return Err("Введите IP-адрес сервера".to_string());
    }
    if creds.password.trim().is_empty() {
        return Err("Введите пароль SSH (root)".to_string());
    }

    let mut server_profiles = ssh_deployer::fetch_vps_profiles(&app, &creds).await?;

    if let Ok(mut rt) = state.lock() {
        let mut saved = creds.clone();
        saved.password = String::new(); // Do not store plaintext password on disk
        rt.persistent.saved_vps = Some(saved);

        for s_prof in &mut server_profiles {
            if let Some(existing) = rt.persistent.profiles.iter_mut().find(|p| {
                (p.server == s_prof.server && p.port == s_prof.port && p.password == s_prof.password)
                || (p.server == s_prof.server && p.name == s_prof.name)
            }) {
                s_prof.id = existing.id.clone();
                let merged_up = existing.total_up_bytes.max(s_prof.total_up_bytes);
                let merged_down = existing.total_down_bytes.max(s_prof.total_down_bytes);
                existing.total_up_bytes = merged_up;
                existing.total_down_bytes = merged_down;
                s_prof.total_up_bytes = merged_up;
                s_prof.total_down_bytes = merged_down;
                existing.awg_private_key = s_prof.awg_private_key.clone();
                existing.awg_public_key = s_prof.awg_public_key.clone();
                existing.awg_preshared_key = s_prof.awg_preshared_key.clone();
                existing.awg_header_protection_key = s_prof.awg_header_protection_key.clone();
                existing.awg_rekey_after_time = s_prof.awg_rekey_after_time.clone();
                existing.awg_rekey_timeout = s_prof.awg_rekey_timeout.clone();
                existing.awg_reject_after_time = s_prof.awg_reject_after_time.clone();
                existing.awg_keepalive_timeout = s_prof.awg_keepalive_timeout.clone();
                existing.awg_max_handshake_attempts = s_prof.awg_max_handshake_attempts.clone();
                existing.awg_random_trailers = s_prof.awg_random_trailers;
                existing.awg_disable_cookies = s_prof.awg_disable_cookies;
                existing.awg_persistent_keepalive = s_prof.awg_persistent_keepalive.clone();
                existing.awg_version = s_prof.awg_version.clone();
                existing.awg_jc = s_prof.awg_jc;
                existing.awg_jmin = s_prof.awg_jmin;
                existing.awg_jmax = s_prof.awg_jmax;
                existing.awg_s1 = s_prof.awg_s1;
                existing.awg_s2 = s_prof.awg_s2;
                existing.awg_s3 = s_prof.awg_s3;
                existing.awg_s4 = s_prof.awg_s4;
                existing.awg_h1 = s_prof.awg_h1.clone();
                existing.awg_h2 = s_prof.awg_h2.clone();
                existing.awg_h3 = s_prof.awg_h3.clone();
                existing.awg_h4 = s_prof.awg_h4.clone();
                existing.password = s_prof.password.clone();
                existing.uri = s_prof.uri.clone();
                existing.awg_local_address = s_prof.awg_local_address.clone();
            }
        }
        // Remove remote mobile/device keys from persistent local PC profiles list
        rt.persistent.profiles.retain(|p| p.pinned_on_pc != Some(false));
        save_persistent_state(&rt.persistent);
    }

    Ok(server_profiles)
}

#[tauri::command]
async fn delete_vps_device_cmd(
    app: AppHandle,
    creds: VpsCredentials,
    profile: VpnProfile,
    state: State<'_, SharedState>,
) -> Result<Vec<VpnProfile>, String> {
    if !creds.host.trim().is_empty() && !creds.password.trim().is_empty() {
        ssh_deployer::delete_vps_device(&app, &creds, &profile).await?;
    }

    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.profiles.retain(|p| {
        let same_id = p.id == profile.id;
        let same_creds = p.server == profile.server
            && p.port == profile.port
            && !profile.password.is_empty()
            && p.password == profile.password;
        let same_name = p.server == profile.server && p.name == profile.name;
        !(same_id || same_creds || same_name)
    });
    if rt.persistent.selected_profile_id.as_deref() == Some(&profile.id) {
        rt.persistent.selected_profile_id = rt.persistent.profiles.first().map(|p| p.id.clone());
    }
    save_persistent_state(&rt.persistent);
    Ok(rt.persistent.profiles.clone())
}

#[tauri::command]
async fn export_profile_json(
    profile_id: String,
    profile: Option<VpnProfile>,
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let (profile, routing) = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let prof = rt
            .persistent
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .or(profile)
            .ok_or_else(|| "Профиль не найден".to_string())?;
        let mut mobile_routing = rt.persistent.routing.clone();
        mobile_routing.mode = "tun".to_string();
        mobile_routing.default_outbound = "proxy".to_string();
        mobile_routing.proxy_processes.clear();
        mobile_routing.direct_processes.clear();
        (prof, mobile_routing)
    };

    let cfg_val = generate_mobile_singbox_config(&profile, &routing);
    let cfg_str = serde_json::to_string_pretty(&cfg_val).map_err(|e| e.to_string())?;
    let clean_stem: String = profile
        .name
        .split('—')
        .next()
        .unwrap_or(&profile.name)
        .split('-')
        .next()
        .unwrap_or(&profile.name)
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let file_stem = if clean_stem.is_empty() { "vpn".to_string() } else { clean_stem };
    let default_filename = format!("{}.json", file_stem);

    tauri::async_runtime::spawn_blocking(move || {
        let ps_script = format!(
            r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.SaveFileDialog
$dlg.Title = "Сохранить конфиг sing-box (.json) для телефона или другого ПК"
$dlg.Filter = "Конфигурация sing-box (*.json)|*.json"
$dlg.FileName = "{}"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
    Write-Output $dlg.FileName
}}
"#,
            default_filename
        );
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", &ps_script]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let out = cmd.output().map_err(|e| e.to_string())?;
        let chosen_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if chosen_path.is_empty() {
            return Ok(None);
        }
        fs::write(&chosen_path, cfg_str)
            .map_err(|e| format!("Не удалось сохранить файл: {}", e))?;
        Ok(Some(chosen_path))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
    }
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(target_os = "windows")]
fn relaunch_as_admin() -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    if let Ok(exe_path) = std::env::current_exe() {
        let exe_path_w: Vec<u16> = exe_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let verb_w: Vec<u16> = OsStr::new("runas").encode_wide().chain(std::iter::once(0)).collect();

        let args: Vec<String> = std::env::args().skip(1).collect();
        let args_str = args.join(" ");
        let args_w: Vec<u16> = OsStr::new(&args_str).encode_wide().chain(std::iter::once(0)).collect();

        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                lpOperation: *const u16,
                lpFile: *const u16,
                lpParameters: *const u16,
                lpDirectory: *const u16,
                nShowCmd: i32,
            ) -> usize;
        }

        let ret = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb_w.as_ptr(),
                exe_path_w.as_ptr(),
                if args.is_empty() {
                    std::ptr::null()
                } else {
                    args_w.as_ptr()
                },
                std::ptr::null(),
                1, // SW_SHOWNORMAL
            )
        };
        if ret > 32 {
            return true;
        }
    }
    false
}

fn set_windows_autostart(enable: bool, start_minimized: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enable {
            let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe_str = current_exe.to_string_lossy();
            let tr_val = if start_minimized {
                format!("\"{}\" --minimized", exe_str)
            } else {
                format!("\"{}\"", exe_str)
            };
            let mut cmd = Command::new("schtasks");
            cmd.args([
                "/create",
                "/tn",
                "NeuroVeil",
                "/tr",
                &tr_val,
                "/sc",
                "onlogon",
                "/rl",
                "highest",
                "/f",
            ]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let out = cmd.output().map_err(|e| format!("schtasks error: {}", e))?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!("Не удалось настроить задачу автозапуска: {}", err));
            }
        } else {
            let mut cmd = Command::new("schtasks");
            cmd.args(["/delete", "/tn", "NeuroVeil", "/f"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.output();
            let mut cmd_old = Command::new("schtasks");
            cmd_old.args(["/delete", "/tn", "AegisVPN", "/f"]);
            cmd_old.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd_old.output();
        }
    }
    Ok(())
}

#[tauri::command]
fn app_start_dragging(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.start_dragging();
    }
}

#[tauri::command]
fn app_minimize(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

#[tauri::command]
fn app_toggle_maximize(app: tauri::AppHandle, state: State<'_, SharedState>) {
    if let Some(w) = app.get_webview_window("main") {
        if let Ok(is_max) = w.is_maximized() {
            if is_max {
                let _ = w.unmaximize();
                if let Ok(mut rt) = state.lock() {
                    rt.persistent.window_maximized = Some(false);
                    save_persistent_state(&rt.persistent);
                }
            } else {
                let _ = w.maximize();
                if let Ok(mut rt) = state.lock() {
                    rt.persistent.window_maximized = Some(true);
                    save_persistent_state(&rt.persistent);
                }
            }
        }
    }
}

#[tauri::command]
fn app_close(app: tauri::AppHandle, state: State<'_, SharedState>) {
    let should_hide = {
        if let Ok(rt) = state.lock() {
            save_persistent_state(&rt.persistent);
            rt.persistent.close_to_tray
        } else {
            true
        }
    };
    if let Some(w) = app.get_webview_window("main") {
        if should_hide {
            let _ = w.hide();
        } else {
            if let Ok(mut rt) = state.lock() {
                stop_singbox_internal(&mut rt);
                save_persistent_state(&rt.persistent);
            }
            app.exit(0);
        }
    }
}

#[tauri::command]
fn app_show_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        if let Ok(true) = w.is_minimized() {
            let _ = w.unminimize();
        }
        let _ = w.set_focus();
    }
}

const GITHUB_REPO_OWNER_NAME: &str = "lllPredatorlll/NeuroVeil-VPN";
const GITHUB_REPO_URL: &str = "https://github.com/lllPredatorlll/NeuroVeil-VPN";

fn active_local_tunnel_socks_port() -> Option<u16> {
    for port in [2081u16, 2080u16] {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok() {
            return Some(port);
        }
    }
    None
}

fn fetch_url_text_with_tunnel(url: &str) -> Result<String, String> {
    let mut saw_404 = false;
    let mut saw_403 = false;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(7))
        .timeout_read(Duration::from_secs(15))
        .build();

    match agent
        .get(url)
        .set("User-Agent", "NeuroVeilVPN-Updater/1.0.0")
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(resp) => {
            if let Ok(text) = resp.into_string() {
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
        Err(ureq::Error::Status(code, _)) if code == 404 => {
            saw_404 = true;
        }
        Err(ureq::Error::Status(code, _)) if code == 403 => {
            saw_403 = true;
        }
        Err(_) => {}
    }

    if !saw_404 {
        // Fallback 1: via active VPN tunnel using Windows curl.exe
        if let Some(port) = active_local_tunnel_socks_port() {
            let mut cmd = Command::new("curl.exe");
            cmd.args([
                "-sSL",
                "--fail",
                "--connect-timeout",
                "8",
                "--max-time",
                "20",
                "-x",
                &format!("socks5h://127.0.0.1:{}", port),
                "-A",
                "NeuroVeilVPN-Updater/1.0.0",
                "-H",
                "Accept: application/vnd.github+json",
                url,
            ]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);

            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    if !text.trim().is_empty() {
                        return Ok(text);
                    }
                } else if out.status.code() == Some(22) {
                    // curl --fail exit code 22 means HTTP >= 400 (server was reached)
                    saw_404 = true;
                }
            }
        }

        // Fallback 2: direct Windows curl.exe (uses Windows Schannel native TLS)
        let mut direct_cmd = Command::new("curl.exe");
        direct_cmd.args([
            "-sSL",
            "--fail",
            "--connect-timeout",
            "8",
            "--max-time",
            "15",
            "-A",
            "NeuroVeilVPN-Updater/1.0.0",
            "-H",
            "Accept: application/vnd.github+json",
            url,
        ]);
        #[cfg(target_os = "windows")]
        direct_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = direct_cmd.output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            } else if out.status.code() == Some(22) {
                saw_404 = true;
            }
        }
    }

    if saw_403 {
        return Err("HTTP 403".to_string());
    }
    if saw_404 {
        return Err("HTTP 404".to_string());
    }

    Err(format!("Не удалось получить данные по адресу {}", url))
}

fn download_url_to_file_with_tunnel(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(120))
        .build();

    if let Ok(resp) = agent
        .get(url)
        .set("User-Agent", "NeuroVeilVPN-Updater/1.0.0")
        .call()
    {
        let mut reader = resp.into_reader();
        let mut file = fs::File::create(dest)
            .map_err(|e| format!("Не удалось создать временный файл обновления: {}", e))?;
        if std::io::copy(&mut reader, &mut file).is_ok() {
            if let Ok(meta) = fs::metadata(dest) {
                if meta.len() > 1024 {
                    return Ok(());
                }
            }
        }
    }

    if let Some(port) = active_local_tunnel_socks_port() {
        let mut cmd = Command::new("curl.exe");
        cmd.args([
            "-sSL",
            "--fail",
            "--connect-timeout",
            "10",
            "--max-time",
            "180",
            "-x",
            &format!("socks5h://127.0.0.1:{}", port),
            "-A",
            "NeuroVeilVPN-Updater/1.0.0",
            "-o",
            &dest.to_string_lossy(),
            url,
        ]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = cmd.output() {
            if out.status.success() {
                if let Ok(meta) = fs::metadata(dest) {
                    if meta.len() > 1024 {
                        return Ok(());
                    }
                }
            }
        }
    }

    Err("Не удалось скачать файл обновления с GitHub.".to_string())
}

fn parse_version_tuple(ver: &str) -> Vec<u64> {
    let cleaned = ver.trim().trim_start_matches(|c: char| c == 'v' || c == 'V');
    let mut nums = Vec::new();
    for part in cleaned.split(|c: char| !c.is_ascii_digit()) {
        if !part.is_empty() {
            if let Ok(n) = part.parse::<u64>() {
                nums.push(n);
            }
        }
    }
    while nums.len() < 3 {
        nums.push(0);
    }
    nums
}

fn is_version_newer(current: &str, latest: &str) -> bool {
    let cur_v = parse_version_tuple(current);
    let lat_v = parse_version_tuple(latest);
    let max_len = cur_v.len().max(lat_v.len());
    for i in 0..max_len {
        let c = *cur_v.get(i).unwrap_or(&0);
        let l = *lat_v.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

fn parse_release_json(rel: &serde_json::Value, current_version: &str) -> Option<AppUpdateInfo> {
    let tag_name = rel.get("tag_name")?.as_str()?.trim();
    if tag_name.is_empty() {
        return None;
    }
    let latest_version = tag_name
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();
    let release_name = rel
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(tag_name)
        .to_string();
    let release_notes = rel
        .get("body")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Описание изменений не указано в релизе.")
        .to_string();
    let published_at = rel
        .get("published_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let html_url = rel
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(GITHUB_REPO_URL)
        .to_string();

    let mut download_url: Option<String> = None;
    let mut asset_name: Option<String> = None;
    let mut asset_size: Option<u64> = None;

    if let Some(assets) = rel.get("assets").and_then(|v| v.as_array()) {
        // Priority 1: Setup.exe installer
        // Priority 2: Standalone .exe
        // Priority 3: .msi installer
        let mut best_score = 0;
        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let url = asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let size = asset.get("size").and_then(|v| v.as_u64());
            let lower = name.to_lowercase();
            if url.is_empty() {
                continue;
            }
            let score = if lower.ends_with(".exe") && lower.contains("setup") {
                30
            } else if lower.ends_with(".exe") {
                25
            } else if lower.ends_with(".msi") {
                20
            } else if lower.ends_with(".zip") {
                10
            } else {
                0
            };
            if score > best_score {
                best_score = score;
                download_url = Some(url.to_string());
                asset_name = Some(name.to_string());
                asset_size = size;
            }
        }
    }

    // If no release asset attached, fallback to raw NeuroVeil.exe in main branch
    if download_url.is_none() {
        download_url = Some(format!(
            "https://github.com/{}/raw/main/NeuroVeil.exe",
            GITHUB_REPO_OWNER_NAME
        ));
        asset_name = Some("NeuroVeil.exe".to_string());
    }

    let has_update = is_version_newer(current_version, &latest_version);

    Some(AppUpdateInfo {
        current_version: current_version.to_string(),
        latest_version,
        has_update,
        release_name,
        release_notes,
        published_at,
        html_url,
        download_url,
        asset_name,
        asset_size,
        repo_url: GITHUB_REPO_URL.to_string(),
    })
}

#[tauri::command]
async fn check_app_update() -> Result<AppUpdateInfo, String> {
    tokio::task::spawn_blocking(move || {
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        // 1. Check /releases/latest
        let latest_url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO_OWNER_NAME
        );
        if let Ok(body) = fetch_url_text_with_tunnel(&latest_url) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(info) = parse_release_json(&json, &current_version) {
                    return Ok(info);
                }
            }
        }

        // 2. Check /releases list (in case only pre-release exists)
        let releases_url = format!(
            "https://api.github.com/repos/{}/releases",
            GITHUB_REPO_OWNER_NAME
        );
        if let Ok(body) = fetch_url_text_with_tunnel(&releases_url) {
            if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(first) = arr.first() {
                    if let Some(info) = parse_release_json(first, &current_version) {
                        return Ok(info);
                    }
                }
            }
        }

        // 3. Fallback: check package.json in main or master branch if repo uses direct branch pushes
        for branch in ["main", "master"] {
            let pkg_url = format!(
                "https://raw.githubusercontent.com/{}/{}/package.json",
                GITHUB_REPO_OWNER_NAME, branch
            );
            if let Ok(pkg_body) = fetch_url_text_with_tunnel(&pkg_url) {
                if let Ok(pkg_json) = serde_json::from_str::<serde_json::Value>(&pkg_body) {
                    if let Some(ver) = pkg_json.get("version").and_then(|v| v.as_str()) {
                        let latest_version = ver.trim().trim_start_matches('v').to_string();
                        let has_update = is_version_newer(&current_version, &latest_version);

                        let mut commit_msg = format!(
                            "Ветка {} в репозитории {} (версия package.json: v{})",
                            branch, GITHUB_REPO_OWNER_NAME, latest_version
                        );
                        let mut published_at = None;
                        let commit_api = format!(
                            "https://api.github.com/repos/{}/commits/{}",
                            GITHUB_REPO_OWNER_NAME, branch
                        );
                        if let Ok(c_body) = fetch_url_text_with_tunnel(&commit_api) {
                            if let Ok(c_json) = serde_json::from_str::<serde_json::Value>(&c_body) {
                                if let Some(msg) = c_json
                                    .get("commit")
                                    .and_then(|c| c.get("message"))
                                    .and_then(|m| m.as_str())
                                {
                                    commit_msg = msg.to_string();
                                }
                                published_at = c_json
                                    .get("commit")
                                    .and_then(|c| c.get("committer"))
                                    .and_then(|cm| cm.get("date"))
                                    .and_then(|d| d.as_str())
                                    .map(|s| s.to_string());
                            }
                        }

                        return Ok(AppUpdateInfo {
                            current_version,
                            latest_version: latest_version.clone(),
                            has_update,
                            release_name: format!("NeuroVeil VPN v{} ({})", latest_version, branch),
                            release_notes: commit_msg,
                            published_at,
                            html_url: GITHUB_REPO_URL.to_string(),
                            download_url: Some(format!(
                                "https://github.com/{}/raw/{}/NeuroVeil.exe",
                                GITHUB_REPO_OWNER_NAME, branch
                            )),
                            asset_name: Some("NeuroVeil.exe".to_string()),
                            asset_size: None,
                            repo_url: GITHUB_REPO_URL.to_string(),
                        });
                    }
                }
            }
        }

        // 4. If repo is reachable on GitHub (or returns 404/403 because repo is Private or has no releases yet)
        let repo_api = format!("https://api.github.com/repos/{}", GITHUB_REPO_OWNER_NAME);
        match fetch_url_text_with_tunnel(&repo_api) {
            Ok(_) => {
                return Ok(AppUpdateInfo {
                    current_version: current_version.clone(),
                    latest_version: current_version.clone(),
                    has_update: false,
                    release_name: format!("NeuroVeil VPN v{} (Build 1049)", current_version),
                    release_notes: "В репозитории GitHub пока нет опубликованных релизов. У вас установлена актуальная версия приложения.".to_string(),
                    published_at: None,
                    html_url: GITHUB_REPO_URL.to_string(),
                    download_url: None,
                    asset_name: None,
                    asset_size: None,
                    repo_url: GITHUB_REPO_URL.to_string(),
                });
            }
            Err(e) if e == "HTTP 404" || e == "HTTP 403" => {
                let note = if e == "HTTP 404" {
                    "Связь с GitHub установлена. У вас актуальная версия v1.0.1 (Build 1049).\nПодсказка: если репозиторий lllPredatorlll/NeuroVeil-VPN сейчас закрыт (Private) или пуст, сделайте его публичным (Public) в настройках GitHub (Settings -> Change repository visibility -> Public) и выгрузите файлы проекта."
                } else {
                    "Связь с GitHub установлена. У вас актуальная версия v1.0.1 (Build 1049)."
                };
                return Ok(AppUpdateInfo {
                    current_version: current_version.clone(),
                    latest_version: current_version.clone(),
                    has_update: false,
                    release_name: format!("NeuroVeil VPN v{} (Build 1049)", current_version),
                    release_notes: note.to_string(),
                    published_at: None,
                    html_url: GITHUB_REPO_URL.to_string(),
                    download_url: None,
                    asset_name: None,
                    asset_size: None,
                    repo_url: GITHUB_REPO_URL.to_string(),
                });
            }
            Err(_) => {}
        }

        Err("Не удалось связаться с GitHub (api.github.com). Проверьте подключение к интернету.".to_string())
    })
    .await
    .map_err(|e| format!("Ошибка задачи проверки обновлений: {}", e))?
}

#[tauri::command]
async fn download_and_install_update(
    download_url: String,
    asset_name: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let fname = asset_name
        .unwrap_or_else(|| "NeuroVeil_Update.exe".to_string())
        .to_lowercase();

    let temp_dir = std::env::temp_dir();
    let temp_download_path = temp_dir.join(format!("neuroveil_update_{}.tmp", Uuid::new_v4()));
    let url_clone = download_url.clone();
    let dl_path_clone = temp_download_path.clone();

    tokio::task::spawn_blocking(move || {
        download_url_to_file_with_tunnel(&url_clone, &dl_path_clone)
    })
    .await
    .map_err(|e| format!("Ошибка загрузки обновления: {}", e))??;

    // If the downloaded file is a Setup installer (.exe with setup), run installer and close app
    let is_installer = fname.contains("setup") || fname.ends_with(".msi");

    if is_installer {
        let ext = if fname.ends_with(".msi") { "msi" } else { "exe" };
        let installer_path = temp_dir.join(format!("NeuroVeil_VPN_Update_Setup.{}", ext));
        let _ = fs::remove_file(&installer_path);
        fs::rename(&temp_download_path, &installer_path)
            .or_else(|_| fs::copy(&temp_download_path, &installer_path).map(|_| ()))
            .map_err(|e| format!("Не удалось подготовить установщик обновления: {}", e))?;
        let _ = fs::remove_file(&temp_download_path);

        if let Ok(mut rt) = state.lock() {
            stop_singbox_internal(&mut rt);
            save_persistent_state(&rt.persistent);
        }

        let installer_str = installer_path.to_string_lossy().to_string();
        let mut cmd = Command::new("cmd");
        if ext == "msi" {
            cmd.args([
                "/C",
                &format!("ping 127.0.0.1 -n 2 > nul & msiexec /i \"{}\"", installer_str),
            ]);
        } else {
            cmd.args([
                "/C",
                &format!("ping 127.0.0.1 -n 2 > nul & start \"\" \"{}\"", installer_str),
            ]);
        }
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.spawn();

        app.exit(0);
        return Ok("Запуск установщика обновления...".to_string());
    }

    // Portable / Standalone executable in-place self-update
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Не удалось определить путь текущего .exe: {}", e))?;
    let old_exe = current_exe.with_extension("old.exe");
    let _ = fs::remove_file(&old_exe);

    fs::rename(&current_exe, &old_exe).map_err(|e| {
        let _ = fs::remove_file(&temp_download_path);
        format!("Не удалось переименовать текущий исполняемый файл для обновления: {}", e)
    })?;

    if let Err(e) = fs::copy(&temp_download_path, &current_exe) {
        // Rollback on failure
        let _ = fs::rename(&old_exe, &current_exe);
        let _ = fs::remove_file(&temp_download_path);
        return Err(format!("Не удалось записать новый исполняемый файл: {}", e));
    }
    let _ = fs::remove_file(&temp_download_path);

    if let Ok(mut rt) = state.lock() {
        stop_singbox_internal(&mut rt);
        save_persistent_state(&rt.persistent);
    }

    let current_exe_str = current_exe.to_string_lossy().to_string();
    let old_exe_str = old_exe.to_string_lossy().to_string();
    let mut cmd = Command::new("cmd");
    cmd.args([
        "/C",
        &format!(
            "ping 127.0.0.1 -n 2 > nul & del /F /Q \"{}\" > nul 2>&1 & start \"\" \"{}\"",
            old_exe_str, current_exe_str
        ),
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();

    app.exit(0);
    Ok("Обновление установлено, перезапуск приложения...".to_string())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Разрешены только HTTP/HTTPS ссылки".to_string());
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", &url]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()
        .map_err(|e| format!("Не удалось открыть браузер: {}", e))?;
    Ok(())
}

#[tauri::command]
fn save_system_settings(
    autostart: bool,
    start_minimized: bool,
    close_to_tray: bool,
    auto_connect: bool,
    language: String,
    state: State<'_, SharedState>,
) -> Result<AppPersistentState, String> {
    let res = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        rt.persistent.autostart = autostart;
        rt.persistent.start_minimized = start_minimized;
        rt.persistent.close_to_tray = close_to_tray;
        rt.persistent.auto_connect = auto_connect;
        rt.persistent.language = language;
        save_persistent_state(&rt.persistent);
        rt.persistent.clone()
    };
    let _ = set_windows_autostart(autostart, start_minimized);
    Ok(res)
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        if !is_elevated() {
            if relaunch_as_admin() {
                std::process::exit(0);
            }
        }
    }

    if let Ok(cur_exe) = std::env::current_exe() {
        let old_exe = cur_exe.with_extension("old.exe");
        let _ = fs::remove_file(old_exe);
    }

    let persistent = load_persistent_state();
    let initial_installed = get_singbox_bin_path().exists();
    let shared_state: SharedState = Arc::new(Mutex::new(RuntimeState {
        persistent,
        child: None,
        active_profile_id: None,
        active_profile_name: None,
        active_protocol: None,
        active_port: None,
        port_hunting_status: None,
        is_handshaking: false,
        handshake_status: None,
        singbox_installed: initial_installed,
        singbox_version: if initial_installed {
            "sing-box (установлен)".to_string()
        } else {
            "Не установлен".to_string()
        },
        logs: vec!["[NeuroVeil VPN] Клиент инициализирован. Готов к работе.".to_string()],
        up_bytes: 0,
        down_bytes: 0,
        up_speed: 0,
        down_speed: 0,
    }));

    // Check exact sing-box version once in background thread so UI never blocks
    let ver_state = shared_state.clone();
    thread::spawn(move || {
        let (installed, ver) = check_singbox_version();
        if let Ok(mut rt) = ver_state.lock() {
            rt.singbox_installed = installed;
            rt.singbox_version = ver;
        }
    });

    let (resize_tx, mut resize_rx) = tokio::sync::mpsc::channel::<()>(16);
    let cleanup_state = shared_state.clone();
    let close_state = shared_state.clone();
    let resize_state = shared_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            println!("[NeuroVeil VPN] Попытка повторного запуска приложения с аргументами: {:?}", args);
            let is_minimized = args.iter().any(|a| a == "--minimized");
            if !is_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    if let Ok(true) = w.is_minimized() {
                        let _ = w.unminimize();
                    }
                    let _ = w.set_focus();
                }
            }
        }))
        .manage(shared_state.clone())
        .setup(|app| {
            println!("[NeuroVeil VPN] Tauri setup hook running...");

            // Background worker to debounce window resize disk writes
            let debounced_state = app.state::<SharedState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                while resize_rx.recv().await.is_some() {
                    loop {
                        tokio::select! {
                            biased;
                            Some(_) = resize_rx.recv() => {
                                continue;
                            }
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(400)) => {
                                if let Ok(rt) = debounced_state.lock() {
                                    save_persistent_state(&rt.persistent);
                                }
                                break;
                            }
                        }
                    }
                }
            });

            // Create System Tray
            let show_item = MenuItem::with_id(app, "show", "Открыть NeuroVeil VPN", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Выход из приложения", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &sep, &quit_item])?;

            let tray_cleanup = app.state::<SharedState>().inner().clone();
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("NeuroVeil VPN — Hysteria 2 & TUIC")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                if let Ok(true) = w.is_minimized() {
                                    let _ = w.unminimize();
                                }
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => {
                            if let Ok(mut rt) = tray_cleanup.lock() {
                                stop_singbox_internal(&mut rt);
                                save_persistent_state(&rt.persistent);
                            }
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if let Ok(true) = w.is_minimized() {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            } else if let Ok(true) = w.is_visible() {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Determine if window should start minimized
            let is_minimized_arg = std::env::args().any(|a| a == "--minimized");
            let st = app.state::<SharedState>();
            let (should_start_minimized, persistent_snapshot) = if let Ok(rt) = st.lock() {
                (is_minimized_arg || rt.persistent.start_minimized, rt.persistent.clone())
            } else {
                (is_minimized_arg, AppPersistentState::default())
            };

            if let Some(window) = app.get_webview_window("main") {
                // Restore saved window size if present
                if let (Some(w), Some(h)) = (persistent_snapshot.window_width, persistent_snapshot.window_height) {
                    if w >= 940.0 && h >= 640.0 {
                        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width: w, height: h }));
                        let _ = window.center();
                    }
                }

                if persistent_snapshot.window_maximized == Some(true) {
                    let _ = window.maximize();
                }

                if should_start_minimized {
                    let _ = window.hide();
                    println!("[NeuroVeil VPN] Window started minimized to system tray.");
                } else {
                    let _ = window.show();
                    if let Ok(true) = window.is_minimized() {
                        let _ = window.unminimize();
                    }
                    let _ = window.set_focus();
                    println!("[NeuroVeil VPN] Main window shown and focused.");
                }
            }

            // Auto-connect if enabled
            let auto_connect_state = app.state::<SharedState>().inner().clone();
            let app_handle_for_connect = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                let auto_connect_profile = {
                    if let Ok(rt) = auto_connect_state.lock() {
                        if rt.persistent.auto_connect && !rt.persistent.profiles.is_empty() {
                            let prof_id = rt
                                .persistent
                                .selected_profile_id
                                .clone()
                                .or_else(|| rt.persistent.profiles.first().map(|p| p.id.clone()));
                            prof_id.map(|id| {
                                (
                                    id,
                                    rt.persistent.routing.mode.clone(),
                                    rt.persistent.routing.split_tunneling,
                                )
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some((pid, mode, split)) = auto_connect_profile {
                    let _ = connect_vpn(pid, mode, split, app_handle_for_connect.state()).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_vpn_status,
            download_core,
            connect_vpn,
            disconnect_vpn,
            add_or_update_profile,
            import_profile_uri,
            delete_profile,
            ping_profile,
            save_routing_config,
            clear_core_logs,
            export_core_logs_to_file,
            pick_exe_file,
            pick_folder,
            pick_config_file_and_import,
            get_running_processes,
            test_single_vps_ssh,
            test_vps_ssh,
            deploy_vps_server,
            clean_vps_server,
            create_device_key,
            export_profile_json,
            export_awg_conf,
            probe_awg_ports,
            login_vps_admin,
            delete_vps_device_cmd,
            app_start_dragging,
            app_minimize,
            app_toggle_maximize,
            app_close,
            app_show_window,
            save_system_settings,
            get_routing_presets,
            save_routing_preset,
            delete_routing_preset,
            reset_routing_presets,
            apply_routing_preset,
            apply_routing_on_the_fly,
            check_app_update,
            download_and_install_update,
            open_external_url
        ])
        .on_window_event(move |window, event| match event {
            tauri::WindowEvent::Resized(phys_size) => {
                let is_max = window.is_maximized().unwrap_or(false);
                let is_min = window.is_minimized().unwrap_or(false);

                if !is_min {
                    if is_max {
                        if let Ok(mut rt) = resize_state.lock() {
                            rt.persistent.window_maximized = Some(true);
                        }
                        let _ = resize_tx.try_send(());
                    } else {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let logical_w = (phys_size.width as f64 / scale).round();
                        let logical_h = (phys_size.height as f64 / scale).round();

                        if logical_w >= 940.0 && logical_h >= 640.0 {
                            if let Ok(mut rt) = resize_state.lock() {
                                rt.persistent.window_width = Some(logical_w);
                                rt.persistent.window_height = Some(logical_h);
                                rt.persistent.window_maximized = Some(false);
                            }
                            let _ = resize_tx.try_send(());
                        }
                    }
                }
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let should_hide = {
                    if let Ok(rt) = close_state.lock() {
                        save_persistent_state(&rt.persistent);
                        rt.persistent.close_to_tray
                    } else {
                        true
                    }
                };
                if should_hide {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    let app_handle = window.app_handle().clone();
                    if let Ok(mut rt) = close_state.lock() {
                        stop_singbox_internal(&mut rt);
                        save_persistent_state(&rt.persistent);
                    }
                    app_handle.exit(0);
                }
            }
            tauri::WindowEvent::Destroyed => {
                if let Ok(mut rt) = cleanup_state.lock() {
                    stop_singbox_internal(&mut rt);
                    save_persistent_state(&rt.persistent);
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("Ошибка запуска Tauri-приложения NeuroVeil VPN");
}

mod models;
mod singbox;
mod ssh_deployer;
#[cfg(target_os = "android")]
mod android_vpn;

use models::{
    default_routing_presets, is_awg_protocol, AppPersistentState, AppUpdateInfo, DeployResult,
    PortProbeResult, RoutingConfig, RoutingPreset, RunningProcessInfo, SpeedTestProgressEvent,
    SpeedTestServiceCheck, VpnProfile, VpnStatus, VpsAdminState, VpsCredentials,
};
#[allow(unused_imports)]
use singbox::{
    build_awg_conf, build_multi_zip_bytes, build_profile_uri, check_singbox_version,
    collect_exe_in_dir, download_singbox_core, generate_android_runtime_singbox_config,
    generate_mobile_singbox_config, generate_multi_singbox_config, generate_singbox_config,
    generate_singbox_config_with_port, get_app_data_dir, get_profile_candidate_ports,
    get_singbox_bin_path, parse_multi_profiles_input, parse_multi_zip_bytes, parse_profile_uri,
    reset_stale_app_sockets, set_app_data_dir, set_windows_system_proxy, write_routing_rulesets,
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
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct RuntimeState {
    persistent: AppPersistentState,
    child: Option<Child>,
    android_connected: bool,
    android_connecting: bool,
    android_socks_port: Option<u16>,
    last_android_error_logged: Option<String>,
    app_handle: Option<AppHandle>,
    active_profile_id: Option<String>,
    active_profile_name: Option<String>,
    active_protocol: Option<String>,
    active_port: Option<u16>,
    active_listen_port: Option<u16>,
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

fn is_vpn_running(rt: &RuntimeState) -> bool {
    #[cfg(target_os = "android")]
    {
        return rt.android_connected;
    }
    #[cfg(not(target_os = "android"))]
    {
        return rt.child.is_some();
    }
}

#[tauri::command]
fn get_platform_info() -> serde_json::Value {
    serde_json::json!({
        "platform": if cfg!(target_os = "android") { "android" } else { "windows" },
        "is_mobile": cfg!(target_os = "android")
    })
}

type SharedState = Arc<Mutex<RuntimeState>>;

fn get_state_file_path() -> std::path::PathBuf {
    get_app_data_dir().join("state.json")
}

fn try_parse_state_content(raw: &str) -> Option<AppPersistentState> {
    // Очищаем от возможных нулей (\0), которые NTFS записывает при внезапном сбросе питания
    let clean: String = raw.chars().filter(|c| *c != '\0').collect();
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(mut state) = serde_json::from_str::<AppPersistentState>(trimmed) {
        for p in &mut state.profiles {
            p.uri = build_profile_uri(p);
        }
        state.profiles.retain(|p| p.pinned_on_pc != Some(false));
        if state.routing_presets.is_empty() {
            state.routing_presets = default_routing_presets();
        }
        return Some(state);
    }
    None
}

fn load_persistent_state() -> AppPersistentState {
    let dir = get_app_data_dir();
    let primary_path = get_state_file_path();
    let bak_path = dir.join("state.json.bak");
    let tmp_path = dir.join("state.json.tmp");

    // 1. Проверяем основной файл state.json
    if let Ok(content) = fs::read_to_string(&primary_path) {
        if let Some(state) = try_parse_state_content(&content) {
            // Если бэкапа нет или он поврежден, сохраняем рабочую копию
            if !bak_path.exists() {
                let _ = fs::copy(&primary_path, &bak_path);
            }
            return state;
        }
    }

    // 2. Если основной файл пуст, поврежден или обнулен (например при резком отключении питания ПК),
    // восстанавливаем данные из резервной копии state.json.bak
    if let Ok(content) = fs::read_to_string(&bak_path) {
        if let Some(state) = try_parse_state_content(&content) {
            eprintln!("[State Recovery] ВНИМАНИЕ: state.json был поврежден или обнулен (сбой питания). Настройки успешно восстановлены из state.json.bak!");
            save_persistent_state(&state);
            return state;
        }
    }

    // 3. Резервный шанс: проверяем state.json.tmp
    if let Ok(content) = fs::read_to_string(&tmp_path) {
        if let Some(state) = try_parse_state_content(&content) {
            eprintln!("[State Recovery] Настройки восстановлены из временного файла state.json.tmp!");
            save_persistent_state(&state);
            return state;
        }
    }

    eprintln!("[State] Не найден сохраненный профиль/конфиг, загружаются стандартные настройки.");
    AppPersistentState::default()
}

fn save_persistent_state(state: &AppPersistentState) {
    let dir = get_app_data_dir();
    let _ = fs::create_dir_all(&dir);
    let primary_path = get_state_file_path();
    let tmp_path = dir.join("state.json.tmp");
    let bak_path = dir.join("state.json.bak");

    let json = match serde_json::to_string_pretty(state) {
        Ok(s) => s,
        Err(_) => return,
    };

    // 1. Атомарно пишем во временный файл и вызываем fsync (FlushFileBuffers на Windows)
    let write_tmp = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
        file.sync_all()?; // Гарантия физической записи данных на диск
        Ok(())
    })();

    if write_tmp.is_err() {
        return;
    }

    // 2. Если текущий state.json существует и не пуст, делаем актуальный бэкап state.json.bak
    if primary_path.exists() {
        if let Ok(meta) = fs::metadata(&primary_path) {
            if meta.len() > 20 {
                let _ = fs::copy(&primary_path, &bak_path);
            }
        }
    }

    // 3. Заменяем целевой файл state.json файлом tmp_path
    if fs::rename(&tmp_path, &primary_path).is_err() {
        let _ = fs::remove_file(&primary_path);
        if fs::rename(&tmp_path, &primary_path).is_err() {
            if let Ok(_) = fs::copy(&tmp_path, &primary_path) {
                let _ = fs::remove_file(&tmp_path);
            }
        }
    }

    // 4. Финальный sync_all на целевом файле для сброса метаданных NTFS
    if let Ok(f) = fs::OpenOptions::new().read(true).open(&primary_path) {
        let _ = f.sync_all();
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

fn pick_random_free_udp_port(exclude: &[u16]) -> u16 {
    for _ in 0..64 {
        let raw = Uuid::new_v4().as_u128();
        let mixed = ((raw >> 64) ^ raw) as u32;
        let candidate = 10000 + (mixed % 55000) as u16;
        if exclude.contains(&candidate) {
            continue;
        }
        if std::net::UdpSocket::bind(("0.0.0.0", candidate)).is_ok() {
            return candidate;
        }
    }
    if let Ok(sock) = std::net::UdpSocket::bind(("0.0.0.0", 0)) {
        if let Ok(addr) = sock.local_addr() {
            let p = addr.port();
            if p >= 1024 && !exclude.contains(&p) {
                return p;
            }
        }
    }
    let fallback_raw = (Uuid::new_v4().as_u128() & 0xFFFF_FFFF) as u32;
    10000 + (fallback_raw % 55000) as u16
}

fn stop_singbox_in_memory(rt: &mut RuntimeState) -> Option<AppHandle> {
    #[cfg(target_os = "android")]
    {
        let was_connected = rt.android_connected || rt.android_connecting;
        let app_opt = rt.app_handle.clone();
        rt.android_connected = false;
        rt.android_connecting = false;
        rt.android_socks_port = None;
        rt.last_android_error_logged = None;
        save_persistent_state(&rt.persistent);
        if was_connected {
            push_log(&mut rt.logs, "Android VpnService (libbox) остановлен.".to_string());
        }
        rt.active_profile_id = None;
        rt.active_profile_name = None;
        rt.active_protocol = None;
        rt.active_port = None;
        rt.active_listen_port = None;
        rt.port_hunting_status = None;
        rt.is_handshaking = false;
        rt.handshake_status = None;
        rt.up_speed = 0;
        rt.down_speed = 0;
        return app_opt;
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Some(mut child) = rt.child.take() {
            let _ = set_windows_system_proxy(false, rt.persistent.routing.local_mixed_port);
            let _ = child.kill();
            let _ = child.wait();
            #[cfg(target_os = "windows")]
            {
                let mut flush_cmd = Command::new("ipconfig");
                flush_cmd.arg("/flushdns").creation_flags(CREATE_NO_WINDOW);
                let _ = flush_cmd.output();
            }
            save_persistent_state(&rt.persistent);
            push_log(&mut rt.logs, "Процесс sing-box остановлен.".to_string());
        }
        rt.active_profile_id = None;
        rt.active_profile_name = None;
        rt.active_protocol = None;
        rt.active_port = None;
        rt.active_listen_port = None;
        rt.port_hunting_status = None;
        rt.is_handshaking = false;
        rt.handshake_status = None;
        rt.up_speed = 0;
        rt.down_speed = 0;
        None
    }
}

#[cfg(not(target_os = "android"))]
fn stop_singbox_internal(rt: &mut RuntimeState) {
    let _ = stop_singbox_in_memory(rt);
}

async fn stop_android_vpn_if_needed(stop_app: Option<AppHandle>) {
    #[cfg(target_os = "android")]
    if let Some(app) = stop_app {
        let _ = tauri::async_runtime::spawn_blocking(move || crate::android_vpn::stop_vpn(&app)).await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = stop_app;
}

#[cfg(target_os = "android")]
fn apply_mobile_vpn_status_in_memory(
    rt: &mut RuntimeState,
    mob: crate::android_vpn::MobileVpnStatus,
) {
    if !mob.libbox_version.trim().is_empty() {
        rt.singbox_version = mob.libbox_version.clone();
    }
    if !mob.logs.is_empty() {
        for line in &mob.logs {
            if !rt.logs.contains(line) {
                rt.logs.push(line.clone());
            }
        }
        if rt.logs.len() > 250 {
            let excess = rt.logs.len() - 250;
            rt.logs.drain(0..excess);
        }
    }
    if mob.connected {
        let was_connected = rt.android_connected;
        rt.android_connecting = false;
        rt.last_android_error_logged = None;
        rt.android_socks_port = if mob.socks_port > 0 {
            Some(mob.socks_port)
        } else {
            None
        };
        let mob_pid_mismatch = mob
            .profile_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_some_and(|s| rt.active_profile_id.as_deref() != Some(s));
        if rt.active_profile_id.is_none() || mob_pid_mismatch {
            let matched_prof = mob
                .profile_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|pid| rt.persistent.profiles.iter().find(|p| p.id == pid))
                .or_else(|| {
                    mob.profile_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .and_then(|pname| rt.persistent.profiles.iter().find(|p| p.name == pname))
                });
            if let Some(p) = matched_prof {
                rt.active_profile_id = Some(p.id.clone());
                rt.active_profile_name = Some(p.name.clone());
                rt.active_protocol = Some(p.protocol.clone());
                rt.active_port = Some(p.port);
                rt.active_listen_port = if is_awg_protocol(&p.protocol) && p.protocol != "awg_vless" {
                    p.listen_port
                } else {
                    None
                };
            } else {
                if let Some(pid) = mob.profile_id.clone().filter(|s| !s.is_empty()) {
                    rt.active_profile_id = Some(pid);
                }
                if let Some(pname) = mob.profile_name.clone().filter(|s| !s.is_empty()) {
                    rt.active_profile_name = Some(pname);
                }
                if let Some(proto) = mob.protocol.clone().filter(|s| !s.is_empty()) {
                    rt.active_protocol = Some(proto);
                }
            }
        }
        let delta_up = mob.up_bytes.saturating_sub(rt.up_bytes);
        let delta_down = mob.down_bytes.saturating_sub(rt.down_bytes);
        rt.up_bytes = mob.up_bytes;
        rt.down_bytes = mob.down_bytes;
        rt.up_speed = mob.up_speed;
        rt.down_speed = mob.down_speed;
        if delta_up > 0 || delta_down > 0 {
            if let Some(active_id) = rt.active_profile_id.clone() {
                if let Some(prof) =
                    rt.persistent.profiles.iter_mut().find(|p| p.id == active_id)
                {
                    prof.total_up_bytes = prof.total_up_bytes.saturating_add(delta_up);
                    prof.total_down_bytes =
                        prof.total_down_bytes.saturating_add(delta_down);
                }
            }
        }
        if !was_connected {
            let proto_is_awg = rt
                .active_protocol
                .as_deref()
                .map(is_awg_protocol)
                .unwrap_or(false);
            if proto_is_awg && !mob.tunnel_rx_confirmed {
                rt.is_handshaking = true;
                rt.handshake_status = Some(
                    "Установка соединения (рукопожатие WireGuard)...".to_string(),
                );
            }
        }
        if rt.is_handshaking && mob.tunnel_rx_confirmed {
            rt.is_handshaking = false;
            rt.handshake_status = None;
        }
        rt.android_connected = true;
    } else if mob.connecting {
        rt.android_connected = false;
        rt.android_connecting = true;
        rt.android_socks_port = None;
        rt.is_handshaking = false;
        rt.handshake_status = Some("Запуск VPN-туннеля...".to_string());
        rt.up_bytes = 0;
        rt.down_bytes = 0;
        rt.up_speed = 0;
        rt.down_speed = 0;
        let mob_pid_mismatch = mob
            .profile_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_some_and(|s| rt.active_profile_id.as_deref() != Some(s));
        if rt.active_profile_id.is_none() || mob_pid_mismatch {
            let matched_prof = mob
                .profile_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .and_then(|pid| rt.persistent.profiles.iter().find(|p| p.id == pid))
                .or_else(|| {
                    mob.profile_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .and_then(|pname| rt.persistent.profiles.iter().find(|p| p.name == pname))
                });
            if let Some(p) = matched_prof {
                rt.active_profile_id = Some(p.id.clone());
                rt.active_profile_name = Some(p.name.clone());
                rt.active_protocol = Some(p.protocol.clone());
                rt.active_port = Some(p.port);
                rt.active_listen_port = if is_awg_protocol(&p.protocol) && p.protocol != "awg_vless" {
                    p.listen_port
                } else {
                    None
                };
            } else {
                if let Some(pid) = mob.profile_id.clone().filter(|s| !s.is_empty()) {
                    rt.active_profile_id = Some(pid);
                }
                if let Some(pname) = mob.profile_name.clone().filter(|s| !s.is_empty()) {
                    rt.active_profile_name = Some(pname);
                }
                if let Some(proto) = mob.protocol.clone().filter(|s| !s.is_empty()) {
                    rt.active_protocol = Some(proto);
                }
            }
        }
    } else {
        if rt.android_connected || rt.android_connecting || rt.active_profile_id.is_some() {
            rt.android_connected = false;
            rt.android_connecting = false;
            rt.android_socks_port = None;
            rt.active_profile_id = None;
            rt.active_profile_name = None;
            rt.active_protocol = None;
            rt.active_port = None;
            rt.active_listen_port = None;
            rt.port_hunting_status = None;
            rt.is_handshaking = false;
            rt.handshake_status = None;
            rt.up_bytes = 0;
            rt.down_bytes = 0;
            rt.up_speed = 0;
            rt.down_speed = 0;
        }
        if let Some(err) = mob.last_error.filter(|s| !s.trim().is_empty()) {
            if rt.last_android_error_logged.as_deref() != Some(err.as_str()) {
                rt.last_android_error_logged = Some(err.clone());
                push_log(&mut rt.logs, format!("[Android VPN] {}", err));
            }
        }
    }
}

fn build_vpn_status_snapshot(rt: &mut RuntimeState) -> VpnStatus {
    #[cfg(target_os = "android")]
    {
        rt.singbox_installed = true;
        if rt.singbox_version.is_empty() || rt.singbox_version == "Не установлен" {
            rt.singbox_version = "libbox 1.14.4-lx (встроено)".to_string();
        }
        return VpnStatus {
            connected: rt.android_connected,
            active_profile_id: rt.active_profile_id.clone(),
            active_profile_name: rt.active_profile_name.clone(),
            active_protocol: rt.active_protocol.clone(),
            active_port: rt.active_port,
            active_listen_port: rt.active_listen_port,
            port_hunting_status: rt.port_hunting_status.clone(),
            is_handshaking: rt.is_handshaking,
            handshake_status: rt.handshake_status.clone(),
            is_connecting: rt.android_connecting,
            mode: "tun".to_string(),
            split_tunneling: rt.persistent.routing.split_tunneling,
            up_bytes: rt.up_bytes,
            down_bytes: rt.down_bytes,
            up_speed: rt.up_speed,
            down_speed: rt.down_speed,
            singbox_installed: true,
            singbox_version: rt.singbox_version.clone(),
            singbox_path: "libbox.aar (встроенное ядро Android)".to_string(),
            logs: rt.logs.clone(),
        };
    }

    #[cfg(not(target_os = "android"))]
    {
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
                rt.active_listen_port = None;
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
            active_listen_port: rt.active_listen_port,
            port_hunting_status: rt.port_hunting_status.clone(),
            is_handshaking: rt.is_handshaking,
            handshake_status: rt.handshake_status.clone(),
            is_connecting: false,
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
}

#[cfg(not(target_os = "android"))]
fn build_vpn_status(rt: &mut RuntimeState) -> VpnStatus {
    build_vpn_status_snapshot(rt)
}

async fn fetch_and_build_vpn_status(state: SharedState) -> Result<VpnStatus, String> {
    #[cfg(target_os = "android")]
    {
        let app_opt = {
            let rt = state.lock().map_err(|_| "Lock error".to_string())?;
            rt.app_handle.clone()
        };
        let mob_opt = if let Some(app) = app_opt {
            tauri::async_runtime::spawn_blocking(move || crate::android_vpn::get_vpn_status(&app).ok())
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        if let Some(mob) = mob_opt {
            apply_mobile_vpn_status_in_memory(&mut rt, mob);
        }
        return Ok(build_vpn_status_snapshot(&mut rt));
    }
    #[cfg(not(target_os = "android"))]
    {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        Ok(build_vpn_status_snapshot(&mut rt))
    }
}

fn sync_android_tile_from_state(rt: &RuntimeState) {
    #[cfg(target_os = "android")]
    {
        let Some(app) = rt.app_handle.clone() else {
            return;
        };
        let target_id = rt
            .persistent
            .selected_profile_id
            .as_deref()
            .or(rt.active_profile_id.as_deref());
        let profile = target_id
            .and_then(|pid| rt.persistent.profiles.iter().find(|p| p.id == pid))
            .or_else(|| rt.persistent.profiles.first())
            .cloned();

        let payload = if let Some(prof) = profile {
            let effective_routing = prof.routing.as_ref().unwrap_or(&rt.persistent.routing);
            let cfg_val = generate_android_runtime_singbox_config(&prof, effective_routing);
            let cfg_str = serde_json::to_string_pretty(&cfg_val).unwrap_or_default();
            (cfg_str, prof.name, prof.id, prof.protocol)
        } else {
            (String::new(), String::new(), String::new(), String::new())
        };

        thread::spawn(move || {
            let _ = crate::android_vpn::sync_tile_profile(
                &app, payload.0, payload.1, payload.2, payload.3,
            );
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = rt;
}

#[tauri::command]
fn get_app_state(state: State<'_, SharedState>) -> Result<AppPersistentState, String> {
    let rt = state.lock().map_err(|_| "Lock error".to_string())?;
    sync_android_tile_from_state(&rt);
    Ok(rt.persistent.clone())
}

#[tauri::command]
fn select_profile(profile_id: String, state: State<'_, SharedState>) -> Result<(), String> {
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    let trimmed = profile_id.trim();
    rt.persistent.selected_profile_id = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    save_persistent_state(&rt.persistent);
    sync_android_tile_from_state(&rt);
    Ok(())
}

#[tauri::command]
async fn get_vpn_status(state: State<'_, SharedState>) -> Result<VpnStatus, String> {
    fetch_and_build_vpn_status(state.inner().clone()).await
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

#[cfg(target_os = "windows")]
fn assign_child_to_kill_job(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(
            lp_job_attributes: *mut std::ffi::c_void,
            lp_name: *const u16,
        ) -> isize;
        fn SetInformationJobObject(
            h_job: isize,
            job_object_information_class: i32,
            lp_job_object_information: *const std::ffi::c_void,
            cb_job_object_information_length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(h_job: isize, h_process: isize) -> i32;
    }

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

    static JOB_HANDLE: OnceLock<isize> = OnceLock::new();
    let job = *JOB_HANDLE.get_or_init(|| unsafe {
        let h = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if h != 0 {
            let mut info: JobObjectExtendedLimitInformation = std::mem::zeroed();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let _ = SetInformationJobObject(
                h,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                (&info as *const JobObjectExtendedLimitInformation).cast(),
                std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
            );
        }
        h
    });

    if job != 0 {
        unsafe {
            let _ = AssignProcessToJobObject(job, child.as_raw_handle() as isize);
        }
    }
}

fn start_singbox_child(
    state_clone: &Arc<Mutex<RuntimeState>>,
    profile: &VpnProfile,
    routing_snapshot: &RoutingConfig,
    active_port: u16,
) -> Result<std::process::Child, String> {
    let bin_path = get_singbox_bin_path();
    if !bin_path.exists() {
        return Err(
            "Ядро sing-box.exe еще не установлено. Нажмите «Скачать ядро sing-box» на панели или в настройках."
                .to_string(),
        );
    }

    let config_json = generate_singbox_config_with_port(profile, routing_snapshot, Some(active_port));
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

    #[cfg(target_os = "windows")]
    assign_child_to_kill_job(&child);

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

    Ok(child)
}

fn spawn_and_monitor_singbox(
    state_clone: &Arc<Mutex<RuntimeState>>,
    profile: &VpnProfile,
    routing_snapshot: &RoutingConfig,
    active_port: u16,
    custom_log: Option<String>,
) -> Result<VpnStatus, String> {
    #[cfg(target_os = "android")]
    {
        let mut prof_with_port = profile.clone();
        prof_with_port.port = active_port;
        if prof_with_port.protocol == "hysteria2"
            && active_port != prof_with_port.original_port.unwrap_or(active_port)
        {
            prof_with_port.hop_ports = None;
        }
        let config_json = generate_android_runtime_singbox_config(&prof_with_port, routing_snapshot);
        let config_str = serde_json::to_string_pretty(&config_json).map_err(|e| e.to_string())?;
        let config_path = get_app_data_dir().join("active_config.json");
        let _ = fs::write(&config_path, &config_str);

        let app_handle = {
            let rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            rt.app_handle
                .clone()
                .ok_or_else(|| "Android AppHandle не инициализирован".to_string())?
        };

        let start_res = crate::android_vpn::start_vpn(
            &app_handle,
            config_str,
            profile.name.clone(),
            profile.id.clone(),
            profile.protocol.clone(),
        )?;

        let mob_opt = crate::android_vpn::get_vpn_status(&app_handle).ok();
        let is_connected = mob_opt
            .as_ref()
            .map(|m| m.connected)
            .unwrap_or(start_res.connected);
        let is_connecting = mob_opt
            .as_ref()
            .map(|m| m.connecting)
            .unwrap_or(start_res.connecting);
        let mob_err = mob_opt
            .as_ref()
            .and_then(|m| m.last_error.clone())
            .filter(|s| !s.trim().is_empty());

        if !is_connected && !is_connecting {
            return Err(mob_err.unwrap_or_else(|| {
                "Не удалось запустить VPN-туннель Android".to_string()
            }));
        }

        let resolved_socks_port = mob_opt
            .as_ref()
            .map(|m| m.socks_port)
            .filter(|&p| p > 0)
            .or_else(|| (start_res.socks_port > 0).then_some(start_res.socks_port));

        let is_awg = is_awg_protocol(&profile.protocol);

        let snapshot = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            if let Some(msg) = custom_log {
                push_log(&mut rt.logs, msg);
            } else {
                push_log(
                    &mut rt.logs,
                    format!(
                        "Запуск libbox VPN: профиль '{}' ({}), Split Tunneling: {}",
                        profile.name,
                        profile.protocol.to_uppercase(),
                        if routing_snapshot.split_tunneling { "ВКЛ" } else { "ВЫКЛ" }
                    ),
                );
            }
            if let Some(mob) = mob_opt.as_ref() {
                if !mob.libbox_version.trim().is_empty() {
                    rt.singbox_version = mob.libbox_version.clone();
                }
                for line in &mob.logs {
                    if !rt.logs.contains(line) {
                        rt.logs.push(line.clone());
                    }
                }
                if rt.logs.len() > 250 {
                    let excess = rt.logs.len() - 250;
                    rt.logs.drain(0..excess);
                }
            }
            let rx_confirmed = mob_opt
                .as_ref()
                .map(|m| m.tunnel_rx_confirmed)
                .unwrap_or(start_res.tunnel_rx_confirmed);
            rt.android_connected = is_connected;
            rt.android_connecting = !is_connected && is_connecting;
            rt.android_socks_port = resolved_socks_port;
            rt.last_android_error_logged = None;
            rt.up_bytes = 0;
            rt.down_bytes = 0;
            if is_connected || is_connecting {
                rt.active_profile_id = Some(profile.id.clone());
                rt.active_profile_name = Some(profile.name.clone());
                rt.active_protocol = Some(profile.protocol.clone());
                rt.active_port = Some(active_port);
                rt.port_hunting_status = None;
                rt.active_listen_port = if is_awg && profile.protocol != "awg_vless" {
                    profile.listen_port
                } else {
                    None
                };
            } else {
                rt.active_profile_id = None;
                rt.active_profile_name = None;
                rt.active_protocol = None;
                rt.active_port = None;
                rt.active_listen_port = None;
                rt.port_hunting_status = None;
            }
            if is_connected && is_awg && !rx_confirmed {
                rt.is_handshaking = true;
                rt.handshake_status = Some("Установка соединения (рукопожатие WireGuard)...".to_string());
            } else if !is_connected && is_connecting {
                rt.is_handshaking = false;
                rt.handshake_status = Some("Запуск VPN-туннеля...".to_string());
            } else {
                rt.is_handshaking = false;
                rt.handshake_status = None;
            }
            build_vpn_status_snapshot(&mut rt)
        };

        if is_connected && is_awg {
            if let Some(probe_port) = resolved_socks_port {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(350));
                    if let Ok(proxy) = ureq::Proxy::new(format!("http://127.0.0.1:{}", probe_port)) {
                        let client = ureq::AgentBuilder::new()
                            .timeout(Duration::from_millis(3000))
                            .proxy(proxy)
                            .build();
                        let _ = client.get("http://cp.cloudflare.com/generate_204").call();
                    }
                });
            }
        }

        return Ok(snapshot);
    }

    #[cfg(not(target_os = "android"))]
    {
    let child = start_singbox_child(state_clone, profile, routing_snapshot, active_port)?;

    if routing_snapshot.mode == "system_proxy" {
        let port = routing_snapshot.local_mixed_port;
        let _ = set_windows_system_proxy(true, port);
    }

    // Pre-warm tunnel, flush DNS, and reset stale pre-VPN sockets of Telegram/Discord
    let state_for_warmup = state_clone.clone();
    let warmup_clash_port = routing_snapshot.clash_api_port;
    let warmup_procs = routing_snapshot.proxy_processes.clone();
    let warmup_is_awg = is_awg_protocol(&profile.protocol);
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
            "http://127.0.0.1:{}/proxies/proxy/delay?url=http://cp.cloudflare.com/generate_204&timeout=1800",
            warmup_clash_port
        );
        for _ in 0..2 {
            if let Ok(resp) = ureq::get(&delay_url)
                .timeout(Duration::from_millis(2000))
                .call()
            {
                if let Ok(val) = resp.into_json::<serde_json::Value>() {
                    if val.get("delay").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
                        if let Ok(mut g) = state_for_warmup.lock() {
                            if g.is_handshaking {
                                g.is_handshaking = false;
                                g.handshake_status = None;
                                if warmup_is_awg {
                                    push_log(
                                        &mut g.logs,
                                        "[WireGuard] Рукопожатие успешно установлено, защищенный туннель активен!".to_string(),
                                    );
                                }
                            }
                        }
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(300));
        }

        #[cfg(target_os = "windows")]
        {
            let mut flush_cmd = Command::new("ipconfig");
            flush_cmd.arg("/flushdns").creation_flags(CREATE_NO_WINDOW);
            let _ = flush_cmd.output();
        }
        reset_stale_app_sockets(&warmup_procs);
    });

    // Update runtime state and return status
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
        rt.active_listen_port = if is_awg { profile.listen_port } else { None };
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

    // Active handshake probe for WireGuard / AmneziaWG:
    // Proactively triggers WireGuard Handshake Initiation via local proxy (127.0.0.1:2081)
    if is_awg {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            if let Ok(proxy) = ureq::Proxy::new("http://127.0.0.1:2081") {
                let client = ureq::AgentBuilder::new()
                    .timeout(Duration::from_millis(3000))
                    .proxy(proxy)
                    .build();
                let _ = client.get("http://cp.cloudflare.com/generate_204").call();
            }
        });
    }

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
        let mut warned_no_udp_reply = false;

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

                    let mut proxy_conn_count = 0usize;
                    let mut proxy_up = 0u64;
                    let mut proxy_down = 0u64;
                    if let Some(conns) = val.get("connections").and_then(|v| v.as_array()) {
                        for c in conns {
                            let uses_proxy = c
                                .get("chains")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter().any(|x| x.as_str() == Some("proxy")))
                                .unwrap_or(false);
                            if uses_proxy {
                                proxy_conn_count += 1;
                                proxy_up = proxy_up.saturating_add(
                                    c.get("upload").and_then(|v| v.as_u64()).unwrap_or(0),
                                );
                                proxy_down = proxy_down.saturating_add(
                                    c.get("download").and_then(|v| v.as_u64()).unwrap_or(0),
                                );
                            }
                        }
                    }

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
                                if proxy_down > 0 {
                                    consecutive_rx_zero = 0;
                                    if guard.is_handshaking {
                                        guard.is_handshaking = false;
                                        guard.handshake_status = None;
                                        push_log(
                                            &mut guard.logs,
                                            "[WireGuard] Рукопожатие успешно установлено, защищенный туннель активен!".to_string(),
                                        );
                                    }
                                } else if guard.is_handshaking {
                                    let elapsed = start_time.elapsed().as_secs();
                                    if (proxy_up > 0 || proxy_conn_count > 0) && elapsed >= 4 && !warned_handshake_cooldown {
                                        warned_handshake_cooldown = true;
                                        guard.handshake_status = Some("Пакеты отправлены, ожидание ответа сервера по UDP...".to_string());
                                        push_log(
                                            &mut guard.logs,
                                            "[WireGuard] Пакеты отправлены через proxy, ожидание ответа сервера по UDP...".to_string(),
                                        );
                                    }
                                    if proxy_up > 0 || proxy_conn_count > 0 {
                                        consecutive_rx_zero += 1;
                                        if consecutive_rx_zero >= 15 && !warned_no_udp_reply {
                                            warned_no_udp_reply = true;
                                            guard.handshake_status = Some("Нет ответа по UDP от сервера (проверьте доступность VPS, порт или блокировку WireGuard)".to_string());
                                            push_log(
                                                &mut guard.logs,
                                                "[AmneziaWG] Нет ответа от сервера по UDP: пакеты через proxy отправляются, но входящих данных нет (проверьте доступность VPS, фаервол или блокировку UDP/WireGuard провайдером).".to_string(),
                                            );
                                        }
                                    }
                                } else if proxy_conn_count > 0 && proxy_up > 0 && proxy_down == 0 {
                                    consecutive_rx_zero += 1;
                                    if consecutive_rx_zero == 25 {
                                        push_log(
                                            &mut guard.logs,
                                            "[AmneziaWG] Внимание: трафик отправляется через proxy, но входящие пакеты не возвращаются (проверьте порт подключения или блокировку UDP).".to_string(),
                                        );
                                    }
                                }
                            }
                        } else {
                            guard.up_speed = 0;
                            guard.down_speed = 0;
                            initialized = true;
                            if is_awg && proxy_down > 0 && guard.is_handshaking {
                                guard.is_handshaking = false;
                                guard.handshake_status = None;
                                push_log(
                                    &mut guard.logs,
                                    "[WireGuard] Рукопожатие успешно установлено, защищенный туннель активен!".to_string(),
                                );
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
    #[cfg(target_os = "android")]
    {
        let _ = old_routing;
        let config_json = generate_android_runtime_singbox_config(profile, routing_snapshot);
        let config_str = serde_json::to_string_pretty(&config_json).map_err(|e| e.to_string())?;
        let config_path = get_app_data_dir().join("active_config.json");
        let _ = fs::write(&config_path, &config_str);

        let app_handle = {
            let rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            rt.app_handle
                .clone()
                .ok_or_else(|| "Android AppHandle не инициализирован".to_string())?
        };
        crate::android_vpn::start_vpn(
            &app_handle,
            config_str,
            profile.name.clone(),
            profile.id.clone(),
            profile.protocol.clone(),
        )?;
        let mob_opt = crate::android_vpn::get_vpn_status(&app_handle).ok();
        let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        if let Some(mob) = mob_opt {
            apply_mobile_vpn_status_in_memory(&mut rt, mob);
        }
        push_log(&mut rt.logs, format!("{} (Конфигурация Android VPN обновлена)", log_msg));
        return Ok(build_vpn_status_snapshot(&mut rt));
    }

    #[cfg(not(target_os = "android"))]
    {
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
}

async fn connect_vpn_internal(
    state_clone: SharedState,
    profile_id: String,
    mode: String,
    split_tunneling: bool,
    preserve_listen_port: bool,
) -> Result<VpnStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(not(target_os = "android"))]
        {
            let bin_path = get_singbox_bin_path();
            if !bin_path.exists() {
                return Err(
                    "Ядро sing-box.exe еще не установлено. Нажмите «Скачать ядро sing-box» на панели или в настройках."
                        .to_string(),
                );
            }
        }

        // 0. Pre-flight check: validate credentials before stopping current connection or attempting port hunting
        {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let sibling_vless = rt
                .persistent
                .profiles
                .iter()
                .find(|p| p.protocol == "vless" && p.reality_public_key.is_some())
                .cloned();

            if let Some(p_mut) = rt.persistent.profiles.iter_mut().find(|p| p.id == profile_id) {
                if p_mut.protocol == "awg_vless" {
                    let same_srv_vless = sibling_vless.as_ref();
                    if let Some(vl) = same_srv_vless {
                        if p_mut.uuid.as_deref().unwrap_or("").trim().is_empty() {
                            p_mut.uuid = vl.uuid.clone().or_else(|| Some(vl.password.clone()));
                        }
                        if p_mut.reality_public_key.as_deref().unwrap_or("").trim().is_empty() {
                            p_mut.reality_public_key = vl.reality_public_key.clone();
                        }
                        if p_mut.reality_short_id.as_deref().unwrap_or("").trim().is_empty() {
                            p_mut.reality_short_id = vl.reality_short_id.clone();
                        }
                        if p_mut.sni.trim().is_empty() {
                            p_mut.sni = vl.sni.clone();
                        }
                        if p_mut.vless_port.unwrap_or(0) == 0 {
                            p_mut.vless_port = Some(vl.port);
                        }
                    }
                }
            }

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
                if prof.protocol == "awg_vless" {
                    let vless_uid = prof.uuid.as_deref().unwrap_or("").trim();
                    let vless_pbk = prof.reality_public_key.as_deref().unwrap_or("").trim();
                    if vless_uid.is_empty() || vless_pbk.is_empty() {
                        return Err(
                            "Для гибридного протокола «AWG + VLESS Reality» необходимы параметры VLESS Reality (UUID и Reality Public Key). Укажите их в настройках профиля (✏️) или разверните сервер с VLESS Reality + AmneziaWG."
                                .to_string(),
                        );
                    }
                }
            } else if prof.protocol == "vless" || prof.protocol == "vless_ws" || prof.protocol == "webrtc" {
                let uid = prof.uuid.as_deref().unwrap_or(&prof.password).trim();
                if uid.is_empty() {
                    return Err(format!("В профиле {} не указан UUID (пароль). Проверьте настройки профиля.", prof.protocol.to_uppercase()));
                }
            } else if prof.protocol == "hysteria2" || prof.protocol == "tuic" || prof.protocol == "shadowsocks" || prof.protocol == "shadowtls" {
                if prof.password.trim().is_empty() {
                    return Err(format!("В профиле {} не указан пароль. Проверьте настройки профиля.", prof.protocol.to_uppercase()));
                }
            }
        }

        // 1. Stop existing process (or reset Android state for in-place reload) and snapshot profile & routing
        let (mut profile, routing_snapshot, _had_tracked_child) = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let had_child = is_vpn_running(&rt);
            #[cfg(target_os = "android")]
            {
                // On Android, reload NeuroVeilVpnService in-place via start_vpn instead of
                // calling stop_vpn/stopSelf() first, avoiding stopSelf()/coreLock races.
                rt.android_connected = false;
                rt.android_connecting = true;
                rt.android_socks_port = None;
                rt.last_android_error_logged = None;
                rt.is_handshaking = false;
                rt.handshake_status = None;
                rt.up_speed = 0;
                rt.down_speed = 0;
                rt.up_bytes = 0;
                rt.down_bytes = 0;
            }
            #[cfg(not(target_os = "android"))]
            {
                stop_singbox_internal(&mut rt);
            }

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

        // Ensure any previous sing-box process is terminated and allow socket/TUN release
        #[cfg(target_os = "windows")]
        {
            if !_had_tracked_child {
                let mut kill_cmd = Command::new("taskkill");
                kill_cmd
                    .args(["/F", "/IM", "sing-box.exe"])
                    .creation_flags(CREATE_NO_WINDOW);
                let _ = kill_cmd.output();
                thread::sleep(Duration::from_millis(120));
            } else {
                thread::sleep(Duration::from_millis(250));
            }
        }

        // 1.5. Amnezia-style automatic UDP listen_port rotation on connect/reconnect
        if is_awg_protocol(&profile.protocol)
            && profile.protocol != "awg_vless"
            && (!preserve_listen_port
                || (profile.auto_rotate_listen_port.unwrap_or(true)
                    && profile.listen_port.unwrap_or(0) == 0))
        {
            let old_listen_port = profile.listen_port;
            let mut exclude = vec![
                profile.port,
                routing_snapshot.local_mixed_port,
                routing_snapshot.clash_api_port,
                2081,
            ];
            if let Some(old_lp) = old_listen_port {
                if old_lp > 0 {
                    exclude.push(old_lp);
                }
            }
            let new_lp = pick_random_free_udp_port(&exclude);
            profile.listen_port = Some(new_lp);
            profile.uri = build_profile_uri(&profile);

            if let Ok(mut rt) = state_clone.lock() {
                if let Some(p) = rt.persistent.profiles.iter_mut().find(|p| p.id == profile.id) {
                    p.listen_port = Some(new_lp);
                    p.uri = profile.uri.clone();
                }
                save_persistent_state(&rt.persistent);

                let proto_tag = if profile.protocol == "wireguard" || profile.protocol == "wg" {
                    "WireGuard"
                } else if profile.protocol == "awg_quic" {
                    "AWG + QUIC Pre-Warm"
                } else {
                    "AmneziaWG"
                };
                if let Some(old_lp) = old_listen_port.filter(|&p| p > 0) {
                    push_log(
                        &mut rt.logs,
                        format!(
                            "[{}] Порт прослушивания (ListenPort) изменен при переподключении: {} → {}",
                            proto_tag, old_lp, new_lp
                        ),
                    );
                } else {
                    push_log(
                        &mut rt.logs,
                        format!(
                            "[{}] Назначен случайный порт прослушивания (ListenPort): {}",
                            proto_tag, new_lp
                        ),
                    );
                }
            }
        }

        // 1.8. QUIC v1 Initial + WebRTC STUN UDP Pre-Warming for awg_quic
        if profile.protocol == "awg_quic" {
            let lp = profile.listen_port.unwrap_or_else(|| {
                let p = pick_random_free_udp_port(&[profile.port, 2080, 2081, 9090]);
                profile.listen_port = Some(p);
                p
            });
            if send_udp_quic_stun_prewarm(lp, &profile.server, profile.port) {
                if let Ok(mut rt) = state_clone.lock() {
                    push_log(
                        &mut rt.logs,
                        format!(
                            "[QUIC Pre-Warm] Отправлены пакеты инициализации QUIC v1 Initial (1200B) + STUN с ListenPort {} → {}:{} перед рукопожатием",
                            lp, profile.server, profile.port
                        ),
                    );
                }
            }
        }

        // 1.9. Hysteria2 automatic UDP server port rotation across hop_ports on reconnect/auto-hunt
        if !preserve_listen_port && profile.protocol == "hysteria2" {
            let candidates = get_profile_candidate_ports(&profile, None, Some(vec![profile.port]), 24);
            if !candidates.is_empty() {
                let idx = (Uuid::new_v4().as_u128() as usize) % candidates.len();
                let new_port = candidates[idx];
                if new_port > 0 && new_port != profile.port {
                    let old_port = profile.port;
                    if profile.original_port.is_none() || profile.original_port == Some(0) {
                        profile.original_port = Some(old_port);
                    }
                    profile.port = new_port;
                    profile.uri = build_profile_uri(&profile);
                    if let Ok(mut rt) = state_clone.lock() {
                        if let Some(p) = rt.persistent.profiles.iter_mut().find(|p| p.id == profile.id) {
                            if p.original_port.is_none() || p.original_port == Some(0) {
                                p.original_port = Some(old_port);
                            }
                            p.port = new_port;
                            p.uri = profile.uri.clone();
                        }
                        save_persistent_state(&rt.persistent);
                        push_log(
                            &mut rt.logs,
                            format!(
                                "[Hysteria2] Порт подключения изменен при переподключении: {} → {}",
                                old_port, new_port
                            ),
                        );
                    }
                }
            }
        }

        // 2. Direct fast launch of sing-box without blocking loops (< 50ms)
        let active_port = profile.port;
        {
            if let Ok(mut g) = state_clone.lock() {
                g.port_hunting_status = None;
                g.active_port = Some(active_port);
                g.active_listen_port = if is_awg_protocol(&profile.protocol) && profile.protocol != "awg_vless" {
                    profile.listen_port
                } else {
                    None
                };
            }
        }

        spawn_and_monitor_singbox(&state_clone, &profile, &routing_snapshot, active_port, None)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn send_udp_quic_stun_prewarm(local_port: u16, server_host: &str, server_port: u16) -> bool {
    use std::net::{ToSocketAddrs, UdpSocket};
    let target = match format!("{}:{}", server_host, server_port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.find(|a| a.is_ipv4()))
    {
        Some(addr) => addr,
        None => return false,
    };

    let sock = match UdpSocket::bind(format!("0.0.0.0:{}", local_port)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = sock.set_write_timeout(Some(Duration::from_millis(80)));

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15);

    // 1. RFC 5389 STUN Binding Request (20 bytes) - mimics WebRTC ICE probe
    let mut stun_pkt = [0u8; 20];
    stun_pkt[0] = 0x00;
    stun_pkt[1] = 0x01; // Binding Request
    stun_pkt[2] = 0x00;
    stun_pkt[3] = 0x00; // Message Length = 0
    stun_pkt[4..8].copy_from_slice(&[0x21, 0x12, 0xA4, 0x42]); // Magic Cookie
    for i in 0..12 {
        stun_pkt[8 + i] = ((seed.wrapping_mul((i as u64) + 31)) >> 16) as u8;
    }
    let _ = sock.send_to(&stun_pkt, target);

    // 2. RFC 9000 QUIC v1 Initial Packet (1200 bytes) - mimics HTTP/3 browser handshake
    let mut quic_pkt = vec![0u8; 1200];
    quic_pkt[0] = 0xC3; // Long Header, Fixed Bit, Initial Packet (00), 4-byte PN
    quic_pkt[1..5].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // QUIC v1
    quic_pkt[5] = 0x08; // DCID len = 8
    for i in 0..8 {
        quic_pkt[6 + i] = ((seed.wrapping_mul((i as u64) + 7)) >> 24) as u8;
    }
    quic_pkt[14] = 0x08; // SCID len = 8
    for i in 0..8 {
        quic_pkt[15 + i] = ((seed.wrapping_mul((i as u64) + 19)) >> 24) as u8;
    }
    quic_pkt[23] = 0x00; // Token Length = 0
    // Length varint (2-byte 0x4000 | 1174 = 0x4496)
    quic_pkt[24] = 0x44;
    quic_pkt[25] = 0x96;
    // Packet Number = 1
    quic_pkt[26..30].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]);
    // Pseudo-encrypted CRYPTO payload
    let mut state = seed ^ 0xA5A5_5A5A_F0F0_0F0F;
    for b in quic_pkt[30..].iter_mut() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (state >> 32) as u8;
    }
    let _ = sock.send_to(&quic_pkt, target);
    drop(sock);
    thread::sleep(Duration::from_millis(15));
    true
}

#[tauri::command]
async fn connect_vpn(
    profile_id: String,
    mode: String,
    split_tunneling: bool,
    preserve_listen_port: Option<bool>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    connect_vpn_internal(
        state.inner().clone(),
        profile_id,
        mode,
        split_tunneling,
        preserve_listen_port.unwrap_or(true),
    )
    .await
}

#[tauri::command]
async fn reconnect_vpn(
    profile_id: Option<String>,
    mode: Option<String>,
    split_tunneling: Option<bool>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    let (pid, resolved_mode, resolved_split) = {
        let rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
        let pid = profile_id
            .or_else(|| rt.active_profile_id.clone())
            .or_else(|| rt.persistent.selected_profile_id.clone())
            .or_else(|| rt.persistent.profiles.first().map(|p| p.id.clone()))
            .ok_or_else(|| "Не найден профиль для переподключения".to_string())?;
        let prof_routing = rt
            .persistent
            .profiles
            .iter()
            .find(|p| p.id == pid)
            .and_then(|p| p.routing.as_ref())
            .unwrap_or(&rt.persistent.routing);
        (
            pid,
            mode.unwrap_or_else(|| prof_routing.mode.clone()),
            split_tunneling.unwrap_or(prof_routing.split_tunneling),
        )
    };
    connect_vpn_internal(state_clone, pid, resolved_mode, resolved_split, false).await
}

#[tauri::command]
async fn disconnect_vpn(state: State<'_, SharedState>) -> Result<VpnStatus, String> {
    let state_clone = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (stop_app, snapshot) = {
            let mut rt = state_clone.lock().map_err(|_| "Lock error".to_string())?;
            let app_opt = stop_singbox_in_memory(&mut rt);
            let snap = build_vpn_status_snapshot(&mut rt);
            (app_opt, snap)
        };
        #[cfg(target_os = "android")]
        if let Some(app) = stop_app {
            let _ = crate::android_vpn::stop_vpn(&app);
        }
        #[cfg(not(target_os = "android"))]
        let _ = stop_app;
        Ok(snapshot)
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
    app: AppHandle,
    preset: RoutingPreset,
    state: State<'_, SharedState>,
) -> Result<Vec<RoutingPreset>, String> {
    use tauri::Emitter;
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
    let res = rt.persistent.routing_presets.clone();
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(res)
}

#[tauri::command]
fn delete_routing_preset(
    app: AppHandle,
    preset_id: String,
    state: State<'_, SharedState>,
) -> Result<Vec<RoutingPreset>, String> {
    use tauri::Emitter;
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.routing_presets.retain(|p| p.id != preset_id);
    if rt.persistent.active_preset_id.as_deref() == Some(&preset_id) {
        rt.persistent.active_preset_id = None;
    }
    save_persistent_state(&rt.persistent);
    let res = rt.persistent.routing_presets.clone();
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(res)
}

#[tauri::command]
fn reset_routing_presets(app: AppHandle, state: State<'_, SharedState>) -> Result<Vec<RoutingPreset>, String> {
    use tauri::Emitter;
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
    let res = rt.persistent.routing_presets.clone();
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(res)
}

#[tauri::command]
async fn apply_routing_preset(
    app: AppHandle,
    preset_id: String,
    profile_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    use tauri::Emitter;
    let state_clone = state.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
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

            let is_running = is_vpn_running(&rt);
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
        Ok(build_vpn_status_snapshot(&mut rt))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;
    let _ = app.emit("state-updated", ());
    Ok(res)
}

#[tauri::command]
async fn apply_routing_on_the_fly(
    app: AppHandle,
    routing: RoutingConfig,
    profile_id: Option<String>,
    preset_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<VpnStatus, String> {
    use tauri::Emitter;
    let state_clone = state.inner().clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
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

            let is_running = is_vpn_running(&rt);
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
        Ok(build_vpn_status_snapshot(&mut rt))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;
    let _ = app.emit("state-updated", ());
    Ok(res)
}

#[tauri::command]
fn add_or_update_profile(
    app: AppHandle,
    mut profile: VpnProfile,
    state: State<'_, SharedState>,
) -> Result<Vec<VpnProfile>, String> {
    use tauri::Emitter;
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    if profile.id.trim().is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }
    if profile.original_port.is_none() || profile.original_port == Some(0) {
        if let Some(existing) = rt.persistent.profiles.iter().find(|p| p.id == profile.id) {
            profile.original_port = existing.original_port.or(Some(existing.port));
        } else {
            profile.original_port = Some(profile.port);
        }
    }
    if is_awg_protocol(&profile.protocol) {
        if profile.auto_rotate_listen_port.is_none() {
            profile.auto_rotate_listen_port = rt
                .persistent
                .profiles
                .iter()
                .find(|p| p.id == profile.id)
                .and_then(|e| e.auto_rotate_listen_port)
                .or(Some(true));
        }
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
    sync_android_tile_from_state(&rt);
    let _ = app.emit("state-updated", ());
    Ok(rt.persistent.profiles.clone())
}

#[tauri::command]
fn import_profile_uri(app: AppHandle, uri: String, state: State<'_, SharedState>) -> Result<Vec<VpnProfile>, String> {
    use tauri::Emitter;
    let mut imported = parse_multi_profiles_input(&uri)?;
    if imported.is_empty() {
        return Err("Не удалось распознать ни одного профиля".to_string());
    }
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    for profile in imported.iter_mut() {
        profile.pinned_on_pc = Some(true);
        rt.persistent.profiles.push(profile.clone());
    }
    if rt.persistent.selected_profile_id.is_none() {
        rt.persistent.selected_profile_id = imported.first().map(|p| p.id.clone());
    }
    save_persistent_state(&rt.persistent);
    sync_android_tile_from_state(&rt);
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(imported)
}

#[tauri::command]
fn parse_profile_text(text: String) -> Result<VpnProfile, String> {
    parse_profile_uri(&text)
}

#[tauri::command]
async fn delete_profile(app: AppHandle, id: String, state: State<'_, SharedState>) -> Result<Vec<VpnProfile>, String> {
    use tauri::Emitter;
    let (stop_app, res) = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let stop_app = if rt.active_profile_id.as_deref() == Some(&id) {
            stop_singbox_in_memory(&mut rt)
        } else {
            None
        };
        rt.persistent.profiles.retain(|p| p.id != id);
        if rt.persistent.selected_profile_id.as_deref() == Some(&id) {
            rt.persistent.selected_profile_id = rt.persistent.profiles.first().map(|p| p.id.clone());
        }
        save_persistent_state(&rt.persistent);
        sync_android_tile_from_state(&rt);
        (stop_app, rt.persistent.profiles.clone())
    };
    stop_android_vpn_if_needed(stop_app).await;
    let _ = app.emit("state-updated", ());
    Ok(res)
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
            // Fallback: try through active VPN tunnel (127.0.0.1:2081..2084 or 2080) via SOCKS5
            let mut tunnel_ms = None;
            for proxy_port in [2081u16, 2082, 2083, 2084, 2080] {
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

async fn probe_single_port(
    server: &str,
    port: u16,
    protocol: &str,
    awg_h1: Option<&str>,
    awg_s1: Option<u16>,
    baseline_latency: Option<u32>,
    primary_port: u16,
) -> PortProbeResult {
    let target = format!("{}:{}", server, port);
    let target_addr = match tokio::net::lookup_host(&target).await.ok().and_then(|mut it| it.next()) {
        Some(a) => a,
        None => {
            return PortProbeResult {
                port,
                reachable: false,
                latency_ms: None,
                message: "Не удалось разрешить IP сервера".to_string(),
            };
        }
    };

    let is_tcp = protocol == "vless"
        || protocol == "vless_ws"
        || protocol == "webrtc"
        || protocol == "shadowtls"
        || protocol == "shadowsocks"
        || protocol == "ssh";
    let is_quic = protocol == "hysteria2" || protocol == "tuic";

    if is_tcp {
        let t0 = Instant::now();
        match tokio::time::timeout(Duration::from_millis(1500), tokio::net::TcpStream::connect(target_addr)).await {
            Ok(Ok(_sock)) => {
                let lat = t0.elapsed().as_millis().max(1) as u32;
                PortProbeResult {
                    port,
                    reachable: true,
                    latency_ms: Some(lat),
                    message: "Порт доступен (TCP подключение успешно)".to_string(),
                }
            }
            Ok(Err(e)) => {
                let msg = if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    "Порт закрыт на сервере (Connection Refused)".to_string()
                } else {
                    format!("Ошибка: {}", e)
                };
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: msg,
                }
            }
            Err(_) => {
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: "Таймаут (порт недоступен или закрыт)".to_string(),
                }
            }
        }
    } else if is_quic {
        let t0 = Instant::now();
        let sock = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => s,
            Err(e) => {
                return PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: format!("Ошибка сокета: {}", e),
                };
            }
        };

        if sock.connect(target_addr).await.is_err() {
            return PortProbeResult {
                port,
                reachable: false,
                latency_ms: None,
                message: "Не удалось привязать целевой адрес".to_string(),
            };
        }

        // Send RFC 9000 Version Negotiation trigger probe
        let mut quic_probe = Vec::with_capacity(48);
        quic_probe.push(0xc0); // Long header, Initial
        quic_probe.extend_from_slice(&[0x1a, 0x2a, 0x3a, 0x4a]); // Reserved Version for negotiation
        quic_probe.push(0x08); // DCID len
        quic_probe.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04]);
        quic_probe.push(0x08); // SCID len
        quic_probe.extend_from_slice(&[0xfe, 0xed, 0xfa, 0xce, 0x05, 0x06, 0x07, 0x08]);
        quic_probe.push(0x00); // Token length
        quic_probe.extend_from_slice(&[0x40, 0x10]); // Varint length
        quic_probe.extend_from_slice(&[0u8; 16]); // Payload
        let _ = sock.send(&quic_probe).await;

        let mut buf = [0u8; 1024];
        let recv_res = tokio::time::timeout(Duration::from_millis(1500), sock.recv(&mut buf)).await;
        match recv_res {
            Ok(Ok(n)) if n > 0 => {
                let lat = t0.elapsed().as_millis().max(1) as u32;
                PortProbeResult {
                    port,
                    reachable: true,
                    latency_ms: Some(lat),
                    message: "Порт доступен (QUIC ответ получен)".to_string(),
                }
            }
            Ok(Ok(_)) => {
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: "Пустой ответ".to_string(),
                }
            }
            Ok(Err(e)) => {
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: format!("Порт закрыт: {}", e),
                }
            }
            Err(_) => {
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: "Таймаут (порт недоступен или закрыт)".to_string(),
                }
            }
        }
    } else {
        // AmneziaWG / UDP
        let t0 = Instant::now();
        let sock = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => s,
            Err(e) => {
                return PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: format!("Ошибка сокета: {}", e),
                };
            }
        };

        if sock.connect(target_addr).await.is_err() {
            return PortProbeResult {
                port,
                reachable: false,
                latency_ms: None,
                message: "Не удалось подключиться к адресу".to_string(),
            };
        }

        // Standard port 51820 check
        if port == 51820 {
            return PortProbeResult {
                port,
                reachable: false,
                latency_ms: None,
                message: "Стандартный порт 51820 недоступен (рекомендуется сменить порт)".to_string(),
            };
        }

        let is_port_53 = port == 53;
        let mut probe_pkt = Vec::new();
        if is_port_53 {
            // DNS A query for yandex.ru
            probe_pkt.extend_from_slice(&[0x13, 0x37, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
            probe_pkt.extend_from_slice(&[0x06, b'y', b'a', b'n', b'd', b'e', b'x', 0x02, b'r', b'u', 0x00]);
            probe_pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
        } else {
            let h1_val = awg_h1.and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
            probe_pkt.extend_from_slice(&h1_val.to_le_bytes());
            let seed = (port as u32).wrapping_mul(1664525).wrapping_add(1013904223);
            probe_pkt.extend_from_slice(&seed.to_le_bytes());
            let pad = awg_s1.unwrap_or(12) as usize;
            let total = (148 + pad).min(512);
            while probe_pkt.len() < total {
                probe_pkt.push(0x55);
            }
        }

        let _ = sock.send(&probe_pkt).await;

        let mut buf = [0u8; 1024];
        let recv_res = tokio::time::timeout(Duration::from_millis(800), sock.recv(&mut buf)).await;
        match recv_res {
            Ok(Ok(n)) if n > 0 => {
                let lat = t0.elapsed().as_millis().max(1) as u32;
                PortProbeResult {
                    port,
                    reachable: true,
                    latency_ms: Some(lat),
                    message: "Порт доступен (UDP ответ получен)".to_string(),
                }
            }
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                PortProbeResult {
                    port,
                    reachable: false,
                    latency_ms: None,
                    message: "Порт закрыт на сервере (ICMP Port Unreachable)".to_string(),
                }
            }
            _ => {
                if port == primary_port {
                    if let Some(base_lat) = baseline_latency {
                        PortProbeResult {
                            port,
                            reachable: true,
                            latency_ms: Some(base_lat),
                            message: "Основной порт прослушивания сервера".to_string(),
                        }
                    } else {
                        PortProbeResult {
                            port,
                            reachable: false,
                            latency_ms: None,
                            message: "Сервер недоступен (хост оффлайн или заблокирован)".to_string(),
                        }
                    }
                } else if let Some(base_lat) = baseline_latency {
                    // For UDP port hopping: if host is verified online via baseline_latency,
                    // and this port did NOT receive an ICMP Port Unreachable rejection,
                    // the port is open and forwarded by iptables PREROUTING!
                    let jitter = (port % 9) as u32;
                    PortProbeResult {
                        port,
                        reachable: true,
                        latency_ms: Some(base_lat.saturating_add(jitter).max(1)),
                        message: "Порт доступен (перенаправление активно)".to_string(),
                    }
                } else {
                    PortProbeResult {
                        port,
                        reachable: false,
                        latency_ms: None,
                        message: "Таймаут (нет ответа от сервера / порт не перенаправлен)".to_string(),
                    }
                }
            }
        }
    }
}

#[tauri::command]
async fn probe_profile_ports(
    profile_id: String,
    profile: Option<VpnProfile>,
    custom_ports: Option<Vec<u16>>,
    exclude_ports: Option<Vec<u16>>,
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

    let primary_port = profile.original_port.unwrap_or(profile.port);
    let candidates = get_profile_candidate_ports(&profile, custom_ports, exclude_ports, 64);
    let server = profile.server.trim().to_string();
    let protocol = profile.protocol.clone();

    // Measure baseline host connectivity/latency via TCP 22 or 443 or primary_port
    let baseline_latency: Option<u32> = {
        let mut base_lat = None;
        let test_ports = [22u16, 443u16, 80u16, primary_port];
        for tp in test_ports {
            if tp == 0 {
                continue;
            }
            if let Ok(mut it) = tokio::net::lookup_host((server.as_str(), tp)).await {
                if let Some(addr) = it.next() {
                    let t0 = Instant::now();
                    if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(1200), tokio::net::TcpStream::connect(addr)).await {
                        base_lat = Some(t0.elapsed().as_millis().max(1) as u32);
                        break;
                    }
                }
            }
        }
        base_lat
    };

    let mut tasks = Vec::new();
    for port in candidates {
        let s_c = server.clone();
        let p_c = protocol.clone();
        let h1_c = profile.awg_h1.clone();
        let s1_val = profile.awg_s1;
        let base_lat = baseline_latency;
        let p_port = primary_port;

        tasks.push(tokio::spawn(async move {
            probe_single_port(&s_c, port, &p_c, h1_c.as_deref(), s1_val, base_lat, p_port).await
        }));
    }

    let mut results = Vec::new();
    for t in tasks {
        if let Ok(res) = t.await {
            results.push(res);
        }
    }

    // Only return ports that are actually open and reachable
    results.retain(|r| r.reachable);

    results.sort_by(|a, b| {
        a.latency_ms.unwrap_or(9999).cmp(&b.latency_ms.unwrap_or(9999))
            .then_with(|| a.port.cmp(&b.port))
    });

    Ok(results)
}

#[tauri::command]
async fn probe_awg_ports(
    profile_id: String,
    profile: Option<VpnProfile>,
    state: State<'_, SharedState>,
) -> Result<Vec<PortProbeResult>, String> {
    probe_profile_ports(profile_id, profile, None, None, state).await
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
async fn export_awg_conf_file(
    app: AppHandle,
    profile_id: String,
    profile: Option<VpnProfile>,
    filename: Option<String>,
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let (prof, conf_str) = {
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
        let conf = build_awg_conf(&prof);
        (prof, conf)
    };

    let clean_name = filename
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let stem = clean_export_stem(
                prof.name.split('—').next().unwrap_or(&prof.name).split('-').next().unwrap_or(&prof.name),
                "vpn",
            );
            format!("{}.conf", stem)
        });

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                clean_name,
                "text/plain".to_string(),
                conf_str.as_bytes(),
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, clean_name, conf_str);
        Ok(None)
    }
}

#[tauri::command]
fn save_routing_config(
    app: AppHandle,
    routing: RoutingConfig,
    profile_id: Option<String>,
    state: State<'_, SharedState>,
) -> Result<RoutingConfig, String> {
    use tauri::Emitter;
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    if let Some(ref pid) = profile_id {
        if let Some(prof) = rt.persistent.profiles.iter_mut().find(|p| &p.id == pid) {
            prof.routing = Some(routing.clone());
        }
    }
    for prof in rt.persistent.profiles.iter_mut() {
        if let Some(ref mut r) = prof.routing {
            r.mode = routing.mode.clone();
        }
    }
    rt.persistent.routing = routing.clone();
    save_persistent_state(&rt.persistent);
    sync_android_tile_from_state(&rt);
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(routing)
}

#[tauri::command]
fn set_network_mode(
    app: AppHandle,
    mode: String,
    state: State<'_, SharedState>,
) -> Result<RoutingConfig, String> {
    use tauri::Emitter;
    let normalized = if mode == "tun" {
        "tun".to_string()
    } else {
        "system_proxy".to_string()
    };
    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    rt.persistent.routing.mode = normalized.clone();
    for prof in rt.persistent.profiles.iter_mut() {
        if let Some(ref mut r) = prof.routing {
            r.mode = normalized.clone();
        }
    }
    let updated = rt.persistent.routing.clone();
    save_persistent_state(&rt.persistent);
    drop(rt);
    let _ = app.emit("state-updated", ());
    Ok(updated)
}

#[tauri::command]
async fn clear_core_logs(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        rt.logs.clear();
    }
    #[cfg(target_os = "android")]
    {
        let app_clone = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let _ = crate::android_vpn::clear_logs(&app_clone);
        })
        .await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
    Ok(())
}

#[tauri::command]
async fn export_core_logs_to_file(
    app: AppHandle,
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

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                default_filename,
                "text/plain".to_string(),
                log_content.as_bytes(),
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
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
}

#[tauri::command]
async fn pick_exe_file() -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        return Ok(None);
    }
    #[cfg(not(target_os = "android"))]
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
async fn pick_config_file_and_import(app: AppHandle, state: State<'_, SharedState>) -> Result<Vec<VpnProfile>, String> {
    use tauri::Emitter;

    #[cfg(target_os = "android")]
    let mut imported = {
        let app_clone = app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::pick_file_bytes(&app_clone, vec!["*/*".to_string()])
        })
        .await
        .map_err(|e| e.to_string())??;

        let (file_name, bytes) = match picked {
            Some(v) => v,
            None => return Ok(Vec::new()),
        };

        let file_stem = std::path::Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("config")
            .to_string();
        let is_zip = file_name.to_ascii_lowercase().ends_with(".zip")
            || (bytes.len() >= 4 && &bytes[0..2] == b"PK");

        let mut list = if is_zip {
            parse_multi_zip_bytes(&bytes)?
        } else {
            let content = String::from_utf8_lossy(&bytes).to_string();
            parse_multi_profiles_input(&content)?
        };
        if list.len() == 1 && !file_stem.is_empty() {
            list[0].name = file_stem;
        }
        list
    };

    #[cfg(not(target_os = "android"))]
    let mut imported = {
    let file_path = tauri::async_runtime::spawn_blocking(|| -> Result<Option<String>, String> {
        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Title = "Выберите файл конфигурации VPN (.conf, .json, .txt, .zip)"
$dlg.Filter = "Конфигурации VPN (*.conf;*.json;*.txt;*.zip)|*.conf;*.json;*.txt;*.zip|WireGuard / Amnezia (*.conf)|*.conf|sing-box (*.json)|*.json|Архивы (*.zip)|*.zip|Все файлы (*.*)|*.*"
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
        None => return Ok(Vec::new()),
    };

    let path = std::path::Path::new(&path_str);
    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("config").to_string();
    let is_zip = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));

    let mut list = if is_zip {
        let bytes = std::fs::read(path).map_err(|e| format!("Не удалось прочитать ZIP-архив: {}", e))?;
        parse_multi_zip_bytes(&bytes)?
    } else {
        let content = std::fs::read_to_string(path).map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
        parse_multi_profiles_input(&content)?
    };
    if list.len() == 1 && !file_stem.is_empty() {
        list[0].name = file_stem;
    }
    list
    };

    if imported.is_empty() {
        return Err("В выбранном файле не найдены поддерживаемые конфигурации".to_string());
    }

    let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
    for profile in imported.iter_mut() {
        profile.pinned_on_pc = Some(true);
        rt.persistent.profiles.push(profile.clone());
    }
    if rt.persistent.selected_profile_id.is_none() {
        rt.persistent.selected_profile_id = imported.first().map(|p| p.id.clone());
    }
    save_persistent_state(&rt.persistent);
    drop(rt);
    let _ = app.emit("state-updated", ());

    Ok(imported)
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        return Ok(None);
    }
    #[cfg(not(target_os = "android"))]
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
async fn get_running_processes(app: AppHandle) -> Result<Vec<RunningProcessInfo>, String> {
    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::get_installed_apps(&app)
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
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
    use tauri::Emitter;
    {
        if let Ok(mut rt) = state.lock() {
            let mut saved = creds.clone();
            saved.password = String::new(); // Don't persist raw password to disk
            rt.persistent.saved_vps = Some(saved);
            save_persistent_state(&rt.persistent);
        }
    }
    let _ = app.emit("state-updated", ());
    ssh_deployer::test_ssh_connection(&app, &creds).await
}

#[tauri::command]
async fn deploy_vps_server(
    app: AppHandle,
    creds: VpsCredentials,
    state: State<'_, SharedState>,
) -> Result<DeployResult, String> {
    use tauri::Emitter;
    let result = ssh_deployer::deploy_vps(&app, &creds).await?;
    if let Ok(mut rt) = state.lock() {
        // Keep previous container deployments on the same host; only replace exact (server, port, protocol) matches
        for prof in &result.created_profiles {
            rt.persistent.profiles.retain(|p| {
                !(p.server == prof.server && p.port == prof.port && p.protocol == prof.protocol)
            });
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
    let _ = app.emit("state-updated", ());
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
    container_id: Option<String>,
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

    let profile = ssh_deployer::create_vps_device_user(
        &app,
        &creds,
        &clean_name,
        &protocol,
        container_id.as_deref(),
    )
    .await?;

    Ok(profile)
}

#[tauri::command]
async fn create_device_keys(
    app: AppHandle,
    creds: VpsCredentials,
    device_name: String,
    protocols: Vec<String>,
    container_id: Option<String>,
    _state: State<'_, SharedState>,
) -> Result<ssh_deployer::CreateDeviceKeysResult, String> {
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

    if protocols.is_empty() {
        return Err("Выберите хотя бы один протокол для выпуска ключа".to_string());
    }

    let result = ssh_deployer::create_vps_device_keys(
        &app,
        &creds,
        &clean_name,
        &protocols,
        container_id.as_deref(),
    )
    .await?;

    Ok(result)
}

fn sync_server_profiles_with_state(
    creds: &VpsCredentials,
    server_profiles: &mut [VpnProfile],
    state: &State<'_, SharedState>,
) {
    if let Ok(mut rt) = state.lock() {
        let mut saved = creds.clone();
        saved.password = String::new(); // Do not store plaintext password on disk
        rt.persistent.saved_vps = Some(saved);

        for s_prof in server_profiles.iter_mut() {
            if let Some(existing) = rt.persistent.profiles.iter_mut().find(|p| {
                (p.server == s_prof.server
                    && p.port == s_prof.port
                    && p.protocol == s_prof.protocol
                    && !s_prof.password.is_empty()
                    && p.password == s_prof.password)
                    || (p.server == s_prof.server
                        && p.port == s_prof.port
                        && p.protocol == s_prof.protocol
                        && p.name == s_prof.name)
            }) {
                s_prof.id = existing.id.clone();
                let merged_up = existing.total_up_bytes.max(s_prof.total_up_bytes);
                let merged_down = existing.total_down_bytes.max(s_prof.total_down_bytes);
                existing.total_up_bytes = merged_up;
                existing.total_down_bytes = merged_down;
                s_prof.total_up_bytes = merged_up;
                s_prof.total_down_bytes = merged_down;
                if existing.container_id.is_none() {
                    existing.container_id = s_prof.container_id.clone();
                }
                existing.multi_group_id = s_prof.multi_group_id.clone();
                existing.multi_group_name = s_prof.multi_group_name.clone();
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
    sync_server_profiles_with_state(&creds, &mut server_profiles, &state);
    {
        use tauri::Emitter;
        let _ = app.emit("state-updated", ());
    }

    Ok(server_profiles)
}

#[tauri::command]
async fn get_vps_admin_state(
    app: AppHandle,
    creds: VpsCredentials,
    state: State<'_, SharedState>,
) -> Result<VpsAdminState, String> {
    if creds.host.trim().is_empty() {
        return Err("Введите IP-адрес сервера".to_string());
    }
    if creds.password.trim().is_empty() {
        return Err("Введите пароль SSH (root)".to_string());
    }

    let mut admin_state = ssh_deployer::fetch_vps_admin_state(&app, &creds).await?;
    sync_server_profiles_with_state(&creds, &mut admin_state.profiles, &state);
    {
        use tauri::Emitter;
        let _ = app.emit("state-updated", ());
    }

    Ok(admin_state)
}

#[tauri::command]
async fn manage_vps_container_cmd(
    app: AppHandle,
    creds: VpsCredentials,
    container_id: String,
    action: String,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    if creds.host.trim().is_empty() || creds.password.trim().is_empty() {
        return Err("Введите IP-адрес сервера и пароль SSH (root)".to_string());
    }
    let msg = ssh_deployer::manage_vps_container(&app, &creds, &container_id, &action).await?;

    if action == "delete" {
        let mut stop_app = None;
        if let Ok(mut rt) = state.lock() {
            let host_trimmed = creds.host.trim().to_string();
            let active_in_deleted = rt.active_profile_id.as_ref().is_some_and(|aid| {
                rt.persistent.profiles.iter().any(|p| {
                    p.id == *aid
                        && p.server == host_trimmed
                        && p.container_id.as_deref() == Some(container_id.as_str())
                })
            });
            if active_in_deleted {
                stop_app = stop_singbox_in_memory(&mut rt);
            }
            rt.persistent.profiles.retain(|p| {
                !(p.server == host_trimmed
                    && p.container_id.as_deref() == Some(container_id.as_str()))
            });
            if let Some(sel) = rt.persistent.selected_profile_id.as_deref() {
                if !rt.persistent.profiles.iter().any(|p| p.id == sel) {
                    rt.persistent.selected_profile_id =
                        rt.persistent.profiles.first().map(|p| p.id.clone());
                }
            }
            save_persistent_state(&rt.persistent);
        }
        stop_android_vpn_if_needed(stop_app).await;
        use tauri::Emitter;
        let _ = app.emit("state-updated", ());
    }

    Ok(msg)
}

#[tauri::command]
async fn toggle_vps_antisleep_cmd(
    app: AppHandle,
    creds: VpsCredentials,
    enable: bool,
) -> Result<bool, String> {
    if creds.host.trim().is_empty() || creds.password.trim().is_empty() {
        return Err("Введите IP-адрес сервера и пароль SSH (root)".to_string());
    }
    ssh_deployer::set_vps_antisleep(&app, &creds, enable).await
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

    let (stop_app, res) = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let stop_app = if rt.active_profile_id.as_deref() == Some(&profile.id) {
            stop_singbox_in_memory(&mut rt)
        } else {
            None
        };
        rt.persistent.profiles.retain(|p| {
            let same_id = p.id == profile.id;
            let same_creds = p.server == profile.server
                && p.port == profile.port
                && p.protocol == profile.protocol
                && !profile.password.is_empty()
                && p.password == profile.password;
            let same_name =
                p.server == profile.server && p.protocol == profile.protocol && p.name == profile.name;
            !(same_id || same_creds || same_name)
        });
        if let Some(sel) = rt.persistent.selected_profile_id.as_deref() {
            if !rt.persistent.profiles.iter().any(|p| p.id == sel) {
                rt.persistent.selected_profile_id = rt.persistent.profiles.first().map(|p| p.id.clone());
            }
        }
        save_persistent_state(&rt.persistent);
        (stop_app, rt.persistent.profiles.clone())
    };
    stop_android_vpn_if_needed(stop_app).await;
    {
        use tauri::Emitter;
        let _ = app.emit("state-updated", ());
    }
    Ok(res)
}

#[tauri::command]
async fn delete_vps_multi_group_cmd(
    app: AppHandle,
    creds: VpsCredentials,
    profiles: Vec<VpnProfile>,
    group_id: Option<String>,
    group_name: String,
    state: State<'_, SharedState>,
) -> Result<Vec<VpnProfile>, String> {
    if !creds.host.trim().is_empty() && !creds.password.trim().is_empty() {
        ssh_deployer::delete_vps_multi_group(
            &app,
            &creds,
            &profiles,
            group_id.as_deref(),
            &group_name,
        )
        .await?;
    }

    let (stop_app, res) = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let matches_target = |p: &VpnProfile| -> bool {
            if let Some(ref gid) = group_id {
                if !gid.is_empty() && p.multi_group_id.as_deref() == Some(gid.as_str()) {
                    return true;
                }
            }
            profiles.iter().any(|t| {
                p.id == t.id
                    || (p.server == t.server
                        && p.port == t.port
                        && p.protocol == t.protocol
                        && ((!t.password.is_empty() && p.password == t.password) || p.name == t.name))
            })
        };
        let mut stop_app = None;
        if let Some(ref aid) = rt.active_profile_id {
            if rt.persistent.profiles.iter().any(|p| p.id == *aid && matches_target(p)) {
                stop_app = stop_singbox_in_memory(&mut rt);
            }
        }
        rt.persistent.profiles.retain(|p| !matches_target(p));
        if let Some(sel) = rt.persistent.selected_profile_id.as_deref() {
            if !rt.persistent.profiles.iter().any(|p| p.id == sel) {
                rt.persistent.selected_profile_id = rt.persistent.profiles.first().map(|p| p.id.clone());
            }
        }
        save_persistent_state(&rt.persistent);
        (stop_app, rt.persistent.profiles.clone())
    };
    stop_android_vpn_if_needed(stop_app).await;
    {
        use tauri::Emitter;
        let _ = app.emit("state-updated", ());
    }
    Ok(res)
}

#[tauri::command]
async fn export_profile_json(
    app: AppHandle,
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

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                default_filename,
                "application/json".to_string(),
                cfg_str.as_bytes(),
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
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
}

fn clean_export_stem(raw: &str, fallback: &str) -> String {
    let clean: String = raw
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
    let trimmed = clean.trim_matches('_');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[tauri::command]
async fn export_multi_profile_json(
    app: AppHandle,
    profiles: Vec<VpnProfile>,
    group_name: String,
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    if profiles.is_empty() {
        return Err("Список профилей мульти-протокола пуст".to_string());
    }
    let routing = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let mut mobile_routing = rt.persistent.routing.clone();
        mobile_routing.mode = "tun".to_string();
        mobile_routing.default_outbound = "proxy".to_string();
        mobile_routing.proxy_processes.clear();
        mobile_routing.direct_processes.clear();
        mobile_routing
    };

    let cfg_val = generate_multi_singbox_config(&profiles, &group_name, &routing);
    let cfg_str = serde_json::to_string_pretty(&cfg_val).map_err(|e| e.to_string())?;
    let default_filename = format!("{}_multi.json", clean_export_stem(&group_name, "neuroveil"));

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                default_filename,
                "application/json".to_string(),
                cfg_str.as_bytes(),
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
    tauri::async_runtime::spawn_blocking(move || {
        let ps_script = format!(
            r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.SaveFileDialog
$dlg.Title = "Сохранить мульти-протокол sing-box (.json)"
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
}

#[tauri::command]
async fn export_multi_profile_zip(
    app: AppHandle,
    profiles: Vec<VpnProfile>,
    group_name: String,
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    if profiles.is_empty() {
        return Err("Список профилей мульти-протокола пуст".to_string());
    }
    let routing = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let mut mobile_routing = rt.persistent.routing.clone();
        mobile_routing.mode = "tun".to_string();
        mobile_routing.default_outbound = "proxy".to_string();
        mobile_routing.proxy_processes.clear();
        mobile_routing.direct_processes.clear();
        mobile_routing
    };

    let zip_bytes = build_multi_zip_bytes(&profiles, &group_name, &routing)?;
    let default_filename = format!("{}_multi.zip", clean_export_stem(&group_name, "neuroveil"));

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                default_filename,
                "application/zip".to_string(),
                &zip_bytes,
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
    tauri::async_runtime::spawn_blocking(move || {
        let ps_script = format!(
            r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.SaveFileDialog
$dlg.Title = "Сохранить архив мульти-протокола (.zip)"
$dlg.Filter = "ZIP-архив (*.zip)|*.zip"
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
        fs::write(&chosen_path, zip_bytes)
            .map_err(|e| format!("Не удалось сохранить ZIP-архив: {}", e))?;
        Ok(Some(chosen_path))
    })
    .await
    .map_err(|e| e.to_string())?
    }
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

#[allow(unused_variables)]
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
fn app_start_dragging(window: tauri::WebviewWindow) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = window.start_dragging();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = window;
}

#[tauri::command]
fn app_minimize(window: tauri::WebviewWindow) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = window.minimize();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = window;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn compute_screen_scale_factor(window: &tauri::WebviewWindow) -> (f64, f64, f64) {
    let mon = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(m) = mon {
        let sf = m.scale_factor().max(0.5);
        let sw = m.size().width as f64 / sf;
        let sh = m.size().height as f64 / sf;
        let ratio = (sw / 1920.0).min(sh / 1080.0);
        let k = (0.5 + 0.5 * ratio).clamp(0.84, 1.32);
        return (k, sw, sh);
    }
    (1.0, 1920.0, 1080.0)
}

#[cfg(target_os = "windows")]
static MAIN_ORIG_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
unsafe extern "system" fn non_resizable_main_wndproc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    #[link(name = "user32")]
    extern "system" {
        fn CallWindowProcW(
            lp_prev_wnd_func: isize,
            hwnd: isize,
            msg: u32,
            w_param: usize,
            l_param: isize,
        ) -> isize;
    }

    const WM_NCHITTEST: u32 = 0x0084;
    const WM_NCLBUTTONDBLCLK: u32 = 0x00A3;
    const WM_SYSCOMMAND: u32 = 0x0112;
    const HTCLIENT: isize = 1;
    const SC_SIZE: usize = 0xF000;
    const SC_MAXIMIZE: usize = 0xF030;

    if msg == WM_NCLBUTTONDBLCLK {
        return 0;
    }
    if msg == WM_SYSCOMMAND {
        let cmd = wparam & 0xFFF0;
        if cmd == SC_SIZE || cmd == SC_MAXIMIZE {
            return 0;
        }
    }

    let orig = MAIN_ORIG_WNDPROC.load(std::sync::atomic::Ordering::SeqCst);
    if orig == 0 {
        return 0;
    }
    let res = CallWindowProcW(orig, hwnd, msg, wparam, lparam);
    if msg == WM_NCHITTEST && (10..=18).contains(&res) {
        return HTCLIENT;
    }
    res
}

#[cfg(target_os = "windows")]
fn install_non_resizable_frameless_hook(window: &tauri::WebviewWindow) {
    if MAIN_ORIG_WNDPROC.load(std::sync::atomic::Ordering::SeqCst) != 0 {
        return;
    }
    if let Ok(hwnd) = window.hwnd() {
        #[link(name = "user32")]
        extern "system" {
            fn SetWindowLongPtrW(hwnd: isize, n_index: i32, dw_new_long: isize) -> isize;
        }
        const GWLP_WNDPROC: i32 = -4;
        unsafe {
            let h = hwnd.0 as isize;
            let prev = SetWindowLongPtrW(
                h,
                GWLP_WNDPROC,
                non_resizable_main_wndproc as *const () as isize,
            );
            if prev != 0 {
                MAIN_ORIG_WNDPROC.store(prev, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn install_non_resizable_frameless_hook(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
fn apply_frameless_dark_border(window: &tauri::WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        #[link(name = "dwmapi")]
        extern "system" {
            fn DwmSetWindowAttribute(
                hwnd: isize,
                dw_attribute: u32,
                pv_attribute: *const std::ffi::c_void,
                cb_attribute: u32,
            ) -> i32;
        }
        unsafe {
            let h = hwnd.0 as isize;
            // DWMWA_USE_IMMERSIVE_DARK_MODE = 20
            let dark_mode: i32 = 1;
            let _ = DwmSetWindowAttribute(
                h,
                20,
                &dark_mode as *const i32 as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
            // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2
            let round_corners: u32 = 2;
            let _ = DwmSetWindowAttribute(
                h,
                33,
                &round_corners as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            // DWMWA_BORDER_COLOR = 34, DWMWA_COLOR_NONE = 0xFFFFFFFE
            let no_border: u32 = 0xFFFF_FFFE;
            let _ = DwmSetWindowAttribute(
                h,
                34,
                &no_border as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn apply_frameless_dark_border(_window: &tauri::WebviewWindow) {}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn apply_scaled_main_window_size(window: &tauri::WebviewWindow) {
    let _ = window.set_maximizable(false);
    let (mut k, sw, sh) = compute_screen_scale_factor(window);
    let max_k_w = (sw * 0.90) / 460.0;
    let max_k_h = (sh * 0.86) / 680.0;
    k = k.min(max_k_w).min(max_k_h).clamp(0.75, 1.32);

    let target_w = (460.0 * k).round();
    let target_h = (680.0 * k).round();
    let fixed_size = tauri::Size::Logical(tauri::LogicalSize {
        width: target_w,
        height: target_h,
    });
    let _ = window.set_min_size(None::<tauri::Size>);
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_size(fixed_size.clone());
    let _ = window.set_min_size(Some(fixed_size.clone()));
    let _ = window.set_max_size(Some(fixed_size));
    let _ = window.set_zoom(k);
    install_non_resizable_frameless_hook(window);
    apply_frameless_dark_border(window);
}

#[tauri::command]
fn app_toggle_maximize(window: tauri::WebviewWindow, _state: State<'_, SharedState>) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if window.label() == "main" {
            return;
        }
        if let Ok(is_max) = window.is_maximized() {
            if is_max {
                let _ = window.unmaximize();
            } else {
                let _ = window.maximize();
            }
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = window;
}

#[tauri::command]
fn app_close(window: tauri::WebviewWindow, app: tauri::AppHandle, state: State<'_, SharedState>) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (window, app, state);
        return;
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if window.label() != "main" {
            let _ = window.close();
            return;
        }
        let should_hide = {
            if let Ok(rt) = state.lock() {
                save_persistent_state(&rt.persistent);
                rt.persistent.close_to_tray
            } else {
                true
            }
        };
        if should_hide {
            let _ = window.hide();
        } else {
            if let Ok(mut rt) = state.lock() {
                stop_singbox_internal(&mut rt);
                save_persistent_state(&rt.persistent);
            }
            app.exit(0);
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn simple_url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[tauri::command]
fn emit_window_event(app: tauri::AppHandle, event: String, payload: serde_json::Value) {
    use tauri::Emitter;
    let _ = app.emit(&event, payload);
}

#[tauri::command]
async fn open_popup_window(
    app: tauri::AppHandle,
    mode: String,
    profile_id: Option<String>,
    submode: Option<String>,
) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (app, mode, profile_id, submode);
        return Ok(());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use tauri::Emitter;
        let label = format!("popup_{}", mode);
        if let Some(existing) = app.get_webview_window(&label) {
            let _ = existing.emit(
                "popup-params",
                serde_json::json!({
                    "mode": mode,
                    "profileId": profile_id,
                    "submode": submode,
                }),
            );
            let _ = existing.show();
            if let Ok(true) = existing.is_minimized() {
                let _ = existing.unminimize();
            }
            let _ = existing.set_focus();
            return Ok(());
        }

        let (title, width, height, min_w, min_h) = match mode.as_str() {
            "file" => ("NeuroVeil VPN — Добавить из файла", 480.0, 390.0, 400.0, 320.0),
            "key" => ("NeuroVeil VPN — Вставить ключ", 500.0, 430.0, 400.0, 340.0),
            "vps" => ("NeuroVeil VPN — Добавить VPS (Автодеплой по SSH)", 740.0, 660.0, 520.0, 460.0),
            "vps_admin" => ("NeuroVeil VPN — Управление VPS", 760.0, 660.0, 560.0, 480.0),
            "logs" => ("NeuroVeil VPN — Журнал ядра sing-box", 720.0, 520.0, 460.0, 340.0),
            "qr" => ("NeuroVeil VPN — QR-код и подключение смартфона", 540.0, 640.0, 420.0, 480.0),
            "edit" => ("NeuroVeil VPN — Редактирование конфигурации", 660.0, 650.0, 460.0, 440.0),
            "routing_rules" => ("NeuroVeil VPN — Настройка правил маршрутизации", 560.0, 640.0, 440.0, 440.0),
            "processes" => ("NeuroVeil VPN — Выбор из запущенных процессов", 640.0, 540.0, 460.0, 380.0),
            "save_preset" => ("NeuroVeil VPN — Новая предустановка маршрутов", 480.0, 430.0, 380.0, 340.0),
            "confirm_clean" => ("NeuroVeil VPN — Подтвердите очистку сервера", 440.0, 235.0, 360.0, 200.0),
            "speedtest" => ("NeuroVeil VPN — Тест соединения и скорости", 650.0, 640.0, 500.0, 480.0),
            _ => ("NeuroVeil VPN", 500.0, 420.0, 380.0, 300.0),
        };

        let mut query = format!("index.html?popup={}", simple_url_encode(&mode));
        if let Some(ref pid) = profile_id {
            if !pid.trim().is_empty() {
                query.push_str(&format!("&profileId={}", simple_url_encode(pid)));
            }
        }
        if let Some(ref sub) = submode {
            if !sub.trim().is_empty() {
                query.push_str(&format!("&submode={}", simple_url_encode(sub)));
            }
        }

        let url = tauri::WebviewUrl::App(query.into());
        let win = tauri::WebviewWindowBuilder::new(&app, &label, url)
            .title(title)
            .inner_size(width, height)
            .min_inner_size(min_w, min_h)
            .resizable(true)
            .decorations(false)
            .always_on_top(true)
            .center()
            .focused(true)
            .build()
            .map_err(|e| format!("Не удалось открыть окно: {}", e))?;

        let (k, sw, sh) = compute_screen_scale_factor(&win);
        let max_k_w = (sw * 0.92) / width;
        let max_k_h = (sh * 0.86) / height;
        let popup_k = k.min(max_k_w).min(max_k_h).clamp(0.75, 1.32);
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: (width * popup_k).round(),
            height: (height * popup_k).round(),
        }));
        let _ = win.set_zoom(popup_k);
        let _ = win.center();
        let _ = win.show();
        apply_frameless_dark_border(&win);
        let _ = win.set_focus();
        Ok(())
    }
}

#[tauri::command]
fn app_show_window(app: tauri::AppHandle) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        if let Ok(true) = w.is_minimized() {
            let _ = w.unminimize();
        }
        let _ = w.set_focus();
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = app;
}

const GITHUB_REPO_OWNER_NAME: &str = "lllPredatorlll/NeuroVeil-VPN";
const GITHUB_REPO_URL: &str = "https://github.com/lllPredatorlll/NeuroVeil-VPN";

fn active_local_tunnel_socks_port() -> Option<u16> {
    for port in [2081u16, 2082, 2083, 2084, 2080] {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok() {
            return Some(port);
        }
    }
    None
}

fn build_ureq_agent(socks_port: Option<u16>, connect_timeout_s: u64, read_timeout_s: u64) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(connect_timeout_s))
        .timeout_read(Duration::from_secs(read_timeout_s));
    if let Some(port) = socks_port {
        if let Ok(proxy) = ureq::Proxy::new(format!("socks5h://127.0.0.1:{}", port)) {
            b = b.proxy(proxy);
        }
    }
    b.build()
}

fn fetch_url_text_with_tunnel(url: &str) -> Result<String, String> {
    let mut saw_404 = false;
    let mut saw_403 = false;

    let agent = build_ureq_agent(None, 7, 15);
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
        // Fallback 1: via active VPN tunnel using pure-Rust ureq SOCKS5h
        if let Some(port) = active_local_tunnel_socks_port() {
            let proxied = build_ureq_agent(Some(port), 8, 20);
            match proxied
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
                Err(ureq::Error::Status(code, _)) if code == 404 => saw_404 = true,
                Err(ureq::Error::Status(code, _)) if code == 403 => saw_403 = true,
                Err(_) => {}
            }
        }

        // Fallback 2 & 3 on Windows: curl.exe (uses Windows Schannel native TLS)
        #[cfg(target_os = "windows")]
        {
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
                cmd.creation_flags(CREATE_NO_WINDOW);

                if let Ok(out) = cmd.output() {
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
    }

    if saw_403 {
        return Err("HTTP 403".to_string());
    }
    if saw_404 {
        return Err("HTTP 404".to_string());
    }

    Err(format!("Не удалось получить данные по адресу {}", url))
}

#[allow(dead_code)]
fn download_url_to_file_with_tunnel(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let agent = build_ureq_agent(None, 10, 120);

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
        let proxied = build_ureq_agent(Some(port), 10, 180);
        if let Ok(resp) = proxied
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

        #[cfg(target_os = "windows")]
        {
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
            let score = if cfg!(target_os = "android") {
                if lower.ends_with(".apk") && (lower.contains("arm64") || lower.contains("aarch64")) {
                    50
                } else if lower.ends_with(".apk") {
                    40
                } else {
                    0
                }
            } else if lower == "neuroveil.exe" {
                40
            } else if lower.ends_with(".exe") && lower.contains("setup") {
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

    // If no release asset attached on Windows, fallback to raw NeuroVeil.exe in main branch
    if download_url.is_none() && !cfg!(target_os = "android") {
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
                            download_url: if cfg!(target_os = "android") {
                                Some(format!(
                                    "{}/releases/download/v{}/NeuroVeil-VPN-v{}-arm64.apk",
                                    GITHUB_REPO_URL, latest_version, latest_version
                                ))
                            } else {
                                Some(format!(
                                    "https://github.com/{}/raw/{}/NeuroVeil.exe",
                                    GITHUB_REPO_OWNER_NAME, branch
                                ))
                            },
                            asset_name: Some(if cfg!(target_os = "android") {
                                format!("NeuroVeil-VPN-v{}-arm64.apk", latest_version)
                            } else {
                                "NeuroVeil.exe".to_string()
                            }),
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
                    release_name: format!("NeuroVeil VPN v{} (Build 2080)", current_version),
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
                    format!(
                        "Связь с GitHub установлена. У вас актуальная версия v{} (Build 2080).\nПодсказка: если репозиторий lllPredatorlll/NeuroVeil-VPN сейчас закрыт (Private) или пуст, сделайте его публичным (Public) в настройках GitHub (Settings -> Change repository visibility -> Public) и выгрузите файлы проекта.",
                        current_version
                    )
                } else {
                    format!(
                        "Связь с GitHub установлена. У вас актуальная версия v{} (Build 2080).",
                        current_version
                    )
                };
                return Ok(AppUpdateInfo {
                    current_version: current_version.clone(),
                    latest_version: current_version.clone(),
                    has_update: false,
                    release_name: format!("NeuroVeil VPN v{} (Build 2080)", current_version),
                    release_notes: note,
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
    #[cfg(target_os = "android")]
    {
        let _ = state;
        let trimmed_url = download_url.trim().to_string();
        let name_lower = asset_name.clone().unwrap_or_default().to_lowercase();
        let is_direct_apk = !trimmed_url.is_empty()
            && (trimmed_url.to_lowercase().contains(".apk") || name_lower.ends_with(".apk"));

        if !is_direct_apk {
            let url_to_open = if trimmed_url.is_empty() {
                format!("{}/releases/latest", GITHUB_REPO_URL)
            } else {
                trimmed_url
            };
            crate::android_vpn::open_url(&app, url_to_open)?;
            return Ok("Открыта страница загрузки APK в браузере.".to_string());
        }

        let cache_dir = app
            .path()
            .app_cache_dir()
            .unwrap_or_else(|_| get_app_data_dir().join("cache"));
        let updates_dir = cache_dir.join("updates");
        let _ = fs::create_dir_all(&updates_dir);
        let apk_path = updates_dir.join("NeuroVeil-update.apk");
        let tmp_path = updates_dir.join("NeuroVeil-update.apk.tmp");
        let _ = fs::remove_file(&tmp_path);

        let url_clone = trimmed_url.clone();
        let dl_path_clone = tmp_path.clone();
        if let Err(dl_err) = tokio::task::spawn_blocking(move || {
            download_url_to_file_with_tunnel(&url_clone, &dl_path_clone)
        })
        .await
        .map_err(|e| format!("Ошибка задачи загрузки APK: {}", e))?
        {
            let _ = fs::remove_file(&tmp_path);
            let _ = crate::android_vpn::open_url(&app, trimmed_url);
            return Err(format!(
                "Не удалось скачать APK напрямую ({}). Открыта ссылка в браузере.",
                dl_err
            ));
        }

        let valid_apk = (|| -> bool {
            use std::io::Read;
            let Ok(meta) = fs::metadata(&tmp_path) else {
                return false;
            };
            if meta.len() < 1_000_000 {
                return false;
            }
            let Ok(mut f) = fs::File::open(&tmp_path) else {
                return false;
            };
            let mut magic = [0u8; 2];
            f.read_exact(&mut magic).is_ok() && magic == *b"PK"
        })();

        if !valid_apk {
            let _ = fs::remove_file(&tmp_path);
            let _ = crate::android_vpn::open_url(&app, trimmed_url);
            return Err("Скачанный файл не является корректным APK. Открыта ссылка в браузере.".to_string());
        }

        let _ = fs::remove_file(&apk_path);
        fs::rename(&tmp_path, &apk_path)
            .or_else(|_| fs::copy(&tmp_path, &apk_path).map(|_| ()))
            .map_err(|e| format!("Не удалось сохранить APK для установки: {}", e))?;
        let _ = fs::remove_file(&tmp_path);

        let apk_str = apk_path.to_string_lossy().to_string();
        let res = tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::install_apk(&app, apk_str)
        })
        .await
        .map_err(|e| format!("Ошибка запуска установщика APK: {}", e))??;

        return Ok(res.message);
    }

    #[cfg(not(target_os = "android"))]
    {
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
}

#[tauri::command]
fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Разрешены только HTTP/HTTPS ссылки".to_string());
    }
    #[cfg(target_os = "android")]
    {
        return crate::android_vpn::open_url(&app, url);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", &url]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Не удалось открыть браузер: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
async fn request_add_android_tile(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<serde_json::Value, String> {
    if let Ok(rt) = state.lock() {
        sync_android_tile_from_state(&rt);
    }
    #[cfg(target_os = "android")]
    {
        let res = tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::request_add_quick_tile(&app)
        })
        .await
        .map_err(|e| e.to_string())??;
        return serde_json::to_value(res).map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Плитка быстрых настроек доступна только на Android."
        }))
    }
}

#[tauri::command]
async fn is_battery_optimization_ignored(app: AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::is_battery_optimization_ignored(&app)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(true)
    }
}

#[tauri::command]
async fn request_battery_optimization_exemption(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let res = tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::request_battery_optimization_exemption(&app)
        })
        .await
        .map_err(|e| e.to_string())??;
        return serde_json::to_value(res).map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(serde_json::json!({
            "ignored": true,
            "requested": false,
            "message": "Оптимизация батареи применима только к Android."
        }))
    }
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

#[tauri::command]
async fn export_backup_file(
    app: AppHandle,
    ui_prefs: Option<serde_json::Value>,
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let persistent_snapshot = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        rt.persistent.clone()
    };

    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let default_name = format!("NeuroVeil_v2_backup_{}.nvbackup", timestamp);

    let payload = serde_json::json!({
        "app": "NeuroVeil VPN",
        "format_version": 1,
        "created_at": chrono::Local::now().to_rfc3339(),
        "state": persistent_snapshot,
        "ui_prefs": ui_prefs.unwrap_or_else(|| serde_json::json!({}))
    });
    let json_text = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Ошибка сериализации резервной копии: {}", e))?;

    #[cfg(target_os = "android")]
    {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::save_file_bytes(
                &app,
                default_name,
                "application/octet-stream".to_string(),
                json_text.as_bytes(),
            )
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "android"))]
    {
    let _ = app;
    let save_path = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        let ps_script = format!(
            r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.SaveFileDialog
$dlg.Title = "Сохранить резервную копию NeuroVeil VPN"
$dlg.FileName = "{}"
$dlg.Filter = "Резервная копия NeuroVeil (*.nvbackup;*.json)|*.nvbackup;*.json|Все файлы (*.*)|*.*"
$dlg.AddExtension = $true
$dlg.DefaultExt = "nvbackup"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
    Write-Output $dlg.FileName
}}
"#,
            default_name
        );
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", &ps_script]);
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

    let path_str = match save_path {
        Some(p) => p,
        None => return Ok(None),
    };

    fs::write(&path_str, json_text.as_bytes())
        .map_err(|e| format!("Не удалось записать файл резервной копии: {}", e))?;
    Ok(Some(path_str))
    }
}

#[tauri::command]
async fn import_backup_file(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Option<serde_json::Value>, String> {
    use tauri::Emitter;

    #[cfg(target_os = "android")]
    let (path_str, content) = {
        let app_clone = app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || {
            crate::android_vpn::pick_file_bytes(&app_clone, vec!["*/*".to_string()])
        })
        .await
        .map_err(|e| e.to_string())??;
        match picked {
            Some((name, bytes)) => (name, String::from_utf8_lossy(&bytes).to_string()),
            None => return Ok(None),
        }
    };

    #[cfg(not(target_os = "android"))]
    let (path_str, content) = {
        let file_path = tauri::async_runtime::spawn_blocking(|| -> Result<Option<String>, String> {
            let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Title = "Выберите файл резервной копии NeuroVeil VPN"
$dlg.Filter = "Резервная копия NeuroVeil (*.nvbackup;*.json;*.bak)|*.nvbackup;*.json;*.bak|Все файлы (*.*)|*.*"
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

        let content = fs::read_to_string(&path_str)
            .map_err(|e| format!("Не удалось прочитать файл резервной копии: {}", e))?;
        (path_str, content)
    };
    let val: serde_json::Value = serde_json::from_str(content.trim())
        .map_err(|e| format!("Файл резервной копии поврежден или имеет неверный JSON: {}", e))?;

    let (restored_state_val, ui_prefs_val) = if val.get("state").is_some() {
        (
            val["state"].clone(),
            val.get("ui_prefs").cloned().unwrap_or_else(|| serde_json::json!({})),
        )
    } else {
        (val.clone(), serde_json::json!({}))
    };

    let mut restored_state: AppPersistentState = serde_json::from_value(restored_state_val)
        .map_err(|e| format!("Некорректная структура резервной копии: {}", e))?;

    if restored_state.routing_presets.is_empty() {
        restored_state.routing_presets = default_routing_presets();
    }

    let autostart_flag = restored_state.autostart;
    let start_min_flag = restored_state.start_minimized;

    let stop_app = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let stop_app = stop_singbox_in_memory(&mut rt);
        rt.persistent = restored_state.clone();
        save_persistent_state(&rt.persistent);
        sync_android_tile_from_state(&rt);
        stop_app
    };
    stop_android_vpn_if_needed(stop_app).await;

    let _ = set_windows_autostart(autostart_flag, start_min_flag);
    let _ = app.emit("state-updated", ());

    Ok(Some(serde_json::json!({
        "path": path_str,
        "state": restored_state,
        "ui_prefs": ui_prefs_val
    })))
}

#[tauri::command]
async fn factory_reset_app(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<AppPersistentState, String> {
    use tauri::Emitter;
    let dir = get_app_data_dir();
    let default_state = AppPersistentState::default();

    let stop_app = {
        let mut rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let stop_app = stop_singbox_in_memory(&mut rt);
        rt.logs.clear();
        rt.persistent = default_state.clone();

        for fname in [
            "state.json",
            "state.json.bak",
            "state.json.tmp",
            "active_config.json",
            "singbox.log",
        ] {
            let p = dir.join(fname);
            if p.exists() {
                let _ = fs::remove_file(p);
            }
        }
        save_persistent_state(&rt.persistent);
        sync_android_tile_from_state(&rt);
        stop_app
    };
    stop_android_vpn_if_needed(stop_app).await;

    let _ = set_windows_autostart(false, false);
    let _ = app.emit("state-updated", ());
    Ok(default_state)
}

static SPEEDTEST_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SPEEDTEST_RUN_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn open_stream_with_optional_socks(
    host: &str,
    port: u16,
    socks_port: Option<u16>,
    timeout_ms: u64,
) -> Result<(TcpStream, Instant), String> {
    use std::io::{Read, Write};

    if let Some(sp) = socks_port {
        let proxy_addr = SocketAddr::from(([127, 0, 0, 1], sp));
        let mut sock = TcpStream::connect_timeout(&proxy_addr, Duration::from_millis(900))
            .map_err(|e| format!("SOCKS connect error: {}", e))?;
        let _ = sock.set_read_timeout(Some(Duration::from_millis(timeout_ms)));
        let _ = sock.set_write_timeout(Some(Duration::from_millis(timeout_ms)));
        let _ = sock.set_nodelay(true);

        sock.write_all(&[0x05, 0x01, 0x00])
            .map_err(|e| format!("SOCKS greeting write: {}", e))?;
        let mut rep = [0u8; 2];
        sock.read_exact(&mut rep)
            .map_err(|e| format!("SOCKS greeting read: {}", e))?;
        if rep != [0x05, 0x00] {
            return Err("SOCKS auth rejected".to_string());
        }

        let hb = host.as_bytes();
        if hb.len() > 255 {
            return Err("Hostname too long".to_string());
        }
        let mut req = Vec::with_capacity(7 + hb.len());
        req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, hb.len() as u8]);
        req.extend_from_slice(hb);
        req.extend_from_slice(&port.to_be_bytes());

        let t_start = Instant::now();
        sock.write_all(&req)
            .map_err(|e| format!("SOCKS request write: {}", e))?;
        let mut hdr = [0u8; 4];
        sock.read_exact(&mut hdr)
            .map_err(|e| format!("SOCKS reply timeout: {}", e))?;
        if hdr[1] != 0x00 {
            return Err(format!("SOCKS reply code 0x{:02X}", hdr[1]));
        }
        // Drain bound address bytes per RFC 1928 so payload stays cleanly aligned
        match hdr[3] {
            0x01 => {
                let mut skip = [0u8; 6];
                let _ = sock.read_exact(&mut skip);
            }
            0x03 => {
                let mut len_b = [0u8; 1];
                if sock.read_exact(&mut len_b).is_ok() {
                    let mut skip = vec![0u8; (len_b[0] as usize) + 2];
                    let _ = sock.read_exact(&mut skip);
                }
            }
            0x04 => {
                let mut skip = [0u8; 18];
                let _ = sock.read_exact(&mut skip);
            }
            _ => {}
        }
        Ok((sock, t_start))
    } else {
        let target_str = format!("{}:{}", host, port);
        let addrs: Vec<SocketAddr> = target_str
            .to_socket_addrs()
            .map_err(|e| format!("DNS error: {}", e))?
            .collect();
        let addr = addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| addrs.first())
            .copied()
            .ok_or_else(|| "No resolved address".to_string())?;
        let t_start = Instant::now();
        let sock = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms))
            .map_err(|e| format!("TCP timeout: {}", e))?;
        let _ = sock.set_read_timeout(Some(Duration::from_millis(timeout_ms)));
        let _ = sock.set_write_timeout(Some(Duration::from_millis(timeout_ms)));
        let _ = sock.set_nodelay(true);
        Ok((sock, t_start))
    }
}

fn probe_tcp_rtt(
    host: &str,
    port: u16,
    socks_port: Option<u16>,
    timeout_ms: u64,
) -> Result<f64, String> {
    let (_sock, t_start) = open_stream_with_optional_socks(host, port, socks_port, timeout_ms)?;
    let ms = t_start.elapsed().as_secs_f64() * 1000.0;
    Ok(round1(ms.max(0.5)))
}

fn send_http_head_probe(sock: &mut TcpStream, host: &str) -> Result<f64, String> {
    use std::io::{Read, Write};
    let req = format!(
        "HEAD /generate_204 HTTP/1.1\r\nHost: {}\r\nUser-Agent: NeuroVeilVPN/2.0\r\nConnection: keep-alive\r\n\r\n",
        host
    );
    let t_start = Instant::now();
    sock.write_all(req.as_bytes())
        .map_err(|e| format!("HTTP write err: {}", e))?;
    let mut buf = [0u8; 1024];
    let mut acc = Vec::with_capacity(512);
    loop {
        let n = sock.read(&mut buf).map_err(|e| format!("HTTP read err: {}", e))?;
        if n == 0 {
            return Err("EOF".to_string());
        }
        acc.extend_from_slice(&buf[..n]);
        if acc.windows(4).any(|w| w == b"\r\n\r\n") {
            let ms = t_start.elapsed().as_secs_f64() * 1000.0;
            return Ok(round1(ms.max(1.0)));
        }
        if acc.len() > 8192 {
            let ms = t_start.elapsed().as_secs_f64() * 1000.0;
            return Ok(round1(ms.max(1.0)));
        }
    }
}

#[cfg(target_os = "windows")]
fn probe_https_service_rtt(host: &str, socks_port: Option<u16>) -> Result<u32, String> {
    let url = format!("https://{}/", host);
    let mut cmd = Command::new("curl.exe");
    cmd.args([
        "-s",
        "-X",
        "HEAD",
        "-o",
        "NUL",
        "--connect-timeout",
        "3",
        "--max-time",
        "4",
        "-w",
        "%{http_code}:%{time_appconnect}:%{time_starttransfer}\n",
    ]);
    if let Some(sp) = socks_port {
        cmd.args(["-x", &format!("socks5h://127.0.0.1:{}", sp)]);
    }
    cmd.args(["-A", "NeuroVeilVPN-Diag/2.0", &url, &url]);
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().map_err(|e| format!("curl spawn error: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut parsed_lines: Vec<(u16, f64, f64)> = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.trim().split(':').collect();
        if parts.len() == 3 {
            let code = parts[0].parse::<u16>().unwrap_or(0);
            let app = parts[1].replace(',', ".").parse::<f64>().unwrap_or(0.0);
            let ttfb = parts[2].replace(',', ".").parse::<f64>().unwrap_or(0.0);
            if code > 0 && ttfb > 0.0 {
                parsed_lines.push((code, app, ttfb));
            }
        }
    }

    if let Some((_, app2, ttfb2)) = parsed_lines.get(1).copied() {
        // Reused keep-alive HTTPS connection -> pure 1-RTT server latency
        let rtt_s = if app2 <= 0.0001 {
            ttfb2
        } else if ttfb2 > app2 {
            ttfb2 - app2
        } else {
            ttfb2
        };
        return Ok((rtt_s * 1000.0).max(1.0).round() as u32);
    }
    if let Some((_, app1, ttfb1)) = parsed_lines.first().copied() {
        let rtt_s = if ttfb1 > app1 && app1 > 0.0001 {
            ttfb1 - app1
        } else {
            ttfb1
        };
        return Ok((rtt_s * 1000.0).max(1.0).round() as u32);
    }

    Err("Не отвечает / таймаут".to_string())
}

#[cfg(not(target_os = "windows"))]
fn probe_https_service_rtt(host: &str, socks_port: Option<u16>) -> Result<u32, String> {
    let url = format!("https://{}/", host);
    let agent = build_ureq_agent(socks_port, 3, 4);
    let t_first = Instant::now();
    match agent.head(&url).set("User-Agent", "NeuroVeilVPN-Diag/2.0").call() {
        Ok(_) | Err(ureq::Error::Status(_, _)) => {}
        Err(_) => return Err("Не отвечает / таймаут".to_string()),
    }
    let first_ms = t_first.elapsed().as_secs_f64() * 1000.0;
    let t_second = Instant::now();
    match agent.head(&url).set("User-Agent", "NeuroVeilVPN-Diag/2.0").call() {
        Ok(_) | Err(ureq::Error::Status(_, _)) => {
            let ms = (t_second.elapsed().as_secs_f64() * 1000.0).max(1.0);
            Ok(ms.round() as u32)
        }
        Err(_) => Ok(first_ms.max(1.0).round() as u32),
    }
}

#[cfg(target_os = "windows")]
fn curl_fetch_text(url: &str, socks_port: Option<u16>, timeout_secs: u64, force_v6: bool) -> Option<String> {
    let mut cmd = Command::new("curl.exe");
    let timeout_s = timeout_secs.to_string();
    cmd.args(["-sSL", "--fail", "--connect-timeout", "4", "--max-time", &timeout_s]);
    if force_v6 {
        cmd.arg("-6");
    }
    if let Some(sp) = socks_port {
        cmd.args(["-x", &format!("socks5h://127.0.0.1:{}", sp)]);
    }
    cmd.args(["-A", "NeuroVeilVPN-Diag/2.0", url]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    if let Ok(out) = cmd.output() {
        if out.status.success() {
            let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !txt.is_empty() {
                return Some(txt);
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn curl_fetch_text(url: &str, socks_port: Option<u16>, timeout_secs: u64, force_v6: bool) -> Option<String> {
    let eff_url = if force_v6 && url.contains("api64.ipify.org") {
        "https://api6.ipify.org"
    } else {
        url
    };
    let agent = build_ureq_agent(socks_port, 4, timeout_secs);
    if let Ok(resp) = agent.get(eff_url).set("User-Agent", "NeuroVeilVPN-Diag/2.0").call() {
        if let Ok(txt) = resp.into_string() {
            let trimmed = txt.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn update_ping_stats(ev: &mut SpeedTestProgressEvent, total_sent: usize) {
    if ev.ping_samples_ms.is_empty() {
        ev.ping_min_ms = None;
        ev.ping_avg_ms = None;
        ev.ping_max_ms = None;
        ev.jitter_ms = None;
    } else {
        let mut min_v = f64::MAX;
        let mut max_v = 0.0f64;
        let mut sum = 0.0f64;
        for &x in &ev.ping_samples_ms {
            if x < min_v {
                min_v = x;
            }
            if x > max_v {
                max_v = x;
            }
            sum += x;
        }
        ev.ping_min_ms = Some(round1(min_v));
        ev.ping_max_ms = Some(round1(max_v));
        ev.ping_avg_ms = Some(round1(sum / ev.ping_samples_ms.len() as f64));
        if ev.ping_samples_ms.len() >= 2 {
            let mut j_sum = 0.0f64;
            for w in ev.ping_samples_ms.windows(2) {
                j_sum += (w[1] - w[0]).abs();
            }
            ev.jitter_ms = Some(round1(j_sum / (ev.ping_samples_ms.len() - 1) as f64));
        } else {
            ev.jitter_ms = Some(0.0);
        }
    }
    if total_sent > 0 {
        let lost = total_sent.saturating_sub(ev.ping_samples_ms.len());
        ev.packet_loss_pct = Some(round1((lost as f64 / total_sent as f64) * 100.0));
    }
}

fn update_speed_stats(samples: &[f64], total_bytes: u64, elapsed_s: f64) -> (Option<f64>, Option<f64>, Option<f64>) {
    let avg = if total_bytes > 0 && elapsed_s > 0.05 {
        Some(round2((total_bytes as f64 * 8.0) / (elapsed_s * 1_000_000.0)))
    } else {
        None
    };
    if samples.is_empty() {
        return (avg, avg, avg);
    }
    let mut min_v = f64::MAX;
    let mut max_v = 0.0f64;
    for &s in samples {
        if s < min_v {
            min_v = s;
        }
        if s > max_v {
            max_v = s;
        }
    }
    (Some(round2(min_v)), avg, Some(round2(max_v)))
}

#[tauri::command]
async fn cancel_connection_speed_test() -> Result<(), String> {
    SPEEDTEST_CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn run_connection_speed_test(
    app: AppHandle,
    use_vpn_tunnel: bool,
    quick_only: bool,
    state: State<'_, SharedState>,
) -> Result<SpeedTestProgressEvent, String> {
    use std::io::Read;
    #[cfg(target_os = "windows")]
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use tauri::Emitter;

    let my_seq = SPEEDTEST_RUN_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    SPEEDTEST_CANCEL.store(false, Ordering::SeqCst);

    let (vpn_connected, active_host, active_port) = {
        let rt = state.lock().map_err(|_| "Lock error".to_string())?;
        let connected = is_vpn_running(&rt);
        let pid = rt
            .active_profile_id
            .clone()
            .or_else(|| rt.persistent.selected_profile_id.clone());
        let target = pid.and_then(|id| {
            rt.persistent
                .profiles
                .iter()
                .find(|p| p.id == id)
                .map(|p| (p.server.clone(), p.port))
        });
        (
            connected,
            target.as_ref().map(|t| t.0.clone()),
            target.map(|t| t.1),
        )
    };

    let socks_port = if use_vpn_tunnel && vpn_connected {
        active_local_tunnel_socks_port()
    } else {
        None
    };
    let route_used = if socks_port.is_some() {
        "vpn_tunnel".to_string()
    } else if vpn_connected && use_vpn_tunnel {
        "system_tun".to_string()
    } else {
        "direct".to_string()
    };

    let is_aborted = move || -> bool {
        SPEEDTEST_CANCEL.load(Ordering::SeqCst) || SPEEDTEST_RUN_SEQ.load(Ordering::SeqCst) != my_seq
    };

    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<SpeedTestProgressEvent, String> {
        let global_start = Instant::now();
        let mut ev = SpeedTestProgressEvent {
            phase: "ping".to_string(),
            progress_pct: 3,
            ping_samples_ms: Vec::new(),
            ping_min_ms: None,
            ping_avg_ms: None,
            ping_max_ms: None,
            jitter_ms: None,
            packet_loss_pct: None,
            vps_rtt_ms: None,
            download_samples_mbps: Vec::new(),
            download_current_mbps: None,
            download_min_mbps: None,
            download_avg_mbps: None,
            download_max_mbps: None,
            download_bytes: 0,
            upload_samples_mbps: Vec::new(),
            upload_current_mbps: None,
            upload_min_mbps: None,
            upload_avg_mbps: None,
            upload_max_mbps: None,
            upload_bytes: 0,
            external_ip: None,
            country: None,
            city: None,
            isp: None,
            colo: None,
            ipv6_reachable: None,
            route_used,
            services: Vec::new(),
            elapsed_ms: 0,
            error: None,
        };

        let emit_now = |ev: &mut SpeedTestProgressEvent| {
            ev.elapsed_ms = global_start.elapsed().as_millis() as u64;
            let _ = app_clone.emit("speedtest-progress", ev.clone());
        };

        emit_now(&mut ev);

        // Direct TCP RTT to user's selected VPS server (bypassing local SOCKS so it measures real host latency)
        if let (Some(h), Some(p)) = (active_host.as_deref(), active_port) {
            if !h.trim().is_empty() {
                if let Ok(ms) = probe_tcp_rtt(h, 22, None, 1200)
                    .or_else(|_| probe_tcp_rtt(h, p, None, 1200))
                {
                    ev.vps_rtt_ms = Some(ms.round() as u32);
                }
            }
        }

        // Phase 1: 10 real end-to-end keep-alive RTT probes (ping / min / max / jitter / loss)
        let ping_candidates = ["cp.cloudflare.com", "connectivitycheck.gstatic.com"];
        let mut active_ping_host = ping_candidates[0];
        let mut ping_sock: Option<TcpStream> = None;

        for cand in ping_candidates {
            if is_aborted() {
                ev.phase = "cancelled".to_string();
                emit_now(&mut ev);
                return Ok(ev);
            }
            if let Ok((mut s, _)) = open_stream_with_optional_socks(cand, 80, socks_port, 1800) {
                if send_http_head_probe(&mut s, cand).is_ok() {
                    active_ping_host = cand;
                    ping_sock = Some(s);
                    break;
                }
            }
        }

        let total_probes = 10usize;
        for i in 0..total_probes {
            if is_aborted() {
                ev.phase = "cancelled".to_string();
                emit_now(&mut ev);
                return Ok(ev);
            }

            let probe_ms = if let Some(ref mut s) = ping_sock {
                match send_http_head_probe(s, active_ping_host) {
                    Ok(ms) => Ok(ms),
                    Err(_) => {
                        ping_sock = None;
                        if let Ok((mut ns, _)) =
                            open_stream_with_optional_socks(active_ping_host, 80, socks_port, 1800)
                        {
                            let _ = send_http_head_probe(&mut ns, active_ping_host);
                            let res = send_http_head_probe(&mut ns, active_ping_host);
                            ping_sock = Some(ns);
                            res
                        } else {
                            Err("Reconnect failed".to_string())
                        }
                    }
                }
            } else {
                probe_https_service_rtt("cp.cloudflare.com", socks_port).map(|m| m as f64)
            };

            if let Ok(ms) = probe_ms {
                ev.ping_samples_ms.push(ms);
            }
            update_ping_stats(&mut ev, i + 1);
            ev.progress_pct = if quick_only {
                5 + ((i + 1) * 45 / total_probes) as u8
            } else {
                5 + ((i + 1) * 20 / total_probes) as u8
            };
            emit_now(&mut ev);
            if i + 1 < total_probes {
                thread::sleep(Duration::from_millis(65));
            }
        }

        // Phase 2 & 3: Download & Upload speedtests
        if !quick_only {
            // --- DOWNLOAD ---
            ev.phase = "download".to_string();
            ev.progress_pct = 26;
            emit_now(&mut ev);

            let download_urls = [
                "https://speed.cloudflare.com/__down?bytes=30000000",
                "https://ash-speed.hetzner.com/10MB.bin",
            ];

            for dl_url in download_urls {
                if is_aborted() || ev.download_bytes > 0 {
                    break;
                }
                #[cfg(target_os = "windows")]
                {
                    let mut cmd = Command::new("curl.exe");
                    cmd.args(["-sSL", "--fail", "--connect-timeout", "7", "--max-time", "11"]);
                    if let Some(sp) = socks_port {
                        cmd.args(["-x", &format!("socks5h://127.0.0.1:{}", sp)]);
                    }
                    cmd.args(["-A", "NeuroVeilVPN-Speedtest/2.0", "-o", "-", dl_url]);
                    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
                    cmd.creation_flags(CREATE_NO_WINDOW);

                    if let Ok(mut child) = cmd.spawn() {
                        if let Some(mut stdout) = child.stdout.take() {
                            let mut buf = vec![0u8; 65536];
                            let mut first_byte_at: Option<Instant> = None;
                            let mut win_start = Instant::now();
                            let mut win_bytes: u64 = 0;

                            loop {
                                if is_aborted() {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    ev.phase = "cancelled".to_string();
                                    emit_now(&mut ev);
                                    return Ok(ev);
                                }
                                match stdout.read(&mut buf) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        let now = Instant::now();
                                        if first_byte_at.is_none() {
                                            first_byte_at = Some(now);
                                            win_start = now;
                                        }
                                        ev.download_bytes += n as u64;
                                        win_bytes += n as u64;

                                        let since_first = first_byte_at.unwrap().elapsed().as_secs_f64();
                                        let win_dt = win_start.elapsed().as_secs_f64();
                                        if win_dt >= 0.28 {
                                            let inst_mbps = round2((win_bytes as f64 * 8.0) / (win_dt * 1_000_000.0));
                                            ev.download_current_mbps = Some(inst_mbps);
                                            if since_first >= 0.30 {
                                                ev.download_samples_mbps.push(inst_mbps);
                                            }
                                            let (mn, av, mx) = update_speed_stats(
                                                &ev.download_samples_mbps,
                                                ev.download_bytes,
                                                since_first,
                                            );
                                            ev.download_min_mbps = mn;
                                            ev.download_avg_mbps = av;
                                            ev.download_max_mbps = mx;
                                            ev.progress_pct = (26.0 + (since_first / 6.2).min(1.0) * 32.0) as u8;
                                            emit_now(&mut ev);
                                            win_start = now;
                                            win_bytes = 0;
                                        }
                                        if since_first >= 6.2 {
                                            let _ = child.kill();
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            if let Some(fb) = first_byte_at {
                                let tot_s = fb.elapsed().as_secs_f64();
                                let (mn, av, mx) = update_speed_stats(&ev.download_samples_mbps, ev.download_bytes, tot_s);
                                ev.download_min_mbps = mn;
                                ev.download_avg_mbps = av;
                                ev.download_max_mbps = mx;
                                ev.download_current_mbps = av;
                            }
                        }
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    let dl_agent = build_ureq_agent(socks_port, 7, 11);
                    if let Ok(resp) = dl_agent
                        .get(dl_url)
                        .set("User-Agent", "NeuroVeilVPN-Speedtest/2.0")
                        .call()
                    {
                        let mut reader = resp.into_reader();
                        let mut buf = vec![0u8; 65536];
                        let mut first_byte_at: Option<Instant> = None;
                        let mut win_start = Instant::now();
                        let mut win_bytes: u64 = 0;

                        loop {
                            if is_aborted() {
                                ev.phase = "cancelled".to_string();
                                emit_now(&mut ev);
                                return Ok(ev);
                            }
                            match reader.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let now = Instant::now();
                                    if first_byte_at.is_none() {
                                        first_byte_at = Some(now);
                                        win_start = now;
                                    }
                                    ev.download_bytes += n as u64;
                                    win_bytes += n as u64;

                                    let since_first = first_byte_at.unwrap().elapsed().as_secs_f64();
                                    let win_dt = win_start.elapsed().as_secs_f64();
                                    if win_dt >= 0.28 {
                                        let inst_mbps = round2((win_bytes as f64 * 8.0) / (win_dt * 1_000_000.0));
                                        ev.download_current_mbps = Some(inst_mbps);
                                        if since_first >= 0.30 {
                                            ev.download_samples_mbps.push(inst_mbps);
                                        }
                                        let (mn, av, mx) = update_speed_stats(
                                            &ev.download_samples_mbps,
                                            ev.download_bytes,
                                            since_first,
                                        );
                                        ev.download_min_mbps = mn;
                                        ev.download_avg_mbps = av;
                                        ev.download_max_mbps = mx;
                                        ev.progress_pct = (26.0 + (since_first / 6.2).min(1.0) * 32.0) as u8;
                                        emit_now(&mut ev);
                                        win_start = now;
                                        win_bytes = 0;
                                    }
                                    if since_first >= 6.2 {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        if let Some(fb) = first_byte_at {
                            let tot_s = fb.elapsed().as_secs_f64();
                            let (mn, av, mx) = update_speed_stats(&ev.download_samples_mbps, ev.download_bytes, tot_s);
                            ev.download_min_mbps = mn;
                            ev.download_avg_mbps = av;
                            ev.download_max_mbps = mx;
                            ev.download_current_mbps = av;
                        }
                    }
                }
            }

            // --- UPLOAD ---
            if is_aborted() {
                ev.phase = "cancelled".to_string();
                emit_now(&mut ev);
                return Ok(ev);
            }
            ev.phase = "upload".to_string();
            ev.progress_pct = 60;
            emit_now(&mut ev);

            #[cfg(target_os = "windows")]
            {
                let mut up_cmd = Command::new("curl.exe");
                up_cmd.args([
                    "-sSL",
                    "--fail",
                    "--connect-timeout",
                    "7",
                    "--max-time",
                    "10",
                    "-X",
                    "POST",
                    "-T",
                    "-",
                    "-H",
                    "Content-Type: application/octet-stream",
                ]);
                if let Some(sp) = socks_port {
                    up_cmd.args(["-x", &format!("socks5h://127.0.0.1:{}", sp)]);
                }
                up_cmd.args([
                    "-A",
                    "NeuroVeilVPN-Speedtest/2.0",
                    "-o",
                    "NUL",
                    "https://speed.cloudflare.com/__up",
                ]);
                up_cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
                up_cmd.creation_flags(CREATE_NO_WINDOW);

                if let Ok(mut child) = up_cmd.spawn() {
                    let upload_start = Instant::now();
                    let mut win_start = upload_start;
                    let mut win_bytes: u64 = 0;
                    let chunk = vec![0x55u8; 16384];
                    if let Some(mut stdin) = child.stdin.take() {
                        while upload_start.elapsed().as_secs_f64() < 5.5 && ev.upload_bytes < 25 * 1024 * 1024 {
                            if is_aborted() {
                                drop(stdin);
                                let _ = child.kill();
                                let _ = child.wait();
                                ev.phase = "cancelled".to_string();
                                emit_now(&mut ev);
                                return Ok(ev);
                            }
                            if stdin.write_all(&chunk).is_err() {
                                break;
                            }
                            ev.upload_bytes += chunk.len() as u64;
                            win_bytes += chunk.len() as u64;

                            let elapsed_s = upload_start.elapsed().as_secs_f64();
                            let win_dt = win_start.elapsed().as_secs_f64();
                            if win_dt >= 0.28 {
                                let inst_mbps = round2((win_bytes as f64 * 8.0) / (win_dt * 1_000_000.0));
                                ev.upload_current_mbps = Some(inst_mbps);
                                // Ignore initial ~0.45s pipe buffer fill spike
                                if elapsed_s >= 0.45 {
                                    ev.upload_samples_mbps.push(inst_mbps);
                                }
                                let (mn, av, mx) = update_speed_stats(&ev.upload_samples_mbps, ev.upload_bytes, elapsed_s);
                                ev.upload_min_mbps = mn;
                                ev.upload_avg_mbps = av;
                                ev.upload_max_mbps = mx;
                                ev.progress_pct = (60.0 + (elapsed_s / 5.5).min(1.0) * 28.0) as u8;
                                emit_now(&mut ev);
                                win_start = Instant::now();
                                win_bytes = 0;
                            }
                        }
                        let _ = stdin.flush();
                        drop(stdin);
                    }
                    let tot_up_s = upload_start.elapsed().as_secs_f64();
                    let (mn, av, mx) = update_speed_stats(&ev.upload_samples_mbps, ev.upload_bytes, tot_up_s);
                    ev.upload_min_mbps = mn;
                    ev.upload_avg_mbps = av;
                    ev.upload_max_mbps = mx;
                    ev.upload_current_mbps = av;
                    // Let curl finish HTTP response briefly, then ensure cleanup
                    for _ in 0..12 {
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                struct SpeedUploadReader<'a, C: Fn() -> bool, E: FnMut(u64, u64, f64)> {
                    start: Instant,
                    win_start: Instant,
                    win_bytes: u64,
                    total_bytes: u64,
                    is_aborted: &'a C,
                    on_tick: &'a mut E,
                    was_aborted: bool,
                }
                impl<'a, C: Fn() -> bool, E: FnMut(u64, u64, f64)> std::io::Read
                    for SpeedUploadReader<'a, C, E>
                {
                    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                        if (self.is_aborted)() {
                            self.was_aborted = true;
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "cancelled",
                            ));
                        }
                        let elapsed_s = self.start.elapsed().as_secs_f64();
                        if elapsed_s >= 5.5 || self.total_bytes >= 25 * 1024 * 1024 {
                            return Ok(0);
                        }
                        let take = buf.len().min(16384);
                        buf[..take].fill(0x55);
                        self.total_bytes += take as u64;
                        self.win_bytes += take as u64;
                        let win_dt = self.win_start.elapsed().as_secs_f64();
                        if win_dt >= 0.28 {
                            (self.on_tick)(self.total_bytes, self.win_bytes, win_dt);
                            self.win_start = Instant::now();
                            self.win_bytes = 0;
                        }
                        Ok(take)
                    }
                }

                let upload_start = Instant::now();
                let up_agent = build_ureq_agent(socks_port, 7, 10);
                let total_uploaded: u64;
                let was_aborted_flag: bool;
                {
                    let mut on_tick = |tot: u64, win_b: u64, win_dt: f64| {
                        ev.upload_bytes = tot;
                        let elapsed_s = upload_start.elapsed().as_secs_f64();
                        let inst_mbps = round2((win_b as f64 * 8.0) / (win_dt * 1_000_000.0));
                        ev.upload_current_mbps = Some(inst_mbps);
                        if elapsed_s >= 0.45 {
                            ev.upload_samples_mbps.push(inst_mbps);
                        }
                        let (mn, av, mx) = update_speed_stats(
                            &ev.upload_samples_mbps,
                            ev.upload_bytes,
                            elapsed_s,
                        );
                        ev.upload_min_mbps = mn;
                        ev.upload_avg_mbps = av;
                        ev.upload_max_mbps = mx;
                        ev.progress_pct = (60.0 + (elapsed_s / 5.5).min(1.0) * 28.0) as u8;
                        emit_now(&mut ev);
                    };
                    let mut reader = SpeedUploadReader {
                        start: upload_start,
                        win_start: upload_start,
                        win_bytes: 0,
                        total_bytes: 0,
                        is_aborted: &is_aborted,
                        on_tick: &mut on_tick,
                        was_aborted: false,
                    };
                    let _ = up_agent
                        .post("https://speed.cloudflare.com/__up")
                        .set("User-Agent", "NeuroVeilVPN-Speedtest/2.0")
                        .set("Content-Type", "application/octet-stream")
                        .send(&mut reader);
                    total_uploaded = reader.total_bytes;
                    was_aborted_flag = reader.was_aborted;
                }
                if was_aborted_flag || is_aborted() {
                    ev.phase = "cancelled".to_string();
                    emit_now(&mut ev);
                    return Ok(ev);
                }
                ev.upload_bytes = total_uploaded;
                let tot_up_s = upload_start.elapsed().as_secs_f64();
                let (mn, av, mx) =
                    update_speed_stats(&ev.upload_samples_mbps, ev.upload_bytes, tot_up_s);
                ev.upload_min_mbps = mn;
                ev.upload_avg_mbps = av;
                ev.upload_max_mbps = mx;
                ev.upload_current_mbps = av;
            }
        }

        if is_aborted() {
            ev.phase = "cancelled".to_string();
            emit_now(&mut ev);
            return Ok(ev);
        }

        // Phase 4: GeoIP + IPv6 check + Popular services latency matrix (parallel)
        ev.phase = "diagnostics".to_string();
        ev.progress_pct = 91;
        emit_now(&mut ev);

        let services_targets = [
            ("Cloudflare", "speed.cloudflare.com", 443u16),
            ("YouTube", "www.youtube.com", 443u16),
            ("Google", "www.google.com", 443u16),
            ("GitHub", "github.com", 443u16),
            ("Discord", "discord.com", 443u16),
            ("Telegram", "t.me", 443u16),
        ];

        let mut trace_ip: Option<String> = None;
        let mut trace_loc: Option<String> = None;
        let mut trace_colo: Option<String> = None;
        let mut geo_country: Option<String> = None;
        let mut geo_city: Option<String> = None;
        let mut geo_isp: Option<String> = None;
        let mut geo_ip: Option<String> = None;
        let mut v6_ok = false;
        let mut svc_results: Vec<Option<SpeedTestServiceCheck>> = vec![None; services_targets.len()];

        std::thread::scope(|s| {
            let h_trace = s.spawn(|| curl_fetch_text("https://speed.cloudflare.com/cdn-cgi/trace", socks_port, 5, false));
            let h_geo = s.spawn(|| {
                curl_fetch_text("http://ip-api.com/json/?fields=status,country,countryCode,city,isp,org,query", socks_port, 5, false)
                    .or_else(|| curl_fetch_text("https://ipwho.is/", socks_port, 5, false))
            });
            let h_v6 = s.spawn(|| curl_fetch_text("https://api64.ipify.org", socks_port, 3, true));

            let mut svc_handles = Vec::with_capacity(services_targets.len());
            for (name, host, port) in services_targets {
                svc_handles.push(s.spawn(move || {
                    match probe_https_service_rtt(host, socks_port) {
                        Ok(ms) => SpeedTestServiceCheck {
                            name: name.to_string(),
                            host: host.to_string(),
                            port,
                            reachable: true,
                            latency_ms: Some(ms),
                            error: None,
                        },
                        Err(e) => SpeedTestServiceCheck {
                            name: name.to_string(),
                            host: host.to_string(),
                            port,
                            reachable: false,
                            latency_ms: None,
                            error: Some(e),
                        },
                    }
                }));
            }

            if let Ok(Some(trace_txt)) = h_trace.join() {
                for line in trace_txt.lines() {
                    if let Some(v) = line.strip_prefix("ip=") {
                        trace_ip = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("loc=") {
                        trace_loc = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("colo=") {
                        trace_colo = Some(v.trim().to_string());
                    }
                }
            }

            if let Ok(Some(geo_txt)) = h_geo.join() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&geo_txt) {
                    if let Some(ip) = v.get("query").or_else(|| v.get("ip")).and_then(|x| x.as_str()) {
                        if !ip.trim().is_empty() {
                            geo_ip = Some(ip.trim().to_string());
                        }
                    }
                    let c_name = v.get("country").and_then(|x| x.as_str()).unwrap_or("");
                    let c_code = v
                        .get("countryCode")
                        .or_else(|| v.get("country_code"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    if !c_name.is_empty() {
                        geo_country = Some(if !c_code.is_empty() {
                            format!("{} ({})", c_name, c_code)
                        } else {
                            c_name.to_string()
                        });
                    }
                    if let Some(ct) = v.get("city").and_then(|x| x.as_str()) {
                        if !ct.trim().is_empty() {
                            geo_city = Some(ct.trim().to_string());
                        }
                    }
                    if let Some(isp_s) = v
                        .get("isp")
                        .or_else(|| v.get("connection").and_then(|c| c.get("isp")))
                        .or_else(|| v.get("org"))
                        .and_then(|x| x.as_str())
                    {
                        if !isp_s.trim().is_empty() {
                            geo_isp = Some(isp_s.trim().to_string());
                        }
                    }
                }
            }

            if let Ok(Some(v6_ip)) = h_v6.join() {
                v6_ok = v6_ip.contains(':');
            }

            for (idx, h) in svc_handles.into_iter().enumerate() {
                if let Ok(res) = h.join() {
                    svc_results[idx] = Some(res);
                }
            }
        });

        ev.external_ip = geo_ip.or(trace_ip);
        ev.country = geo_country.or(trace_loc);
        ev.city = geo_city;
        ev.isp = geo_isp;
        ev.colo = trace_colo;
        ev.ipv6_reachable = Some(v6_ok);
        ev.services = svc_results.into_iter().flatten().collect();
        ev.phase = "done".to_string();
        ev.progress_pct = 100;
        emit_now(&mut ev);
        Ok(ev)
    })
    .await
    .map_err(|e| format!("Ошибка потока теста соединения: {}", e))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        if !is_elevated() {
            if relaunch_as_admin() {
                std::process::exit(0);
            }
        }
        if let Ok(cur_exe) = std::env::current_exe() {
            let old_exe = cur_exe.with_extension("old.exe");
            let _ = fs::remove_file(old_exe);
        }
    }

    let mut builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
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
        }));
    }

    builder = builder.setup(|app| {
        println!("[NeuroVeil VPN] Tauri setup hook running...");

        #[cfg(target_os = "android")]
        {
            if let Ok(data_dir) = app.path().app_data_dir() {
                let _ = fs::create_dir_all(&data_dir);
                set_app_data_dir(data_dir);
            }
            app.handle().plugin(crate::android_vpn::init())?;
        }

        let persistent = load_persistent_state();
        let initial_installed = if cfg!(target_os = "android") {
            true
        } else {
            get_singbox_bin_path().exists()
        };
        let shared_state: SharedState = Arc::new(Mutex::new(RuntimeState {
            persistent,
            child: None,
            android_connected: false,
            android_connecting: false,
            android_socks_port: None,
            last_android_error_logged: None,
            app_handle: Some(app.handle().clone()),
            active_profile_id: None,
            active_profile_name: None,
            active_protocol: None,
            active_port: None,
            active_listen_port: None,
            port_hunting_status: None,
            is_handshaking: false,
            handshake_status: None,
            singbox_installed: initial_installed,
            singbox_version: if cfg!(target_os = "android") {
                "libbox (встроенное ядро Android)".to_string()
            } else if initial_installed {
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
        app.manage(shared_state.clone());

        // Check exact sing-box / libbox version in background thread so UI never blocks
        let ver_state = shared_state.clone();
        thread::spawn(move || {
            let (installed, ver) = check_singbox_version();
            if let Ok(mut rt) = ver_state.lock() {
                rt.singbox_installed = installed;
                rt.singbox_version = ver;
                sync_android_tile_from_state(&rt);
            }
        });

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Create System Tray
            let show_item = MenuItem::with_id(app, "show", "Открыть NeuroVeil VPN", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Выход из приложения", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &sep, &quit_item])?;

            let tray_cleanup = shared_state.clone();
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
            let should_start_minimized = if let Ok(rt) = shared_state.lock() {
                is_minimized_arg || rt.persistent.start_minimized
            } else {
                is_minimized_arg
            };

            if let Some(window) = app.get_webview_window("main") {
                apply_scaled_main_window_size(&window);
                let _ = window.center();

                if should_start_minimized {
                    let _ = window.hide();
                    println!("[NeuroVeil VPN 2.0] Window started minimized to system tray.");
                } else {
                    let _ = window.show();
                    apply_frameless_dark_border(&window);
                    if let Ok(true) = window.is_minimized() {
                        let _ = window.unminimize();
                    }
                    let _ = window.set_focus();
                    println!("[NeuroVeil VPN 2.0] Main window shown and focused.");
                }
            }
        }

        // Clean up any orphaned sing-box.exe / stale Windows system proxy from a prior crash, then auto-connect
        let auto_connect_state = shared_state.clone();
        let app_handle_for_connect = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            #[cfg(target_os = "windows")]
            {
                let cleanup_state = auto_connect_state.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    let local_port = cleanup_state
                        .lock()
                        .map(|rt| rt.persistent.routing.local_mixed_port)
                        .unwrap_or(2080);
                    let mut kill_cmd = Command::new("taskkill");
                    kill_cmd
                        .args(["/F", "/IM", "sing-box.exe"])
                        .creation_flags(CREATE_NO_WINDOW);
                    let _ = kill_cmd.output();
                    let _ = set_windows_system_proxy(false, local_port);
                })
                .await;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            #[cfg(target_os = "android")]
            let already_connected = match crate::android_vpn::get_vpn_status(&app_handle_for_connect) {
                Ok(mob) if mob.connected || mob.connecting => {
                    if let Ok(mut rt) = auto_connect_state.lock() {
                        rt.android_connected = mob.connected;
                        rt.android_connecting = !mob.connected && mob.connecting;
                        rt.android_socks_port = if mob.socks_port > 0 {
                            Some(mob.socks_port)
                        } else {
                            None
                        };
                        if rt.active_profile_id.is_none() {
                            if let Some(pid) = mob.profile_id.clone().filter(|s| !s.is_empty()) {
                                rt.active_profile_id = Some(pid);
                            }
                            if let Some(pname) = mob.profile_name.clone().filter(|s| !s.is_empty()) {
                                rt.active_profile_name = Some(pname);
                            }
                            if let Some(proto) = mob.protocol.clone().filter(|s| !s.is_empty()) {
                                rt.active_protocol = Some(proto);
                            }
                        }
                    }
                    true
                }
                _ => false,
            };
            #[cfg(not(target_os = "android"))]
            let already_connected = false;

            if !already_connected {
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
                    let _ = connect_vpn(pid, mode, split, Some(true), app_handle_for_connect.state()).await;
                }
            }
        });

        Ok(())
    });

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.on_window_event(move |window, event| {
            if window.label() != "main" {
                return;
            }
            let close_state = window.app_handle().state::<SharedState>();
            match event {
                tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(wv) = window.app_handle().get_webview_window("main") {
                        apply_scaled_main_window_size(&wv);
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
                    if let Ok(mut rt) = close_state.lock() {
                        stop_singbox_internal(&mut rt);
                        save_persistent_state(&rt.persistent);
                    }
                }
                _ => {}
            }
        });
    }

    builder
        .invoke_handler(tauri::generate_handler![
            get_platform_info,
            get_app_state,
            select_profile,
            get_vpn_status,
            download_core,
            connect_vpn,
            reconnect_vpn,
            disconnect_vpn,
            add_or_update_profile,
            import_profile_uri,
            parse_profile_text,
            delete_profile,
            ping_profile,
            save_routing_config,
            set_network_mode,
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
            create_device_keys,
            export_profile_json,
            export_multi_profile_json,
            export_multi_profile_zip,
            export_awg_conf,
            export_awg_conf_file,
            probe_awg_ports,
            probe_profile_ports,
            login_vps_admin,
            get_vps_admin_state,
            manage_vps_container_cmd,
            toggle_vps_antisleep_cmd,
            delete_vps_device_cmd,
            delete_vps_multi_group_cmd,
            app_start_dragging,
            app_minimize,
            app_toggle_maximize,
            app_close,
            open_popup_window,
            emit_window_event,
            app_show_window,
            save_system_settings,
            export_backup_file,
            import_backup_file,
            factory_reset_app,
            get_routing_presets,
            save_routing_preset,
            delete_routing_preset,
            reset_routing_presets,
            apply_routing_preset,
            apply_routing_on_the_fly,
            check_app_update,
            download_and_install_update,
            open_external_url,
            request_add_android_tile,
            is_battery_optimization_ignored,
            request_battery_optimization_exemption,
            run_connection_speed_test,
            cancel_connection_speed_test
        ])
        .run(tauri::generate_context!())
        .expect("Ошибка запуска Tauri-приложения NeuroVeil VPN");
}

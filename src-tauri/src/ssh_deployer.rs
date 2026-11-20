use crate::models::{DeployResult, VpnProfile, VpsAdminState, VpsContainerInfo, VpsCredentials};
use crate::singbox::build_profile_uri;
use russh::client::{self, Handle, KeyboardInteractiveAuthResponse};
use russh::{ChannelMsg, Preferred};
use serde_json::json;
use std::collections::HashSet;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

fn allocate_free_port(preferred: u16, used: &mut HashSet<u16>, is_high_udp: bool) -> u16 {
    if !is_high_udp && preferred > 0 && !used.contains(&preferred) {
        used.insert(preferred);
        return preferred;
    }
    if is_high_udp && preferred >= 1024 && !used.contains(&preferred) {
        used.insert(preferred);
        return preferred;
    }
    if !is_high_udp {
        let mut candidates = Vec::new();
        if preferred > 0 && preferred < 60000 {
            for step in 1u16..=25u16 {
                let c = preferred.saturating_add(step * 1000);
                if c > 1024 && c < 65000 {
                    candidates.push(c);
                }
            }
            for step in 1u16..=50u16 {
                let c = preferred.saturating_add(step);
                if c > 1024 && c < 65000 {
                    candidates.push(c);
                }
            }
        }
        candidates.extend_from_slice(&[2053, 2083, 2087, 2096, 8443, 8444, 9443, 9444, 7443, 6443]);
        for c in candidates {
            if !used.contains(&c) {
                used.insert(c);
                return c;
            }
        }
    }
    for _ in 0..500 {
        let p = random_u32_in_range(20000, 58000) as u16;
        if !used.contains(&p) {
            used.insert(p);
            return p;
        }
    }
    for p in 20000u16..=59000u16 {
        if !used.contains(&p) {
            used.insert(p);
            return p;
        }
    }
    preferred.max(10443)
}

fn allocate_free_subnet_octet(used: &mut HashSet<u8>) -> u8 {
    for octet in 0u8..=250u8 {
        if !used.contains(&octet) {
            used.insert(octet);
            return octet;
        }
    }
    10
}

fn generate_random_base64_32() -> String {
    use base64::Engine;
    let u1 = Uuid::new_v4();
    let u2 = Uuid::new_v4();
    let bytes = [u1.as_bytes().as_slice(), u2.as_bytes().as_slice()].concat();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

fn random_u32_in_range(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    let u = Uuid::new_v4();
    let b = u.as_bytes();
    let n = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
    min + (n % (max - min + 1))
}

#[allow(dead_code)]
fn generate_random_h_values() -> (String, String, String, String) {
    let mut vals = Vec::with_capacity(4);
    while vals.len() < 4 {
        let v = random_u32_in_range(10_000_000, 2_147_483_640);
        if !vals.contains(&v) {
            vals.push(v);
        }
    }
    (vals[0].to_string(), vals[1].to_string(), vals[2].to_string(), vals[3].to_string())
}

struct SshClientHandler;

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

fn emit_log(app: &AppHandle, msg: &str) {
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
    let line = format!("[{}] {}", timestamp, msg);
    let _ = app.emit("vps-log", line);
}

async fn connect_via_socks5(
    proxy_addr: &str,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, String> {
    let mut stream = tokio::time::timeout(Duration::from_secs(7), TcpStream::connect(proxy_addr))
        .await
        .map_err(|_| format!("Таймаут подключения к прокси {}", proxy_addr))?
        .map_err(|e| format!("Не удалось подключиться к прокси {}: {}", proxy_addr, e))?;
    let _ = stream.set_nodelay(true);

    // SOCKS5 greeting: VER=5, NMETHODS=1, METHOD=0 (No auth)
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|e| e.to_string())?;

    let mut reply = [0u8; 2];
    stream
        .read_exact(&mut reply)
        .await
        .map_err(|e| format!("Прокси {} не ответил по SOCKS5: {}", proxy_addr, e))?;

    if reply[0] != 0x05 || reply[1] != 0x00 {
        return Err(format!("Прокси {} отклонил SOCKS5 без авторизации", proxy_addr));
    }

    // SOCKS5 CONNECT request using domain name (ATYP=0x03)
    let host_bytes = target_host.as_bytes();
    let mut req = Vec::with_capacity(7 + host_bytes.len());
    req.push(0x05); // VER
    req.push(0x01); // CMD = CONNECT
    req.push(0x00); // RSV
    req.push(0x03); // ATYP = DOMAINNAME
    req.push(host_bytes.len() as u8);
    req.extend_from_slice(host_bytes);
    req.push((target_port >> 8) as u8);
    req.push((target_port & 0xff) as u8);

    stream.write_all(&req).await.map_err(|e| e.to_string())?;

    let mut hdr = [0u8; 4];
    stream
        .read_exact(&mut hdr)
        .await
        .map_err(|e| format!("Ошибка чтения ответа SOCKS5: {}", e))?;

    if hdr[1] != 0x00 {
        return Err(format!(
            "Прокси не смог соединиться с {}:{} (SOCKS5 код {})",
            target_host, target_port, hdr[1]
        ));
    }

    // Drain bound address
    match hdr[3] {
        0x01 => {
            let mut buf = [0u8; 6];
            let _ = stream.read_exact(&mut buf).await;
        }
        0x03 => {
            let mut len = [0u8; 1];
            let _ = stream.read_exact(&mut len).await;
            let mut buf = vec![0u8; (len[0] as usize) + 2];
            let _ = stream.read_exact(&mut buf).await;
        }
        0x04 => {
            let mut buf = [0u8; 18];
            let _ = stream.read_exact(&mut buf).await;
        }
        _ => {}
    }

    Ok(stream)
}

async fn connect_via_http_connect(
    proxy_addr: &str,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, String> {
    let mut stream = tokio::time::timeout(Duration::from_secs(7), TcpStream::connect(proxy_addr))
        .await
        .map_err(|_| format!("Таймаут подключения к HTTP-прокси {}", proxy_addr))?
        .map_err(|e| format!("Не удалось подключиться к HTTP-прокси {}: {}", proxy_addr, e))?;
    let _ = stream.set_nodelay(true);

    let req = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
        target_host, target_port, target_host, target_port
    );
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; 1024];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Ошибка чтения ответа HTTP CONNECT: {}", e))?;
    let resp = String::from_utf8_lossy(&buf[..n]);
    if !resp.starts_with("HTTP/1.1 200") && !resp.starts_with("HTTP/1.0 200") {
        return Err(format!(
            "HTTP-прокси вернул ошибку: {}",
            resp.lines().next().unwrap_or("Unknown")
        ));
    }

    Ok(stream)
}

async fn detect_active_tunnel_proxy() -> Option<&'static str> {
    if tokio::time::timeout(
        Duration::from_millis(250),
        TcpStream::connect("127.0.0.1:2081"),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
    {
        return Some("socks5://127.0.0.1:2081");
    }
    if tokio::time::timeout(
        Duration::from_millis(250),
        TcpStream::connect("127.0.0.1:2080"),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
    {
        return Some("socks5://127.0.0.1:2080");
    }
    None
}

async fn connect_ssh_endpoint_single(
    app: &AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    proxy_raw: &str,
) -> Result<Handle<SshClientHandler>, String> {
    let clean_host = host.trim();
    let proxy_raw = proxy_raw.trim();

    let tcp_stream = if !proxy_raw.is_empty() {
        let clean_proxy = proxy_raw
            .trim_start_matches("socks5://")
            .trim_start_matches("socks5h://")
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim();

        emit_log(
            app,
            &format!(
                "Подключение к {}:{} через активный туннель ({})...",
                clean_host, port, clean_proxy
            ),
        );

        if proxy_raw.starts_with("http://") {
            connect_via_http_connect(clean_proxy, clean_host, port).await?
        } else {
            match connect_via_socks5(clean_proxy, clean_host, port).await {
                Ok(s) => s,
                Err(_) => connect_via_http_connect(clean_proxy, clean_host, port).await?,
            }
        }
    } else {
        let addr_str = format!("{}:{}", clean_host, port);
        let addrs: Vec<SocketAddr> = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("Не удалось разрешить адрес {}: {}", addr_str, e))?
            .collect();

        let addr = *addrs
            .first()
            .ok_or_else(|| format!("Не найден IP-адрес для {}", addr_str))?;

        let s = tokio::time::timeout(Duration::from_secs(12), TcpStream::connect(addr))
            .await
            .map_err(|_| {
                format!(
                    "Таймаут TCP-соединения с {}. Проверьте IP или блокировку со стороны провайдера.",
                    addr_str
                )
            })?
            .map_err(|e| format!("Ошибка TCP-подключения к {}: {}", addr_str, e))?;
        let _ = s.set_nodelay(true);
        emit_log(
            app,
            &format!(
                "TCP-порт {} открыт. Выполнение SSH-рукопожатия (обмен ключами)...",
                port
            ),
        );
        s
    };

    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(180)),
        preferred: Preferred::DEFAULT,
        ..Default::default()
    });

    let mut session = tokio::time::timeout(
        Duration::from_secs(25),
        client::connect_stream(config, tcp_stream, SshClientHandler),
    )
    .await
    .map_err(|_| {
        format!(
            "Таймаут SSH-рукопожатия с {}:{} (сервер не ответил вовремя). Проверьте доступность SSH или используйте VNC-консоль хостинга.",
            clean_host, port
        )
    })?
    .map_err(|e| format!("Ошибка SSH-рукопожатия с {}: {}", clean_host, e))?;

    let auth_res = session
        .authenticate_password(username.trim(), password)
        .await
        .map_err(|e| format!("Ошибка авторизации SSH: {}", e))?;

    if !auth_res.success() {
        let ki = session
            .authenticate_keyboard_interactive_start(username.trim(), None::<String>)
            .await
            .map_err(|e| format!("Ошибка авторизации SSH: {}", e))?;

        match ki {
            KeyboardInteractiveAuthResponse::Success => {}
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let responses = vec![password.to_string(); prompts.len()];
                let ki2 = session
                    .authenticate_keyboard_interactive_respond(responses)
                    .await
                    .map_err(|e| format!("Ошибка авторизации SSH: {}", e))?;
                if !matches!(ki2, KeyboardInteractiveAuthResponse::Success) {
                    return Err("Неверный логин или пароль SSH".to_string());
                }
            }
            _ => return Err("Неверный логин или пароль SSH".to_string()),
        }
    }

    Ok(session)
}

async fn connect_ssh_endpoint(
    app: &AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    proxy_raw: &str,
) -> Result<Handle<SshClientHandler>, String> {
    match connect_ssh_endpoint_single(app, host, port, username, password, proxy_raw).await {
        Ok(sess) => Ok(sess),
        Err(first_err) => {
            if first_err.contains("Неверный логин или пароль SSH") {
                return Err(first_err);
            }
            if let Some(tunnel_proxy) = detect_active_tunnel_proxy().await {
                if proxy_raw.trim() != tunnel_proxy {
                    emit_log(
                        app,
                        &format!(
                            "Прямое SSH-подключение не удалось ({}). Повторная попытка через активный VPN-туннель...",
                            first_err
                        ),
                    );
                    match connect_ssh_endpoint_single(
                        app,
                        host,
                        port,
                        username,
                        password,
                        tunnel_proxy,
                    )
                    .await
                    {
                        Ok(sess) => {
                            emit_log(
                                app,
                                &format!(
                                    "SSH-соединение с {}:{} успешно установлено через активный VPN-туннель!",
                                    host.trim(),
                                    port
                                ),
                            );
                            return Ok(sess);
                        }
                        Err(tunnel_err) => {
                            if tunnel_proxy == "socks5://127.0.0.1:2081" {
                                if let Ok(sess2) = connect_ssh_endpoint_single(
                                    app,
                                    host,
                                    port,
                                    username,
                                    password,
                                    "socks5://127.0.0.1:2080",
                                )
                                .await
                                {
                                    emit_log(
                                        app,
                                        &format!(
                                            "SSH-соединение с {}:{} успешно установлено через активный VPN-туннель (2080)!",
                                            host.trim(),
                                            port
                                        ),
                                    );
                                    return Ok(sess2);
                                }
                            }
                            emit_log(
                                app,
                                &format!(
                                    "Попытка через активный туннель также не удалась: {}",
                                    tunnel_err
                                ),
                            );
                        }
                    }
                }
            }
            Err(first_err)
        }
    }
}

async fn connect_ssh(
    app: &AppHandle,
    creds: &VpsCredentials,
) -> Result<Handle<SshClientHandler>, String> {
    let host = creds.host.trim();
    let port = creds.ssh_port;
    let proxy_raw = creds.ssh_proxy.trim();

    connect_ssh_endpoint(app, host, port, &creds.username, &creds.password, proxy_raw).await
}

async fn run_remote_cmd(
    session: &Handle<SshClientHandler>,
    cmd: &str,
) -> Result<(u32, String), String> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("Ошибка открытия SSH-канала: {}", e))?;

    channel
        .exec(true, cmd)
        .await
        .map_err(|e| format!("Ошибка отправки SSH-команды: {}", e))?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code = 0u32;

    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => {
                stdout.extend_from_slice(data);
            }
            ChannelMsg::ExtendedData { ref data, ext } => {
                if ext == 1 {
                    stderr.extend_from_slice(data);
                }
            }
            ChannelMsg::ExitStatus { exit_status } => {
                exit_code = exit_status;
            }
            _ => {}
        }
    }

    let out_str = String::from_utf8_lossy(&stdout).trim().to_string();
    let err_str = String::from_utf8_lossy(&stderr).trim().to_string();

    let combined = if err_str.is_empty() {
        out_str
    } else if out_str.is_empty() {
        err_str
    } else {
        format!("{}\n{}", out_str, err_str)
    };

    Ok((exit_code, combined))
}

pub async fn test_single_endpoint(
    app: &AppHandle,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    proxy_raw: &str,
) -> Result<String, String> {
    let clean_host = host.trim();
    if clean_host.is_empty() {
        return Err("Укажите IP-адрес или домен сервера".to_string());
    }
    if password.is_empty() {
        return Err("Укажите пароль root / SSH".to_string());
    }

    emit_log(
        app,
        &format!(
            "Проверка доступности {}:{} (пользователь: {})...",
            clean_host, port, username.trim()
        ),
    );

    let t0 = std::time::Instant::now();
    let sess = connect_ssh_endpoint(app, clean_host, port, username, password, proxy_raw).await?;
    let ping_ms = t0.elapsed().as_millis();

    let (_, info) = run_remote_cmd(
        &sess,
        "uname -srm && (grep PRETTY_NAME /etc/os-release | cut -d= -f2 | tr -d '\"' || true)",
    )
    .await?;

    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "test completed", "en")
        .await;

    let summary = info.lines().collect::<Vec<_>>().join(" • ");
    let result = format!("{} мс • {}", ping_ms, summary);
    emit_log(
        app,
        &format!("Сервер {}:{} доступен! ({})", clean_host, port, result),
    );
    Ok(result)
}

pub async fn test_ssh_connection(app: &AppHandle, creds: &VpsCredentials) -> Result<String, String> {
    emit_log(
        app,
        &format!(
            "Проверка SSH-подключения к {}:{} (пользователь: {})...",
            creds.host, creds.ssh_port, creds.username
        ),
    );

    let sess = connect_ssh(app, creds).await?;
    let (_, info) = run_remote_cmd(
        &sess,
        "uname -srm && (grep PRETTY_NAME /etc/os-release | cut -d= -f2 | tr -d '\"' || true)",
    )
    .await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "check completed", "en")
        .await;
    let summary = info.lines().collect::<Vec<_>>().join(" | ");
    emit_log(app, &format!("Успешное подключение! Система: {}", summary));
    Ok(summary)
}

pub async fn deploy_vps(app: &AppHandle, creds: &VpsCredentials) -> Result<DeployResult, String> {
    let any_proto = creds.deploy_vless
        || creds.deploy_hy2
        || creds.deploy_tuic
        || creds.deploy_ss
        || creds.deploy_awg
        || creds.deploy_awg_quic
        || creds.deploy_awg_vless
        || creds.deploy_awg2
        || creds.deploy_ssh
        || creds.deploy_shadowtls
        || creds.deploy_vless_ws
        || creds.deploy_webrtc;

    if !any_proto {
        return Err("Выберите хотя бы один протокол для развертывания (VLESS Reality, Hysteria 2, TUIC v5, Shadowsocks, Amnezia 3.1, AWG + QUIC Pre-Warm, Гибрид AWG + Reality, AmneziaWG 2.0, ShadowTLS, Cloudflare WS, WebRTC или SSH Tunnel)".to_string());
    }

    let need_vless_server = creds.deploy_vless || creds.deploy_awg_vless;
    let need_awg_server = creds.deploy_awg || creds.deploy_awg_quic || creds.deploy_awg_vless;
    let need_container = need_vless_server
        || need_awg_server
        || creds.deploy_hy2
        || creds.deploy_tuic
        || creds.deploy_ss
        || creds.deploy_awg2
        || creds.deploy_shadowtls
        || creds.deploy_vless_ws
        || creds.deploy_webrtc;

    let mut creds = creds.clone();
    if need_awg_server && (creds.awg_port == 0 || creds.awg_port == 51820) {
        creds.awg_port = random_u32_in_range(20000, 55000) as u16;
        emit_log(app, &format!("Авто-рандомизация: выбран порт Amnezia 3.1 UDP {}", creds.awg_port));
    }
    if creds.deploy_awg2 && (creds.awg2_port == 0 || creds.awg2_port == 51821 || creds.awg2_port == 51820) {
        creds.awg2_port = random_u32_in_range(20000, 55000) as u16;
        emit_log(app, &format!("Авто-рандомизация: выбран порт AmneziaWG 2.0 UDP {}", creds.awg2_port));
    }
    if need_vless_server && creds.vless_port == 0 {
        creds.vless_port = 443;
    }

    let ssh_port = if creds.ssh_port == 0 { 22 } else { creds.ssh_port };
    let container_id = format!("c_{}", &Uuid::new_v4().simple().to_string()[..6]);
    let container_dir = format!("/opt/neuroveil/containers/{}", container_id);

    emit_log(
        app,
        &format!(
            "=== Старт развертывания нового контейнера {} на {} ===",
            container_id, creds.host
        ),
    );
    let sess = connect_ssh(app, &creds).await?;
    emit_log(app, "SSH-соединение установлено успешно.");

    // 1. Check OS and install dependencies
    emit_log(
        app,
        "Шаг 1/6: Проверка зависимостей (curl, openssl, tar, iproute2, iptables)...",
    );
    let prep_cmd = format!(
        r#"
export DEBIAN_FRONTEND=noninteractive
mkdir -p /etc/ssh/sshd_config.d /opt/neuroveil/containers
echo "Port 22" > /etc/ssh/sshd_config.d/99-neuroveil-ports.conf
if [ "{port}" != "22" ] && [ -n "{port}" ]; then
    echo "Port {port}" >> /etc/ssh/sshd_config.d/99-neuroveil-ports.conf
fi
cat << 'SSHEOF' > /etc/ssh/sshd_config.d/99-neuroveil-performance.conf
UseDNS no
MaxStartups 100:30:300
ClientAliveInterval 30
ClientAliveCountMax 5
LoginGraceTime 30
SSHEOF
rm -f /etc/systemd/system/ssh.socket.d/neuroveil-ports.conf
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl reload ssh >/dev/null 2>&1 || systemctl reload sshd >/dev/null 2>&1 || true

if command -v apt-get >/dev/null 2>&1; then
    apt-get update -y >/dev/null 2>&1 || true
    apt-get install -y curl openssl tar ca-certificates iptables iproute2 >/dev/null 2>&1 || true
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y curl openssl tar ca-certificates iptables iproute >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y curl openssl tar ca-certificates iptables iproute >/dev/null 2>&1 || true
fi
uname -m
"#,
        port = ssh_port
    );
    let (_, arch_out) = run_remote_cmd(&sess, &prep_cmd).await?;
    let arch_line = arch_out.lines().last().unwrap_or("x86_64").trim();
    let sb_arch = if arch_line.contains("aarch64") || arch_line.contains("arm64") {
        "arm64"
    } else {
        "amd64"
    };
    emit_log(
        app,
        &format!(
            "Определена архитектура сервера: {} (sing-box {})",
            arch_line, sb_arch
        ),
    );

    if !need_container {
        // SSH-only deployment
        let _ = run_remote_cmd(
            &sess,
            r#"
mkdir -p /etc/ssh/sshd_config.d
cat << 'EOF' > /etc/ssh/sshd_config.d/99-neuroveil-forwarding.conf
AllowTcpForwarding yes
GatewayPorts clientspecified
TCPKeepAlive yes
ClientAliveInterval 30
ClientAliveCountMax 5
EOF
systemctl reload ssh >/dev/null 2>&1 || systemctl reload sshd >/dev/null 2>&1 || true
"#,
        )
        .await;
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "deploy completed", "en")
            .await;

        let connect_host = creds.host.trim().to_string();
        let ssh_name = format!("SSH Tunnel ({}:{})", connect_host, ssh_port);
        let mut ssh_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: ssh_name,
            protocol: "ssh".to_string(),
            server: connect_host,
            port: ssh_port,
            sni: if creds.username.trim().is_empty() {
                "root".to_string()
            } else {
                creds.username.trim().to_string()
            },
            password: creds.password.clone(),
            insecure: false,
            up_mbps: 100,
            down_mbps: 100,
            tls_fragment: Some(false),
            pinned_on_pc: Some(true),
            ..Default::default()
        };
        ssh_profile.uri = build_profile_uri(&ssh_profile);

        emit_log(app, "=== Готово! Профиль SSH Tunnel добавлен в клиент ===");
        return Ok(DeployResult {
            success: true,
            message: "Профиль SSH Tunnel настроен и готов к подключению.".to_string(),
            created_profiles: vec![ssh_profile],
            new_ssh_port: None,
            container_id: None,
        });
    }

    // 2. Install sing-box binary (non-destructive if already installed/running!) & check Docker runtime
    emit_log(
        app,
        "Шаг 2/6: Проверка ядра sing-box и среды контейнеризации Docker...",
    );
    let install_sb_cmd = format!(
        r#"
set -e
mkdir -p "{cdir}" /opt/neuroveil/containers /tmp/sb-install
if [ ! -x /usr/local/bin/sing-box ]; then
    SB_TAG=$(curl -fsSL https://api.github.com/repos/Leadaxe/sing-box-lx/releases/latest | grep '"tag_name":' | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$SB_TAG" ]; then SB_TAG="v1.14.4-lx.32"; fi
    SB_VER="${{SB_TAG#v}}"
    URL="https://github.com/Leadaxe/sing-box-lx/releases/download/${{SB_TAG}}/sing-box-${{SB_VER}}-linux-{arch}.tar.gz"
    if ! curl -fsSL "$URL" -o /tmp/sb-install/sb.tar.gz; then
        SB_VER_UP=$(curl -fsSL https://api.github.com/repos/SagerNet/sing-box/releases/latest | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
        if [ -z "$SB_VER_UP" ]; then SB_VER_UP="1.14.4"; fi
        URL="https://github.com/SagerNet/sing-box/releases/download/v${{SB_VER_UP}}/sing-box-${{SB_VER_UP}}-linux-{arch}.tar.gz"
        curl -fsSL "$URL" -o /tmp/sb-install/sb.tar.gz
    fi
    tar -xzf /tmp/sb-install/sb.tar.gz -C /tmp/sb-install
    mv -f /tmp/sb-install/sing-box-*/sing-box /usr/local/bin/sing-box.tmp
    chmod +x /usr/local/bin/sing-box.tmp
    mv -f /usr/local/bin/sing-box.tmp /usr/local/bin/sing-box
fi
rm -rf /tmp/sb-install

DOCKER_OK=0
if ! command -v docker >/dev/null 2>&1; then
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y docker.io >/dev/null 2>&1 || true
    fi
    if ! command -v docker >/dev/null 2>&1; then
        curl -fsSL https://get.docker.com | sh >/dev/null 2>&1 || true
    fi
fi
if command -v docker >/dev/null 2>&1; then
    systemctl enable --now docker >/dev/null 2>&1 || true
    if docker info >/dev/null 2>&1; then
        if docker image inspect debian:bookworm-slim >/dev/null 2>&1 || docker pull debian:bookworm-slim >/dev/null 2>&1; then
            DOCKER_OK=1
        fi
    fi
fi
echo "SB_VER=$(/usr/local/bin/sing-box version | head -n 1)"
echo "DOCKER_OK=$DOCKER_OK"
"#,
        cdir = container_dir,
        arch = sb_arch
    );
    let (code, sb_ver_out) = run_remote_cmd(&sess, &install_sb_cmd).await?;
    if code != 0 {
        return Err(format!("Ошибка установки sing-box на сервере: {}", sb_ver_out));
    }
    let use_docker = sb_ver_out.lines().any(|l| l.trim() == "DOCKER_OK=1");
    let sb_ver_line = sb_ver_out
        .lines()
        .find(|l| l.starts_with("SB_VER="))
        .map(|l| l.trim_start_matches("SB_VER="))
        .unwrap_or("sing-box");
    emit_log(
        app,
        &format!(
            "Ядро: {} | Среда контейнера: {}",
            sb_ver_line,
            if use_docker {
                format!("Docker (neuroveil-{})", container_id)
            } else {
                format!("Systemd сервис (neuroveil-{})", container_id)
            }
        ),
    );

    // Probe currently occupied ports and 10.8.X.0/24 subnets on VPS so repeated deploys never conflict
    let (_, probe_out) = run_remote_cmd(
        &sess,
        r#"
echo "===PORTS==="
ss -tulnH 2>/dev/null | awk '{print $5}' | rev | cut -d: -f1 | rev | sort -un || true
grep -hoE '"listen_port"\s*:\s*[0-9]+' /etc/sing-box/config.json /opt/neuroveil/containers/*/config.json 2>/dev/null | grep -oE '[0-9]+' || true
echo "===SUBNETS==="
grep -hoE '10\.8\.[0-9]+' /etc/sing-box/config.json /opt/neuroveil/containers/*/config.json 2>/dev/null | sort -u || true
echo "===CONTAINERS==="
ls -1d /opt/neuroveil/containers/*/meta.json /etc/sing-box/config.json 2>/dev/null | wc -l || echo 0
"#,
    )
    .await
    .unwrap_or((0, String::new()));

    let mut used_ports: HashSet<u16> = HashSet::new();
    let mut used_octets: HashSet<u8> = HashSet::new();
    let mut existing_container_count: usize = 0;
    let mut section = "";
    for line in probe_out.lines() {
        let t = line.trim();
        if t == "===PORTS===" {
            section = "ports";
            continue;
        } else if t == "===SUBNETS===" {
            section = "subnets";
            continue;
        } else if t == "===CONTAINERS===" {
            section = "containers";
            continue;
        }
        match section {
            "ports" => {
                if let Ok(p) = t.parse::<u16>() {
                    if p > 0 {
                        used_ports.insert(p);
                    }
                }
            }
            "subnets" => {
                if let Some(oct_str) = t.strip_prefix("10.8.") {
                    if let Ok(o) = oct_str.parse::<u8>() {
                        used_octets.insert(o);
                    }
                }
            }
            "containers" => {
                if let Ok(cnt) = t.parse::<usize>() {
                    existing_container_count = cnt;
                }
            }
            _ => {}
        }
    }

    let is_additional_container = existing_container_count > 0;
    if need_vless_server {
        let orig = creds.vless_port;
        creds.vless_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.vless_port != orig {
            emit_log(app, &format!("Порт {} занят другим контейнером -> для нового контейнера назначен порт VLESS {}", orig, creds.vless_port));
        }
    }
    if creds.deploy_hy2 {
        let orig = creds.hy2_port;
        creds.hy2_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.hy2_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт Hysteria 2: {}", orig, creds.hy2_port));
        }
    }
    if creds.deploy_tuic {
        let orig = creds.tuic_port;
        creds.tuic_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.tuic_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт TUIC: {}", orig, creds.tuic_port));
        }
    }
    if creds.deploy_ss || creds.deploy_shadowtls {
        let orig = creds.ss_port;
        creds.ss_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.ss_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт Shadowsocks: {}", orig, creds.ss_port));
        }
    }
    if creds.deploy_shadowtls {
        let orig = creds.shadowtls_port;
        creds.shadowtls_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.shadowtls_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт ShadowTLS: {}", orig, creds.shadowtls_port));
        }
    }
    if creds.deploy_vless_ws {
        let orig = creds.vless_ws_port;
        creds.vless_ws_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.vless_ws_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт VLESS WS: {}", orig, creds.vless_ws_port));
        }
    }
    if creds.deploy_webrtc {
        let orig = creds.webrtc_port;
        creds.webrtc_port = allocate_free_port(orig, &mut used_ports, false);
        if creds.webrtc_port != orig {
            emit_log(app, &format!("Порт {} занят -> для нового контейнера назначен порт WebRTC: {}", orig, creds.webrtc_port));
        }
    }

    let mut awg1_octet = 0u8;
    if need_awg_server {
        creds.awg_port = allocate_free_port(creds.awg_port, &mut used_ports, true);
        awg1_octet = allocate_free_subnet_octet(&mut used_octets);
        emit_log(app, &format!("Amnezia 3.1 в контейнере {}: UDP {} | подсеть 10.8.{}.0/24", container_id, creds.awg_port, awg1_octet));
    }
    let mut awg2_octet = 1u8;
    if creds.deploy_awg2 {
        creds.awg2_port = allocate_free_port(creds.awg2_port, &mut used_ports, true);
        awg2_octet = allocate_free_subnet_octet(&mut used_octets);
        emit_log(app, &format!("AmneziaWG 2.0 в контейнере {}: UDP {} | подсеть 10.8.{}.0/24", container_id, creds.awg2_port, awg2_octet));
    }

    // 3. Generate TLS Certificate & Keys inside container_dir
    let sni = if creds.sni.trim().is_empty() {
        "www.bing.com".to_string()
    } else {
        creds.sni.trim().to_string()
    };

    let (vless_uuid, vless_priv, vless_pub, vless_sid) = if need_vless_server {
        emit_log(app, "Шаг 3/6: Генерация ключей X25519 для VLESS Reality...");
        let (k_code, k_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate reality-keypair").await?;
        if k_code != 0 {
            return Err(format!("Ошибка генерации ключей Reality: {}", k_out));
        }
        let mut priv_key = String::new();
        let mut pub_key = String::new();
        for line in k_out.lines() {
            let l = line.trim();
            if l.starts_with("PrivateKey:") {
                priv_key = l.trim_start_matches("PrivateKey:").trim().to_string();
            } else if l.starts_with("PublicKey:") {
                pub_key = l.trim_start_matches("PublicKey:").trim().to_string();
            }
        }
        if priv_key.is_empty() || pub_key.is_empty() {
            return Err(format!("Не удалось получить ключи Reality из вывода:\n{}", k_out));
        }
        let (_, sid_out) = run_remote_cmd(&sess, "openssl rand -hex 8").await?;
        let sid = sid_out.trim().lines().last().unwrap_or("0123456789abcdef").trim().to_string();
        let uuid = Uuid::new_v4().to_string();

        let reality_save_cmd = format!(
            r#"cat << 'EOF_REAL' > "{cdir}/reality.json"
{{
  "private_key": "{}",
  "public_key": "{}",
  "short_id": "{}"
}}
EOF_REAL
chmod 600 "{cdir}/reality.json"
"#,
            priv_key, pub_key, sid, cdir = container_dir
        );
        let _ = run_remote_cmd(&sess, &reality_save_cmd).await;
        emit_log(app, &format!("Ключи VLESS Reality сохранены в {}/reality.json.", container_dir));
        (uuid, priv_key, pub_key, sid)
    } else {
        (Uuid::new_v4().to_string(), String::new(), String::new(), String::new())
    };

    let awg1_client_ip = format!("10.8.{}.2/32", awg1_octet);
    let awg1_server_ip = format!("10.8.{}.1/24", awg1_octet);
    let awg1_cidr = format!("10.8.{}.0/24", awg1_octet);

    let (
        awg_server_priv,
        awg_server_pub,
        awg_client_priv,
        awg_client_pub,
        awg_hpk,
        awg_psk,
        awg_jc,
        awg_jmin,
        awg_jmax,
        awg_s1,
        awg_s2,
        awg_s3,
        awg_s4,
        awg_h1,
        awg_h2,
        awg_h3,
        awg_h4,
        awg_rekey_after_time,
        awg_rekey_timeout,
        awg_reject_after_time,
        awg_keepalive_timeout,
        awg_max_handshake_attempts,
        awg_persistent_keepalive,
        awg_random_trailers,
        awg_disable_cookies,
    ) = if need_awg_server {
        emit_log(app, "Шаг 3/6: Генерация ключей и параметров для Amnezia 3.1...");
        let (k1_code, k1_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
        if k1_code != 0 {
            return Err(format!("Ошибка генерации ключей WireGuard (сервер Amnezia 3.1): {}", k1_out));
        }
        let mut s_priv = String::new();
        let mut s_pub = String::new();
        for line in k1_out.lines() {
            let l = line.trim();
            if l.starts_with("PrivateKey:") {
                s_priv = l.trim_start_matches("PrivateKey:").trim().to_string();
            } else if l.starts_with("PublicKey:") {
                s_pub = l.trim_start_matches("PublicKey:").trim().to_string();
            }
        }

        let (k2_code, k2_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
        if k2_code != 0 {
            return Err(format!("Ошибка генерации ключей WireGuard (клиент Amnezia 3.1): {}", k2_out));
        }
        let mut c_priv = String::new();
        let mut c_pub = String::new();
        for line in k2_out.lines() {
            let l = line.trim();
            if l.starts_with("PrivateKey:") {
                c_priv = l.trim_start_matches("PrivateKey:").trim().to_string();
            } else if l.starts_with("PublicKey:") {
                c_pub = l.trim_start_matches("PublicKey:").trim().to_string();
            }
        }

        let hpk = generate_random_base64_32();
        let psk = generate_random_base64_32();
        let jc = creds.awg_jc.unwrap_or_else(|| random_u32_in_range(4, 7) as u16);
        let jmin = creds.awg_jmin.unwrap_or_else(|| random_u32_in_range(10, 25) as u16);
        let jmax = creds.awg_jmax.unwrap_or_else(|| random_u32_in_range(45, 75) as u16);
        let s1 = creds.awg_s1.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 28) as u16);
        let mut s2 = creds.awg_s2.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 28) as u16);
        if s1 + 56 == s2 {
            s2 += 1;
        }
        let s3 = creds.awg_s3.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 28) as u16);
        let s4 = creds.awg_s4.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 28) as u16);
        let h1 = creds.awg_h1.clone().filter(|s| !s.trim().is_empty() && s.trim() != "1").unwrap_or_else(|| random_u32_in_range(100_000_000, 499_999_999).to_string());
        let h2 = creds.awg_h2.clone().filter(|s| !s.trim().is_empty() && s.trim() != "2").unwrap_or_else(|| random_u32_in_range(500_000_000, 999_999_999).to_string());
        let h3 = creds.awg_h3.clone().filter(|s| !s.trim().is_empty() && s.trim() != "3").unwrap_or_else(|| random_u32_in_range(1_000_000_000, 1_499_999_999).to_string());
        let h4 = creds.awg_h4.clone().filter(|s| !s.trim().is_empty() && s.trim() != "4").unwrap_or_else(|| random_u32_in_range(1_500_000_000, 2_140_000_000).to_string());

        let rekey_after = creds.awg_rekey_after_time.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(90, 110), random_u32_in_range(115, 140)));
        let rekey_to = creds.awg_rekey_timeout.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(2, 4), random_u32_in_range(5, 8)));
        let reject_after = creds.awg_reject_after_time.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(140, 160), random_u32_in_range(170, 200)));
        let keepalive_to = creds.awg_keepalive_timeout.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(4, 7), random_u32_in_range(12, 18)));
        let max_handshake = creds.awg_max_handshake_attempts.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(12, 16), random_u32_in_range(18, 24)));
        let persistent_ka = creds.awg_persistent_keepalive.clone().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}-{}", random_u32_in_range(20, 25), random_u32_in_range(30, 40)));

        let disable_cookies = creds.awg_disable_cookies.unwrap_or(true);
        let random_trailers = creds.awg_random_trailers.unwrap_or(true);

        let awg_save_cmd = format!(
            r#"cat << 'EOF_AWG' > "{cdir}/awg.json"
{{
  "server_private_key": "{s_priv}",
  "server_public_key": "{s_pub}",
  "client_private_key": "{c_priv}",
  "client_public_key": "{c_pub}",
  "header_protection_key": "{hpk}",
  "preshared_key": "{psk}",
  "port": {p},
  "hop_ports": "{hop}",
  "jc": {jc},
  "jmin": {jmin},
  "jmax": {jmax},
  "s1": {s1},
  "s2": {s2},
  "s3": {s3},
  "s4": {s4},
  "h1": "{h1}",
  "h2": "{h2}",
  "h3": "{h3}",
  "h4": "{h4}",
  "rekey_after_time": "{rekey_after}",
  "rekey_timeout": "{rekey_to}",
  "reject_after_time": "{reject_after}",
  "keepalive_timeout": "{keepalive_to}",
  "max_handshake_attempts": "{max_handshake}",
  "random_trailers": {random_trailers},
  "disable_cookies": {disable_cookies},
  "persistent_keepalive": "{persistent_ka}",
  "devices": [
    {{
      "name": "default",
      "client_private_key": "{c_priv}",
      "client_public_key": "{c_pub}",
      "preshared_key": "{psk}",
      "ip": "{client_ip}"
    }}
  ]
}}
EOF_AWG
chmod 600 "{cdir}/awg.json"
"#,
            cdir = container_dir,
            s_priv = s_priv,
            s_pub = s_pub,
            c_priv = c_priv,
            c_pub = c_pub,
            hpk = hpk,
            psk = psk,
            p = creds.awg_port,
            hop = creds.awg_hop_ports,
            jc = jc,
            jmin = jmin,
            jmax = jmax,
            s1 = s1,
            s2 = s2,
            s3 = s3,
            s4 = s4,
            h1 = h1,
            h2 = h2,
            h3 = h3,
            h4 = h4,
            rekey_after = rekey_after,
            rekey_to = rekey_to,
            reject_after = reject_after,
            keepalive_to = keepalive_to,
            max_handshake = max_handshake,
            random_trailers = random_trailers,
            disable_cookies = disable_cookies,
            persistent_ka = persistent_ka,
            client_ip = awg1_client_ip
        );
        let _ = run_remote_cmd(&sess, &awg_save_cmd).await?;
        (
            s_priv, s_pub, c_priv, c_pub, hpk, psk,
            jc, jmin, jmax, s1, s2, s3, s4,
            h1, h2, h3, h4,
            rekey_after, rekey_to, reject_after, keepalive_to, max_handshake, persistent_ka,
            random_trailers, disable_cookies,
        )
    } else {
        (
            String::new(), String::new(), String::new(), String::new(), String::new(), String::new(),
            0, 0, 0, 0, 0, 0, 0,
            String::new(), String::new(), String::new(), String::new(),
            String::new(), String::new(), String::new(), String::new(), String::new(), String::new(),
            true, true,
        )
    };

    let awg2_client_ip_32 = format!("10.8.{}.2/32", awg2_octet);
    let awg2_client_ip_24 = format!("10.8.{}.2/24", awg2_octet);
    let awg2_server_ip = format!("10.8.{}.1/24", awg2_octet);
    let awg2_cidr = format!("10.8.{}.0/24", awg2_octet);

    let (
        awg2_server_priv,
        awg2_server_pub,
        awg2_client_priv,
        awg2_client_pub,
        awg2_psk,
    ) = if creds.deploy_awg2 {
        emit_log(app, "Шаг 3/6: Генерация ключей для AmneziaWG 2.0 (чистый WireGuard)...");
        let (k1_code, k1_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
        if k1_code != 0 {
            return Err(format!("Ошибка генерации ключей WireGuard (сервер AmneziaWG 2.0): {}", k1_out));
        }
        let mut s_priv = String::new();
        let mut s_pub = String::new();
        for line in k1_out.lines() {
            let l = line.trim();
            if l.starts_with("PrivateKey:") {
                s_priv = l.trim_start_matches("PrivateKey:").trim().to_string();
            } else if l.starts_with("PublicKey:") {
                s_pub = l.trim_start_matches("PublicKey:").trim().to_string();
            }
        }

        let (k2_code, k2_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
        if k2_code != 0 {
            return Err(format!("Ошибка генерации ключей WireGuard (клиент AmneziaWG 2.0): {}", k2_out));
        }
        let mut c_priv = String::new();
        let mut c_pub = String::new();
        for line in k2_out.lines() {
            let l = line.trim();
            if l.starts_with("PrivateKey:") {
                c_priv = l.trim_start_matches("PrivateKey:").trim().to_string();
            } else if l.starts_with("PublicKey:") {
                c_pub = l.trim_start_matches("PublicKey:").trim().to_string();
            }
        }

        let psk = generate_random_base64_32();

        let awg2_save_cmd = format!(
            r#"cat << 'EOF_AWG2' > "{cdir}/awg2.json"
{{
  "server_private_key": "{s_priv}",
  "server_public_key": "{s_pub}",
  "client_private_key": "{c_priv}",
  "client_public_key": "{c_pub}",
  "preshared_key": "{psk}",
  "port": {p},
  "devices": [
    {{
      "name": "default",
      "client_private_key": "{c_priv}",
      "client_public_key": "{c_pub}",
      "preshared_key": "{psk}",
      "ip": "{client_ip}"
    }}
  ]
}}
EOF_AWG2
chmod 600 "{cdir}/awg2.json"
"#,
            cdir = container_dir,
            s_priv = s_priv,
            s_pub = s_pub,
            c_priv = c_priv,
            c_pub = c_pub,
            psk = psk,
            p = creds.awg2_port,
            client_ip = awg2_client_ip_32
        );
        let _ = run_remote_cmd(&sess, &awg2_save_cmd).await?;
        (s_priv, s_pub, c_priv, c_pub, psk)
    } else {
        (String::new(), String::new(), String::new(), String::new(), String::new())
    };

    if creds.deploy_hy2 || creds.deploy_tuic || creds.deploy_webrtc || creds.deploy_vless_ws {
        emit_log(
            app,
            &format!(
                "Генерация TLS-сертификата (ECDSA prime256v1) для SNI '{}'...",
                sni
            ),
        );
        let cert_cmd = format!(
            r#"
set -e
openssl ecparam -genkey -name prime256v1 -out "{cdir}/key.pem"
openssl req -new -x509 -key "{cdir}/key.pem" -out "{cdir}/cert.pem" -subj "/CN={sni}" -days 3650
chmod 600 "{cdir}/key.pem" "{cdir}/cert.pem"
"#,
            cdir = container_dir,
            sni = sni
        );
        let (cert_code, cert_out) = run_remote_cmd(&sess, &cert_cmd).await?;
        if cert_code != 0 {
            return Err(format!("Ошибка генерации сертификата: {}", cert_out));
        }
        emit_log(app, &format!("TLS-сертификаты созданы в {}/.", container_dir));
    }

    // 4. Optimize Kernel UDP buffers & BBR
    emit_log(
        app,
        "Шаг 4/6: Оптимизация ядра Linux (TCP BBR + увеличение буферов)...",
    );
    let sysctl_cmd = r#"
cat << 'EOF' > /etc/sysctl.d/99-neuroveil-vpn.conf
net.ipv4.ip_forward = 1
net.core.rmem_max = 26214400
net.core.wmem_max = 26214400
net.core.rmem_default = 26214400
net.core.wmem_default = 26214400
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
EOF
sysctl --system >/dev/null 2>&1 || true
mkdir -p /etc/ssh/sshd_config.d
cat << 'EOF' > /etc/ssh/sshd_config.d/99-neuroveil-forwarding.conf
AllowTcpForwarding yes
GatewayPorts clientspecified
TCPKeepAlive yes
ClientAliveInterval 30
ClientAliveCountMax 5
EOF
systemctl reload ssh >/dev/null 2>&1 || systemctl reload sshd >/dev/null 2>&1 || true
"#;
    let _ = run_remote_cmd(&sess, sysctl_cmd).await;

    // 5. Construct container config.json
    emit_log(
        app,
        &format!("Шаг 5/6: Формирование конфигурации контейнера {}...", container_id),
    );
    let cfg_cert_path = if use_docker {
        "/etc/sing-box/cert.pem".to_string()
    } else {
        format!("{}/cert.pem", container_dir)
    };
    let cfg_key_path = if use_docker {
        "/etc/sing-box/key.pem".to_string()
    } else {
        format!("{}/key.pem", container_dir)
    };

    let hy2_password = Uuid::new_v4().simple().to_string();
    let hy2_obfs = if creds.enable_salamander {
        Some(Uuid::new_v4().simple().to_string())
    } else {
        None
    };
    let tuic_uuid = Uuid::new_v4().to_string();
    let tuic_password = Uuid::new_v4().simple().to_string();
    let ss_password = if creds.ss_method.starts_with("2022-blake3-aes-128") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(Uuid::new_v4().as_bytes())
    } else if creds.ss_method.starts_with("2022-blake3") {
        use base64::Engine;
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();
        let bytes = [u1.as_bytes().as_slice(), u2.as_bytes().as_slice()].concat();
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    } else {
        Uuid::new_v4().simple().to_string()
    };

    let mut inbounds = Vec::new();
    let mut fw_rules = String::new();
    let mut proto_labels: Vec<String> = Vec::new();
    let mut proto_ports: Vec<u16> = Vec::new();

    if need_vless_server {
        proto_labels.push("VLESS".to_string());
        proto_ports.push(creds.vless_port);
        let reality_sni = if creds.reality_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.reality_sni.trim().to_string()
        };
        let vless_in = json!({
            "type": "vless",
            "tag": "vless-in",
            "listen": "::",
            "listen_port": creds.vless_port,
            "users": [
                {
                    "name": "neuroveil-user",
                    "uuid": vless_uuid,
                    "flow": "xtls-rprx-vision"
                }
            ],
            "tls": {
                "enabled": true,
                "server_name": reality_sni,
                "reality": {
                    "enabled": true,
                    "handshake": {
                        "server": reality_sni,
                        "server_port": 443
                    },
                    "private_key": vless_priv,
                    "short_id": [vless_sid]
                }
            }
        });
        inbounds.push(vless_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.vless_port
        ));
    }

    if creds.deploy_hy2 {
        proto_labels.push("Hysteria 2".to_string());
        proto_ports.push(creds.hy2_port);
        let mut hy2_in = json!({
            "type": "hysteria2",
            "tag": "hy2-in",
            "listen": "::",
            "listen_port": creds.hy2_port,
            "users": [
                {
                    "name": "neuroveil-user",
                    "password": hy2_password
                }
            ],
            "masquerade": format!("https://{}", sni),
            "tls": {
                "enabled": true,
                "server_name": sni,
                "alpn": ["h3"],
                "certificate_path": cfg_cert_path.clone(),
                "key_path": cfg_key_path.clone()
            }
        });
        if let Some(ref obfs_pw) = hy2_obfs {
            hy2_in["obfs"] = json!({
                "type": "salamander",
                "password": obfs_pw
            });
        }
        inbounds.push(hy2_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.hy2_port
        ));
        if creds.enable_port_hopping && !is_additional_container {
            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow 20000:65000/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port=20000-65000/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport 20000:65000 -j REDIRECT --to-ports {p} 2>/dev/null || iptables -t nat -A PREROUTING -p udp --dport 20000:65000 -j REDIRECT --to-ports {p} >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport 20000:65000 -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport 20000:65000 -j ACCEPT >/dev/null 2>&1 || true\n",
                p = creds.hy2_port
            ));
        }
    }

    if creds.deploy_tuic {
        proto_labels.push("TUIC v5".to_string());
        proto_ports.push(creds.tuic_port);
        let tuic_in = json!({
            "type": "tuic",
            "tag": "tuic-in",
            "listen": "::",
            "listen_port": creds.tuic_port,
            "users": [
                {
                    "name": "neuroveil-user",
                    "uuid": tuic_uuid,
                    "password": tuic_password
                }
            ],
            "congestion_control": "bbr",
            "tls": {
                "enabled": true,
                "server_name": sni,
                "alpn": ["h3"],
                "certificate_path": cfg_cert_path.clone(),
                "key_path": cfg_key_path.clone()
            }
        });
        inbounds.push(tuic_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.tuic_port
        ));
    }

    if creds.deploy_ss || creds.deploy_shadowtls {
        let ss_listen = if creds.deploy_ss { "::" } else { "127.0.0.1" };
        if creds.deploy_ss {
            proto_labels.push("Shadowsocks".to_string());
            proto_ports.push(creds.ss_port);
        }
        let ss_in = json!({
            "type": "shadowsocks",
            "tag": "ss-in",
            "listen": ss_listen,
            "listen_port": creds.ss_port,
            "method": creds.ss_method,
            "password": ss_password.clone(),
            "multiplex": {
                "enabled": true
            }
        });
        inbounds.push(ss_in);
        if creds.deploy_ss {
            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = creds.ss_port
            ));
        }
    }

    let shadowtls_pass = Uuid::new_v4().simple().to_string();
    if creds.deploy_shadowtls {
        proto_labels.push("ShadowTLS v3".to_string());
        proto_ports.push(creds.shadowtls_port);
        let st_sni = if creds.shadowtls_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.shadowtls_sni.trim().to_string()
        };
        let st_in = json!({
            "type": "shadowtls",
            "tag": "shadowtls-in",
            "listen": "::",
            "listen_port": creds.shadowtls_port,
            "version": 3,
            "users": [
                {
                    "name": "neuroveil-user",
                    "password": shadowtls_pass
                }
            ],
            "handshake": {
                "server": st_sni,
                "server_port": 443
            },
            "detour": "ss-in"
        });
        inbounds.push(st_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.shadowtls_port
        ));
    }

    if creds.deploy_vless_ws {
        proto_labels.push("VLESS WS".to_string());
        proto_ports.push(creds.vless_ws_port);
        let vless_ws_in = json!({
            "type": "vless",
            "tag": "vless-ws-in",
            "listen": "::",
            "listen_port": creds.vless_ws_port,
            "users": [
                {
                    "name": "neuroveil-ws-user",
                    "uuid": vless_uuid.clone()
                }
            ],
            "transport": {
                "type": "ws",
                "path": "/neuroveil-ws"
            },
            "tls": {
                "enabled": true,
                "server_name": sni.clone(),
                "alpn": ["http/1.1"],
                "certificate_path": cfg_cert_path.clone(),
                "key_path": cfg_key_path.clone()
            }
        });
        inbounds.push(vless_ws_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.vless_ws_port
        ));
    }

    if creds.deploy_webrtc {
        proto_labels.push("WebRTC".to_string());
        proto_ports.push(creds.webrtc_port);
        let webrtc_in = json!({
            "type": "vless",
            "tag": "webrtc-in",
            "listen": "::",
            "listen_port": creds.webrtc_port,
            "users": [
                {
                    "name": "neuroveil-webrtc-user",
                    "uuid": vless_uuid.clone()
                }
            ],
            "transport": {
                "type": "grpc",
                "service_name": "neuroveil-webrtc"
            },
            "tls": {
                "enabled": true,
                "server_name": sni.clone(),
                "alpn": ["h2"],
                "certificate_path": cfg_cert_path.clone(),
                "key_path": cfg_key_path.clone()
            }
        });
        inbounds.push(webrtc_in);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
            p = creds.webrtc_port
        ));
    }

    let mut endpoints = Vec::new();
    let mut route_rules = Vec::new();

    if need_awg_server {
        proto_labels.push("Amnezia 3.1".to_string());
        proto_ports.push(creds.awg_port);
        let mut awg_ep = json!({
            "type": "wireguard",
            "tag": "awg-in",
            "listen_port": creds.awg_port,
            "address": [awg1_server_ip],
            "private_key": awg_server_priv,
            "header_protection_key": awg_hpk,
            "random_trailers": awg_random_trailers,
            "disable_cookies": awg_disable_cookies,
            "rekey_after_time": awg_rekey_after_time,
            "rekey_timeout": awg_rekey_timeout,
            "reject_after_time": awg_reject_after_time,
            "keepalive_timeout": awg_keepalive_timeout,
            "max_handshake_attempts": awg_max_handshake_attempts,
            "jc": awg_jc,
            "jmin": awg_jmin,
            "jmax": awg_jmax,
            "s1": awg_s1,
            "s2": awg_s2,
            "s3": awg_s3,
            "s4": awg_s4,
            "peers": [
                {
                    "public_key": awg_client_pub,
                    "pre_shared_key": awg_psk,
                    "allowed_ips": [awg1_client_ip]
                }
            ]
        });
        if let Ok(num) = awg_h1.trim().parse::<u32>() { awg_ep["h1"] = json!(num); } else { awg_ep["h1"] = json!(awg_h1); }
        if let Ok(num) = awg_h2.trim().parse::<u32>() { awg_ep["h2"] = json!(num); } else { awg_ep["h2"] = json!(awg_h2); }
        if let Ok(num) = awg_h3.trim().parse::<u32>() { awg_ep["h3"] = json!(num); } else { awg_ep["h3"] = json!(awg_h3); }
        if let Ok(num) = awg_h4.trim().parse::<u32>() { awg_ep["h4"] = json!(num); } else { awg_ep["h4"] = json!(awg_h4); }
        endpoints.push(awg_ep);

        route_rules.push(json!({
            "inbound": ["awg-in"],
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "source_ip_cidr": [awg1_cidr],
            "outbound": "direct"
        }));

        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -s {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -d {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -C POSTROUTING -s {cidr} -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s {cidr} -j MASQUERADE >/dev/null 2>&1 || true\n",
            p = creds.awg_port,
            cidr = awg1_cidr
        ));
    }

    if creds.deploy_awg2 {
        proto_labels.push("AWG 2.0".to_string());
        proto_ports.push(creds.awg2_port);
        let awg2_ep = json!({
            "type": "wireguard",
            "tag": "awg2-in",
            "listen_port": creds.awg2_port,
            "address": [awg2_server_ip],
            "private_key": awg2_server_priv,
            "mtu": 1420,
            "peers": [
                {
                    "public_key": awg2_client_pub,
                    "pre_shared_key": awg2_psk,
                    "allowed_ips": [awg2_client_ip_32]
                }
            ]
        });
        endpoints.push(awg2_ep);

        route_rules.push(json!({
            "inbound": ["awg2-in"],
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "source_ip_cidr": [awg2_cidr],
            "outbound": "direct"
        }));

        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -s {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -d {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -C POSTROUTING -s {cidr} -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s {cidr} -j MASQUERADE >/dev/null 2>&1 || true\n",
            p = creds.awg2_port,
            cidr = awg2_cidr
        ));
    }

    // Exempt this container's dedicated UDP ports from any port-hopping PREROUTING REDIRECT rules
    let mut active_udp_ports: Vec<u16> = Vec::new();
    if creds.deploy_hy2 && creds.hy2_port > 0 { active_udp_ports.push(creds.hy2_port); }
    if creds.deploy_tuic && creds.tuic_port > 0 { active_udp_ports.push(creds.tuic_port); }
    if creds.deploy_ss && creds.ss_port > 0 { active_udp_ports.push(creds.ss_port); }
    if need_awg_server && creds.awg_port > 0 { active_udp_ports.push(creds.awg_port); }
    if creds.deploy_awg2 && creds.awg2_port > 0 { active_udp_ports.push(creds.awg2_port); }
    for udp_p in active_udp_ports {
        fw_rules.push_str(&format!(
            "iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n",
            p = udp_p
        ));
    }

    let mut server_config = json!({
        "log": {
            "level": "info",
            "timestamp": true
        },
        "inbounds": inbounds,
        "outbounds": [
            {
                "type": "direct",
                "tag": "direct"
            }
        ]
    });
    if !endpoints.is_empty() {
        server_config["endpoints"] = json!(endpoints);
    }
    if !route_rules.is_empty() {
        server_config["route"] = json!({
            "rules": route_rules,
            "final": "direct"
        });
    }

    let config_str = serde_json::to_string_pretty(&server_config).unwrap();
    let runtime_name = if use_docker { "docker" } else { "systemd" };
    let container_display_name = format!(
        "Контейнер #{} ({})",
        existing_container_count + 1,
        proto_labels.join(", ")
    );
    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let meta_json = json!({
        "id": container_id,
        "name": container_display_name,
        "runtime": runtime_name,
        "docker_name": format!("neuroveil-{}", container_id),
        "service_name": format!("neuroveil-{}", container_id),
        "created_at": created_at,
        "protocols": proto_labels,
        "ports": proto_ports,
        "enable_port_hopping": creds.enable_port_hopping && !is_additional_container
    });
    let meta_str = serde_json::to_string_pretty(&meta_json).unwrap();

    let start_container_block = if use_docker {
        format!(
            r#"
docker rm -f "neuroveil-{cid}" >/dev/null 2>&1 || true
TUN_FLAG=""
if [ -e /dev/net/tun ]; then TUN_FLAG="--device=/dev/net/tun"; fi
docker run -d \
  --name "neuroveil-{cid}" \
  --restart=always \
  --network=host \
  --cap-add=NET_ADMIN \
  $TUN_FLAG \
  -e ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true \
  -v /usr/local/bin/sing-box:/usr/local/bin/sing-box:ro \
  -v "{cdir}":/etc/sing-box \
  debian:bookworm-slim \
  /usr/local/bin/sing-box run -c /etc/sing-box/config.json
sleep 1
docker inspect -f '{{{{.State.Running}}}}' "neuroveil-{cid}"
"#,
            cid = container_id,
            cdir = container_dir
        )
    } else {
        format!(
            r#"
cat << 'EOF_SVC' > "/etc/systemd/system/neuroveil-{cid}.service"
[Unit]
Description=NeuroVeil VPN Container {cid}
After=network.target nss-lookup.target

[Service]
Type=simple
Environment=ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true
ExecStart=/usr/local/bin/sing-box run -c {cdir}/config.json
Restart=on-failure
RestartSec=3
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF_SVC
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable "neuroveil-{cid}" >/dev/null 2>&1
systemctl restart "neuroveil-{cid}"
sleep 1
systemctl is-active "neuroveil-{cid}"
"#,
            cid = container_id,
            cdir = container_dir
        )
    };

    let service_and_fw_cmd = format!(
        r#"
mkdir -p "{cdir}"
cat << 'EOF_CFG' > "{cdir}/config.json"
{config_str}
EOF_CFG
cat << 'EOF_META' > "{cdir}/meta.json"
{meta_str}
EOF_META

if command -v ufw >/dev/null 2>&1; then
    ufw allow 22/tcp >/dev/null 2>&1 || true
    if [ "{port}" != "22" ] && [ -n "{port}" ]; then ufw allow {port}/tcp >/dev/null 2>&1 || true; fi
fi
iptables -C INPUT -p tcp --dport 22 -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport 22 -j ACCEPT >/dev/null 2>&1 || true
if [ "{port}" != "22" ] && [ -n "{port}" ]; then iptables -C INPUT -p tcp --dport {port} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {port} -j ACCEPT >/dev/null 2>&1 || true; fi

{fw_rules}

if command -v firewall-cmd >/dev/null 2>&1; then
    firewall-cmd --reload >/dev/null 2>&1 || true
fi
if command -v netfilter-persistent >/dev/null 2>&1; then
    netfilter-persistent save >/dev/null 2>&1 || true
fi
mkdir -p /etc/iptables
iptables-save > /etc/iptables/rules.v4 2>/dev/null || true

{start_container_block}
"#,
        cdir = container_dir,
        config_str = config_str,
        meta_str = meta_str,
        port = ssh_port,
        fw_rules = fw_rules,
        start_container_block = start_container_block
    );

    emit_log(
        app,
        &format!(
            "Шаг 6/6: Применение правил фаервола и запуск изолированного контейнера {}...",
            container_id
        ),
    );
    let (svc_code, svc_out) = run_remote_cmd(&sess, &service_and_fw_cmd).await?;
    let started_ok = if use_docker {
        svc_code == 0 && svc_out.lines().any(|l| l.trim() == "true")
    } else {
        svc_code == 0 && svc_out.contains("active")
    };
    if !started_ok {
        let diag_cmd = if use_docker {
            format!("docker logs --tail 25 neuroveil-{}", container_id)
        } else {
            format!("journalctl -u neuroveil-{} -n 25 --no-pager", container_id)
        };
        let (_, journal) = run_remote_cmd(&sess, &diag_cmd).await.unwrap_or((0, String::new()));
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "error", "en")
            .await;
        return Err(format!(
            "Контейнер neuroveil-{} не запустился ({}):\n{}",
            container_id, svc_out, journal
        ));
    }
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "deploy completed", "en")
        .await;

    let connect_host = creds.host.trim().to_string();
    let format_prof_name = |label: &str, p: u16| -> String {
        if is_additional_container {
            format!("{} ({}:{})", label, connect_host, p)
        } else {
            format!("{} ({})", label, connect_host)
        }
    };

    let mut created_profiles = Vec::new();

    if creds.deploy_vless {
        let reality_sni = if creds.reality_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.reality_sni.trim().to_string()
        };
        let mut vless_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("VLESS Reality", creds.vless_port),
            protocol: "vless".to_string(),
            server: connect_host.clone(),
            port: creds.vless_port,
            sni: reality_sni,
            insecure: false,
            password: vless_uuid.clone(),
            uuid: Some(vless_uuid.clone()),
            up_mbps: 100,
            down_mbps: 100,
            reality_public_key: Some(vless_pub.clone()),
            reality_short_id: Some(vless_sid.clone()),
            reality_flow: Some("xtls-rprx-vision".to_string()),
            tls_fragment: Some(true),
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        vless_profile.uri = build_profile_uri(&vless_profile);
        created_profiles.push(vless_profile);
    }

    if creds.deploy_hy2 {
        let mut hy2_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("Hysteria 2", creds.hy2_port),
            protocol: "hysteria2".to_string(),
            server: connect_host.clone(),
            port: creds.hy2_port,
            sni: sni.clone(),
            insecure: true,
            password: hy2_password,
            obfs_password: hy2_obfs,
            up_mbps: 100,
            down_mbps: 100,
            hop_ports: if creds.enable_port_hopping && !is_additional_container {
                Some("20000:65000".to_string())
            } else {
                None
            },
            hop_interval: if creds.enable_port_hopping && !is_additional_container {
                Some("30s".to_string())
            } else {
                None
            },
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        hy2_profile.uri = build_profile_uri(&hy2_profile);
        created_profiles.push(hy2_profile);
    }

    if creds.deploy_tuic {
        let mut tuic_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("TUIC v5", creds.tuic_port),
            protocol: "tuic".to_string(),
            server: connect_host.clone(),
            port: creds.tuic_port,
            sni: sni.clone(),
            insecure: true,
            password: tuic_password,
            uuid: Some(tuic_uuid),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        tuic_profile.uri = build_profile_uri(&tuic_profile);
        created_profiles.push(tuic_profile);
    }

    if creds.deploy_ss {
        let mut ss_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("Shadowsocks", creds.ss_port),
            protocol: "shadowsocks".to_string(),
            server: connect_host.clone(),
            port: creds.ss_port,
            insecure: false,
            password: ss_password.clone(),
            up_mbps: 100,
            down_mbps: 100,
            ss_method: Some(creds.ss_method.clone()),
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        ss_profile.uri = build_profile_uri(&ss_profile);
        created_profiles.push(ss_profile);
    }

    if need_awg_server {
        let mut base_awg_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("Amnezia 3.1", creds.awg_port),
            protocol: "amneziawg".to_string(),
            server: connect_host.clone(),
            port: creds.awg_port,
            password: awg_client_priv.clone(),
            awg_private_key: Some(awg_client_priv),
            awg_public_key: Some(awg_server_pub),
            awg_preshared_key: Some(awg_psk),
            awg_local_address: Some(awg1_client_ip),
            awg_version: Some("3.1".to_string()),
            awg_header_protection_key: Some(awg_hpk),
            awg_jc: Some(awg_jc),
            awg_jmin: Some(awg_jmin),
            awg_jmax: Some(awg_jmax),
            awg_s1: Some(awg_s1),
            awg_s2: Some(awg_s2),
            awg_s3: Some(awg_s3),
            awg_s4: Some(awg_s4),
            awg_h1: Some(awg_h1),
            awg_h2: Some(awg_h2),
            awg_h3: Some(awg_h3),
            awg_h4: Some(awg_h4),
            awg_rekey_after_time: Some(awg_rekey_after_time),
            awg_rekey_timeout: Some(awg_rekey_timeout),
            awg_reject_after_time: Some(awg_reject_after_time),
            awg_keepalive_timeout: Some(awg_keepalive_timeout),
            awg_max_handshake_attempts: Some(awg_max_handshake_attempts),
            awg_random_trailers: Some(awg_random_trailers),
            awg_disable_cookies: Some(awg_disable_cookies),
            awg_persistent_keepalive: Some(awg_persistent_keepalive),
            awg_hop_ports: Some(creds.awg_hop_ports.clone()),
            awg_port_probing: Some(creds.awg_port_probing.unwrap_or(true)),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        base_awg_profile.uri = build_profile_uri(&base_awg_profile);

        if creds.deploy_awg {
            created_profiles.push(base_awg_profile.clone());
        }

        if creds.deploy_awg_quic {
            let mut quic_profile = base_awg_profile.clone();
            quic_profile.id = Uuid::new_v4().to_string();
            quic_profile.name = format_prof_name("AWG + QUIC Pre-Warm", creds.awg_port);
            quic_profile.protocol = "awg_quic".to_string();
            quic_profile.uri = build_profile_uri(&quic_profile);
            created_profiles.push(quic_profile);
        }

        if creds.deploy_awg_vless {
            let reality_sni = if creds.reality_sni.trim().is_empty() {
                "www.microsoft.com".to_string()
            } else {
                creds.reality_sni.trim().to_string()
            };
            let mut hybrid_profile = base_awg_profile.clone();
            hybrid_profile.id = Uuid::new_v4().to_string();
            hybrid_profile.name = format_prof_name("Hybrid AWG + Reality", creds.awg_port);
            hybrid_profile.protocol = "awg_vless".to_string();
            hybrid_profile.vless_port = Some(creds.vless_port);
            hybrid_profile.uuid = Some(vless_uuid.clone());
            hybrid_profile.reality_public_key = Some(vless_pub.clone());
            hybrid_profile.reality_short_id = Some(vless_sid.clone());
            hybrid_profile.reality_flow = Some("xtls-rprx-vision".to_string());
            hybrid_profile.sni = reality_sni;
            hybrid_profile.uri = build_profile_uri(&hybrid_profile);
            created_profiles.push(hybrid_profile);
        }
    }

    if creds.deploy_awg2 {
        let mut awg2_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("AmneziaWG 2.0", creds.awg2_port),
            protocol: "awg2".to_string(),
            server: connect_host.clone(),
            port: creds.awg2_port,
            password: awg2_client_priv.clone(),
            awg_private_key: Some(awg2_client_priv),
            awg_public_key: Some(awg2_server_pub),
            awg_preshared_key: Some(awg2_psk),
            awg_local_address: Some(awg2_client_ip_24),
            awg_version: Some("2.0".to_string()),
            awg_port_probing: Some(false),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        awg2_profile.uri = build_profile_uri(&awg2_profile);
        created_profiles.push(awg2_profile);
    }

    if creds.deploy_ssh {
        let ssh_name = format!("SSH Tunnel ({}:{})", creds.host.trim(), ssh_port);
        let mut ssh_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: ssh_name,
            protocol: "ssh".to_string(),
            server: connect_host.clone(),
            port: ssh_port,
            sni: if creds.username.trim().is_empty() {
                "root".to_string()
            } else {
                creds.username.trim().to_string()
            },
            password: creds.password.clone(),
            insecure: false,
            up_mbps: 100,
            down_mbps: 100,
            tls_fragment: Some(false),
            pinned_on_pc: Some(true),
            ..Default::default()
        };
        ssh_profile.uri = build_profile_uri(&ssh_profile);
        created_profiles.push(ssh_profile);
    }

    if creds.deploy_shadowtls {
        let st_sni = if creds.shadowtls_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.shadowtls_sni.trim().to_string()
        };
        let mut st_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("ShadowTLS v3", creds.shadowtls_port),
            protocol: "shadowtls".to_string(),
            server: connect_host.clone(),
            port: creds.shadowtls_port,
            password: ss_password.clone(),
            shadowtls_password: Some(shadowtls_pass),
            shadowtls_sni: Some(st_sni),
            ss_method: Some(creds.ss_method.clone()),
            up_mbps: 100,
            down_mbps: 100,
            tls_fragment: Some(false),
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        st_profile.uri = build_profile_uri(&st_profile);
        created_profiles.push(st_profile);
    }

    if creds.deploy_vless_ws {
        let cdn_host = creds.cdn_domain.clone().filter(|d| !d.trim().is_empty()).unwrap_or_else(|| connect_host.clone());
        let mut cdn_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("Cloudflare CDN WS", creds.vless_ws_port),
            protocol: "vless_ws".to_string(),
            server: connect_host.clone(),
            port: creds.vless_ws_port,
            insecure: true,
            uuid: Some(vless_uuid.clone()),
            password: vless_uuid.clone(),
            ws_path: Some("/neuroveil-ws".to_string()),
            cdn_host: Some(cdn_host),
            sni: sni.clone(),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        cdn_profile.uri = build_profile_uri(&cdn_profile);
        created_profiles.push(cdn_profile);
    }

    if creds.deploy_webrtc {
        let mut webrtc_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("WebRTC Steganography", creds.webrtc_port),
            protocol: "webrtc".to_string(),
            server: connect_host.clone(),
            port: creds.webrtc_port,
            insecure: true,
            uuid: Some(vless_uuid.clone()),
            password: vless_uuid.clone(),
            ws_path: Some("neuroveil-webrtc".to_string()),
            sni: sni.clone(),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        webrtc_profile.uri = build_profile_uri(&webrtc_profile);
        created_profiles.push(webrtc_profile);
    }

    emit_log(
        app,
        &format!(
            "=== Готово! Контейнер {} развернут ({}), профили добавлены в клиент ===",
            container_id, runtime_name
        ),
    );

    Ok(DeployResult {
        success: true,
        message: format!(
            "Контейнер {} успешно создан! Профили готовы к подключению.",
            container_id
        ),
        created_profiles,
        new_ssh_port: None,
        container_id: Some(container_id),
    })
}

#[derive(Clone, Debug)]
struct ResolvedContainer {
    id: String,
    cdir: String,
    runtime: String,
    unit_name: String,
    enable_port_hopping: bool,
}

async fn resolve_container_target(
    sess: &Handle<SshClientHandler>,
    requested_id: Option<&str>,
    port_hint: Option<u16>,
) -> Result<ResolvedContainer, String> {
    let req = requested_id.map(|s| s.trim()).filter(|s| !s.is_empty());

    let inspect_one = |cid: &str, raw_meta: &str| -> ResolvedContainer {
        let meta = serde_json::from_str::<serde_json::Value>(raw_meta).ok();
        let runtime = meta
            .as_ref()
            .and_then(|m| m["runtime"].as_str())
            .unwrap_or("systemd")
            .to_string();
        let unit_name = if runtime == "docker" {
            meta.as_ref()
                .and_then(|m| m["docker_name"].as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("neuroveil-{}", cid))
        } else {
            meta.as_ref()
                .and_then(|m| m["service_name"].as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("neuroveil-{}", cid))
        };
        let enable_port_hopping = meta
            .as_ref()
            .and_then(|m| m["enable_port_hopping"].as_bool())
            .unwrap_or(false);
        ResolvedContainer {
            id: cid.to_string(),
            cdir: format!("/opt/neuroveil/containers/{}", cid),
            runtime,
            unit_name,
            enable_port_hopping,
        }
    };

    if let Some(cid) = req {
        if cid == "legacy-singbox" {
            let (code, _) = run_remote_cmd(sess, "test -f /etc/sing-box/config.json").await?;
            if code != 0 {
                return Err("На сервере не найден /etc/sing-box/config.json.".to_string());
            }
            return Ok(ResolvedContainer {
                id: "legacy-singbox".to_string(),
                cdir: "/etc/sing-box".to_string(),
                runtime: "systemd".to_string(),
                unit_name: "sing-box".to_string(),
                enable_port_hopping: true,
            });
        }
        let cdir = format!("/opt/neuroveil/containers/{}", cid);
        let (code, _) = run_remote_cmd(sess, &format!("test -f \"{}/config.json\"", cdir)).await?;
        if code != 0 {
            return Err(format!("Контейнер {} не найден на сервере.", cid));
        }
        let (_, meta_raw) = run_remote_cmd(sess, &format!("cat \"{}/meta.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        return Ok(inspect_one(cid, &meta_raw));
    }

    // Auto-discover containers when container_id is not explicitly specified
    let (_, list_out) = run_remote_cmd(
        sess,
        "for d in /opt/neuroveil/containers/*; do [ -f \"$d/config.json\" ] && basename \"$d\"; done 2>/dev/null || true",
    )
    .await
    .unwrap_or((1, String::new()));

    let cids: Vec<String> = list_out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if let Some(target_port) = port_hint.filter(|&p| p > 0) {
        for cid in &cids {
            let cdir = format!("/opt/neuroveil/containers/{}", cid);
            let (_, cfg_raw) = run_remote_cmd(sess, &format!("cat \"{}/config.json\" 2>/dev/null || true", cdir))
                .await
                .unwrap_or((1, String::new()));
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&cfg_raw) {
                let in_ib = cfg["inbounds"]
                    .as_array()
                    .map(|arr| arr.iter().any(|ib| ib["listen_port"].as_u64() == Some(target_port as u64)))
                    .unwrap_or(false);
                let in_ep = cfg["endpoints"]
                    .as_array()
                    .map(|arr| arr.iter().any(|ep| ep["listen_port"].as_u64() == Some(target_port as u64)))
                    .unwrap_or(false);
                if in_ib || in_ep {
                    let (_, meta_raw) = run_remote_cmd(sess, &format!("cat \"{}/meta.json\" 2>/dev/null || true", cdir))
                        .await
                        .unwrap_or((1, String::new()));
                    return Ok(inspect_one(cid, &meta_raw));
                }
            }
        }
    }

    let (legacy_code, _) = run_remote_cmd(sess, "test -f /etc/sing-box/config.json").await.unwrap_or((1, String::new()));
    if legacy_code == 0 && (port_hint.is_none() || cids.is_empty()) {
        return Ok(ResolvedContainer {
            id: "legacy-singbox".to_string(),
            cdir: "/etc/sing-box".to_string(),
            runtime: "systemd".to_string(),
            unit_name: "sing-box".to_string(),
            enable_port_hopping: true,
        });
    }

    if let Some(first_cid) = cids.first() {
        let cdir = format!("/opt/neuroveil/containers/{}", first_cid);
        let (_, meta_raw) = run_remote_cmd(sess, &format!("cat \"{}/meta.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        return Ok(inspect_one(first_cid, &meta_raw));
    }

    if legacy_code == 0 {
        return Ok(ResolvedContainer {
            id: "legacy-singbox".to_string(),
            cdir: "/etc/sing-box".to_string(),
            runtime: "systemd".to_string(),
            unit_name: "sing-box".to_string(),
            enable_port_hopping: true,
        });
    }

    Err("На сервере не развернут ни один контейнер VPN. Сначала разверните VPN в разделе «Добавление и Автодеплой».".to_string())
}

fn build_container_restart_cmd(target: &ResolvedContainer, start_if_active: bool) -> String {
    if target.runtime == "docker" {
        if start_if_active {
            format!(
                "docker restart \"{u}\" >/dev/null 2>&1 || docker start \"{u}\" >/dev/null 2>&1\n\
                 sleep 1\n\
                 if [ \"$(docker inspect -f '{{{{.State.Running}}}}' \"{u}\" 2>/dev/null)\" = \"true\" ]; then echo \"active\"; else echo \"inactive\"; fi",
                u = target.unit_name
            )
        } else {
            format!("docker stop \"{u}\" >/dev/null 2>&1 || true\necho \"stopped\"", u = target.unit_name)
        }
    } else if start_if_active {
        format!(
            "systemctl restart \"{u}\"\nsleep 1\nsystemctl is-active \"{u}\"",
            u = target.unit_name
        )
    } else {
        format!("systemctl stop \"{u}\" || true\necho \"stopped\"", u = target.unit_name)
    }
}

pub async fn clean_vps(app: &AppHandle, creds: &VpsCredentials) -> Result<String, String> {
    emit_log(
        app,
        &format!("=== Старт очистки сервера {} ===", creds.host),
    );
    let sess = connect_ssh(app, creds).await?;
    emit_log(
        app,
        "SSH-соединение установлено. Остановка и удаление всех контейнеров и служб VPN...",
    );

    let clean_cmd = format!(
        r#"
if command -v docker >/dev/null 2>&1; then
    NV_CIDS=$(docker ps -aq --filter "name=neuroveil-" 2>/dev/null || true)
    if [ -n "$NV_CIDS" ]; then
        docker rm -f $NV_CIDS >/dev/null 2>&1 || true
    fi
fi
for unit in /etc/systemd/system/neuroveil-*.service; do
    if [ -f "$unit" ]; then
        uname=$(basename "$unit")
        systemctl stop "$uname" >/dev/null 2>&1 || true
        systemctl disable "$uname" >/dev/null 2>&1 || true
        rm -f "$unit"
    fi
done
rm -rf /opt/neuroveil/containers

systemctl stop sing-box >/dev/null 2>&1 || true
systemctl disable sing-box >/dev/null 2>&1 || true
systemctl stop fail2ban >/dev/null 2>&1 || true
systemctl disable fail2ban >/dev/null 2>&1 || true
rm -f /etc/systemd/system/sing-box.service
rm -f /etc/systemd/system/neuroveil-iptables.service
rm -f /etc/iptables/rules.v4
rm -f /etc/systemd/system/ssh.socket.d/neuroveil-ports.conf
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl restart ssh.socket >/dev/null 2>&1 || systemctl restart ssh >/dev/null 2>&1 || systemctl restart sshd >/dev/null 2>&1 || true
rm -rf /etc/sing-box
rm -f /usr/local/bin/sing-box
rm -f /etc/sysctl.d/99-neuroveil-vpn.conf
rm -f /etc/sysctl.d/99-aegis-vpn.conf
rm -f /etc/ssh/sshd_config.d/99-neuroveil*.conf
rm -f /etc/ssh/sshd_config.d/99-aegis*.conf
systemctl reload ssh >/dev/null 2>&1 || systemctl reload sshd >/dev/null 2>&1 || true

if command -v ufw >/dev/null 2>&1; then
    ufw delete allow {vless_port}/tcp >/dev/null 2>&1 || true
    ufw delete allow {hy2_port}/udp >/dev/null 2>&1 || true
    ufw delete allow {tuic_port}/udp >/dev/null 2>&1 || true
fi
iptables -P INPUT ACCEPT >/dev/null 2>&1 || true
iptables -P FORWARD ACCEPT >/dev/null 2>&1 || true
iptables -P OUTPUT ACCEPT >/dev/null 2>&1 || true
iptables -F >/dev/null 2>&1 || true
iptables -X >/dev/null 2>&1 || true
iptables -t nat -F >/dev/null 2>&1 || true
iptables -t nat -X >/dev/null 2>&1 || true
echo "CLEAN_OK"
"#,
        vless_port = creds.vless_port,
        hy2_port = creds.hy2_port,
        tuic_port = creds.tuic_port
    );

    let (code, out) = run_remote_cmd(&sess, &clean_cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "clean completed", "en")
        .await;
    if code != 0 && !out.contains("CLEAN_OK") {
        return Err(format!("Ошибка при очистке сервера: {}", out));
    }

    emit_log(
        app,
        "Удалены: все контейнеры NeuroVeil, сервис sing-box, конфигурации и правила портов.",
    );

    emit_log(app, "=== Сервер полностью очищен! ===");

    Ok("Сервер успешно очищен от всех контейнеров и компонентов VPN.".to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateDeviceKeysResult {
    pub created: Vec<VpnProfile>,
    pub skipped: Vec<String>,
}

pub async fn create_vps_device_keys(
    app: &AppHandle,
    creds: &VpsCredentials,
    device_name: &str,
    protocols: &[String],
    container_id: Option<&str>,
) -> Result<CreateDeviceKeysResult, String> {
    let clean_name = device_name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Введите название устройства или клиента".to_string());
    }

    let mut unique_protos: Vec<String> = Vec::new();
    for p in protocols {
        let norm = p.trim().to_lowercase();
        if !norm.is_empty() && !unique_protos.contains(&norm) {
            unique_protos.push(norm);
        }
    }
    if unique_protos.is_empty() {
        return Err("Выберите хотя бы один протокол для выпуска ключа".to_string());
    }

    emit_log(
        app,
        &format!(
            "Создание ключей на сервере {} для устройства «{}» ({})...",
            creds.host,
            clean_name,
            unique_protos.join(", ").to_uppercase()
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let target = resolve_container_target(&sess, container_id, None).await?;
    let (code, raw_cfg) = run_remote_cmd(&sess, &format!("cat \"{}/config.json\"", target.cdir)).await?;
    if code != 0 || raw_cfg.trim().is_empty() {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "err", "en")
            .await;
        return Err(format!(
            "В контейнере {} не найден config.json.",
            target.id
        ));
    }

    let mut cfg: serde_json::Value = serde_json::from_str(&raw_cfg)
        .map_err(|e| format!("Ошибка чтения серверного config.json: {}", e))?;

    let connect_host = creds.host.trim().to_string();
    let awg_file = format!("{}/awg.json", target.cdir);
    let awg2_file = format!("{}/awg2.json", target.cdir);

    let mut awg_json_opt: Option<serde_json::Value> = None;
    let mut awg_loaded = false;
    let mut awg2_json_opt: Option<serde_json::Value> = None;
    let mut awg2_loaded = false;
    let mut reality_json_opt: Option<serde_json::Value> = None;
    let mut reality_loaded = false;

    let mut cfg_dirty = false;
    let mut awg_dirty = false;
    let mut awg2_dirty = false;

    let mut created_profiles: Vec<VpnProfile> = Vec::new();
    let mut skipped_reasons: Vec<String> = Vec::new();

    for proto in &unique_protos {
        if proto == "amneziawg" || proto == "awg_quic" || proto == "awg_vless" || proto == "awg2" {
            let is_awg2 = proto == "awg2";
            let target_tag = if is_awg2 { "awg2-in" } else { "awg-in" };
            let default_port = if is_awg2 { creds.awg2_port } else { creds.awg_port };

            if is_awg2 && !awg2_loaded {
                let (_, raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg2_file)).await?;
                awg2_json_opt = serde_json::from_str(&raw).ok();
                awg2_loaded = true;
            } else if !is_awg2 && !awg_loaded {
                let (_, raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg_file)).await?;
                awg_json_opt = serde_json::from_str(&raw).ok();
                awg_loaded = true;
            }

            let awg_json_ref = if is_awg2 { &mut awg2_json_opt } else { &mut awg_json_opt };
            let Some(awg_json) = awg_json_ref.as_mut() else {
                skipped_reasons.push(format!(
                    "В контейнере {} не найден файл {} для {}.",
                    target.id,
                    if is_awg2 { "awg2.json" } else { "awg.json" },
                    proto
                ));
                continue;
            };

            // For hybrid AWG + Reality, verify plain VLESS inbound exists first
            let mut hybrid_vless_data: Option<(u16, String, String, Option<String>, Option<String>)> = None;
            if proto == "awg_vless" {
                if !reality_loaded {
                    let (_, real_raw) = run_remote_cmd(&sess, &format!("cat \"{}/reality.json\" 2>/dev/null", target.cdir)).await?;
                    reality_json_opt = serde_json::from_str(&real_raw).ok();
                    reality_loaded = true;
                }
                let mut vless_found = false;
                if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
                    for inbound in inbounds.iter_mut() {
                        let ib_type = inbound["type"].as_str().unwrap_or("");
                        let tr_type = inbound["transport"]["type"].as_str().unwrap_or("");
                        if ib_type == "vless" && tr_type.is_empty() {
                            let v_port = inbound["listen_port"].as_u64().unwrap_or(creds.vless_port as u64) as u16;
                            let v_sni = inbound["tls"]["server_name"]
                                .as_str()
                                .filter(|s| !s.trim().is_empty())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| {
                                    if creds.reality_sni.trim().is_empty() {
                                        "www.microsoft.com".to_string()
                                    } else {
                                        creds.reality_sni.trim().to_string()
                                    }
                                });
                            let v_uuid = Uuid::new_v4().to_string();
                            if let Some(users) = inbound["users"].as_array_mut() {
                                users.push(json!({
                                    "name": format!("{} (Hybrid)", clean_name),
                                    "uuid": v_uuid,
                                    "flow": "xtls-rprx-vision"
                                }));
                                cfg_dirty = true;
                                vless_found = true;
                            }
                            let pk = reality_json_opt
                                .as_ref()
                                .and_then(|j| j["public_key"].as_str())
                                .map(|s| s.to_string());
                            let sid = reality_json_opt
                                .as_ref()
                                .and_then(|j| j["short_id"].as_str())
                                .map(|s| s.to_string());
                            hybrid_vless_data = Some((v_port, v_sni, v_uuid, pk, sid));
                            break;
                        }
                    }
                }
                if !vless_found {
                    skipped_reasons.push(format!(
                        "В контейнере {} не найден VLESS Reality для гибрида AWG + Reality.",
                        target.id
                    ));
                    continue;
                }
            }

            let Some(endpoints) = cfg["endpoints"].as_array_mut() else {
                skipped_reasons.push("В конфиге контейнера отсутствует секция endpoints.".to_string());
                continue;
            };

            let mut wg_idx = None;
            for (idx, ep) in endpoints.iter().enumerate() {
                if ep["type"].as_str() == Some("wireguard") && ep["tag"].as_str() == Some(target_tag) {
                    wg_idx = Some(idx);
                    break;
                }
            }
            if wg_idx.is_none() {
                for (idx, ep) in endpoints.iter().enumerate() {
                    if ep["type"].as_str() == Some("wireguard") && ep["listen_port"].as_u64() == Some(default_port as u64) {
                        wg_idx = Some(idx);
                        break;
                    }
                }
            }
            if wg_idx.is_none() && !is_awg2 {
                for (idx, ep) in endpoints.iter().enumerate() {
                    if ep["type"].as_str() == Some("wireguard") {
                        wg_idx = Some(idx);
                        break;
                    }
                }
            }
            let Some(wg_idx) = wg_idx else {
                skipped_reasons.push(format!(
                    "В контейнере {} не найден WireGuard endpoint для {}.",
                    target.id, proto
                ));
                continue;
            };

            let (k_code, k_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
            if k_code != 0 {
                skipped_reasons.push(format!("Ошибка генерации ключей WireGuard для {}: {}", proto, k_out));
                continue;
            }
            let mut dev_priv = String::new();
            let mut dev_pub = String::new();
            for line in k_out.lines() {
                let l = line.trim();
                if l.starts_with("PrivateKey:") {
                    dev_priv = l.trim_start_matches("PrivateKey:").trim().to_string();
                } else if l.starts_with("PublicKey:") {
                    dev_pub = l.trim_start_matches("PublicKey:").trim().to_string();
                }
            }

            let wg_ep = &mut endpoints[wg_idx];
            let subnet_prefix = wg_ep["address"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .and_then(|cidr| {
                    let ip_part = cidr.split('/').next()?;
                    let mut parts: Vec<&str> = ip_part.split('.').collect();
                    if parts.len() == 4 {
                        parts.pop();
                        Some(format!("{}.", parts.join(".")))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| if is_awg2 { "10.8.1.".to_string() } else { "10.8.0.".to_string() });

            let port = wg_ep["listen_port"].as_u64().unwrap_or(default_port as u64) as u16;
            let psk = awg_json["preshared_key"].as_str().map(|s| s.to_string());
            let Some(peers) = wg_ep["peers"].as_array_mut() else {
                skipped_reasons.push(format!("В endpoint wireguard контейнера {} отсутствует массив peers.", target.id));
                continue;
            };
            let mut used_ips = std::collections::HashSet::new();
            for p in peers.iter() {
                if let Some(arr) = p["allowed_ips"].as_array() {
                    for a in arr {
                        if let Some(s) = a.as_str() {
                            used_ips.insert(s.to_string());
                        }
                    }
                }
            }
            let mut chosen_octet = 2;
            for octet in 2..=254 {
                let candidate = format!("{}{}/32", subnet_prefix, octet);
                if !used_ips.contains(&candidate) {
                    chosen_octet = octet;
                    break;
                }
            }
            let peer_ip = format!("{}{}/32", subnet_prefix, chosen_octet);

            let mut new_peer_json = json!({
                "public_key": dev_pub,
                "allowed_ips": [peer_ip]
            });
            if let Some(ref p) = psk {
                if !p.trim().is_empty() {
                    new_peer_json["pre_shared_key"] = json!(p.trim());
                }
            }
            peers.push(new_peer_json);
            cfg_dirty = true;

            if !awg_json["devices"].is_array() {
                let def_priv = awg_json["client_private_key"].as_str().unwrap_or("").to_string();
                let def_pub = awg_json["client_public_key"].as_str().unwrap_or("").to_string();
                let def_ip = format!("{}2/32", subnet_prefix);
                awg_json["devices"] = json!([
                    {
                        "name": "default",
                        "client_private_key": def_priv,
                        "client_public_key": def_pub,
                        "ip": def_ip
                    }
                ]);
            }
            let dev_label = if unique_protos.len() > 1 {
                match proto.as_str() {
                    "awg_quic" => format!("{} (AWG+QUIC)", clean_name),
                    "awg_vless" => format!("{} (AWG+Reality)", clean_name),
                    "awg2" => format!("{} (AWG 2.0)", clean_name),
                    _ => clean_name.clone(),
                }
            } else {
                clean_name.clone()
            };
            if let Some(devs) = awg_json["devices"].as_array_mut() {
                let mut dev_entry = json!({
                    "name": dev_label,
                    "protocol": proto,
                    "client_private_key": dev_priv,
                    "client_public_key": dev_pub,
                    "ip": peer_ip
                });
                if let Some(ref p) = psk {
                    if !p.trim().is_empty() {
                        dev_entry["preshared_key"] = json!(p.trim());
                    }
                }
                if let Some((vp, ref vsni, ref vuuid, _, _)) = hybrid_vless_data {
                    dev_entry["vless_port"] = json!(vp);
                    dev_entry["vless_sni"] = json!(vsni);
                    dev_entry["vless_uuid"] = json!(vuuid);
                }
                devs.push(dev_entry);
            }
            if is_awg2 {
                awg2_dirty = true;
            } else {
                awg_dirty = true;
            }

            let server_pub = awg_json["server_public_key"].as_str().unwrap_or("").to_string();
            let client_addr = if is_awg2 {
                peer_ip.replace("/32", "/24")
            } else {
                peer_ip.clone()
            };

            let mut awg_profile = if is_awg2 {
                VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: dev_label,
                    protocol: proto.to_string(),
                    server: connect_host.clone(),
                    port,
                    password: dev_priv.clone(),
                    awg_private_key: Some(dev_priv),
                    awg_public_key: Some(server_pub),
                    awg_preshared_key: psk,
                    awg_local_address: Some(client_addr),
                    awg_version: Some("2.0".to_string()),
                    awg_port_probing: Some(false),
                    up_mbps: 100,
                    down_mbps: 100,
                    pinned_on_pc: Some(false),
                    container_id: Some(target.id.clone()),
                    ..Default::default()
                }
            } else {
                let hop_ports = awg_json["hop_ports"].as_str().unwrap_or("20000:65000").to_string();
                let hpk = awg_json["header_protection_key"].as_str().map(|s| s.to_string());
                let jc = awg_json["jc"].as_u64().map(|v| v as u16).or(Some(4));
                let jmin = awg_json["jmin"].as_u64().map(|v| v as u16).or(Some(40));
                let jmax = awg_json["jmax"].as_u64().map(|v| v as u16).or(Some(70));
                let s1 = awg_json["s1"].as_u64().map(|v| v as u16).or(Some(15));
                let s2 = awg_json["s2"].as_u64().map(|v| v as u16).or(Some(25));
                let s3 = awg_json["s3"].as_u64().map(|v| v as u16);
                let s4 = awg_json["s4"].as_u64().map(|v| v as u16);
                let h1 = awg_json["h1"].as_str().map(|s| s.to_string()).or_else(|| awg_json["h1"].as_u64().map(|u| u.to_string())).or(Some("1".to_string()));
                let h2 = awg_json["h2"].as_str().map(|s| s.to_string()).or_else(|| awg_json["h2"].as_u64().map(|u| u.to_string())).or(Some("2".to_string()));
                let h3 = awg_json["h3"].as_str().map(|s| s.to_string()).or_else(|| awg_json["h3"].as_u64().map(|u| u.to_string())).or(Some("3".to_string()));
                let h4 = awg_json["h4"].as_str().map(|s| s.to_string()).or_else(|| awg_json["h4"].as_u64().map(|u| u.to_string())).or(Some("4".to_string()));

                let mut prof = VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: dev_label,
                    protocol: proto.to_string(),
                    server: connect_host.clone(),
                    port,
                    password: dev_priv.clone(),
                    awg_private_key: Some(dev_priv),
                    awg_public_key: Some(server_pub),
                    awg_preshared_key: psk,
                    awg_local_address: Some(client_addr),
                    awg_version: Some("3.1".to_string()),
                    awg_header_protection_key: hpk,
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
                    awg_rekey_after_time: awg_json["rekey_after_time"].as_str().map(|s| s.to_string()),
                    awg_rekey_timeout: awg_json["rekey_timeout"].as_str().map(|s| s.to_string()),
                    awg_reject_after_time: awg_json["reject_after_time"].as_str().map(|s| s.to_string()),
                    awg_keepalive_timeout: awg_json["keepalive_timeout"].as_str().map(|s| s.to_string()),
                    awg_max_handshake_attempts: awg_json["max_handshake_attempts"].as_str().map(|s| s.to_string()),
                    awg_random_trailers: awg_json["random_trailers"].as_bool(),
                    awg_disable_cookies: awg_json["disable_cookies"].as_bool(),
                    awg_persistent_keepalive: awg_json["persistent_keepalive"].as_str().map(|s| s.to_string()),
                    awg_hop_ports: Some(hop_ports),
                    awg_port_probing: Some(true),
                    up_mbps: 100,
                    down_mbps: 100,
                    pinned_on_pc: Some(false),
                    container_id: Some(target.id.clone()),
                    ..Default::default()
                };
                if let Some((vp, vsni, vuuid, pk, sid)) = hybrid_vless_data {
                    prof.vless_port = Some(vp);
                    prof.sni = vsni;
                    prof.uuid = Some(vuuid);
                    prof.reality_public_key = pk;
                    prof.reality_short_id = sid;
                    prof.reality_flow = Some("xtls-rprx-vision".to_string());
                }
                prof
            };
            awg_profile.uri = build_profile_uri(&awg_profile);
            created_profiles.push(awg_profile);
            continue;
        }

        if proto == "shadowtls" {
            let mut st_port: Option<u16> = None;
            let mut st_sni = "www.microsoft.com".to_string();
            let mut detour_tag = "ss-in".to_string();
            let st_user_pass = Uuid::new_v4().simple().to_string();
            let mut st_updated = false;

            if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
                for inbound in inbounds.iter_mut() {
                    if inbound["type"].as_str() == Some("shadowtls") {
                        st_port = Some(inbound["listen_port"].as_u64().unwrap_or(creds.shadowtls_port as u64) as u16);
                        if let Some(s) = inbound["handshake"]["server"].as_str() {
                            if !s.trim().is_empty() {
                                st_sni = s.to_string();
                            }
                        }
                        if let Some(dt) = inbound["detour"].as_str() {
                            if !dt.trim().is_empty() {
                                detour_tag = dt.to_string();
                            }
                        }
                        if let Some(users) = inbound["users"].as_array_mut() {
                            users.push(json!({
                                "name": clean_name,
                                "password": st_user_pass
                            }));
                            st_updated = true;
                            cfg_dirty = true;
                        }
                        break;
                    }
                }
            }
            if !st_updated {
                skipped_reasons.push(format!(
                    "В контейнере {} не найден активный инбаунд ShadowTLS v3.",
                    target.id
                ));
                continue;
            }
            let ss_ib = cfg["inbounds"]
                .as_array()
                .and_then(|arr| arr.iter().find(|i| i["tag"].as_str() == Some(detour_tag.as_str())));
            let ss_password = ss_ib
                .and_then(|i| {
                    i["password"].as_str().or_else(|| {
                        i["users"].as_array().and_then(|u| u.first()).and_then(|u| u["password"].as_str())
                    })
                })
                .unwrap_or("")
                .to_string();
            let ss_method = ss_ib
                .and_then(|i| i["method"].as_str())
                .unwrap_or("2022-blake3-aes-128-gcm")
                .to_string();
            let prof_name = if unique_protos.len() > 1 {
                format!("{} (ShadowTLS)", clean_name)
            } else {
                clean_name.clone()
            };
            let mut st_profile = VpnProfile {
                id: Uuid::new_v4().to_string(),
                name: prof_name,
                protocol: "shadowtls".to_string(),
                server: connect_host.clone(),
                port: st_port.unwrap_or(creds.shadowtls_port),
                sni: st_sni.clone(),
                shadowtls_sni: Some(st_sni),
                shadowtls_password: Some(st_user_pass),
                password: ss_password,
                ss_method: Some(ss_method),
                up_mbps: 100,
                down_mbps: 100,
                tls_fragment: Some(false),
                pinned_on_pc: Some(false),
                container_id: Some(target.id.clone()),
                ..Default::default()
            };
            st_profile.uri = build_profile_uri(&st_profile);
            created_profiles.push(st_profile);
            continue;
        }

        let Some(inbounds) = cfg["inbounds"].as_array_mut() else {
            skipped_reasons.push("В серверном конфиге отсутствует секция inbounds.".to_string());
            continue;
        };

        let new_password = Uuid::new_v4().simple().to_string();
        let new_uuid = Uuid::new_v4().to_string();
        let mut ss_user_pw = String::new();
        let mut inbound_ss_method = None;
        let mut found_port = if proto == "vless" {
            creds.vless_port
        } else if proto == "tuic" {
            creds.tuic_port
        } else if proto == "shadowsocks" {
            creds.ss_port
        } else if proto == "vless_ws" {
            creds.vless_ws_port
        } else if proto == "webrtc" {
            creds.webrtc_port
        } else {
            creds.hy2_port
        };
        let mut found_sni = if proto == "vless" {
            if creds.reality_sni.trim().is_empty() {
                "www.microsoft.com".to_string()
            } else {
                creds.reality_sni.trim().to_string()
            }
        } else if creds.sni.trim().is_empty() {
            "www.bing.com".to_string()
        } else {
            creds.sni.trim().to_string()
        };
        let mut found_obfs: Option<String> = None;
        let mut found_ws_path: Option<String> = None;
        let mut updated = false;

        for inbound in inbounds.iter_mut() {
            let ib_type = inbound["type"].as_str().unwrap_or("");
            let tr_type = inbound["transport"]["type"].as_str().unwrap_or("");
            let matches_plain_vless = proto == "vless" && ib_type == "vless" && tr_type.is_empty();
            let matches_ws = proto == "vless_ws" && ib_type == "vless" && tr_type == "ws";
            let matches_grpc = proto == "webrtc" && ib_type == "vless" && tr_type == "grpc";
            let matches_direct = (proto != "vless" && proto != "vless_ws" && proto != "webrtc") && ib_type == proto;
            if !(matches_plain_vless || matches_ws || matches_grpc || matches_direct) {
                continue;
            }
            if ib_type == "shadowsocks" && inbound["listen"].as_str() == Some("127.0.0.1") {
                continue;
            }

            if let Some(p) = inbound["listen_port"].as_u64() {
                found_port = p as u16;
            }
            if let Some(s) = inbound["tls"]["server_name"].as_str() {
                if !s.trim().is_empty() {
                    found_sni = s.to_string();
                }
            }
            if let Some(obfs_pw) = inbound["obfs"]["password"].as_str() {
                found_obfs = Some(obfs_pw.to_string());
            }
            if proto == "vless_ws" {
                found_ws_path = Some(inbound["transport"]["path"].as_str().unwrap_or("/neuroveil-ws").to_string());
            } else if proto == "webrtc" {
                found_ws_path = Some(inbound["transport"]["service_name"].as_str().unwrap_or("neuroveil-webrtc").to_string());
            }

            if ib_type == "shadowsocks" {
                if let Some(m) = inbound["method"].as_str() {
                    inbound_ss_method = Some(m.to_string());
                }
                ss_user_pw = inbound["password"].as_str().unwrap_or("").to_string();
                if ss_user_pw.is_empty() {
                    ss_user_pw = inbound["users"]
                        .as_array()
                        .and_then(|u| u.first())
                        .and_then(|u| u["password"].as_str())
                        .unwrap_or("")
                        .to_string();
                }
                updated = !ss_user_pw.is_empty();
            } else if let Some(users) = inbound["users"].as_array_mut() {
                if proto == "vless" {
                    users.push(json!({
                        "name": clean_name,
                        "uuid": new_uuid,
                        "flow": "xtls-rprx-vision"
                    }));
                } else if proto == "vless_ws" || proto == "webrtc" {
                    users.push(json!({
                        "name": clean_name,
                        "uuid": new_uuid
                    }));
                } else if proto == "tuic" {
                    users.push(json!({
                        "name": clean_name,
                        "uuid": new_uuid,
                        "password": new_password
                    }));
                } else {
                    users.push(json!({
                        "name": clean_name,
                        "password": new_password
                    }));
                }
                updated = true;
                cfg_dirty = true;
            }
            break;
        }

        if !updated {
            skipped_reasons.push(format!(
                "В контейнере {} не найден активный инбаунд для протокола {}.",
                target.id, proto
            ));
            continue;
        }

        let (reality_pub, reality_sid) = if proto == "vless" {
            if !reality_loaded {
                let (_, real_raw) = run_remote_cmd(&sess, &format!("cat \"{}/reality.json\" 2>/dev/null", target.cdir)).await?;
                reality_json_opt = serde_json::from_str(&real_raw).ok();
                reality_loaded = true;
            }
            if let Some(ref real_json) = reality_json_opt {
                (
                    real_json["public_key"].as_str().map(|s| s.to_string()),
                    real_json["short_id"].as_str().map(|s| s.to_string()),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let prof_name = if unique_protos.len() > 1 {
            let tag = match proto.as_str() {
                "vless" => "VLESS",
                "hysteria2" => "Hy2",
                "tuic" => "TUIC",
                "shadowsocks" => "SS",
                "vless_ws" => "CDN WS",
                "webrtc" => "WebRTC",
                other => other,
            };
            format!("{} ({})", clean_name, tag)
        } else {
            clean_name.clone()
        };

        let cdn_host = if proto == "vless_ws" {
            Some(
                creds
                    .cdn_domain
                    .clone()
                    .filter(|d| !d.trim().is_empty())
                    .unwrap_or_else(|| connect_host.clone()),
            )
        } else {
            None
        };

        let mut profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: prof_name,
            protocol: proto.to_string(),
            server: connect_host.clone(),
            port: found_port,
            sni: found_sni,
            insecure: proto != "vless",
            password: if proto == "vless" || proto == "vless_ws" || proto == "webrtc" {
                new_uuid.clone()
            } else if proto == "shadowsocks" {
                ss_user_pw
            } else {
                new_password
            },
            uuid: if proto == "tuic" || proto == "vless" || proto == "vless_ws" || proto == "webrtc" {
                Some(new_uuid)
            } else {
                None
            },
            obfs_password: found_obfs,
            up_mbps: 100,
            down_mbps: 100,
            hop_ports: if proto == "hysteria2" && target.enable_port_hopping {
                Some("20000:65000".to_string())
            } else {
                None
            },
            hop_interval: if proto == "hysteria2" && target.enable_port_hopping {
                Some("30s".to_string())
            } else {
                None
            },
            reality_public_key: reality_pub,
            reality_short_id: reality_sid,
            reality_flow: if proto == "vless" { Some("xtls-rprx-vision".to_string()) } else { None },
            ss_method: if proto == "shadowsocks" {
                inbound_ss_method.or_else(|| Some(creds.ss_method.clone()))
            } else {
                None
            },
            ws_path: found_ws_path,
            cdn_host,
            pinned_on_pc: Some(false),
            container_id: Some(target.id.clone()),
            ..Default::default()
        };
        profile.uri = build_profile_uri(&profile);
        created_profiles.push(profile);
    }

    if created_profiles.is_empty() {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "err", "en")
            .await;
        return Err(skipped_reasons.join(" "));
    }

    if cfg_dirty || awg_dirty || awg2_dirty {
        let mut cmd_parts = Vec::new();
        if cfg_dirty {
            let new_cfg_str = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
            cmd_parts.push(format!(
                "cat << 'EOF_CFG' > \"{}/config.json\"\n{}\nEOF_CFG",
                target.cdir, new_cfg_str
            ));
        }
        if awg_dirty {
            if let Some(ref awg_json) = awg_json_opt {
                let new_awg_str = serde_json::to_string_pretty(awg_json).unwrap_or_default();
                cmd_parts.push(format!(
                    "cat << 'EOF_AWG' > \"{}\"\n{}\nEOF_AWG\nchmod 600 \"{}\"",
                    awg_file, new_awg_str, awg_file
                ));
            }
        }
        if awg2_dirty {
            if let Some(ref awg2_json) = awg2_json_opt {
                let new_awg2_str = serde_json::to_string_pretty(awg2_json).unwrap_or_default();
                cmd_parts.push(format!(
                    "cat << 'EOF_AWG2' > \"{}\"\n{}\nEOF_AWG2\nchmod 600 \"{}\"",
                    awg2_file, new_awg2_str, awg2_file
                ));
            }
        }
        cmd_parts.push(build_container_restart_cmd(&target, true));
        let update_cmd = cmd_parts.join("\n");

        let (restart_code, restart_out) = run_remote_cmd(&sess, &update_cmd).await?;
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;

        if restart_code != 0 || !restart_out.contains("active") {
            return Err(format!(
                "Ошибка перезапуска контейнера {} после добавления устройства: {}",
                target.id, restart_out
            ));
        }
    } else {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;
    }

    emit_log(
        app,
        &format!(
            "Создано ключей ({}) для «{}» в контейнере {}!",
            created_profiles.len(),
            clean_name,
            target.id
        ),
    );

    Ok(CreateDeviceKeysResult {
        created: created_profiles,
        skipped: skipped_reasons,
    })
}

pub async fn create_vps_device_user(
    app: &AppHandle,
    creds: &VpsCredentials,
    device_name: &str,
    protocol: &str,
    container_id: Option<&str>,
) -> Result<VpnProfile, String> {
    let res = create_vps_device_keys(
        app,
        creds,
        device_name,
        &[protocol.to_string()],
        container_id,
    )
    .await?;
    res.created
        .into_iter()
        .next()
        .ok_or_else(|| "Не удалось создать ключ для выбранного протокола".to_string())
}

fn parse_container_profiles(
    cid: &str,
    connect_host: &str,
    creds: &VpsCredentials,
    cfg: &serde_json::Value,
    real_raw: &str,
    awg_raw: &str,
    awg2_raw: &str,
    enable_port_hopping: bool,
    multi_container_host: bool,
) -> (Vec<VpnProfile>, Vec<String>, Vec<u16>) {
    let (cached_real_pub, cached_real_sid) = if let Ok(real_json) = serde_json::from_str::<serde_json::Value>(real_raw) {
        (
            real_json["public_key"].as_str().map(|s| s.to_string()),
            real_json["short_id"].as_str().map(|s| s.to_string()),
        )
    } else {
        (None, None)
    };

    let default_inbounds = Vec::new();
    let inbounds = cfg["inbounds"].as_array().unwrap_or(&default_inbounds);

    let mut profiles: Vec<VpnProfile> = Vec::new();
    let mut proto_labels: Vec<String> = Vec::new();
    let mut ports: Vec<u16> = Vec::new();

    let push_label_port = |labels: &mut Vec<String>, port_list: &mut Vec<u16>, lbl: &str, p: u16| {
        if !labels.iter().any(|x| x == lbl) {
            labels.push(lbl.to_string());
        }
        if p > 0 && !port_list.contains(&p) {
            port_list.push(p);
        }
    };

    let format_base_name = |proto_label: &str, port: u16| -> String {
        if multi_container_host && port > 0 {
            format!("{} ({}:{})", proto_label, connect_host, port)
        } else {
            format!("{} ({})", proto_label, connect_host)
        }
    };

    for inbound in inbounds {
        let ib_type = inbound["type"].as_str().unwrap_or("");
        if ib_type != "hysteria2" && ib_type != "tuic" && ib_type != "vless" && ib_type != "shadowsocks" && ib_type != "shadowtls" {
            continue;
        }

        if ib_type == "shadowsocks" && inbound["listen"].as_str() == Some("127.0.0.1") {
            continue;
        }

        if ib_type == "shadowtls" {
            let port = inbound["listen_port"].as_u64().unwrap_or(8443) as u16;
            push_label_port(&mut proto_labels, &mut ports, "ShadowTLS v3", port);
            let st_sni = inbound["handshake"]["server"]
                .as_str()
                .unwrap_or("www.microsoft.com")
                .to_string();
            let detour_tag = inbound["detour"].as_str().unwrap_or("ss-in");
            let ss_ib = inbounds.iter().find(|i| i["tag"].as_str() == Some(detour_tag));
            let ss_password = ss_ib
                .and_then(|i| {
                    i["password"].as_str().or_else(|| {
                        i["users"].as_array().and_then(|u| u.first()).and_then(|u| u["password"].as_str())
                    })
                })
                .unwrap_or("")
                .to_string();
            let ss_method = ss_ib
                .and_then(|i| i["method"].as_str())
                .unwrap_or("2022-blake3-aes-128-gcm")
                .to_string();

            if let Some(users) = inbound["users"].as_array() {
                for u in users {
                    let user_name = u["name"].as_str().unwrap_or("neuroveil-user");
                    let st_pass = u["password"].as_str().unwrap_or("").to_string();
                    if st_pass.is_empty() || ss_password.is_empty() {
                        continue;
                    }
                    let is_base = user_name == "neuroveil-user" || user_name == "default";
                    let prof_name = if is_base {
                        format_base_name("ShadowTLS v3", port)
                    } else {
                        user_name.to_string()
                    };
                    let mut prof = VpnProfile {
                        id: Uuid::new_v4().to_string(),
                        name: prof_name,
                        protocol: "shadowtls".to_string(),
                        server: connect_host.to_string(),
                        port,
                        sni: st_sni.clone(),
                        shadowtls_sni: Some(st_sni.clone()),
                        shadowtls_password: Some(st_pass),
                        password: ss_password.clone(),
                        ss_method: Some(ss_method.clone()),
                        up_mbps: 100,
                        down_mbps: 100,
                        tls_fragment: Some(false),
                        pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                        container_id: Some(cid.to_string()),
                        ..Default::default()
                    };
                    prof.uri = build_profile_uri(&prof);
                    profiles.push(prof);
                }
            }
            continue;
        }

        let transport_type = inbound["transport"]["type"].as_str().unwrap_or("");
        let actual_protocol = if ib_type == "vless" && transport_type == "ws" {
            "vless_ws"
        } else if ib_type == "vless" && transport_type == "grpc" {
            "webrtc"
        } else {
            ib_type
        };

        let port = inbound["listen_port"]
            .as_u64()
            .unwrap_or(match actual_protocol {
                "vless" => 443,
                "vless_ws" => 8080,
                "webrtc" => 2053,
                "tuic" => 9443,
                "shadowsocks" => 8388,
                _ => 8443,
            }) as u16;
        let proto_label = match actual_protocol {
            "vless" => "VLESS Reality",
            "vless_ws" => "VLESS WS",
            "webrtc" => "WebRTC",
            "tuic" => "TUIC v5",
            "shadowsocks" => "Shadowsocks",
            _ => "Hysteria 2",
        };
        push_label_port(&mut proto_labels, &mut ports, proto_label, port);

        let sni = inbound["tls"]["server_name"]
            .as_str()
            .unwrap_or(if actual_protocol == "vless" { "www.microsoft.com" } else { "www.bing.com" })
            .to_string();
        let obfs_password = inbound["obfs"]["password"].as_str().map(|s| s.to_string());
        let ws_path = if actual_protocol == "vless_ws" {
            Some(inbound["transport"]["path"].as_str().unwrap_or("/neuroveil-ws").to_string())
        } else if actual_protocol == "webrtc" {
            Some(inbound["transport"]["service_name"].as_str().unwrap_or("neuroveil-webrtc").to_string())
        } else {
            None
        };
        let cdn_host = if actual_protocol == "vless_ws" {
            Some(creds.cdn_domain.clone().filter(|d| !d.trim().is_empty()).unwrap_or_else(|| connect_host.to_string()))
        } else {
            None
        };

        if let Some(users) = inbound["users"].as_array() {
            for u in users {
                let user_name = u["name"].as_str().unwrap_or("neuroveil-user");
                let uuid = u["uuid"].as_str().map(|s| s.to_string());
                let password = if ib_type == "vless" {
                    uuid.clone().unwrap_or_default()
                } else {
                    u["password"].as_str().unwrap_or("").to_string()
                };
                if password.is_empty() {
                    continue;
                }

                let full_label = match actual_protocol {
                    "vless" => "VLESS Reality",
                    "vless_ws" => "Cloudflare CDN WS",
                    "webrtc" => "WebRTC Steganography",
                    "tuic" => "TUIC v5",
                    "shadowsocks" => "Shadowsocks",
                    _ => "Hysteria 2",
                };
                let is_base = user_name == "neuroveil-user"
                    || user_name == "default"
                    || user_name == "neuroveil-ws-user"
                    || user_name == "neuroveil-webrtc-user";
                let prof_name = if is_base {
                    format_base_name(full_label, port)
                } else {
                    user_name.to_string()
                };
                let mut prof = VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: prof_name,
                    protocol: actual_protocol.to_string(),
                    server: connect_host.to_string(),
                    port,
                    sni: sni.clone(),
                    insecure: actual_protocol != "vless",
                    password,
                    uuid,
                    obfs_password: obfs_password.clone(),
                    up_mbps: 100,
                    down_mbps: 100,
                    hop_ports: if actual_protocol == "hysteria2" && enable_port_hopping {
                        Some("20000:65000".to_string())
                    } else {
                        None
                    },
                    hop_interval: if actual_protocol == "hysteria2" && enable_port_hopping {
                        Some("30s".to_string())
                    } else {
                        None
                    },
                    reality_public_key: if actual_protocol == "vless" { cached_real_pub.clone() } else { None },
                    reality_short_id: if actual_protocol == "vless" { cached_real_sid.clone() } else { None },
                    reality_flow: if actual_protocol == "vless" { Some("xtls-rprx-vision".to_string()) } else { None },
                    ss_method: if actual_protocol == "shadowsocks" { inbound["method"].as_str().map(|s| s.to_string()) } else { None },
                    ws_path: ws_path.clone(),
                    cdn_host: cdn_host.clone(),
                    pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                    container_id: Some(cid.to_string()),
                    ..Default::default()
                };
                prof.uri = build_profile_uri(&prof);
                profiles.push(prof);
            }
        } else if ib_type == "shadowsocks" {
            let password = inbound["password"].as_str().unwrap_or("").to_string();
            if !password.is_empty() {
                let prof_name = format_base_name("Shadowsocks", port);
                let mut prof = VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: prof_name,
                    protocol: "shadowsocks".to_string(),
                    server: connect_host.to_string(),
                    port,
                    sni: sni.clone(),
                    insecure: false,
                    password,
                    up_mbps: 100,
                    down_mbps: 100,
                    ss_method: inbound["method"].as_str().map(|s| s.to_string()),
                    pinned_on_pc: Some(true),
                    container_id: Some(cid.to_string()),
                    ..Default::default()
                };
                prof.uri = build_profile_uri(&prof);
                profiles.push(prof);
            }
        }
    }

    if let Some(endpoints) = cfg["endpoints"].as_array() {
        let awg_json = serde_json::from_str::<serde_json::Value>(awg_raw).ok();
        let awg2_json = serde_json::from_str::<serde_json::Value>(awg2_raw).ok();

        for ep in endpoints {
            if ep["type"].as_str() == Some("wireguard") {
                let tag = ep["tag"].as_str().unwrap_or("");
                let ep_port = ep["listen_port"].as_u64().unwrap_or(51820) as u16;
                let is_awg2 = tag == "awg2-in" || (awg2_json.is_some() && tag.contains("awg2"));
                let active_json = if is_awg2 { awg2_json.as_ref() } else { awg_json.as_ref() };
                let proto_str = if is_awg2 { "awg2" } else { "amneziawg" };
                let proto_ver = if is_awg2 { "2.0" } else { "3.1" };
                let proto_name = if is_awg2 { "AmneziaWG 2.0" } else { "Amnezia 3.1" };
                push_label_port(&mut proto_labels, &mut ports, if is_awg2 { "AWG 2.0" } else { "Amnezia 3.1" }, ep_port);

                let port = ep_port;
                let server_pub = active_json.and_then(|j| j["server_public_key"].as_str()).unwrap_or("").to_string();
                let client_priv = active_json.and_then(|j| j["client_private_key"].as_str()).unwrap_or("").to_string();
                let psk = active_json.and_then(|j| j["preshared_key"].as_str()).map(|s| s.to_string());
                let hpk = if is_awg2 { None } else { active_json.and_then(|j| j["header_protection_key"].as_str()).map(|s| s.to_string()) };
                let hop_ports = if is_awg2 { None } else { Some(active_json.and_then(|j| j["hop_ports"].as_str()).unwrap_or("20000:65000").to_string()) };
                let jc = if is_awg2 { None } else { ep["jc"].as_u64().map(|v| v as u16).or(Some(4)) };
                let jmin = if is_awg2 { None } else { ep["jmin"].as_u64().map(|v| v as u16).or(Some(40)) };
                let jmax = if is_awg2 { None } else { ep["jmax"].as_u64().map(|v| v as u16).or(Some(70)) };
                let s1 = if is_awg2 { None } else { ep["s1"].as_u64().map(|v| v as u16).or(Some(15)) };
                let s2 = if is_awg2 { None } else { ep["s2"].as_u64().map(|v| v as u16).or(Some(25)) };
                let s3 = if is_awg2 { None } else { ep["s3"].as_u64().map(|v| v as u16) };
                let s4 = if is_awg2 { None } else { ep["s4"].as_u64().map(|v| v as u16) };
                let h1 = if is_awg2 { None } else { ep["h1"].as_str().map(|s| s.to_string()).or_else(|| ep["h1"].as_u64().map(|u| u.to_string())) };
                let h2 = if is_awg2 { None } else { ep["h2"].as_str().map(|s| s.to_string()).or_else(|| ep["h2"].as_u64().map(|u| u.to_string())) };
                let h3 = if is_awg2 { None } else { ep["h3"].as_str().map(|s| s.to_string()).or_else(|| ep["h3"].as_u64().map(|u| u.to_string())) };
                let h4 = if is_awg2 { None } else { ep["h4"].as_str().map(|s| s.to_string()).or_else(|| ep["h4"].as_u64().map(|u| u.to_string())) };

                if let Some(devices) = active_json.and_then(|j| j["devices"].as_array()) {
                    for d in devices {
                        let peer_name = d["name"].as_str().unwrap_or(proto_name);
                        let is_base = peer_name == "default" || peer_name == "pc" || peer_name == "neuroveil-user";
                        let prof_name = if is_base {
                            format_base_name(proto_name, port)
                        } else {
                            peer_name.to_string()
                        };
                        let c_priv = d["client_private_key"].as_str().unwrap_or(&client_priv).to_string();
                        if c_priv.trim().is_empty() {
                            continue;
                        }
                        let raw_proto = d["protocol"].as_str().unwrap_or(proto_str);
                        let restored_proto = if !is_awg2 && (raw_proto == "awg_quic" || raw_proto == "awg_vless" || raw_proto == "amneziawg") {
                            raw_proto
                        } else {
                            proto_str
                        };
                        let raw_addr = d["ip"].as_str().unwrap_or(if is_awg2 { "10.8.1.2/24" } else { "10.8.0.2/32" });
                        let peer_addr = if is_awg2 { raw_addr.replace("/32", "/24") } else { raw_addr.to_string() };
                        let peer_psk = d["preshared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                        let mut prof = VpnProfile {
                            id: Uuid::new_v4().to_string(),
                            name: prof_name,
                            protocol: restored_proto.to_string(),
                            server: connect_host.to_string(),
                            port,
                            password: c_priv.clone(),
                            awg_private_key: Some(c_priv),
                            awg_public_key: Some(server_pub.clone()),
                            awg_preshared_key: peer_psk,
                            awg_local_address: Some(peer_addr),
                            awg_version: Some(proto_ver.to_string()),
                            awg_header_protection_key: hpk.clone(),
                            awg_jc: jc,
                            awg_jmin: jmin,
                            awg_jmax: jmax,
                            awg_s1: s1,
                            awg_s2: s2,
                            awg_s3: s3,
                            awg_s4: s4,
                            awg_h1: h1.clone(),
                            awg_h2: h2.clone(),
                            awg_h3: h3.clone(),
                            awg_h4: h4.clone(),
                            awg_rekey_after_time: if is_awg2 { None } else { active_json.and_then(|j| j["rekey_after_time"].as_str()).map(|s| s.to_string()) },
                            awg_rekey_timeout: if is_awg2 { None } else { active_json.and_then(|j| j["rekey_timeout"].as_str()).map(|s| s.to_string()) },
                            awg_reject_after_time: if is_awg2 { None } else { active_json.and_then(|j| j["reject_after_time"].as_str()).map(|s| s.to_string()) },
                            awg_keepalive_timeout: if is_awg2 { None } else { active_json.and_then(|j| j["keepalive_timeout"].as_str()).map(|s| s.to_string()) },
                            awg_max_handshake_attempts: if is_awg2 { None } else { active_json.and_then(|j| j["max_handshake_attempts"].as_str()).map(|s| s.to_string()) },
                            awg_random_trailers: if is_awg2 { None } else { active_json.and_then(|j| j["random_trailers"].as_bool()) },
                            awg_disable_cookies: if is_awg2 { None } else { active_json.and_then(|j| j["disable_cookies"].as_bool()) },
                            awg_persistent_keepalive: if is_awg2 { None } else { active_json.and_then(|j| j["persistent_keepalive"].as_str()).map(|s| s.to_string()) },
                            awg_hop_ports: hop_ports.clone(),
                            awg_port_probing: Some(!is_awg2),
                            pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                            container_id: Some(cid.to_string()),
                            ..Default::default()
                        };
                        if restored_proto == "awg_vless" {
                            prof.vless_port = d["vless_port"].as_u64().map(|v| v as u16);
                            prof.sni = d["vless_sni"].as_str().unwrap_or("").to_string();
                            prof.uuid = d["vless_uuid"].as_str().map(|s| s.to_string());
                            prof.reality_public_key = cached_real_pub.clone();
                            prof.reality_short_id = cached_real_sid.clone();
                            if prof.uuid.is_some() {
                                prof.reality_flow = Some("xtls-rprx-vision".to_string());
                            }
                        }
                        prof.uri = build_profile_uri(&prof);
                        profiles.push(prof);
                    }
                } else if let Some(peers) = ep["peers"].as_array() {
                    if !client_priv.trim().is_empty() {
                        for (idx, p) in peers.iter().enumerate() {
                            let is_base = idx == 0;
                            let peer_name = if is_base {
                                proto_name.to_string()
                            } else {
                                format!("{} Устройство {}", proto_name, idx + 1)
                            };
                            let prof_name = if is_base {
                                format_base_name(proto_name, port)
                            } else {
                                peer_name
                            };
                            let default_ip = if is_awg2 { "10.8.1.2/24" } else { "10.8.0.2/32" };
                            let raw_addr = p["allowed_ips"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).unwrap_or(default_ip);
                            let peer_addr = if is_awg2 { raw_addr.replace("/32", "/24") } else { raw_addr.to_string() };
                            let peer_psk = p["pre_shared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                            let mut prof = VpnProfile {
                                id: Uuid::new_v4().to_string(),
                                name: prof_name,
                                protocol: proto_str.to_string(),
                                server: connect_host.to_string(),
                                port,
                                password: client_priv.clone(),
                                awg_private_key: Some(client_priv.clone()),
                                awg_public_key: Some(server_pub.clone()),
                                awg_preshared_key: peer_psk,
                                awg_local_address: Some(peer_addr),
                                awg_version: Some(proto_ver.to_string()),
                                awg_header_protection_key: hpk.clone(),
                                awg_jc: jc,
                                awg_jmin: jmin,
                                awg_jmax: jmax,
                                awg_s1: s1,
                                awg_s2: s2,
                                awg_s3: s3,
                                awg_s4: s4,
                                awg_h1: h1.clone(),
                                awg_h2: h2.clone(),
                                awg_h3: h3.clone(),
                                awg_h4: h4.clone(),
                                awg_rekey_after_time: if is_awg2 { None } else { active_json.and_then(|j| j["rekey_after_time"].as_str()).map(|s| s.to_string()) },
                                awg_rekey_timeout: if is_awg2 { None } else { active_json.and_then(|j| j["rekey_timeout"].as_str()).map(|s| s.to_string()) },
                                awg_reject_after_time: if is_awg2 { None } else { active_json.and_then(|j| j["reject_after_time"].as_str()).map(|s| s.to_string()) },
                                awg_keepalive_timeout: if is_awg2 { None } else { active_json.and_then(|j| j["keepalive_timeout"].as_str()).map(|s| s.to_string()) },
                                awg_max_handshake_attempts: if is_awg2 { None } else { active_json.and_then(|j| j["max_handshake_attempts"].as_str()).map(|s| s.to_string()) },
                                awg_random_trailers: if is_awg2 { None } else { active_json.and_then(|j| j["random_trailers"].as_bool()) },
                                awg_disable_cookies: if is_awg2 { None } else { active_json.and_then(|j| j["disable_cookies"].as_bool()) },
                                awg_persistent_keepalive: if is_awg2 { None } else { active_json.and_then(|j| j["persistent_keepalive"].as_str()).map(|s| s.to_string()) },
                                awg_hop_ports: hop_ports.clone(),
                                awg_port_probing: Some(!is_awg2),
                                pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                                container_id: Some(cid.to_string()),
                                ..Default::default()
                            };
                            prof.uri = build_profile_uri(&prof);
                            profiles.push(prof);
                        }
                    }
                }
            }
        }
    }

    (profiles, proto_labels, ports)
}

pub async fn fetch_vps_admin_state(
    app: &AppHandle,
    creds: &VpsCredentials,
) -> Result<VpsAdminState, String> {
    emit_log(
        app,
        &format!(
            "Авторизация на VPS {} и получение списка контейнеров и ключей...",
            creds.host
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let (_, docker_ps) = run_remote_cmd(
        &sess,
        "command -v docker >/dev/null 2>&1 && docker ps -a --format '{{.Names}}|{{.State}}' 2>/dev/null || true",
    )
    .await
    .unwrap_or((1, String::new()));

    let mut docker_states: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in docker_ps.lines() {
        if let Some((name, st)) = line.trim().split_once('|') {
            docker_states.insert(name.trim().to_string(), st.trim().to_string());
        }
    }

    let (legacy_code, legacy_cfg_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/config.json 2>/dev/null || true")
        .await
        .unwrap_or((1, String::new()));
    let (_, cdirs_raw) = run_remote_cmd(
        &sess,
        "for d in /opt/neuroveil/containers/*; do [ -f \"$d/config.json\" ] && basename \"$d\"; done 2>/dev/null || true",
    )
    .await
    .unwrap_or((1, String::new()));

    let container_ids: Vec<String> = cdirs_raw
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let has_legacy = legacy_code == 0 && !legacy_cfg_raw.trim().is_empty();
    let multi_container_host = container_ids.len() + (if has_legacy { 1 } else { 0 }) > 1;
    let connect_host = creds.host.trim().to_string();

    let (_, ipt_in_raw) = run_remote_cmd(&sess, "iptables -nvxL INPUT 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, ipt_out_raw) = run_remote_cmd(&sess, "iptables -nvxL OUTPUT 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, ipt_fwd_raw) = run_remote_cmd(&sess, "iptables -nvxL FORWARD 2>/dev/null").await.unwrap_or((1, String::new()));

    let mut containers: Vec<VpsContainerInfo> = Vec::new();
    let mut all_profiles: Vec<VpnProfile> = Vec::new();

    if has_legacy {
        let (_, legacy_status_out) = run_remote_cmd(&sess, "systemctl is-active sing-box 2>/dev/null || echo inactive")
            .await
            .unwrap_or((1, "inactive".to_string()));
        let (_, real_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/reality.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, awg_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, awg2_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg2.json 2>/dev/null || true").await.unwrap_or((1, String::new()));

        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&legacy_cfg_raw) {
            let (profs, protos, ports) = parse_container_profiles(
                "legacy-singbox",
                &connect_host,
                creds,
                &cfg,
                &real_raw,
                &awg_raw,
                &awg2_raw,
                creds.enable_port_hopping,
                multi_container_host,
            );
            let client_count = profs.len();
            all_profiles.extend(profs);
            containers.push(VpsContainerInfo {
                id: "legacy-singbox".to_string(),
                name: "Основной сервис (sing-box)".to_string(),
                runtime: "systemd".to_string(),
                status: if legacy_status_out.trim() == "active" { "running".to_string() } else { "stopped".to_string() },
                created_at: "Системный (/etc/sing-box)".to_string(),
                protocols: protos,
                ports,
                client_count,
                is_legacy: true,
            });
        }
    }

    for cid in &container_ids {
        let cdir = format!("/opt/neuroveil/containers/{}", cid);
        let (_, raw_cfg) = run_remote_cmd(&sess, &format!("cat \"{}/config.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&raw_cfg) else {
            continue;
        };
        let (_, meta_raw) = run_remote_cmd(&sess, &format!("cat \"{}/meta.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        let (_, real_raw) = run_remote_cmd(&sess, &format!("cat \"{}/reality.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        let (_, awg_raw) = run_remote_cmd(&sess, &format!("cat \"{}/awg.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        let (_, awg2_raw) = run_remote_cmd(&sess, &format!("cat \"{}/awg2.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));

        let meta = serde_json::from_str::<serde_json::Value>(&meta_raw).ok();
        let runtime = meta
            .as_ref()
            .and_then(|m| m["runtime"].as_str())
            .unwrap_or("systemd")
            .to_string();
        let c_name = meta
            .as_ref()
            .and_then(|m| m["name"].as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Контейнер {}", cid));
        let created_at = meta
            .as_ref()
            .and_then(|m| m["created_at"].as_str())
            .unwrap_or("")
            .to_string();
        let enable_port_hopping = meta
            .as_ref()
            .and_then(|m| m["enable_port_hopping"].as_bool())
            .unwrap_or(false);

        let status_str = if runtime == "docker" {
            let dname = meta
                .as_ref()
                .and_then(|m| m["docker_name"].as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("neuroveil-{}", cid));
            match docker_states.get(&dname).map(|s| s.as_str()) {
                Some("running") => "running".to_string(),
                _ => "stopped".to_string(),
            }
        } else {
            let sname = meta
                .as_ref()
                .and_then(|m| m["service_name"].as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("neuroveil-{}", cid));
            let (_, act_out) = run_remote_cmd(&sess, &format!("systemctl is-active \"{}\" 2>/dev/null || echo inactive", sname))
                .await
                .unwrap_or((1, "inactive".to_string()));
            if act_out.trim() == "active" {
                "running".to_string()
            } else {
                "stopped".to_string()
            }
        };

        let (profs, protos, ports) = parse_container_profiles(
            cid,
            &connect_host,
            creds,
            &cfg,
            &real_raw,
            &awg_raw,
            &awg2_raw,
            enable_port_hopping,
            multi_container_host,
        );
        let client_count = profs.len();
        all_profiles.extend(profs);

        containers.push(VpsContainerInfo {
            id: cid.clone(),
            name: c_name,
            runtime,
            status: status_str,
            created_at,
            protocols: protos,
            ports,
            client_count,
            is_legacy: false,
        });
    }

    let parse_port_bytes = |raw: &str, marker_prefix: &str, port: u16| -> u64 {
        let needle = format!("{}:{}", marker_prefix, port);
        let mut sum = 0u64;
        for line in raw.lines() {
            if line.contains(&needle) {
                let mut parts = line.split_whitespace();
                let _pkts = parts.next();
                if let Some(bytes_str) = parts.next() {
                    if let Ok(b) = bytes_str.parse::<u64>() {
                        sum = sum.saturating_add(b);
                    }
                }
            }
        }
        sum
    };

    let parse_ip_bytes = |raw: &str, ip_clean: &str, is_src: bool| -> u64 {
        let mut sum = 0u64;
        for line in raw.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 9 {
                let bytes_val = cols[1].parse::<u64>().unwrap_or(0);
                let src = cols[7].trim_end_matches("/32");
                let dst = cols[8].trim_end_matches("/32");
                if is_src && src == ip_clean {
                    sum = sum.saturating_add(bytes_val);
                } else if !is_src && dst == ip_clean {
                    sum = sum.saturating_add(bytes_val);
                }
            }
        }
        sum
    };

    let mut acct_setup = String::new();
    for p in &mut all_profiles {
        let port = p.port;
        let is_tcp = p.protocol == "vless" || p.protocol == "vless_ws" || p.protocol == "webrtc" || p.protocol == "shadowsocks" || p.protocol == "shadowtls" || p.protocol == "ssh";
        let is_udp = p.protocol != "vless" && p.protocol != "vless_ws" && p.protocol != "webrtc" && p.protocol != "shadowtls" && p.protocol != "ssh";
        if is_tcp {
            acct_setup.push_str(&format!(
                "iptables -C INPUT -p tcp --dport {port} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {port} -j ACCEPT 2>/dev/null || true\n\
                 iptables -C OUTPUT -p tcp --sport {port} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {port} -j ACCEPT 2>/dev/null || true\n"
            ));
        }
        if is_udp {
            acct_setup.push_str(&format!(
                "iptables -C INPUT -p udp --dport {port} -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport {port} -j ACCEPT 2>/dev/null || true\n\
                 iptables -C OUTPUT -p udp --sport {port} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p udp --sport {port} -j ACCEPT 2>/dev/null || true\n"
            ));
        }
        let mut up_b = parse_port_bytes(&ipt_in_raw, "dpt", port);
        let mut down_b = parse_port_bytes(&ipt_out_raw, "spt", port);

        if let Some(ref peer_ip_cidr) = p.awg_local_address {
            let ip_only = peer_ip_cidr.split('/').next().unwrap_or("").trim();
            if !ip_only.is_empty() {
                acct_setup.push_str(&format!(
                    "iptables -C FORWARD -s {ip}/32 -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {ip}/32 -j ACCEPT 2>/dev/null || true\n\
                     iptables -C FORWARD -d {ip}/32 -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {ip}/32 -j ACCEPT 2>/dev/null || true\n",
                    ip = ip_only
                ));
                let peer_up = parse_ip_bytes(&ipt_fwd_raw, ip_only, true);
                let peer_down = parse_ip_bytes(&ipt_fwd_raw, ip_only, false);
                if peer_up > 0 || peer_down > 0 {
                    up_b = peer_up;
                    down_b = peer_down;
                }
            }
        }
        p.total_up_bytes = up_b;
        p.total_down_bytes = down_b;
    }

    if !acct_setup.is_empty() {
        let _ = run_remote_cmd(&sess, &acct_setup).await;
    }
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    emit_log(
        app,
        &format!(
            "Успешно авторизовано на сервере. Контейнеров: {}, ключей: {}",
            containers.len(),
            all_profiles.len()
        ),
    );

    Ok(VpsAdminState {
        containers,
        profiles: all_profiles,
    })
}

pub async fn fetch_vps_profiles(
    app: &AppHandle,
    creds: &VpsCredentials,
) -> Result<Vec<VpnProfile>, String> {
    let admin_state = fetch_vps_admin_state(app, creds).await?;
    Ok(admin_state.profiles)
}

pub async fn manage_vps_container(
    app: &AppHandle,
    creds: &VpsCredentials,
    container_id: &str,
    action: &str,
) -> Result<String, String> {
    emit_log(
        app,
        &format!(
            "Выполнение действия '{}' над контейнером {} на {}...",
            action, container_id, creds.host
        ),
    );
    let sess = connect_ssh(app, creds).await?;
    let target = resolve_container_target(&sess, Some(container_id), None).await?;

    let cmd = match action {
        "start" | "restart" => {
            if target.runtime == "docker" {
                format!(
                    "docker {act} \"{u}\"\nsleep 1\nif [ \"$(docker inspect -f '{{{{.State.Running}}}}' \"{u}\" 2>/dev/null)\" = \"true\" ]; then echo \"OK_RUNNING\"; else echo \"ERR_STATE\"; fi",
                    act = action,
                    u = target.unit_name
                )
            } else {
                format!(
                    "systemctl {act} \"{u}\"\nsleep 1\nsystemctl is-active \"{u}\"",
                    act = action,
                    u = target.unit_name
                )
            }
        }
        "stop" => {
            if target.runtime == "docker" {
                format!("docker stop \"{u}\" >/dev/null 2>&1 || true\necho \"OK_STOPPED\"", u = target.unit_name)
            } else {
                format!("systemctl stop \"{u}\" >/dev/null 2>&1 || true\necho \"OK_STOPPED\"", u = target.unit_name)
            }
        }
        "delete" => {
            if target.id == "legacy-singbox" {
                "systemctl stop sing-box >/dev/null 2>&1 || true\n\
                 systemctl disable sing-box >/dev/null 2>&1 || true\n\
                 rm -f /etc/systemd/system/sing-box.service\n\
                 systemctl daemon-reload >/dev/null 2>&1 || true\n\
                 rm -rf /etc/sing-box\n\
                 echo \"OK_DELETED\""
                    .to_string()
            } else {
                format!(
                    "if command -v docker >/dev/null 2>&1; then docker rm -f \"{u}\" >/dev/null 2>&1 || true; fi\n\
                     systemctl stop \"{u}\" >/dev/null 2>&1 || true\n\
                     systemctl disable \"{u}\" >/dev/null 2>&1 || true\n\
                     rm -f \"/etc/systemd/system/{u}.service\"\n\
                     systemctl daemon-reload >/dev/null 2>&1 || true\n\
                     rm -rf \"{cdir}\"\n\
                     echo \"OK_DELETED\"",
                    u = target.unit_name,
                    cdir = target.cdir
                )
            }
        }
        other => {
            let _ = sess.disconnect(russh::Disconnect::ByApplication, "err", "en").await;
            return Err(format!("Неизвестное действие над контейнером: {}", other));
        }
    };

    let (code, out) = run_remote_cmd(&sess, &cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    if code != 0 && !out.contains("OK_") && !out.contains("active") {
        return Err(format!("Ошибка управления контейнером {}: {}", target.id, out));
    }

    let msg = match action {
        "start" => format!("Контейнер {} запущен", target.id),
        "restart" => format!("Контейнер {} перезапущен", target.id),
        "stop" => format!("Контейнер {} остановлен", target.id),
        "delete" => format!("Контейнер {} полностью удален с сервера", target.id),
        _ => "Готово".to_string(),
    };
    emit_log(app, &msg);
    Ok(msg)
}

pub async fn delete_vps_device(
    app: &AppHandle,
    creds: &VpsCredentials,
    profile: &VpnProfile,
) -> Result<(), String> {
    emit_log(
        app,
        &format!(
            "Удаление конфигурации «{}» ({}) с сервера {}...",
            profile.name, profile.protocol, creds.host
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let target = match resolve_container_target(
        &sess,
        profile.container_id.as_deref(),
        if profile.port > 0 { Some(profile.port) } else { None },
    )
    .await
    {
        Ok(t) => t,
        Err(_) => {
            let _ = sess.disconnect(russh::Disconnect::ByApplication, "done", "en").await;
            return Ok(());
        }
    };

    let (code, raw_cfg) = run_remote_cmd(&sess, &format!("cat \"{}/config.json\" 2>/dev/null", target.cdir)).await?;
    if code != 0 || raw_cfg.trim().is_empty() {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;
        return Ok(());
    }

    let mut cfg: serde_json::Value = serde_json::from_str(&raw_cfg)
        .map_err(|e| format!("Ошибка разбора серверного config.json: {}", e))?;

    let proto = profile.protocol.as_str();
    let mut awg_update_script = String::new();

    if proto == "amneziawg" || proto == "awg_quic" || proto == "awg_vless" || proto == "awg2" {
        let is_awg2 = proto == "awg2" || profile.awg_version.as_deref() == Some("2.0");
        let awg_file = if is_awg2 {
            format!("{}/awg2.json", target.cdir)
        } else {
            format!("{}/awg.json", target.cdir)
        };
        let (_, awg_raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg_file))
            .await
            .unwrap_or((1, String::new()));

        let prof_priv = profile
            .awg_private_key
            .as_deref()
            .unwrap_or(&profile.password)
            .trim();
        let prof_ip = profile.awg_local_address.as_deref().unwrap_or("").trim();
        let mut removed_pub_key: Option<String> = None;
        let mut removed_ip: Option<String> = if !prof_ip.is_empty() {
            Some(prof_ip.to_string())
        } else {
            None
        };
        let mut removed_vless_uuid: Option<String> = profile.uuid.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let mut remaining_devices_count = 0usize;

        if let Ok(mut awg_json) = serde_json::from_str::<serde_json::Value>(&awg_raw) {
            if let Some(devs) = awg_json["devices"].as_array_mut() {
                devs.retain(|d| {
                    let d_priv = d["client_private_key"].as_str().unwrap_or("").trim();
                    let d_ip = d["ip"].as_str().unwrap_or("").trim();
                    let d_name = d["name"].as_str().unwrap_or("").trim();
                    let is_match = (!prof_priv.is_empty() && d_priv == prof_priv)
                        || (!prof_ip.is_empty() && d_ip == prof_ip)
                        || d_name == profile.name.trim();
                    if is_match {
                        if let Some(pk) = d["client_public_key"].as_str() {
                            removed_pub_key = Some(pk.to_string());
                        }
                        if let Some(ip) = d["ip"].as_str() {
                            removed_ip = Some(ip.to_string());
                        }
                        if removed_vless_uuid.is_none() {
                            removed_vless_uuid = d["vless_uuid"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                        }
                        false
                    } else {
                        true
                    }
                });
                remaining_devices_count = devs.len();
                if let Some(first) = devs.first() {
                    let f_priv = first["client_private_key"].as_str().unwrap_or("").to_string();
                    let f_pub = first["client_public_key"].as_str().unwrap_or("").to_string();
                    awg_json["client_private_key"] = json!(f_priv);
                    awg_json["client_public_key"] = json!(f_pub);
                } else {
                    awg_json["client_private_key"] = json!("");
                    awg_json["client_public_key"] = json!("");
                }
            } else {
                let base_priv = awg_json["client_private_key"].as_str().unwrap_or("").trim();
                if base_priv == prof_priv || prof_priv.is_empty() {
                    if let Some(pk) = awg_json["client_public_key"].as_str() {
                        removed_pub_key = Some(pk.to_string());
                    }
                    awg_json["client_private_key"] = json!("");
                    awg_json["client_public_key"] = json!("");
                    awg_json["devices"] = json!([]);
                    remaining_devices_count = 0;
                }
            }

            if remaining_devices_count == 0 {
                awg_update_script = format!("rm -f \"{}\"\n", awg_file);
            } else {
                let new_awg_str = serde_json::to_string_pretty(&awg_json).unwrap_or_default();
                awg_update_script = format!(
                    "cat << 'EOF_AWG' > \"{awg_file}\"\n{new_awg_str}\nEOF_AWG\nchmod 600 \"{awg_file}\"\n",
                    awg_file = awg_file,
                    new_awg_str = new_awg_str
                );
            }
        }

        if proto == "awg_vless" {
            if let Some(ref vu) = removed_vless_uuid {
                if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
                    for ib in inbounds.iter_mut() {
                        let ib_type = ib["type"].as_str().unwrap_or("");
                        let tr_type = ib["transport"]["type"].as_str().unwrap_or("");
                        if ib_type == "vless" && tr_type.is_empty() {
                            if let Some(users) = ib["users"].as_array_mut() {
                                users.retain(|u| u["uuid"].as_str().unwrap_or("").trim() != vu.as_str());
                            }
                        }
                    }
                }
            }
        }

        if let Some(endpoints) = cfg["endpoints"].as_array_mut() {
            let mut ep_tags_to_remove = Vec::new();
            let mut cidrs_to_remove: Vec<String> = Vec::new();
            endpoints.retain_mut(|ep| {
                if ep["type"].as_str() != Some("wireguard") {
                    return true;
                }
                let ep_port = ep["listen_port"].as_u64().unwrap_or(0) as u16;
                let tag = ep["tag"].as_str().unwrap_or("").to_string();
                let matches_ep = (profile.port > 0 && ep_port == profile.port)
                    || (is_awg2 && tag == "awg2-in")
                    || (!is_awg2 && tag == "awg-in");
                if !matches_ep {
                    return true;
                }

                if let Some(peers) = ep["peers"].as_array_mut() {
                    let initial_len = peers.len();
                    peers.retain(|p| {
                        let p_pub = p["public_key"].as_str().unwrap_or("").trim();
                        if let Some(ref rem_pub) = removed_pub_key {
                            if !rem_pub.is_empty() && p_pub == rem_pub {
                                return false;
                            }
                        }
                        if let Some(ref rem_ip) = removed_ip {
                            if let Some(allowed) = p["allowed_ips"].as_array() {
                                if allowed.iter().any(|ip| ip.as_str() == Some(rem_ip.as_str())) {
                                    return false;
                                }
                            }
                        }
                        true
                    });
                    if peers.len() == initial_len && remaining_devices_count == 0 && peers.len() == 1 {
                        peers.clear();
                    }
                    if peers.is_empty() {
                        ep_tags_to_remove.push(tag);
                        if let Some(addr_arr) = ep["address"].as_array() {
                            for a in addr_arr {
                                if let Some(s) = a.as_str() {
                                    if let Some(ip_part) = s.split('/').next() {
                                        let mut parts: Vec<&str> = ip_part.split('.').collect();
                                        if parts.len() == 4 {
                                            parts.pop();
                                            cidrs_to_remove.push(format!("{}.0/24", parts.join(".")));
                                        }
                                    }
                                }
                            }
                        }
                        return false;
                    }
                }
                true
            });

            if !ep_tags_to_remove.is_empty() {
                if let Some(rules) = cfg["route"]["rules"].as_array_mut() {
                    rules.retain(|r| {
                        if let Some(inb) = r["inbound"].as_array() {
                            if inb.iter().any(|v| {
                                v.as_str()
                                    .map(|s| ep_tags_to_remove.iter().any(|t| t == s))
                                    .unwrap_or(false)
                            }) {
                                return false;
                            }
                        }
                        if let Some(src_arr) = r["source_ip_cidr"].as_array() {
                            if src_arr.iter().any(|v| {
                                v.as_str()
                                    .map(|s| cidrs_to_remove.iter().any(|c| c == s))
                                    .unwrap_or(false)
                            }) {
                                return false;
                            }
                        }
                        true
                    });
                }
            }
        }
    } else if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
        let prof_pw = profile.password.trim();
        let prof_st_pw = profile.shadowtls_password.as_deref().unwrap_or("").trim();
        let prof_uuid = profile.uuid.as_deref().unwrap_or(prof_pw).trim();
        let prof_name = profile.name.trim();

        inbounds.retain_mut(|ib| {
            let ib_type = ib["type"].as_str().unwrap_or("");
            let tr_type = ib["transport"]["type"].as_str().unwrap_or("");
            let actual_ib_proto = if ib_type == "vless" && tr_type == "ws" {
                "vless_ws"
            } else if ib_type == "vless" && tr_type == "grpc" {
                "webrtc"
            } else {
                ib_type
            };
            let ib_port = ib["listen_port"].as_u64().unwrap_or(0) as u16;
            if actual_ib_proto != proto || (profile.port > 0 && ib_port != profile.port) {
                return true;
            }
            if let Some(users) = ib["users"].as_array_mut() {
                let before = users.len();
                users.retain(|u| {
                    let u_name = u["name"].as_str().unwrap_or("").trim();
                    let u_uuid = u["uuid"].as_str().unwrap_or("").trim();
                    let u_pw = u["password"].as_str().unwrap_or("").trim();
                    let matched = if proto == "shadowtls" {
                        (!prof_st_pw.is_empty() && u_pw == prof_st_pw) || u_name == prof_name
                    } else {
                        (!prof_uuid.is_empty() && u_uuid == prof_uuid)
                            || (!prof_pw.is_empty() && u_pw == prof_pw)
                            || (!prof_pw.is_empty() && u_uuid == prof_pw)
                            || u_name == prof_name
                    };
                    !matched
                });
                if users.len() == before && users.len() == 1 {
                    users.clear();
                }
                !users.is_empty()
            } else {
                false
            }
        });
    }

    let has_inbounds = cfg["inbounds"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_endpoints = cfg["endpoints"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if !has_endpoints {
        if let Some(obj) = cfg.as_object_mut() {
            obj.remove("endpoints");
        }
    }

    let new_cfg_str = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    let service_action = build_container_restart_cmd(&target, has_inbounds || has_endpoints);

    let update_cmd = format!(
        r#"
cat << 'EOF_CFG' > "{cdir}/config.json"
{new_cfg_str}
EOF_CFG
{awg_update_script}
{service_action}
"#,
        cdir = target.cdir,
        new_cfg_str = new_cfg_str,
        awg_update_script = awg_update_script,
        service_action = service_action
    );

    let _ = run_remote_cmd(&sess, &update_cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    emit_log(
        app,
        &format!(
            "Конфигурация «{}» успешно удалена с сервера {} (контейнер {})!",
            profile.name, creds.host, target.id
        ),
    );
    Ok(())
}

use crate::models::{
    BridgeClientConfig, CreateBridgeClientRequest, DeployResult, VpnProfile, VpsAdminState,
    VpsContainerInfo, VpsCredentials, VpsMultiProtocolGroup,
};
use crate::singbox::{build_awg_conf, build_bridge_exit_nodes, build_profile_uri, urlencoding};
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

const ENIGMA_SRV_C_SRC: &str = include_str!("enigma_srv.c");

fn build_enigma_binary_install_cmd() -> String {
    format!(
        r#"
set -e
if ! command -v gcc >/dev/null 2>&1; then
    if command -v apt-get >/dev/null 2>&1; then
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -y >/dev/null 2>&1 || true
        apt-get install -y gcc libc6-dev >/dev/null 2>&1 || true
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y gcc glibc-devel >/dev/null 2>&1 || true
    elif command -v yum >/dev/null 2>&1; then
        yum install -y gcc glibc-devel >/dev/null 2>&1 || true
    elif command -v apk >/dev/null 2>&1; then
        apk add --no-cache gcc musl-dev >/dev/null 2>&1 || true
    fi
fi
if ! command -v gcc >/dev/null 2>&1; then
    echo "ERR_NO_GCC"
    exit 1
fi
mkdir -p /usr/local/src /usr/local/bin
cat << 'EOF_ENIGMA_C_SRC' > /usr/local/src/neuroveil-enigma-srv.c
{src}
EOF_ENIGMA_C_SRC
gcc -O3 -std=c99 -Wall -Wextra -o /usr/local/bin/neuroveil-enigma-srv.tmp /usr/local/src/neuroveil-enigma-srv.c
chmod +x /usr/local/bin/neuroveil-enigma-srv.tmp
mv -f /usr/local/bin/neuroveil-enigma-srv.tmp /usr/local/bin/neuroveil-enigma-srv
echo "ENIGMA_BIN_OK"
"#,
        src = ENIGMA_SRV_C_SRC
    )
}

fn build_enigma_service_start_cmd(cid: &str, cdir: &str, ext_port: u16, wg_port: u16) -> String {
    format!(
        r#"
if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    cat << 'EOF_ENIGMA_SVC' > "/etc/systemd/system/neuroveil-enigma-{cid}.service"
[Unit]
Description=NeuroVeil Enigma v1 Polymorphic Gateway ({cid})
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/neuroveil-enigma-srv {ext_p} {wg_p} {cdir}/enigma.nvkey
Restart=always
RestartSec=2
OOMScoreAdjust=-900
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF_ENIGMA_SVC
    systemctl daemon-reload >/dev/null 2>&1 || true
    systemctl enable "neuroveil-enigma-{cid}" >/dev/null 2>&1 || true
    systemctl restart "neuroveil-enigma-{cid}"
    sleep 1
    if systemctl is-active --quiet "neuroveil-enigma-{cid}"; then
        echo "ENIGMA_ACTIVE"
    else
        journalctl -u "neuroveil-enigma-{cid}" -n 15 --no-pager || true
        exit 1
    fi
else
    if [ -f "{cdir}/enigma.pid" ]; then
        kill "$(cat "{cdir}/enigma.pid")" >/dev/null 2>&1 || true
    fi
    nohup /usr/local/bin/neuroveil-enigma-srv {ext_p} {wg_p} "{cdir}/enigma.nvkey" >/var/log/neuroveil-enigma-{cid}.log 2>&1 &
    echo $! > "{cdir}/enigma.pid"
    sleep 1
    if kill -0 "$(cat "{cdir}/enigma.pid")" 2>/dev/null; then
        echo "ENIGMA_ACTIVE"
    else
        cat /var/log/neuroveil-enigma-{cid}.log 2>/dev/null || true
        exit 1
    fi
fi
"#,
        cid = cid,
        cdir = cdir,
        ext_p = ext_port,
        wg_p = wg_port
    )
}

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

async fn detect_active_tunnel_proxy() -> Option<String> {
    for port in [2081u16, 2082, 2083, 2084, 2080] {
        let addr = format!("127.0.0.1:{}", port);
        if tokio::time::timeout(
            Duration::from_millis(220),
            TcpStream::connect(&addr),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
        {
            return Some(format!("socks5://{}", addr));
        }
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
                        &tunnel_proxy,
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
        || creds.deploy_enigma
        || creds.deploy_ssh
        || creds.deploy_shadowtls
        || creds.deploy_vless_ws
        || creds.deploy_webrtc;

    if !any_proto {
        return Err("Выберите хотя бы один протокол для развертывания (VLESS Reality, Hysteria 2, TUIC v5, Shadowsocks, Enigma v1, Amnezia 3.1, AWG + QUIC Pre-Warm, Гибрид AWG + Reality, AmneziaWG 2.0, ShadowTLS, Cloudflare WS, WebRTC или SSH Tunnel)".to_string());
    }

    let need_vless_server = creds.deploy_vless || creds.deploy_awg_vless;
    let need_awg_server = creds.deploy_awg || creds.deploy_awg_quic || creds.deploy_awg_vless;
    let need_container = need_vless_server
        || need_awg_server
        || creds.deploy_hy2
        || creds.deploy_tuic
        || creds.deploy_ss
        || creds.deploy_awg2
        || creds.deploy_enigma
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
    if creds.deploy_enigma && (creds.enigma_port == 0 || creds.enigma_port == 51820 || creds.enigma_port == 51821) {
        creds.enigma_port = random_u32_in_range(20000, 55000) as u16;
        emit_log(app, &format!("Авто-рандомизация: выбран внешний порт Enigma v1 UDP {}", creds.enigma_port));
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
grep -hoE '"(listen_port|external_port|wg_port)"\s*:\s*[0-9]+' /etc/sing-box/config.json /opt/neuroveil/containers/*/config.json /opt/neuroveil/containers/*/enigma.json 2>/dev/null | grep -oE '[0-9]+' || true
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
    let mut enigma_octet = 7u8;
    let mut enigma_wg_port = 0u16;
    if creds.deploy_enigma {
        creds.enigma_port = allocate_free_port(creds.enigma_port, &mut used_ports, true);
        enigma_wg_port = allocate_free_port(random_u32_in_range(40000, 58000) as u16, &mut used_ports, true);
        enigma_octet = allocate_free_subnet_octet(&mut used_octets);
        emit_log(
            app,
            &format!(
                "Enigma v1 в контейнере {}: внешний UDP {} -> локальный WG UDP {} | подсеть 10.8.{}.0/24",
                container_id, creds.enigma_port, enigma_wg_port, enigma_octet
            ),
        );
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

    let enigma_client_ip_32 = format!("10.8.{}.2/32", enigma_octet);
    let enigma_client_ip_24 = format!("10.8.{}.2/24", enigma_octet);
    let enigma_server_ip = format!("10.8.{}.1/24", enigma_octet);
    let enigma_cidr = format!("10.8.{}.0/24", enigma_octet);

    let (
        enigma_server_priv,
        enigma_server_pub,
        enigma_client_priv,
        enigma_client_pub,
        enigma_psk,
        enigma_keybook_b64,
    ) = if creds.deploy_enigma {
        emit_log(app, "Шаг 3/6: Генерация ключей и книги роторов (.nvkey 16 KB) для Enigma v1...");
        let (s_priv, s_pub) = generate_wg_keypair_remote(&sess).await?;
        let (c_priv, c_pub) = generate_wg_keypair_remote(&sess).await?;
        let psk = generate_random_base64_32();
        let kb_b64 = crate::enigma::EnigmaKeybook::generate()?.to_base64();
        let enigma_hop = if creds.enable_port_hopping && !is_additional_container && !creds.deploy_hy2 {
            "20000:65000"
        } else {
            ""
        };

        let enigma_save_cmd = format!(
            r#"set -e
printf '%s' '{kb_b64}' | base64 -d > "{cdir}/enigma.nvkey"
chmod 600 "{cdir}/enigma.nvkey"
NV_SZ=$(wc -c < "{cdir}/enigma.nvkey" | tr -d ' ')
if [ "$NV_SZ" != "16384" ]; then
    echo "ERR_NVKEY_SIZE=$NV_SZ"
    exit 1
fi
cat << 'EOF_ENIGMA_JSON' > "{cdir}/enigma.json"
{{
  "server_private_key": "{s_priv}",
  "server_public_key": "{s_pub}",
  "client_private_key": "{c_priv}",
  "client_public_key": "{c_pub}",
  "preshared_key": "{psk}",
  "external_port": {ext_p},
  "wg_port": {wg_p},
  "keybook_b64": "{kb_b64}",
  "mimic": "auto",
  "hop_ports": "{enigma_hop}",
  "devices": [
    {{
      "name": "default",
      "protocol": "enigma",
      "client_private_key": "{c_priv}",
      "client_public_key": "{c_pub}",
      "preshared_key": "{psk}",
      "ip": "{client_ip}",
      "mimic": "auto"
    }}
  ]
}}
EOF_ENIGMA_JSON
chmod 600 "{cdir}/enigma.json"
echo "ENIGMA_KEYS_OK"
"#,
            cdir = container_dir,
            kb_b64 = kb_b64,
            s_priv = s_priv,
            s_pub = s_pub,
            c_priv = c_priv,
            c_pub = c_pub,
            psk = psk,
            ext_p = creds.enigma_port,
            wg_p = enigma_wg_port,
            enigma_hop = enigma_hop,
            client_ip = enigma_client_ip_32
        );
        let (ek_code, ek_out) = run_remote_cmd(&sess, &enigma_save_cmd).await?;
        if ek_code != 0 || !ek_out.contains("ENIGMA_KEYS_OK") {
            return Err(format!("Ошибка записи книги ключей Enigma на VPS: {}", ek_out));
        }

        emit_log(app, "Компиляция и установка шлюза Enigma v1 на VPS (C99)... ");
        let (eb_code, eb_out) = run_remote_cmd(&sess, &build_enigma_binary_install_cmd()).await?;
        if eb_code != 0 || !eb_out.contains("ENIGMA_BIN_OK") {
            return Err(format!("Ошибка сборки шлюза Enigma v1 на VPS: {}", eb_out));
        }

        (s_priv, s_pub, c_priv, c_pub, psk, kb_b64)
    } else {
        (String::new(), String::new(), String::new(), String::new(), String::new(), String::new())
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
    emit_log(
        app,
        "Активация лёгкого фонового анти-слипа VPS (ARP/UDP keepalive + защита от swap)...",
    );
    let _ = run_remote_cmd(&sess, build_vps_antisleep_install_cmd()).await;

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

    if creds.deploy_enigma {
        proto_labels.push("Enigma v1".to_string());
        proto_ports.push(creds.enigma_port);
        let enigma_ep = json!({
            "type": "wireguard",
            "tag": "enigma-in",
            "listen_port": enigma_wg_port,
            "address": [enigma_server_ip],
            "private_key": enigma_server_priv,
            "mtu": 1280,
            "peers": [
                {
                    "public_key": enigma_client_pub,
                    "pre_shared_key": enigma_psk,
                    "allowed_ips": [enigma_client_ip_32]
                }
            ]
        });
        endpoints.push(enigma_ep);

        route_rules.push(json!({
            "inbound": ["enigma-in"],
            "outbound": "direct"
        }));
        route_rules.push(json!({
            "source_ip_cidr": [enigma_cidr],
            "outbound": "direct"
        }));

        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {ext_p}/udp >/dev/null 2>&1 || true; ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={ext_p}/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {ext_p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {ext_p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {ext_p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {ext_p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C INPUT -p udp ! -i lo --dport {wg_p} -j DROP 2>/dev/null || iptables -I INPUT 1 -p udp ! -i lo --dport {wg_p} -j DROP >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -s {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -d {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -C POSTROUTING -s {cidr} -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s {cidr} -j MASQUERADE >/dev/null 2>&1 || true\n",
            ext_p = creds.enigma_port,
            wg_p = enigma_wg_port,
            cidr = enigma_cidr
        ));

        if creds.enable_port_hopping && !is_additional_container && !creds.deploy_hy2 {
            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow 20000:65000/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port=20000-65000/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport 20000:65000 -j REDIRECT --to-ports {ext_p} 2>/dev/null || iptables -t nat -A PREROUTING -p udp --dport 20000:65000 -j REDIRECT --to-ports {ext_p} >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport 20000:65000 -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport 20000:65000 -j ACCEPT >/dev/null 2>&1 || true\n",
                ext_p = creds.enigma_port
            ));
        }
    }

    // Exempt this container's dedicated UDP ports from any port-hopping PREROUTING REDIRECT rules
    let mut active_udp_ports: Vec<u16> = Vec::new();
    if creds.deploy_hy2 && creds.hy2_port > 0 { active_udp_ports.push(creds.hy2_port); }
    if creds.deploy_tuic && creds.tuic_port > 0 { active_udp_ports.push(creds.tuic_port); }
    if creds.deploy_ss && creds.ss_port > 0 { active_udp_ports.push(creds.ss_port); }
    if need_awg_server && creds.awg_port > 0 { active_udp_ports.push(creds.awg_port); }
    if creds.deploy_awg2 && creds.awg2_port > 0 { active_udp_ports.push(creds.awg2_port); }
    if creds.deploy_enigma && creds.enigma_port > 0 { active_udp_ports.push(creds.enigma_port); }
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
        "enable_port_hopping": creds.enable_port_hopping && !is_additional_container,
        "has_enigma": creds.deploy_enigma,
        "enigma_port": creds.enigma_port,
        "enigma_wg_port": enigma_wg_port
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
OOMScoreAdjust=-900
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

    if creds.deploy_enigma {
        emit_log(
            app,
            &format!(
                "Запуск шлюза Enigma v1 (UDP {} <-> 127.0.0.1:{})...",
                creds.enigma_port, enigma_wg_port
            ),
        );
        let enigma_svc_cmd = build_enigma_service_start_cmd(
            &container_id,
            &container_dir,
            creds.enigma_port,
            enigma_wg_port,
        );
        let (e_code, e_out) = run_remote_cmd(&sess, &enigma_svc_cmd).await?;
        if e_code != 0 || !e_out.contains("ENIGMA_ACTIVE") {
            let _ = sess
                .disconnect(russh::Disconnect::ByApplication, "error", "en")
                .await;
            return Err(format!(
                "Шлюз Enigma v1 (neuroveil-enigma-{}) не запустился: {}",
                container_id, e_out
            ));
        }
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

    if creds.deploy_enigma {
        let enigma_hop = if creds.enable_port_hopping && !is_additional_container && !creds.deploy_hy2 {
            Some("20000:65000".to_string())
        } else {
            None
        };
        let mut enigma_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: format_prof_name("Enigma v1", creds.enigma_port),
            protocol: "enigma".to_string(),
            server: connect_host.clone(),
            port: creds.enigma_port,
            password: enigma_client_priv.clone(),
            awg_private_key: Some(enigma_client_priv),
            awg_public_key: Some(enigma_server_pub),
            awg_preshared_key: Some(enigma_psk),
            awg_local_address: Some(enigma_client_ip_24),
            awg_hop_ports: enigma_hop.clone(),
            hop_ports: enigma_hop,
            enigma_keybook: Some(enigma_keybook_b64),
            enigma_mimic: Some("auto".to_string()),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            container_id: Some(container_id.clone()),
            ..Default::default()
        };
        enigma_profile.uri = build_profile_uri(&enigma_profile);
        created_profiles.push(enigma_profile);
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
for pf in /opt/neuroveil/containers/*/enigma.pid /etc/sing-box/enigma.pid; do
    if [ -f "$pf" ]; then
        kill "$(cat "$pf" 2>/dev/null)" >/dev/null 2>&1 || true
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
rm -f /usr/local/bin/neuroveil-enigma-srv
rm -f /usr/local/src/neuroveil-enigma-srv.c
rm -f /var/log/neuroveil-enigma-*.log
rm -f /usr/local/bin/neuroveil-antisleep.sh
rm -f /var/run/neuroveil-antisleep.pid
rm -f /etc/sysctl.d/99-neuroveil-vpn.conf
rm -f /etc/sysctl.d/99-neuroveil-antisleep.conf
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
struct StoredMultiItem {
    protocol: String,
    port: u16,
    credential: String,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredMultiGroup {
    id: String,
    name: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    items: Vec<StoredMultiItem>,
}

fn profile_credential(p: &VpnProfile) -> String {
    match p.protocol.as_str() {
        "vless" | "vless_ws" | "webrtc" => p
            .uuid
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&p.password)
            .trim()
            .to_string(),
        "shadowtls" => p
            .shadowtls_password
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&p.password)
            .trim()
            .to_string(),
        "amneziawg" | "awg_quic" | "awg_vless" | "awg2" | "enigma" | "wireguard" => p
            .awg_private_key
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&p.password)
            .trim()
            .to_string(),
        _ => p.password.trim().to_string(),
    }
}

fn apply_multi_groups(
    profiles: &mut [VpnProfile],
    cid: &str,
    multi_raw: &str,
) -> Vec<VpsMultiProtocolGroup> {
    let trimmed = multi_raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(stored_groups) = serde_json::from_str::<Vec<StoredMultiGroup>>(trimmed) else {
        return Vec::new();
    };

    let mut assigned: HashSet<usize> = HashSet::new();
    let mut out_groups: Vec<VpsMultiProtocolGroup> = Vec::new();

    for grp in stored_groups {
        let mut matched_indices: Vec<usize> = Vec::new();
        for item in &grp.items {
            let item_cred = item.credential.trim();
            // 1. Exact credential match (protocol + port + credential)
            let mut found_idx = profiles.iter().enumerate().find_map(|(idx, p)| {
                if assigned.contains(&idx) || matched_indices.contains(&idx) {
                    return None;
                }
                if p.protocol == item.protocol
                    && (item.port == 0 || p.port == item.port)
                    && !item_cred.is_empty()
                    && profile_credential(p) == item_cred
                {
                    Some(idx)
                } else {
                    None
                }
            });

            // 2. Fallback match by IP or label (e.g. Shadowsocks sharing inbound password)
            if found_idx.is_none() {
                found_idx = profiles.iter().enumerate().find_map(|(idx, p)| {
                    if assigned.contains(&idx) || matched_indices.contains(&idx) {
                        return None;
                    }
                    let ip_match = item
                        .ip
                        .as_deref()
                        .map(|ip| !ip.trim().is_empty() && p.awg_local_address.as_deref() == Some(ip))
                        .unwrap_or(false);
                    let label_match = !item.label.trim().is_empty() && p.name.trim() == item.label.trim();
                    if p.protocol == item.protocol
                        && (item.port == 0 || p.port == item.port)
                        && (ip_match || label_match)
                    {
                        Some(idx)
                    } else {
                        None
                    }
                });
            }

            if let Some(idx) = found_idx {
                matched_indices.push(idx);
            }
        }

        if matched_indices.len() >= 2 {
            let mut proto_list = Vec::new();
            let mut id_list = Vec::new();
            for &idx in &matched_indices {
                assigned.insert(idx);
                profiles[idx].multi_group_id = Some(grp.id.clone());
                profiles[idx].multi_group_name = Some(grp.name.clone());
                proto_list.push(profiles[idx].protocol.clone());
                id_list.push(profiles[idx].id.clone());
            }
            out_groups.push(VpsMultiProtocolGroup {
                id: grp.id,
                name: grp.name,
                container_id: cid.to_string(),
                created_at: grp.created_at,
                protocols: proto_list,
                profile_ids: id_list,
            });
        }
    }

    out_groups
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateDeviceKeysResult {
    pub created: Vec<VpnProfile>,
    pub skipped: Vec<String>,
}

async fn generate_wg_keypair_remote(sess: &Handle<SshClientHandler>) -> Result<(String, String), String> {
    let (code, out) = run_remote_cmd(sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
    if code != 0 {
        return Err(format!("Ошибка генерации ключей WireGuard: {}", out));
    }
    let mut priv_key = String::new();
    let mut pub_key = String::new();
    for line in out.lines() {
        let l = line.trim();
        if l.starts_with("PrivateKey:") {
            priv_key = l.trim_start_matches("PrivateKey:").trim().to_string();
        } else if l.starts_with("PublicKey:") {
            pub_key = l.trim_start_matches("PublicKey:").trim().to_string();
        }
    }
    if priv_key.is_empty() || pub_key.is_empty() {
        return Err(format!("Не удалось разобрать ключи WireGuard:\n{}", out));
    }
    Ok((priv_key, pub_key))
}

async fn generate_reality_keypair_remote(
    sess: &Handle<SshClientHandler>,
) -> Result<(String, String, String), String> {
    let (code, out) = run_remote_cmd(sess, "/usr/local/bin/sing-box generate reality-keypair").await?;
    if code != 0 {
        return Err(format!("Ошибка генерации ключей Reality: {}", out));
    }
    let mut priv_key = String::new();
    let mut pub_key = String::new();
    for line in out.lines() {
        let l = line.trim();
        if l.starts_with("PrivateKey:") {
            priv_key = l.trim_start_matches("PrivateKey:").trim().to_string();
        } else if l.starts_with("PublicKey:") {
            pub_key = l.trim_start_matches("PublicKey:").trim().to_string();
        }
    }
    if priv_key.is_empty() || pub_key.is_empty() {
        return Err(format!("Не удалось получить ключи Reality:\n{}", out));
    }
    let (_, sid_out) = run_remote_cmd(sess, "openssl rand -hex 8").await?;
    let sid = sid_out
        .trim()
        .lines()
        .last()
        .unwrap_or("0123456789abcdef")
        .trim()
        .to_string();
    Ok((priv_key, pub_key, sid))
}

async fn probe_vps_used_ports_and_octets(
    sess: &Handle<SshClientHandler>,
    cfg: &serde_json::Value,
) -> (HashSet<u16>, HashSet<u8>) {
    let (_, probe_out) = run_remote_cmd(
        sess,
        r#"
echo "===PORTS==="
ss -tulnH 2>/dev/null | awk '{print $5}' | rev | cut -d: -f1 | rev | sort -un || true
grep -hoE '"(listen_port|external_port|wg_port)"\s*:\s*[0-9]+' /etc/sing-box/config.json /etc/sing-box/enigma.json /opt/neuroveil/containers/*/config.json /opt/neuroveil/containers/*/enigma.json 2>/dev/null | grep -oE '[0-9]+' || true
echo "===SUBNETS==="
grep -hoE '10\.8\.[0-9]+' /etc/sing-box/config.json /opt/neuroveil/containers/*/config.json 2>/dev/null | sort -u || true
"#,
    )
    .await
    .unwrap_or((0, String::new()));

    let mut used_ports: HashSet<u16> = HashSet::new();
    let mut used_octets: HashSet<u8> = HashSet::new();
    let mut section = "";
    for line in probe_out.lines() {
        let t = line.trim();
        if t == "===PORTS===" {
            section = "ports";
            continue;
        } else if t == "===SUBNETS===" {
            section = "subnets";
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
            _ => {}
        }
    }

    if let Some(inbounds) = cfg["inbounds"].as_array() {
        for ib in inbounds {
            if let Some(p) = ib["listen_port"].as_u64() {
                if p > 0 {
                    used_ports.insert(p as u16);
                }
            }
        }
    }
    if let Some(endpoints) = cfg["endpoints"].as_array() {
        for ep in endpoints {
            if let Some(p) = ep["listen_port"].as_u64() {
                if p > 0 {
                    used_ports.insert(p as u16);
                }
            }
            if let Some(addrs) = ep["address"].as_array() {
                for a in addrs {
                    if let Some(s) = a.as_str() {
                        if let Some(rest) = s.strip_prefix("10.8.") {
                            if let Some(oct_str) = rest.split('.').next() {
                                if let Ok(oct) = oct_str.parse::<u8>() {
                                    used_octets.insert(oct);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (used_ports, used_octets)
}

async fn ensure_container_tls_certs_remote(
    sess: &Handle<SshClientHandler>,
    target: &ResolvedContainer,
    sni: &str,
    certs_ready: &mut bool,
) -> Result<(String, String), String> {
    if !*certs_ready {
        let cert_cmd = format!(
            r#"
if [ ! -s "{cdir}/cert.pem" ] || [ ! -s "{cdir}/key.pem" ]; then
    openssl ecparam -genkey -name prime256v1 -out "{cdir}/key.pem" && \
    openssl req -new -x509 -key "{cdir}/key.pem" -out "{cdir}/cert.pem" -subj "/CN={sni}" -days 3650 && \
    chmod 600 "{cdir}/key.pem" "{cdir}/cert.pem"
fi
"#,
            cdir = target.cdir,
            sni = sni
        );
        let (code, out) = run_remote_cmd(sess, &cert_cmd).await?;
        if code != 0 {
            return Err(format!("Ошибка генерации TLS-сертификатов контейнера: {}", out));
        }
        *certs_ready = true;
    }

    if target.runtime == "docker" {
        Ok((
            "/etc/sing-box/cert.pem".to_string(),
            "/etc/sing-box/key.pem".to_string(),
        ))
    } else {
        Ok((
            format!("{}/cert.pem", target.cdir),
            format!("{}/key.pem", target.cdir),
        ))
    }
}

fn ensure_route_direct_for_wg(cfg: &mut serde_json::Value, tag: &str, cidr: &str) {
    if !cfg["outbounds"].is_array() {
        cfg["outbounds"] = json!([{ "type": "direct", "tag": "direct" }]);
    } else if let Some(obs) = cfg["outbounds"].as_array_mut() {
        if !obs.iter().any(|o| o["tag"].as_str() == Some("direct")) {
            obs.push(json!({ "type": "direct", "tag": "direct" }));
        }
    }

    if !cfg["route"].is_object() {
        cfg["route"] = json!({ "rules": [], "final": "direct" });
    }
    if cfg["route"]["final"].as_str().unwrap_or("").is_empty() {
        cfg["route"]["final"] = json!("direct");
    }
    if !cfg["route"]["rules"].is_array() {
        cfg["route"]["rules"] = json!([]);
    }
    if let Some(rules) = cfg["route"]["rules"].as_array_mut() {
        let has_inbound_rule = rules.iter().any(|r| {
            r["inbound"]
                .as_array()
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(tag)))
                .unwrap_or(false)
        });
        if !has_inbound_rule {
            rules.push(json!({
                "inbound": [tag],
                "outbound": "direct"
            }));
        }
        let has_cidr_rule = rules.iter().any(|r| {
            r["source_ip_cidr"]
                .as_array()
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(cidr)))
                .unwrap_or(false)
        });
        if !has_cidr_rule {
            rules.push(json!({
                "source_ip_cidr": [cidr],
                "outbound": "direct"
            }));
        }
    }
}

fn append_tcp_fw_rule(fw: &mut String, port: u16) {
    if port == 0 {
        return;
    }
    fw.push_str(&format!(
        "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
         if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
         iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
         iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
        p = port
    ));
}

fn append_udp_fw_rule(fw: &mut String, port: u16, wg_cidr: Option<&str>) {
    if port == 0 {
        return;
    }
    fw.push_str(&format!(
        "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
         if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
         iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
         iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
         iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n",
        p = port
    ));
    if let Some(cidr) = wg_cidr {
        fw.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C FORWARD -s {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -d {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -C POSTROUTING -s {cidr} -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s {cidr} -j MASQUERADE >/dev/null 2>&1 || true\n",
            cidr = cidr
        ));
    }
}

fn find_wg_endpoint_idx(cfg: &serde_json::Value, is_awg2: bool, target_tag: &str, default_port: u16) -> Option<usize> {
    let endpoints = cfg["endpoints"].as_array()?;
    for (idx, ep) in endpoints.iter().enumerate() {
        if ep["type"].as_str() == Some("wireguard") && ep["tag"].as_str() == Some(target_tag) {
            return Some(idx);
        }
    }
    if target_tag == "enigma-in" {
        return None;
    }
    for (idx, ep) in endpoints.iter().enumerate() {
        let tag = ep["tag"].as_str().unwrap_or("");
        if ep["type"].as_str() == Some("wireguard")
            && tag != "enigma-in"
            && ep["listen_port"].as_u64() == Some(default_port as u64)
        {
            return Some(idx);
        }
    }
    if !is_awg2 {
        for (idx, ep) in endpoints.iter().enumerate() {
            let tag = ep["tag"].as_str().unwrap_or("");
            if ep["type"].as_str() == Some("wireguard") && tag != "awg2-in" && tag != "enigma-in" {
                return Some(idx);
            }
        }
    }
    None
}

async fn ensure_vless_reality_ready(
    app: &AppHandle,
    sess: &Handle<SshClientHandler>,
    target: &ResolvedContainer,
    creds: &VpsCredentials,
    cfg: &mut serde_json::Value,
    reality_json_opt: &mut Option<serde_json::Value>,
    reality_loaded: &mut bool,
    reality_dirty: &mut bool,
    cfg_dirty: &mut bool,
    host_probe: &mut Option<(HashSet<u16>, HashSet<u8>)>,
    fw_rules: &mut String,
) -> Result<(), String> {
    if !*reality_loaded {
        let (_, real_raw) = run_remote_cmd(sess, &format!("cat \"{}/reality.json\" 2>/dev/null", target.cdir)).await?;
        *reality_json_opt = serde_json::from_str(&real_raw).ok();
        *reality_loaded = true;
    }

    let plain_vless_idx = cfg["inbounds"].as_array().and_then(|arr| {
        arr.iter().position(|ib| {
            ib["type"].as_str() == Some("vless")
                && ib["transport"]["type"].as_str().unwrap_or("").is_empty()
        })
    });

    let reality_valid = reality_json_opt
        .as_ref()
        .map(|j| {
            !j["private_key"].as_str().unwrap_or("").trim().is_empty()
                && !j["public_key"].as_str().unwrap_or("").trim().is_empty()
                && !j["short_id"].as_str().unwrap_or("").trim().is_empty()
        })
        .unwrap_or(false);

    if !reality_valid {
        let (priv_key, pub_key, sid) = generate_reality_keypair_remote(sess).await?;
        *reality_json_opt = Some(json!({
            "private_key": priv_key,
            "public_key": pub_key,
            "short_id": sid
        }));
        *reality_dirty = true;

        if let Some(idx) = plain_vless_idx {
            if let Some(ib) = cfg["inbounds"].as_array_mut().and_then(|a| a.get_mut(idx)) {
                let empty_users = ib["users"].as_array().map(|u| u.is_empty()).unwrap_or(true);
                let empty_priv = ib["tls"]["reality"]["private_key"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .is_empty();
                if empty_users || empty_priv {
                    ib["tls"]["reality"]["private_key"] = json!(priv_key);
                    ib["tls"]["reality"]["short_id"] = json!([sid]);
                    *cfg_dirty = true;
                }
            }
        }
        emit_log(
            app,
            &format!(
                "В контейнере {} отсутствовал reality.json — ключи VLESS Reality сгенерированы автоматически.",
                target.id
            ),
        );
    }

    if plain_vless_idx.is_none() {
        if host_probe.is_none() {
            *host_probe = Some(probe_vps_used_ports_and_octets(sess, cfg).await);
        }
        let (used_ports, _) = host_probe.as_mut().unwrap();
        let pref_port = if creds.vless_port == 0 { 443 } else { creds.vless_port };
        let v_port = allocate_free_port(pref_port, used_ports, false);

        let reality_sni = if creds.reality_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.reality_sni.trim().to_string()
        };
        let rj = reality_json_opt.as_ref().unwrap();
        let v_priv = rj["private_key"].as_str().unwrap_or("").to_string();
        let v_sid = rj["short_id"].as_str().unwrap_or("").to_string();

        if !cfg["inbounds"].is_array() {
            cfg["inbounds"] = json!([]);
        }
        if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
            inbounds.push(json!({
                "type": "vless",
                "tag": "vless-in",
                "listen": "::",
                "listen_port": v_port,
                "users": [],
                "tls": {
                    "enabled": true,
                    "server_name": reality_sni,
                    "reality": {
                        "enabled": true,
                        "handshake": {
                            "server": reality_sni,
                            "server_port": 443
                        },
                        "private_key": v_priv,
                        "short_id": [v_sid]
                    }
                }
            }));
        }
        append_tcp_fw_rule(fw_rules, v_port);
        *cfg_dirty = true;
        emit_log(
            app,
            &format!(
                "В контейнере {} автоматически создан инбаунд VLESS Reality (порт TCP {}).",
                target.id, v_port
            ),
        );
    }

    Ok(())
}

async fn ensure_awg_ready(
    app: &AppHandle,
    sess: &Handle<SshClientHandler>,
    target: &ResolvedContainer,
    creds: &VpsCredentials,
    is_awg2: bool,
    cfg: &mut serde_json::Value,
    awg_json_opt: &mut Option<serde_json::Value>,
    awg_loaded: &mut bool,
    awg_dirty: &mut bool,
    cfg_dirty: &mut bool,
    host_probe: &mut Option<(HashSet<u16>, HashSet<u8>)>,
    fw_rules: &mut String,
) -> Result<(), String> {
    let file_name = if is_awg2 { "awg2.json" } else { "awg.json" };
    let file_path = format!("{}/{}", target.cdir, file_name);
    let target_tag = if is_awg2 { "awg2-in" } else { "awg-in" };
    let default_port = if is_awg2 { creds.awg2_port } else { creds.awg_port };

    if !*awg_loaded {
        let (_, raw) = run_remote_cmd(sess, &format!("cat \"{}\" 2>/dev/null", file_path)).await?;
        *awg_json_opt = serde_json::from_str(&raw).ok();
        *awg_loaded = true;
    }

    let ep_idx = find_wg_endpoint_idx(cfg, is_awg2, target_tag, default_port);
    let json_valid = awg_json_opt
        .as_ref()
        .map(|j| !j["server_public_key"].as_str().unwrap_or("").trim().is_empty())
        .unwrap_or(false);

    if json_valid && ep_idx.is_some() {
        return Ok(());
    }

    if host_probe.is_none() {
        *host_probe = Some(probe_vps_used_ports_and_octets(sess, cfg).await);
    }
    let (used_ports, used_octets) = host_probe.as_mut().unwrap();

    let (ep_existing_port, ep_existing_octet, ep_has_peers) = if let Some(idx) = ep_idx {
        let ep = &cfg["endpoints"][idx];
        let p = ep["listen_port"].as_u64().unwrap_or(0) as u16;
        let oct = ep["address"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .and_then(|s| s.strip_prefix("10.8."))
            .and_then(|rest| rest.split('.').next())
            .and_then(|o| o.parse::<u8>().ok());
        let has_p = ep["peers"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
        (if p > 0 { Some(p) } else { None }, oct, has_p)
    } else {
        (None, None, false)
    };

    let subnet_octet = ep_existing_octet.unwrap_or_else(|| allocate_free_subnet_octet(used_octets));
    let port = if let Some(ep_p) = ep_existing_port {
        used_ports.insert(ep_p);
        ep_p
    } else if let Some(jp) = awg_json_opt
        .as_ref()
        .and_then(|j| j["port"].as_u64())
        .map(|v| v as u16)
        .filter(|&p| p >= 1024 && !used_ports.contains(&p))
    {
        used_ports.insert(jp);
        jp
    } else {
        let preferred = if is_awg2 {
            if creds.awg2_port == 0 || creds.awg2_port == 51821 || creds.awg2_port == 51820 {
                random_u32_in_range(20000, 55000) as u16
            } else {
                creds.awg2_port
            }
        } else if creds.awg_port == 0 || creds.awg_port == 51820 {
            random_u32_in_range(20000, 55000) as u16
        } else {
            creds.awg_port
        };
        allocate_free_port(preferred, used_ports, true)
    };

    if !json_valid {
        let (s_priv, s_pub) = generate_wg_keypair_remote(sess).await?;
        let psk = generate_random_base64_32();

        if is_awg2 {
            *awg_json_opt = Some(json!({
                "server_private_key": s_priv,
                "server_public_key": s_pub,
                "client_private_key": "",
                "client_public_key": "",
                "preshared_key": psk,
                "port": port,
                "devices": []
            }));
        } else {
            let hpk = generate_random_base64_32();
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
            let hop_ports = if creds.awg_hop_ports.trim().is_empty() {
                "20000:65000".to_string()
            } else {
                creds.awg_hop_ports.trim().to_string()
            };

            *awg_json_opt = Some(json!({
                "server_private_key": s_priv,
                "server_public_key": s_pub,
                "client_private_key": "",
                "client_public_key": "",
                "header_protection_key": hpk,
                "preshared_key": psk,
                "port": port,
                "hop_ports": hop_ports,
                "jc": jc,
                "jmin": jmin,
                "jmax": jmax,
                "s1": s1,
                "s2": s2,
                "s3": s3,
                "s4": s4,
                "h1": h1,
                "h2": h2,
                "h3": h3,
                "h4": h4,
                "rekey_after_time": rekey_after,
                "rekey_timeout": rekey_to,
                "reject_after_time": reject_after,
                "keepalive_timeout": keepalive_to,
                "max_handshake_attempts": max_handshake,
                "random_trailers": random_trailers,
                "disable_cookies": disable_cookies,
                "persistent_keepalive": persistent_ka,
                "devices": []
            }));
        }

        if let Some(idx) = ep_idx {
            if !ep_has_peers {
                if let Some(ep) = cfg["endpoints"].as_array_mut().and_then(|a| a.get_mut(idx)) {
                    ep["private_key"] = json!(awg_json_opt.as_ref().unwrap()["server_private_key"]);
                    *cfg_dirty = true;
                }
            }
        }
        *awg_dirty = true;
        emit_log(
            app,
            &format!(
                "В контейнере {} отсутствовал {} — параметры {} созданы автоматически.",
                target.id,
                file_name,
                if is_awg2 { "AmneziaWG 2.0" } else { "Amnezia 3.1" }
            ),
        );
    }

    if ep_idx.is_none() {
        let aj = awg_json_opt.as_mut().unwrap();
        if aj["port"].as_u64() != Some(port as u64) {
            aj["port"] = json!(port);
            *awg_dirty = true;
        }
        let server_priv = aj["server_private_key"].as_str().unwrap_or("").to_string();
        let server_ip = format!("10.8.{}.1/24", subnet_octet);
        let cidr = format!("10.8.{}.0/24", subnet_octet);

        let ep_json = if is_awg2 {
            json!({
                "type": "wireguard",
                "tag": "awg2-in",
                "listen_port": port,
                "address": [server_ip],
                "private_key": server_priv,
                "mtu": 1420,
                "peers": []
            })
        } else {
            let mut ep = json!({
                "type": "wireguard",
                "tag": "awg-in",
                "listen_port": port,
                "address": [server_ip],
                "private_key": server_priv,
                "header_protection_key": aj["header_protection_key"].as_str().unwrap_or(""),
                "random_trailers": aj["random_trailers"].as_bool().unwrap_or(true),
                "disable_cookies": aj["disable_cookies"].as_bool().unwrap_or(true),
                "rekey_after_time": aj["rekey_after_time"].as_str().unwrap_or("95-125"),
                "rekey_timeout": aj["rekey_timeout"].as_str().unwrap_or("3-7"),
                "reject_after_time": aj["reject_after_time"].as_str().unwrap_or("150-180"),
                "keepalive_timeout": aj["keepalive_timeout"].as_str().unwrap_or("5-15"),
                "max_handshake_attempts": aj["max_handshake_attempts"].as_str().unwrap_or("14-20"),
                "jc": aj["jc"].as_u64().unwrap_or(4),
                "jmin": aj["jmin"].as_u64().unwrap_or(15),
                "jmax": aj["jmax"].as_u64().unwrap_or(65),
                "s1": aj["s1"].as_u64().unwrap_or(15),
                "s2": aj["s2"].as_u64().unwrap_or(25),
                "s3": aj["s3"].as_u64().unwrap_or(16),
                "s4": aj["s4"].as_u64().unwrap_or(20),
                "peers": []
            });
            for hk in ["h1", "h2", "h3", "h4"] {
                if let Some(s) = aj[hk].as_str() {
                    if let Ok(num) = s.trim().parse::<u32>() {
                        ep[hk] = json!(num);
                    } else {
                        ep[hk] = json!(s);
                    }
                } else if let Some(u) = aj[hk].as_u64() {
                    ep[hk] = json!(u);
                }
            }
            ep
        };

        if !cfg["endpoints"].is_array() {
            cfg["endpoints"] = json!([]);
        }
        if let Some(endpoints) = cfg["endpoints"].as_array_mut() {
            endpoints.push(ep_json);
        }
        ensure_route_direct_for_wg(cfg, target_tag, &cidr);
        append_udp_fw_rule(fw_rules, port, Some(&cidr));
        *cfg_dirty = true;
        emit_log(
            app,
            &format!(
                "В контейнере {} автоматически создан endpoint {} (порт UDP {}, подсеть {}).",
                target.id, target_tag, port, cidr
            ),
        );
    }

    Ok(())
}

async fn ensure_standard_inbound_ready(
    app: &AppHandle,
    sess: &Handle<SshClientHandler>,
    target: &ResolvedContainer,
    creds: &VpsCredentials,
    proto: &str,
    cfg: &mut serde_json::Value,
    cfg_dirty: &mut bool,
    host_probe: &mut Option<(HashSet<u16>, HashSet<u8>)>,
    certs_ready: &mut bool,
    fw_rules: &mut String,
) -> Result<(), String> {
    let has_matching = cfg["inbounds"]
        .as_array()
        .map(|arr| {
            arr.iter().any(|ib| {
                let ib_type = ib["type"].as_str().unwrap_or("");
                let tr_type = ib["transport"]["type"].as_str().unwrap_or("");
                if ib_type == "shadowsocks" && ib["listen"].as_str() == Some("127.0.0.1") {
                    return false;
                }
                match proto {
                    "vless_ws" => ib_type == "vless" && tr_type == "ws",
                    "webrtc" => ib_type == "vless" && tr_type == "grpc",
                    "shadowtls" => ib_type == "shadowtls",
                    other => ib_type == other,
                }
            })
        })
        .unwrap_or(false);

    if has_matching {
        return Ok(());
    }

    if host_probe.is_none() {
        *host_probe = Some(probe_vps_used_ports_and_octets(sess, cfg).await);
    }
    let (used_ports, _) = host_probe.as_mut().unwrap();
    if !cfg["inbounds"].is_array() {
        cfg["inbounds"] = json!([]);
    }

    let sni = if creds.sni.trim().is_empty() {
        "www.bing.com".to_string()
    } else {
        creds.sni.trim().to_string()
    };

    match proto {
        "hysteria2" => {
            let port = allocate_free_port(if creds.hy2_port == 0 { 8443 } else { creds.hy2_port }, used_ports, false);
            let (cert_path, key_path) = ensure_container_tls_certs_remote(sess, target, &sni, certs_ready).await?;
            let mut hy2_in = json!({
                "type": "hysteria2",
                "tag": "hy2-in",
                "listen": "::",
                "listen_port": port,
                "users": [],
                "masquerade": format!("https://{}", sni),
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "alpn": ["h3"],
                    "certificate_path": cert_path,
                    "key_path": key_path
                }
            });
            if creds.enable_salamander {
                hy2_in["obfs"] = json!({
                    "type": "salamander",
                    "password": Uuid::new_v4().simple().to_string()
                });
            }
            cfg["inbounds"].as_array_mut().unwrap().push(hy2_in);
            append_udp_fw_rule(fw_rules, port, None);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд Hysteria 2 (порт UDP {}).", target.id, port));
        }
        "tuic" => {
            let port = allocate_free_port(if creds.tuic_port == 0 { 9443 } else { creds.tuic_port }, used_ports, false);
            let (cert_path, key_path) = ensure_container_tls_certs_remote(sess, target, &sni, certs_ready).await?;
            let tuic_in = json!({
                "type": "tuic",
                "tag": "tuic-in",
                "listen": "::",
                "listen_port": port,
                "users": [],
                "congestion_control": "bbr",
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "alpn": ["h3"],
                    "certificate_path": cert_path,
                    "key_path": key_path
                }
            });
            cfg["inbounds"].as_array_mut().unwrap().push(tuic_in);
            append_udp_fw_rule(fw_rules, port, None);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд TUIC v5 (порт UDP {}).", target.id, port));
        }
        "shadowsocks" => {
            let port = allocate_free_port(if creds.ss_port == 0 { 8388 } else { creds.ss_port }, used_ports, false);
            let method = if creds.ss_method.trim().is_empty() {
                "2022-blake3-aes-128-gcm".to_string()
            } else {
                creds.ss_method.trim().to_string()
            };
            let ss_password = if method.starts_with("2022-blake3-aes-128") {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(Uuid::new_v4().as_bytes())
            } else if method.starts_with("2022-blake3") {
                use base64::Engine;
                let u1 = Uuid::new_v4();
                let u2 = Uuid::new_v4();
                let bytes = [u1.as_bytes().as_slice(), u2.as_bytes().as_slice()].concat();
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            } else {
                Uuid::new_v4().simple().to_string()
            };
            let ss_in = json!({
                "type": "shadowsocks",
                "tag": "ss-in",
                "listen": "::",
                "listen_port": port,
                "method": method,
                "password": ss_password,
                "multiplex": { "enabled": true }
            });
            cfg["inbounds"].as_array_mut().unwrap().push(ss_in);
            append_tcp_fw_rule(fw_rules, port);
            append_udp_fw_rule(fw_rules, port, None);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд Shadowsocks (порт {}).", target.id, port));
        }
        "shadowtls" => {
            let ss_detour_tag = cfg["inbounds"]
                .as_array()
                .and_then(|arr| arr.iter().find(|i| i["type"].as_str() == Some("shadowsocks")))
                .and_then(|i| i["tag"].as_str())
                .map(|s| s.to_string());
            let detour_tag = if let Some(t) = ss_detour_tag {
                t
            } else {
                let ss_port = allocate_free_port(if creds.ss_port == 0 { 8388 } else { creds.ss_port }, used_ports, false);
                let method = if creds.ss_method.trim().is_empty() {
                    "2022-blake3-aes-128-gcm".to_string()
                } else {
                    creds.ss_method.trim().to_string()
                };
                let ss_password = if method.starts_with("2022-blake3-aes-128") {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD.encode(Uuid::new_v4().as_bytes())
                } else if method.starts_with("2022-blake3") {
                    use base64::Engine;
                    let u1 = Uuid::new_v4();
                    let u2 = Uuid::new_v4();
                    let bytes = [u1.as_bytes().as_slice(), u2.as_bytes().as_slice()].concat();
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                } else {
                    Uuid::new_v4().simple().to_string()
                };
                cfg["inbounds"].as_array_mut().unwrap().push(json!({
                    "type": "shadowsocks",
                    "tag": "ss-in",
                    "listen": "127.0.0.1",
                    "listen_port": ss_port,
                    "method": method,
                    "password": ss_password,
                    "multiplex": { "enabled": true }
                }));
                "ss-in".to_string()
            };
            let st_port = allocate_free_port(if creds.shadowtls_port == 0 { 8443 } else { creds.shadowtls_port }, used_ports, false);
            let st_sni = if creds.shadowtls_sni.trim().is_empty() {
                "www.microsoft.com".to_string()
            } else {
                creds.shadowtls_sni.trim().to_string()
            };
            cfg["inbounds"].as_array_mut().unwrap().push(json!({
                "type": "shadowtls",
                "tag": "shadowtls-in",
                "listen": "::",
                "listen_port": st_port,
                "version": 3,
                "users": [],
                "handshake": {
                    "server": st_sni,
                    "server_port": 443
                },
                "detour": detour_tag
            }));
            append_tcp_fw_rule(fw_rules, st_port);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд ShadowTLS v3 (порт TCP {}).", target.id, st_port));
        }
        "vless_ws" => {
            let port = allocate_free_port(if creds.vless_ws_port == 0 { 8080 } else { creds.vless_ws_port }, used_ports, false);
            let (cert_path, key_path) = ensure_container_tls_certs_remote(sess, target, &sni, certs_ready).await?;
            cfg["inbounds"].as_array_mut().unwrap().push(json!({
                "type": "vless",
                "tag": "vless-ws-in",
                "listen": "::",
                "listen_port": port,
                "users": [],
                "transport": {
                    "type": "ws",
                    "path": "/neuroveil-ws"
                },
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "alpn": ["http/1.1"],
                    "certificate_path": cert_path,
                    "key_path": key_path
                }
            }));
            append_tcp_fw_rule(fw_rules, port);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд VLESS WS (порт TCP {}).", target.id, port));
        }
        "webrtc" => {
            let port = allocate_free_port(if creds.webrtc_port == 0 { 2053 } else { creds.webrtc_port }, used_ports, false);
            let (cert_path, key_path) = ensure_container_tls_certs_remote(sess, target, &sni, certs_ready).await?;
            cfg["inbounds"].as_array_mut().unwrap().push(json!({
                "type": "vless",
                "tag": "webrtc-in",
                "listen": "::",
                "listen_port": port,
                "users": [],
                "transport": {
                    "type": "grpc",
                    "service_name": "neuroveil-webrtc"
                },
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "alpn": ["h2"],
                    "certificate_path": cert_path,
                    "key_path": key_path
                }
            }));
            append_tcp_fw_rule(fw_rules, port);
            *cfg_dirty = true;
            emit_log(app, &format!("В контейнере {} автоматически создан инбаунд WebRTC (порт TCP {}).", target.id, port));
        }
        _ => {}
    }

    Ok(())
}

async fn ensure_enigma_ready(
    app: &AppHandle,
    sess: &Handle<SshClientHandler>,
    target: &ResolvedContainer,
    creds: &VpsCredentials,
    cfg: &mut serde_json::Value,
    enigma_json_opt: &mut Option<serde_json::Value>,
    enigma_loaded: &mut bool,
    enigma_dirty: &mut bool,
    enigma_need_daemon: &mut bool,
    cfg_dirty: &mut bool,
    host_probe: &mut Option<(HashSet<u16>, HashSet<u8>)>,
    fw_rules: &mut String,
) -> Result<(), String> {
    let file_path = format!("{}/enigma.json", target.cdir);
    if !*enigma_loaded {
        let (_, raw) = run_remote_cmd(sess, &format!("cat \"{}\" 2>/dev/null || true", file_path)).await?;
        *enigma_json_opt = serde_json::from_str(&raw).ok();
        *enigma_loaded = true;
    }

    let ep_idx = find_wg_endpoint_idx(cfg, false, "enigma-in", 0);
    let json_valid = enigma_json_opt
        .as_ref()
        .map(|j| {
            !j["server_public_key"].as_str().unwrap_or("").trim().is_empty()
                && !j["keybook_b64"].as_str().unwrap_or("").trim().is_empty()
                && j["external_port"].as_u64().unwrap_or(0) > 0
                && j["wg_port"].as_u64().unwrap_or(0) > 0
        })
        .unwrap_or(false);

    if json_valid && ep_idx.is_some() {
        return Ok(());
    }

    if host_probe.is_none() {
        *host_probe = Some(probe_vps_used_ports_and_octets(sess, cfg).await);
    }
    let (used_ports, used_octets) = host_probe.as_mut().unwrap();

    let (ep_existing_wg_port, ep_existing_octet) = if let Some(idx) = ep_idx {
        let ep = &cfg["endpoints"][idx];
        let p = ep["listen_port"].as_u64().unwrap_or(0) as u16;
        let oct = ep["address"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .and_then(|s| s.strip_prefix("10.8."))
            .and_then(|rest| rest.split('.').next())
            .and_then(|o| o.parse::<u8>().ok());
        (if p > 0 { Some(p) } else { None }, oct)
    } else {
        (None, None)
    };

    let subnet_octet = ep_existing_octet.unwrap_or_else(|| allocate_free_subnet_octet(used_octets));
    let wg_port = if let Some(p) = ep_existing_wg_port {
        used_ports.insert(p);
        p
    } else if let Some(jp) = enigma_json_opt
        .as_ref()
        .and_then(|j| j["wg_port"].as_u64())
        .map(|v| v as u16)
        .filter(|&p| p >= 1024 && !used_ports.contains(&p))
    {
        used_ports.insert(jp);
        jp
    } else {
        allocate_free_port(random_u32_in_range(40000, 58000) as u16, used_ports, true)
    };

    let ext_port = if let Some(jp) = enigma_json_opt
        .as_ref()
        .and_then(|j| j["external_port"].as_u64())
        .map(|v| v as u16)
        .filter(|&p| p >= 1024 && !used_ports.contains(&p))
    {
        used_ports.insert(jp);
        jp
    } else {
        let pref = if creds.enigma_port == 0 || creds.enigma_port == 51820 || creds.enigma_port == 51821 {
            random_u32_in_range(20000, 55000) as u16
        } else {
            creds.enigma_port
        };
        allocate_free_port(pref, used_ports, true)
    };

    if !json_valid {
        let (s_priv, s_pub) = generate_wg_keypair_remote(sess).await?;
        let psk = generate_random_base64_32();
        let kb_b64 = crate::enigma::EnigmaKeybook::generate()?.to_base64();
        let has_hy2 = cfg["inbounds"]
            .as_array()
            .map(|a| a.iter().any(|ib| ib["type"].as_str() == Some("hysteria2")))
            .unwrap_or(false);
        let hop_ports = if target.enable_port_hopping && !has_hy2 {
            "20000:65000"
        } else {
            ""
        };
        *enigma_json_opt = Some(json!({
            "server_private_key": s_priv,
            "server_public_key": s_pub,
            "client_private_key": "",
            "client_public_key": "",
            "preshared_key": psk,
            "external_port": ext_port,
            "wg_port": wg_port,
            "keybook_b64": kb_b64,
            "mimic": "auto",
            "hop_ports": hop_ports,
            "devices": []
        }));
        *enigma_dirty = true;
        *enigma_need_daemon = true;
    }

    let ej = enigma_json_opt.as_ref().unwrap();
    let server_priv = ej["server_private_key"].as_str().unwrap_or("").to_string();
    let server_ip = format!("10.8.{}.1/24", subnet_octet);
    let cidr = format!("10.8.{}.0/24", subnet_octet);

    if ep_idx.is_none() {
        let ep_json = json!({
            "type": "wireguard",
            "tag": "enigma-in",
            "listen_port": wg_port,
            "address": [server_ip],
            "private_key": server_priv,
            "mtu": 1280,
            "peers": []
        });
        if !cfg["endpoints"].is_array() {
            cfg["endpoints"] = json!([]);
        }
        if let Some(endpoints) = cfg["endpoints"].as_array_mut() {
            endpoints.push(ep_json);
        }
        ensure_route_direct_for_wg(cfg, "enigma-in", &cidr);
        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {ext_p}/udp >/dev/null 2>&1 || true; ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={ext_p}/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {ext_p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {ext_p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {ext_p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {ext_p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C INPUT -p udp ! -i lo --dport {wg_p} -j DROP 2>/dev/null || iptables -I INPUT 1 -p udp ! -i lo --dport {wg_p} -j DROP >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -s {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -s {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C FORWARD -d {cidr} -j ACCEPT 2>/dev/null || iptables -I FORWARD -d {cidr} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -C POSTROUTING -s {cidr} -j MASQUERADE 2>/dev/null || iptables -t nat -A POSTROUTING -s {cidr} -j MASQUERADE >/dev/null 2>&1 || true\n\
             iptables -t nat -C PREROUTING -p udp --dport {ext_p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {ext_p} -j RETURN >/dev/null 2>&1 || true\n",
            ext_p = ext_port,
            wg_p = wg_port,
            cidr = cidr
        ));
        *cfg_dirty = true;
        *enigma_need_daemon = true;
    }

    if *enigma_need_daemon {
        let (has_bin, _) = run_remote_cmd(sess, "test -x /usr/local/bin/neuroveil-enigma-srv").await?;
        if has_bin != 0 {
            emit_log(app, "Компиляция шлюза Enigma v1 на VPS (C99)...");
            let (eb_code, eb_out) = run_remote_cmd(sess, &build_enigma_binary_install_cmd()).await?;
            if eb_code != 0 || !eb_out.contains("ENIGMA_BIN_OK") {
                return Err(format!("Ошибка сборки шлюза Enigma v1 на VPS: {}", eb_out));
            }
        }
        emit_log(
            app,
            &format!(
                "В контейнере {} инициализирована Enigma v1 (UDP {} -> loopback {}).",
                target.id, ext_port, wg_port
            ),
        );
    }

    Ok(())
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
    let enigma_file = format!("{}/enigma.json", target.cdir);
    let reality_file = format!("{}/reality.json", target.cdir);

    let mut awg_json_opt: Option<serde_json::Value> = None;
    let mut awg_loaded = false;
    let mut awg2_json_opt: Option<serde_json::Value> = None;
    let mut awg2_loaded = false;
    let mut enigma_json_opt: Option<serde_json::Value> = None;
    let mut enigma_loaded = false;
    let mut reality_json_opt: Option<serde_json::Value> = None;
    let mut reality_loaded = false;

    let mut cfg_dirty = false;
    let mut awg_dirty = false;
    let mut awg2_dirty = false;
    let mut enigma_dirty = false;
    let mut enigma_need_daemon = false;
    let mut reality_dirty = false;
    let mut host_probe: Option<(HashSet<u16>, HashSet<u8>)> = None;
    let mut certs_ready = false;
    let mut extra_fw_rules = String::new();

    let mut created_profiles: Vec<VpnProfile> = Vec::new();
    let mut skipped_reasons: Vec<String> = Vec::new();

    for proto in &unique_protos {
        if proto == "enigma" {
            if let Err(err) = ensure_enigma_ready(
                app,
                &sess,
                &target,
                creds,
                &mut cfg,
                &mut enigma_json_opt,
                &mut enigma_loaded,
                &mut enigma_dirty,
                &mut enigma_need_daemon,
                &mut cfg_dirty,
                &mut host_probe,
                &mut extra_fw_rules,
            )
            .await
            {
                skipped_reasons.push(err);
                continue;
            }

            let Some(enigma_json) = enigma_json_opt.as_mut() else {
                skipped_reasons.push(format!("Не удалось инициализировать enigma.json в {}.", target.id));
                continue;
            };
            let Some(wg_idx) = find_wg_endpoint_idx(&cfg, false, "enigma-in", 0) else {
                skipped_reasons.push(format!("В контейнере {} не найден endpoint enigma-in.", target.id));
                continue;
            };
            let Some(endpoints) = cfg["endpoints"].as_array_mut() else {
                skipped_reasons.push("В конфиге отсутствует секция endpoints.".to_string());
                continue;
            };

            let (dev_priv, dev_pub) = match generate_wg_keypair_remote(&sess).await {
                Ok(pair) => pair,
                Err(e) => {
                    skipped_reasons.push(e);
                    continue;
                }
            };

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
                .unwrap_or_else(|| "10.8.7.".to_string());

            let ext_port = enigma_json["external_port"].as_u64().unwrap_or(creds.enigma_port as u64) as u16;
            let psk = enigma_json["preshared_key"].as_str().map(|s| s.to_string());
            let kb_b64 = enigma_json["keybook_b64"].as_str().unwrap_or("").to_string();
            let mimic = enigma_json["mimic"].as_str().unwrap_or("auto").to_string();
            let hop_opt = enigma_json["hop_ports"]
                .as_str()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let Some(peers) = wg_ep["peers"].as_array_mut() else {
                skipped_reasons.push(format!("В endpoint enigma-in контейнера {} отсутствует массив peers.", target.id));
                continue;
            };
            let mut used_ips = HashSet::new();
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
            let peer_ip_32 = format!("{}{}/32", subnet_prefix, chosen_octet);
            let peer_ip_24 = format!("{}{}/24", subnet_prefix, chosen_octet);

            let mut new_peer = json!({
                "public_key": dev_pub,
                "allowed_ips": [peer_ip_32]
            });
            if let Some(ref p) = psk {
                if !p.trim().is_empty() {
                    new_peer["pre_shared_key"] = json!(p.trim());
                }
            }
            peers.push(new_peer);
            cfg_dirty = true;

            if !enigma_json["devices"].is_array() {
                enigma_json["devices"] = json!([]);
            }
            let dev_label = if unique_protos.len() > 1 {
                format!("{} (Enigma)", clean_name)
            } else {
                clean_name.clone()
            };
            if let Some(devs) = enigma_json["devices"].as_array_mut() {
                let mut dev_entry = json!({
                    "name": dev_label,
                    "protocol": "enigma",
                    "client_private_key": dev_priv,
                    "client_public_key": dev_pub,
                    "ip": peer_ip_32,
                    "mimic": mimic
                });
                if let Some(ref p) = psk {
                    if !p.trim().is_empty() {
                        dev_entry["preshared_key"] = json!(p.trim());
                    }
                }
                devs.push(dev_entry);
            }
            if enigma_json["client_private_key"].as_str().unwrap_or("").trim().is_empty() {
                enigma_json["client_private_key"] = json!(dev_priv);
            }
            if enigma_json["client_public_key"].as_str().unwrap_or("").trim().is_empty() {
                enigma_json["client_public_key"] = json!(dev_pub);
            }
            enigma_dirty = true;

            let server_pub = enigma_json["server_public_key"].as_str().unwrap_or("").to_string();
            let mut enigma_prof = VpnProfile {
                id: Uuid::new_v4().to_string(),
                name: dev_label,
                protocol: "enigma".to_string(),
                server: connect_host.clone(),
                port: ext_port,
                password: dev_priv.clone(),
                awg_private_key: Some(dev_priv),
                awg_public_key: Some(server_pub),
                awg_preshared_key: psk,
                awg_local_address: Some(peer_ip_24),
                awg_hop_ports: hop_opt.clone(),
                hop_ports: hop_opt,
                enigma_keybook: Some(kb_b64),
                enigma_mimic: Some(mimic),
                up_mbps: 100,
                down_mbps: 100,
                pinned_on_pc: Some(false),
                container_id: Some(target.id.clone()),
                ..Default::default()
            };
            enigma_prof.uri = build_profile_uri(&enigma_prof);
            created_profiles.push(enigma_prof);
            continue;
        }

        if proto == "amneziawg" || proto == "awg_quic" || proto == "awg_vless" || proto == "awg2" {
            let is_awg2 = proto == "awg2";
            let target_tag = if is_awg2 { "awg2-in" } else { "awg-in" };
            let default_port = if is_awg2 { creds.awg2_port } else { creds.awg_port };

            let ensure_res = if is_awg2 {
                ensure_awg_ready(
                    app,
                    &sess,
                    &target,
                    creds,
                    true,
                    &mut cfg,
                    &mut awg2_json_opt,
                    &mut awg2_loaded,
                    &mut awg2_dirty,
                    &mut cfg_dirty,
                    &mut host_probe,
                    &mut extra_fw_rules,
                )
                .await
            } else {
                ensure_awg_ready(
                    app,
                    &sess,
                    &target,
                    creds,
                    false,
                    &mut cfg,
                    &mut awg_json_opt,
                    &mut awg_loaded,
                    &mut awg_dirty,
                    &mut cfg_dirty,
                    &mut host_probe,
                    &mut extra_fw_rules,
                )
                .await
            };
            if let Err(err) = ensure_res {
                skipped_reasons.push(err);
                continue;
            }

            // For hybrid AWG + Reality, auto-provision VLESS Reality + reality.json if missing
            let mut hybrid_vless_data: Option<(u16, String, String, Option<String>, Option<String>)> = None;
            if proto == "awg_vless" {
                if let Err(err) = ensure_vless_reality_ready(
                    app,
                    &sess,
                    &target,
                    creds,
                    &mut cfg,
                    &mut reality_json_opt,
                    &mut reality_loaded,
                    &mut reality_dirty,
                    &mut cfg_dirty,
                    &mut host_probe,
                    &mut extra_fw_rules,
                )
                .await
                {
                    skipped_reasons.push(err);
                    continue;
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
                            if !inbound["users"].is_array() {
                                inbound["users"] = json!([]);
                            }
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
                        "В контейнере {} не удалось инициализировать VLESS Reality для гибрида AWG + Reality.",
                        target.id
                    ));
                    continue;
                }
            }

            let awg_json_ref = if is_awg2 { &mut awg2_json_opt } else { &mut awg_json_opt };
            let Some(awg_json) = awg_json_ref.as_mut() else {
                skipped_reasons.push(format!(
                    "В контейнере {} не удалось инициализировать {} для {}.",
                    target.id,
                    if is_awg2 { "awg2.json" } else { "awg.json" },
                    proto
                ));
                continue;
            };

            let wg_idx = find_wg_endpoint_idx(&cfg, is_awg2, target_tag, default_port);
            let Some(wg_idx) = wg_idx else {
                skipped_reasons.push(format!(
                    "В контейнере {} не найден WireGuard endpoint для {}.",
                    target.id, proto
                ));
                continue;
            };
            let Some(endpoints) = cfg["endpoints"].as_array_mut() else {
                skipped_reasons.push("В конфиге контейнера отсутствует секция endpoints.".to_string());
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
                    _ => format!("{} (AWG 3.1)", clean_name),
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
            if awg_json["client_private_key"].as_str().unwrap_or("").trim().is_empty() {
                awg_json["client_private_key"] = json!(dev_priv);
            }
            if awg_json["client_public_key"].as_str().unwrap_or("").trim().is_empty() {
                awg_json["client_public_key"] = json!(dev_pub);
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
            if let Err(err) = ensure_standard_inbound_ready(
                app,
                &sess,
                &target,
                creds,
                "shadowtls",
                &mut cfg,
                &mut cfg_dirty,
                &mut host_probe,
                &mut certs_ready,
                &mut extra_fw_rules,
            )
            .await
            {
                skipped_reasons.push(err);
                continue;
            }

            let mut st_port: Option<u16> = None;
            let mut st_sni = "www.microsoft.com".to_string();
            let mut detour_tag = "ss-in".to_string();
            let st_user_pass = Uuid::new_v4().simple().to_string();
            let mut st_updated = false;
            let prof_name = if unique_protos.len() > 1 {
                format!("{} (ShadowTLS)", clean_name)
            } else {
                clean_name.clone()
            };

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
                        if !inbound["users"].is_array() {
                            inbound["users"] = json!([]);
                        }
                        if let Some(users) = inbound["users"].as_array_mut() {
                            users.push(json!({
                                "name": prof_name,
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

        if proto == "vless" {
            if let Err(err) = ensure_vless_reality_ready(
                app,
                &sess,
                &target,
                creds,
                &mut cfg,
                &mut reality_json_opt,
                &mut reality_loaded,
                &mut reality_dirty,
                &mut cfg_dirty,
                &mut host_probe,
                &mut extra_fw_rules,
            )
            .await
            {
                skipped_reasons.push(err);
                continue;
            }
        } else if let Err(err) = ensure_standard_inbound_ready(
            app,
            &sess,
            &target,
            creds,
            proto,
            &mut cfg,
            &mut cfg_dirty,
            &mut host_probe,
            &mut certs_ready,
            &mut extra_fw_rules,
        )
        .await
        {
            skipped_reasons.push(err);
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
            } else {
                if !inbound["users"].is_array() {
                    inbound["users"] = json!([]);
                }
                if let Some(users) = inbound["users"].as_array_mut() {
                    if proto == "vless" {
                        users.push(json!({
                            "name": prof_name,
                            "uuid": new_uuid,
                            "flow": "xtls-rprx-vision"
                        }));
                    } else if proto == "vless_ws" || proto == "webrtc" {
                        users.push(json!({
                            "name": prof_name,
                            "uuid": new_uuid
                        }));
                    } else if proto == "tuic" {
                        users.push(json!({
                            "name": prof_name,
                            "uuid": new_uuid,
                            "password": new_password
                        }));
                    } else {
                        users.push(json!({
                            "name": prof_name,
                            "password": new_password
                        }));
                    }
                    updated = true;
                    cfg_dirty = true;
                }
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

    let mut multi_dirty = false;
    let mut multi_json_str = String::new();
    if created_profiles.len() >= 2 {
        let group_id = Uuid::new_v4().to_string();
        let group_name = clean_name.clone();
        let multi_file = format!("{}/multi_protocols.json", target.cdir);
        let (_, existing_multi_raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null || true", multi_file))
            .await
            .unwrap_or((1, String::new()));
        let mut stored_groups: Vec<StoredMultiGroup> = serde_json::from_str(&existing_multi_raw).unwrap_or_default();

        let mut items: Vec<StoredMultiItem> = Vec::new();
        for p in &mut created_profiles {
            p.multi_group_id = Some(group_id.clone());
            p.multi_group_name = Some(group_name.clone());
            items.push(StoredMultiItem {
                protocol: p.protocol.clone(),
                port: p.port,
                credential: profile_credential(p),
                ip: p.awg_local_address.clone(),
                label: p.name.clone(),
            });
        }
        stored_groups.push(StoredMultiGroup {
            id: group_id,
            name: group_name,
            created_at: chrono::Local::now().format("%d.%m.%Y %H:%M").to_string(),
            items,
        });
        multi_json_str = serde_json::to_string_pretty(&stored_groups).map_err(|e| e.to_string())?;
        multi_dirty = true;
    }

    if cfg_dirty || awg_dirty || awg2_dirty || enigma_dirty || enigma_need_daemon || reality_dirty || multi_dirty || !extra_fw_rules.is_empty() {
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
        if enigma_dirty {
            if let Some(ref ej) = enigma_json_opt {
                let kb_b64 = ej["keybook_b64"].as_str().unwrap_or("");
                if !kb_b64.trim().is_empty() {
                    cmd_parts.push(format!(
                        "printf '%s' '{}' | base64 -d > \"{}/enigma.nvkey\"\nchmod 600 \"{}/enigma.nvkey\"",
                        kb_b64, target.cdir, target.cdir
                    ));
                }
                let new_enigma_str = serde_json::to_string_pretty(ej).unwrap_or_default();
                cmd_parts.push(format!(
                    "cat << 'EOF_ENIGMA' > \"{}\"\n{}\nEOF_ENIGMA\nchmod 600 \"{}\"",
                    enigma_file, new_enigma_str, enigma_file
                ));
            }
        }
        if reality_dirty {
            if let Some(ref real_json) = reality_json_opt {
                let new_real_str = serde_json::to_string_pretty(real_json).unwrap_or_default();
                cmd_parts.push(format!(
                    "cat << 'EOF_REAL' > \"{}\"\n{}\nEOF_REAL\nchmod 600 \"{}\"",
                    reality_file, new_real_str, reality_file
                ));
            }
        }
        if multi_dirty {
            let multi_file = format!("{}/multi_protocols.json", target.cdir);
            cmd_parts.push(format!(
                "cat << 'EOF_MULTI' > \"{}\"\n{}\nEOF_MULTI\nchmod 600 \"{}\"",
                multi_file, multi_json_str, multi_file
            ));
        }
        if !extra_fw_rules.trim().is_empty() {
            cmd_parts.push(format!(
                "{}\nif command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --reload >/dev/null 2>&1 || true; fi\nif command -v netfilter-persistent >/dev/null 2>&1; then netfilter-persistent save >/dev/null 2>&1 || true; fi\nif [ -d /etc/iptables ]; then iptables-save > /etc/iptables/rules.v4 2>/dev/null || true; fi",
                extra_fw_rules.trim()
            ));
        }
        let needs_restart = cfg_dirty || awg_dirty || awg2_dirty || reality_dirty;
        if needs_restart {
            cmd_parts.push(build_container_restart_cmd(&target, true));
        }
        let update_cmd = cmd_parts.join("\n");

        let (restart_code, restart_out) = run_remote_cmd(&sess, &update_cmd).await?;
        if needs_restart && (restart_code != 0 || !restart_out.contains("active")) {
            let _ = sess
                .disconnect(russh::Disconnect::ByApplication, "err", "en")
                .await;
            return Err(format!(
                "Ошибка перезапуска контейнера {} после добавления устройства: {}",
                target.id, restart_out
            ));
        }

        if enigma_need_daemon {
            if let Some(ref ej) = enigma_json_opt {
                let ext_p = ej["external_port"].as_u64().unwrap_or(creds.enigma_port as u64) as u16;
                let wg_p = ej["wg_port"].as_u64().unwrap_or(0) as u16;
                if ext_p > 0 && wg_p > 0 {
                    let e_cmd = build_enigma_service_start_cmd(&target.id, &target.cdir, ext_p, wg_p);
                    let (ec, eo) = run_remote_cmd(&sess, &e_cmd).await?;
                    if ec != 0 || !eo.contains("ENIGMA_ACTIVE") {
                        let _ = sess
                            .disconnect(russh::Disconnect::ByApplication, "err", "en")
                            .await;
                        return Err(format!(
                            "Ошибка запуска шлюза Enigma v1 в контейнере {}: {}",
                            target.id, eo
                        ));
                    }
                }
            }
        }

        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;
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
    enigma_raw: &str,
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
        let enigma_json = serde_json::from_str::<serde_json::Value>(enigma_raw).ok();

        for ep in endpoints {
            if ep["type"].as_str() == Some("wireguard") {
                let tag = ep["tag"].as_str().unwrap_or("");
                let ep_port = ep["listen_port"].as_u64().unwrap_or(51820) as u16;
                let is_enigma = tag == "enigma-in"
                    || enigma_json
                        .as_ref()
                        .and_then(|j| j["wg_port"].as_u64())
                        .map(|p| p as u16 == ep_port)
                        .unwrap_or(false);
                if is_enigma {
                    let ext_port = enigma_json
                        .as_ref()
                        .and_then(|j| j["external_port"].as_u64())
                        .map(|p| p as u16)
                        .unwrap_or(ep_port);
                    push_label_port(&mut proto_labels, &mut ports, "Enigma v1", ext_port);
                    let server_pub = enigma_json
                        .as_ref()
                        .and_then(|j| j["server_public_key"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let client_priv = enigma_json
                        .as_ref()
                        .and_then(|j| j["client_private_key"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let psk = enigma_json
                        .as_ref()
                        .and_then(|j| j["preshared_key"].as_str())
                        .map(|s| s.to_string());
                    let kb_b64 = enigma_json
                        .as_ref()
                        .and_then(|j| j["keybook_b64"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let default_mimic = enigma_json
                        .as_ref()
                        .and_then(|j| j["mimic"].as_str())
                        .unwrap_or("auto")
                        .to_string();
                    let hop_opt = enigma_json
                        .as_ref()
                        .and_then(|j| j["hop_ports"].as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());

                    if let Some(devices) = enigma_json.as_ref().and_then(|j| j["devices"].as_array()) {
                        for d in devices {
                            let peer_name = d["name"].as_str().unwrap_or("Enigma v1");
                            let is_base = peer_name == "default" || peer_name == "pc" || peer_name == "neuroveil-user";
                            let prof_name = if is_base {
                                format_base_name("Enigma v1", ext_port)
                            } else {
                                peer_name.to_string()
                            };
                            let c_priv = d["client_private_key"].as_str().unwrap_or(&client_priv).to_string();
                            if c_priv.trim().is_empty() {
                                continue;
                            }
                            let raw_addr = d["ip"].as_str().unwrap_or("10.8.7.2/24");
                            let peer_addr = raw_addr.replace("/32", "/24");
                            let peer_psk = d["preshared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                            let dev_mimic = d["mimic"].as_str().unwrap_or(&default_mimic).to_string();
                            let mut prof = VpnProfile {
                                id: Uuid::new_v4().to_string(),
                                name: prof_name,
                                protocol: "enigma".to_string(),
                                server: connect_host.to_string(),
                                port: ext_port,
                                password: c_priv.clone(),
                                awg_private_key: Some(c_priv),
                                awg_public_key: Some(server_pub.clone()),
                                awg_preshared_key: peer_psk,
                                awg_local_address: Some(peer_addr),
                                awg_hop_ports: hop_opt.clone(),
                                hop_ports: hop_opt.clone(),
                                enigma_keybook: if kb_b64.is_empty() { None } else { Some(kb_b64.clone()) },
                                enigma_mimic: Some(dev_mimic),
                                up_mbps: 100,
                                down_mbps: 100,
                                pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                                container_id: Some(cid.to_string()),
                                ..Default::default()
                            };
                            prof.uri = build_profile_uri(&prof);
                            profiles.push(prof);
                        }
                    } else if let Some(peers) = ep["peers"].as_array() {
                        if !client_priv.trim().is_empty() {
                            for (idx, p) in peers.iter().enumerate() {
                                let is_base = idx == 0;
                                let prof_name = if is_base {
                                    format_base_name("Enigma v1", ext_port)
                                } else {
                                    format!("Enigma v1 Устройство {}", idx + 1)
                                };
                                let raw_addr = p["allowed_ips"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).unwrap_or("10.8.7.2/24");
                                let peer_addr = raw_addr.replace("/32", "/24");
                                let peer_psk = p["pre_shared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                                let mut prof = VpnProfile {
                                    id: Uuid::new_v4().to_string(),
                                    name: prof_name,
                                    protocol: "enigma".to_string(),
                                    server: connect_host.to_string(),
                                    port: ext_port,
                                    password: client_priv.clone(),
                                    awg_private_key: Some(client_priv.clone()),
                                    awg_public_key: Some(server_pub.clone()),
                                    awg_preshared_key: peer_psk,
                                    awg_local_address: Some(peer_addr),
                                    awg_hop_ports: hop_opt.clone(),
                                    hop_ports: hop_opt.clone(),
                                    enigma_keybook: if kb_b64.is_empty() { None } else { Some(kb_b64.clone()) },
                                    enigma_mimic: Some(default_mimic.clone()),
                                    up_mbps: 100,
                                    down_mbps: 100,
                                    pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                                    container_id: Some(cid.to_string()),
                                    ..Default::default()
                                };
                                prof.uri = build_profile_uri(&prof);
                                profiles.push(prof);
                            }
                        }
                    }
                    continue;
                }

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

fn build_vps_antisleep_install_cmd() -> &'static str {
    r#"
cat << 'EOF_AS_SYSCTL' > /etc/sysctl.d/99-neuroveil-antisleep.conf
vm.swappiness = 1
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 15
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.neigh.default.gc_stale_time = 120
EOF_AS_SYSCTL
sysctl -p /etc/sysctl.d/99-neuroveil-antisleep.conf >/dev/null 2>&1 || true
sysctl -w net.netfilter.nf_conntrack_udp_timeout=120 >/dev/null 2>&1 || true
sysctl -w net.netfilter.nf_conntrack_udp_timeout_stream=180 >/dev/null 2>&1 || true

cat << 'EOF_AS_SH' > /usr/local/bin/neuroveil-antisleep.sh
#!/bin/sh
# NeuroVeil VPN Lightweight VPS Anti-Sleep Keepalive
# Keeps vNIC/ARP, hosting UDP/conntrack tables and sing-box pages warm without log noise.
while true; do
    GW=$(ip -4 route show default 2>/dev/null | awk '{print $3; exit}')
    if [ -n "$GW" ]; then
        ping -c 1 -W 1 -s 16 "$GW" >/dev/null 2>&1 || true
    fi
    ping -c 1 -W 1 -s 16 1.1.1.1 >/dev/null 2>&1 || true
    if command -v timeout >/dev/null 2>&1 && command -v bash >/dev/null 2>&1; then
        timeout 1 bash -c 'printf "\x00" > /dev/udp/1.1.1.1/53' >/dev/null 2>&1 || true
    fi
    for pid in $(pidof sing-box 2>/dev/null || pgrep -x sing-box 2>/dev/null || true); do
        if [ -w "/proc/$pid/oom_score_adj" ]; then
            printf '%s\n' '-900' > "/proc/$pid/oom_score_adj" 2>/dev/null || true
        fi
        cat "/proc/$pid/statm" >/dev/null 2>&1 || true
    done
    sleep 35
done
EOF_AS_SH
chmod 755 /usr/local/bin/neuroveil-antisleep.sh

if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    cat << 'EOF_AS_SVC' > /etc/systemd/system/neuroveil-antisleep.service
[Unit]
Description=NeuroVeil VPN Lightweight Anti-Sleep Keepalive
After=network.target network-online.target

[Service]
Type=simple
ExecStart=/bin/sh /usr/local/bin/neuroveil-antisleep.sh
Restart=always
RestartSec=5
OOMScoreAdjust=-900
Nice=10

[Install]
WantedBy=multi-user.target
EOF_AS_SVC
    systemctl daemon-reload >/dev/null 2>&1 || true
    systemctl enable neuroveil-antisleep.service >/dev/null 2>&1 || true
    systemctl restart neuroveil-antisleep.service >/dev/null 2>&1 || true
    if [ "$(systemctl is-active neuroveil-antisleep.service 2>/dev/null || echo inactive)" = "active" ]; then
        echo "ANTISLEEP_ACTIVE"
    else
        echo "ANTISLEEP_FAILED"
    fi
else
    if [ -f /var/run/neuroveil-antisleep.pid ]; then
        kill "$(cat /var/run/neuroveil-antisleep.pid 2>/dev/null)" >/dev/null 2>&1 || true
    fi
    nohup /bin/sh /usr/local/bin/neuroveil-antisleep.sh >/dev/null 2>&1 &
    echo $! > /var/run/neuroveil-antisleep.pid
    echo "ANTISLEEP_ACTIVE"
fi
"#
}

fn build_vps_antisleep_remove_cmd() -> &'static str {
    r#"
if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    systemctl stop neuroveil-antisleep.service >/dev/null 2>&1 || true
    systemctl disable neuroveil-antisleep.service >/dev/null 2>&1 || true
    rm -f /etc/systemd/system/neuroveil-antisleep.service
    systemctl daemon-reload >/dev/null 2>&1 || true
fi
if [ -f /var/run/neuroveil-antisleep.pid ]; then
    kill "$(cat /var/run/neuroveil-antisleep.pid 2>/dev/null)" >/dev/null 2>&1 || true
    rm -f /var/run/neuroveil-antisleep.pid
fi
rm -f /usr/local/bin/neuroveil-antisleep.sh
rm -f /etc/sysctl.d/99-neuroveil-antisleep.conf
echo "ANTISLEEP_OFF"
"#
}

pub async fn set_vps_antisleep(
    app: &AppHandle,
    creds: &VpsCredentials,
    enable: bool,
) -> Result<bool, String> {
    emit_log(
        app,
        if enable {
            "Установка и запуск лёгкого фонового анти-слипа на VPS..."
        } else {
            "Отключение и удаление фонового анти-слипа на VPS..."
        },
    );
    let sess = connect_ssh(app, creds).await?;
    let cmd = if enable {
        build_vps_antisleep_install_cmd()
    } else {
        build_vps_antisleep_remove_cmd()
    };
    let (code, out) = run_remote_cmd(&sess, cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    if enable {
        if !out.contains("ANTISLEEP_ACTIVE") {
            return Err(format!("Не удалось запустить службу анти-слипа на VPS (code {}): {}", code, out.trim()));
        }
        emit_log(
            app,
            "Анти-слип VPS успешно включён (ARP/UDP keepalive 35с + защита sing-box от swap).",
        );
        Ok(true)
    } else {
        emit_log(app, "Анти-слип VPS отключён.");
        Ok(false)
    }
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
    let mut all_multi_groups: Vec<VpsMultiProtocolGroup> = Vec::new();

    if has_legacy {
        let (_, legacy_status_out) = run_remote_cmd(&sess, "systemctl is-active sing-box 2>/dev/null || echo inactive")
            .await
            .unwrap_or((1, "inactive".to_string()));
        let (_, real_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/reality.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, awg_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, awg2_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg2.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, enigma_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/enigma.json 2>/dev/null || true").await.unwrap_or((1, String::new()));
        let (_, multi_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/multi_protocols.json 2>/dev/null || true").await.unwrap_or((1, String::new()));

        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&legacy_cfg_raw) {
            let (mut profs, protos, ports) = parse_container_profiles(
                "legacy-singbox",
                &connect_host,
                creds,
                &cfg,
                &real_raw,
                &awg_raw,
                &awg2_raw,
                &enigma_raw,
                creds.enable_port_hopping,
                multi_container_host,
            );
            let m_groups = apply_multi_groups(&mut profs, "legacy-singbox", &multi_raw);
            let client_count = profs.len();
            all_profiles.extend(profs);
            all_multi_groups.extend(m_groups);
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
        let (_, enigma_raw) = run_remote_cmd(&sess, &format!("cat \"{}/enigma.json\" 2>/dev/null || true", cdir))
            .await
            .unwrap_or((1, String::new()));
        let (_, multi_raw) = run_remote_cmd(&sess, &format!("cat \"{}/multi_protocols.json\" 2>/dev/null || true", cdir))
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

        let (mut profs, protos, ports) = parse_container_profiles(
            cid,
            &connect_host,
            creds,
            &cfg,
            &real_raw,
            &awg_raw,
            &awg2_raw,
            &enigma_raw,
            enable_port_hopping,
            multi_container_host,
        );
        let m_groups = apply_multi_groups(&mut profs, cid, &multi_raw);
        let client_count = profs.len();
        all_profiles.extend(profs);
        all_multi_groups.extend(m_groups);

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
    let (_, antisleep_out) = run_remote_cmd(
        &sess,
        "if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then systemctl is-active neuroveil-antisleep.service 2>/dev/null || echo inactive; elif [ -f /var/run/neuroveil-antisleep.pid ] && kill -0 \"$(cat /var/run/neuroveil-antisleep.pid 2>/dev/null)\" 2>/dev/null; then echo active; else echo inactive; fi",
    )
    .await
    .unwrap_or((1, "inactive".to_string()));
    let anti_sleep_enabled = antisleep_out.lines().any(|l| l.trim() == "active");

    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    emit_log(
        app,
        &format!(
            "Успешно авторизовано на сервере. Контейнеров: {}, ключей: {}, мульти-групп: {}",
            containers.len(),
            all_profiles.len(),
            all_multi_groups.len()
        ),
    );

    Ok(VpsAdminState {
        containers,
        profiles: all_profiles,
        multi_groups: all_multi_groups,
        anti_sleep_enabled,
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
    let cid_clean = target.id.clone();

    let cmd = match action {
        "start" | "restart" => {
            let enigma_part = format!(
                "if [ -f \"/etc/systemd/system/neuroveil-enigma-{cid}.service\" ]; then systemctl {act} \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true; fi",
                cid = cid_clean,
                act = action
            );
            if target.runtime == "docker" {
                format!(
                    "docker {act} \"{u}\"\n{ep}\nsleep 1\nif [ \"$(docker inspect -f '{{{{.State.Running}}}}' \"{u}\" 2>/dev/null)\" = \"true\" ]; then echo \"OK_RUNNING\"; else echo \"ERR_STATE\"; fi",
                    act = action,
                    u = target.unit_name,
                    ep = enigma_part
                )
            } else {
                format!(
                    "systemctl {act} \"{u}\"\n{ep}\nsleep 1\nsystemctl is-active \"{u}\"",
                    act = action,
                    u = target.unit_name,
                    ep = enigma_part
                )
            }
        }
        "stop" => {
            let enigma_stop = format!(
                "systemctl stop \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                 if [ -f \"{cdir}/enigma.pid\" ]; then kill \"$(cat \"{cdir}/enigma.pid\" 2>/dev/null)\" >/dev/null 2>&1 || true; fi",
                cid = cid_clean,
                cdir = target.cdir
            );
            if target.runtime == "docker" {
                format!("docker stop \"{u}\" >/dev/null 2>&1 || true\n{es}\necho \"OK_STOPPED\"", u = target.unit_name, es = enigma_stop)
            } else {
                format!("systemctl stop \"{u}\" >/dev/null 2>&1 || true\n{es}\necho \"OK_STOPPED\"", u = target.unit_name, es = enigma_stop)
            }
        }
        "delete" => {
            if target.id == "legacy-singbox" {
                "systemctl stop sing-box >/dev/null 2>&1 || true\n\
                 systemctl disable sing-box >/dev/null 2>&1 || true\n\
                 systemctl stop neuroveil-enigma-legacy-singbox >/dev/null 2>&1 || true\n\
                 systemctl disable neuroveil-enigma-legacy-singbox >/dev/null 2>&1 || true\n\
                 rm -f /etc/systemd/system/sing-box.service /etc/systemd/system/neuroveil-enigma-legacy-singbox.service\n\
                 systemctl daemon-reload >/dev/null 2>&1 || true\n\
                 rm -rf /etc/sing-box\n\
                 echo \"OK_DELETED\""
                    .to_string()
            } else {
                format!(
                    "if command -v docker >/dev/null 2>&1; then docker rm -f \"{u}\" >/dev/null 2>&1 || true; fi\n\
                     systemctl stop \"{u}\" \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                     systemctl disable \"{u}\" \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                     if [ -f \"{cdir}/enigma.pid\" ]; then kill \"$(cat \"{cdir}/enigma.pid\" 2>/dev/null)\" >/dev/null 2>&1 || true; fi\n\
                     rm -f \"/etc/systemd/system/{u}.service\" \"/etc/systemd/system/neuroveil-enigma-{cid}.service\" \"/var/log/neuroveil-enigma-{cid}.log\"\n\
                     systemctl daemon-reload >/dev/null 2>&1 || true\n\
                     rm -rf \"{cdir}\"\n\
                     echo \"OK_DELETED\"",
                    u = target.unit_name,
                    cid = cid_clean,
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

struct RemovalState {
    awg_json: Option<serde_json::Value>,
    awg_dirty: bool,
    awg_empty: bool,
    awg2_json: Option<serde_json::Value>,
    awg2_dirty: bool,
    awg2_empty: bool,
    enigma_json: Option<serde_json::Value>,
    enigma_dirty: bool,
    enigma_empty: bool,
}

fn remove_profile_in_memory(
    cfg: &mut serde_json::Value,
    state: &mut RemovalState,
    profile: &VpnProfile,
) {
    let proto = profile.protocol.as_str();
    if proto == "amneziawg" || proto == "awg_quic" || proto == "awg_vless" || proto == "awg2" || proto == "enigma" {
        let is_enigma = proto == "enigma";
        let is_awg2 = !is_enigma && (proto == "awg2" || profile.awg_version.as_deref() == Some("2.0"));
        let prof_priv = profile
            .awg_private_key
            .as_deref()
            .unwrap_or(&profile.password)
            .trim();
        let prof_ip = profile.awg_local_address.as_deref().unwrap_or("").trim();
        let prof_ip_host = prof_ip.split('/').next().unwrap_or("").trim();
        let mut removed_pub_key: Option<String> = None;
        let mut removed_ip: Option<String> = if !prof_ip.is_empty() {
            Some(prof_ip.to_string())
        } else {
            None
        };
        let mut removed_vless_uuid: Option<String> = profile
            .uuid
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mut remaining_devices_count = 0usize;

        let target_awg_opt = if is_enigma {
            &mut state.enigma_json
        } else if is_awg2 {
            &mut state.awg2_json
        } else {
            &mut state.awg_json
        };

        if let Some(awg_json) = target_awg_opt.as_mut() {
            if let Some(devs) = awg_json["devices"].as_array_mut() {
                devs.retain(|d| {
                    let d_priv = d["client_private_key"].as_str().unwrap_or("").trim();
                    let d_ip = d["ip"].as_str().unwrap_or("").trim();
                    let d_ip_host = d_ip.split('/').next().unwrap_or("").trim();
                    let d_name = d["name"].as_str().unwrap_or("").trim();
                    let is_match = (!prof_priv.is_empty() && d_priv == prof_priv)
                        || (!prof_ip_host.is_empty() && d_ip_host == prof_ip_host)
                        || d_name == profile.name.trim();
                    if is_match {
                        if let Some(pk) = d["client_public_key"].as_str() {
                            removed_pub_key = Some(pk.to_string());
                        }
                        if let Some(ip) = d["ip"].as_str() {
                            removed_ip = Some(ip.to_string());
                        }
                        if removed_vless_uuid.is_none() {
                            removed_vless_uuid = d["vless_uuid"]
                                .as_str()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty());
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
                } else {
                    remaining_devices_count = 1;
                }
            }

            if is_enigma {
                state.enigma_dirty = true;
                state.enigma_empty = remaining_devices_count == 0;
            } else if is_awg2 {
                state.awg2_dirty = true;
                state.awg2_empty = remaining_devices_count == 0;
            } else {
                state.awg_dirty = true;
                state.awg_empty = remaining_devices_count == 0;
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
                let matches_ep = if is_enigma {
                    tag == "enigma-in"
                } else if is_awg2 {
                    tag == "awg2-in" || (profile.port > 0 && ep_port == profile.port && tag != "enigma-in")
                } else {
                    tag == "awg-in" || (profile.port > 0 && ep_port == profile.port && tag != "enigma-in" && tag != "awg2-in")
                };
                if !matches_ep {
                    return true;
                }

                let rem_ip_host = removed_ip
                    .as_deref()
                    .and_then(|s| s.split('/').next())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());

                if let Some(peers) = ep["peers"].as_array_mut() {
                    let initial_len = peers.len();
                    peers.retain(|p| {
                        let p_pub = p["public_key"].as_str().unwrap_or("").trim();
                        if let Some(ref rem_pub) = removed_pub_key {
                            if !rem_pub.is_empty() && p_pub == rem_pub {
                                return false;
                            }
                        }
                        if let Some(rem_host) = rem_ip_host {
                            if let Some(allowed) = p["allowed_ips"].as_array() {
                                if allowed.iter().any(|ip| {
                                    ip.as_str()
                                        .and_then(|s| s.split('/').next())
                                        .map(|h| h.trim() == rem_host)
                                        .unwrap_or(false)
                                }) {
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
                        (!prof_st_pw.is_empty() && u_pw == prof_st_pw) || (prof_st_pw.is_empty() && u_name == prof_name)
                    } else {
                        (!prof_uuid.is_empty() && u_uuid == prof_uuid)
                            || (!prof_pw.is_empty() && u_pw == prof_pw)
                            || (!prof_pw.is_empty() && u_uuid == prof_pw)
                            || (prof_uuid.is_empty() && prof_pw.is_empty() && u_name == prof_name)
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
}

fn build_multi_prune_script(
    multi_file: &str,
    multi_raw: &str,
    target_group_id: Option<&str>,
    removed_profiles: &[&VpnProfile],
) -> String {
    let trimmed = multi_raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(mut groups) = serde_json::from_str::<Vec<StoredMultiGroup>>(trimmed) else {
        return String::new();
    };
    let initial_len = groups.len();
    let mut changed = false;

    groups.retain_mut(|g| {
        if let Some(gid) = target_group_id {
            if !gid.is_empty() && g.id == gid {
                changed = true;
                return false;
            }
        }
        let item_before = g.items.len();
        g.items.retain(|it| {
            let it_cred = it.credential.trim();
            let hit = removed_profiles.iter().any(|rp| {
                let rp_cred = profile_credential(rp);
                rp.protocol == it.protocol
                    && (it.port == 0 || rp.port == it.port)
                    && ((!it_cred.is_empty() && rp_cred == it_cred)
                        || (it.ip.is_some() && rp.awg_local_address == it.ip)
                        || (!it.label.trim().is_empty() && rp.name.trim() == it.label.trim()))
            });
            !hit
        });
        if g.items.len() != item_before {
            changed = true;
        }
        g.items.len() >= 2
    });

    if !changed && groups.len() == initial_len {
        return String::new();
    }

    if groups.is_empty() {
        format!("rm -f \"{}\"\n", multi_file)
    } else if let Ok(serialized) = serde_json::to_string_pretty(&groups) {
        format!(
            "cat << 'EOF_MULTI' > \"{multi_file}\"\n{serialized}\nEOF_MULTI\nchmod 600 \"{multi_file}\"\n"
        )
    } else {
        String::new()
    }
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

    let awg_file = format!("{}/awg.json", target.cdir);
    let awg2_file = format!("{}/awg2.json", target.cdir);
    let enigma_file = format!("{}/enigma.json", target.cdir);
    let multi_file = format!("{}/multi_protocols.json", target.cdir);

    let proto = profile.protocol.as_str();
    let needs_awg = proto == "amneziawg" || proto == "awg_quic" || proto == "awg_vless";
    let needs_awg2 = proto == "awg2";
    let needs_enigma = proto == "enigma";

    let awg_json = if needs_awg {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let awg2_json = if needs_awg2 {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg2_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let enigma_json = if needs_enigma {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", enigma_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let (_, multi_raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null || true", multi_file))
        .await
        .unwrap_or((1, String::new()));

    let mut rem_state = RemovalState {
        awg_json,
        awg_dirty: false,
        awg_empty: false,
        awg2_json,
        awg2_dirty: false,
        awg2_empty: false,
        enigma_json,
        enigma_dirty: false,
        enigma_empty: false,
    };
    remove_profile_in_memory(&mut cfg, &mut rem_state, profile);

    let mut awg_update_script = String::new();
    if rem_state.awg_dirty {
        if rem_state.awg_empty {
            awg_update_script.push_str(&format!("rm -f \"{}\"\n", awg_file));
        } else if let Some(ref j) = rem_state.awg_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            awg_update_script.push_str(&format!(
                "cat << 'EOF_AWG' > \"{awg_file}\"\n{s}\nEOF_AWG\nchmod 600 \"{awg_file}\"\n"
            ));
        }
    }
    if rem_state.awg2_dirty {
        if rem_state.awg2_empty {
            awg_update_script.push_str(&format!("rm -f \"{}\"\n", awg2_file));
        } else if let Some(ref j) = rem_state.awg2_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            awg_update_script.push_str(&format!(
                "cat << 'EOF_AWG2' > \"{awg2_file}\"\n{s}\nEOF_AWG2\nchmod 600 \"{awg2_file}\"\n"
            ));
        }
    }
    if rem_state.enigma_dirty {
        if rem_state.enigma_empty {
            awg_update_script.push_str(&format!(
                "systemctl stop \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                 systemctl disable \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                 if [ -f \"{cdir}/enigma.pid\" ]; then kill \"$(cat \"{cdir}/enigma.pid\" 2>/dev/null)\" >/dev/null 2>&1 || true; fi\n\
                 rm -f \"/etc/systemd/system/neuroveil-enigma-{cid}.service\" \"{enigma_file}\" \"{cdir}/enigma.nvkey\" \"{cdir}/enigma.pid\"\n\
                 systemctl daemon-reload >/dev/null 2>&1 || true\n",
                cid = target.id,
                cdir = target.cdir,
                enigma_file = enigma_file
            ));
        } else if let Some(ref j) = rem_state.enigma_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            awg_update_script.push_str(&format!(
                "cat << 'EOF_ENIGMA' > \"{enigma_file}\"\n{s}\nEOF_ENIGMA\nchmod 600 \"{enigma_file}\"\n"
            ));
        }
    }
    let multi_script = build_multi_prune_script(&multi_file, &multi_raw, None, &[profile]);
    awg_update_script.push_str(&multi_script);

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

pub async fn delete_vps_multi_group(
    app: &AppHandle,
    creds: &VpsCredentials,
    profiles: &[VpnProfile],
    group_id: Option<&str>,
    group_name: &str,
) -> Result<(), String> {
    if profiles.is_empty() {
        return Ok(());
    }
    emit_log(
        app,
        &format!(
            "Удаление мультипротокольной группы «{}» ({} ключей) с сервера {}...",
            group_name,
            profiles.len(),
            creds.host
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let first = &profiles[0];
    let target = match resolve_container_target(
        &sess,
        first.container_id.as_deref(),
        if first.port > 0 { Some(first.port) } else { None },
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

    let awg_file = format!("{}/awg.json", target.cdir);
    let awg2_file = format!("{}/awg2.json", target.cdir);
    let enigma_file = format!("{}/enigma.json", target.cdir);
    let multi_file = format!("{}/multi_protocols.json", target.cdir);

    let needs_awg = profiles.iter().any(|p| {
        p.protocol == "amneziawg" || p.protocol == "awg_quic" || p.protocol == "awg_vless"
    });
    let needs_awg2 = profiles.iter().any(|p| p.protocol == "awg2");
    let needs_enigma = profiles.iter().any(|p| p.protocol == "enigma");

    let awg_json = if needs_awg {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let awg2_json = if needs_awg2 {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", awg2_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let enigma_json = if needs_enigma {
        let (_, r) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null", enigma_file))
            .await
            .unwrap_or((1, String::new()));
        serde_json::from_str::<serde_json::Value>(&r).ok()
    } else {
        None
    };
    let (_, multi_raw) = run_remote_cmd(&sess, &format!("cat \"{}\" 2>/dev/null || true", multi_file))
        .await
        .unwrap_or((1, String::new()));

    let mut rem_state = RemovalState {
        awg_json,
        awg_dirty: false,
        awg_empty: false,
        awg2_json,
        awg2_dirty: false,
        awg2_empty: false,
        enigma_json,
        enigma_dirty: false,
        enigma_empty: false,
    };

    for prof in profiles {
        remove_profile_in_memory(&mut cfg, &mut rem_state, prof);
    }

    let mut extra_script = String::new();
    if rem_state.awg_dirty {
        if rem_state.awg_empty {
            extra_script.push_str(&format!("rm -f \"{}\"\n", awg_file));
        } else if let Some(ref j) = rem_state.awg_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            extra_script.push_str(&format!(
                "cat << 'EOF_AWG' > \"{awg_file}\"\n{s}\nEOF_AWG\nchmod 600 \"{awg_file}\"\n"
            ));
        }
    }
    if rem_state.awg2_dirty {
        if rem_state.awg2_empty {
            extra_script.push_str(&format!("rm -f \"{}\"\n", awg2_file));
        } else if let Some(ref j) = rem_state.awg2_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            extra_script.push_str(&format!(
                "cat << 'EOF_AWG2' > \"{awg2_file}\"\n{s}\nEOF_AWG2\nchmod 600 \"{awg2_file}\"\n"
            ));
        }
    }
    if rem_state.enigma_dirty {
        if rem_state.enigma_empty {
            extra_script.push_str(&format!(
                "systemctl stop \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                 systemctl disable \"neuroveil-enigma-{cid}\" >/dev/null 2>&1 || true\n\
                 if [ -f \"{cdir}/enigma.pid\" ]; then kill \"$(cat \"{cdir}/enigma.pid\" 2>/dev/null)\" >/dev/null 2>&1 || true; fi\n\
                 rm -f \"/etc/systemd/system/neuroveil-enigma-{cid}.service\" \"{enigma_file}\" \"{cdir}/enigma.nvkey\" \"{cdir}/enigma.pid\"\n\
                 systemctl daemon-reload >/dev/null 2>&1 || true\n",
                cid = target.id,
                cdir = target.cdir,
                enigma_file = enigma_file
            ));
        } else if let Some(ref j) = rem_state.enigma_json {
            let s = serde_json::to_string_pretty(j).unwrap_or_default();
            extra_script.push_str(&format!(
                "cat << 'EOF_ENIGMA' > \"{enigma_file}\"\n{s}\nEOF_ENIGMA\nchmod 600 \"{enigma_file}\"\n"
            ));
        }
    }

    let prof_refs: Vec<&VpnProfile> = profiles.iter().collect();
    let multi_script = build_multi_prune_script(&multi_file, &multi_raw, group_id, &prof_refs);
    extra_script.push_str(&multi_script);

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
{extra_script}
{service_action}
"#,
        cdir = target.cdir,
        new_cfg_str = new_cfg_str,
        extra_script = extra_script,
        service_action = service_action
    );

    let _ = run_remote_cmd(&sess, &update_cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    emit_log(
        app,
        &format!(
            "Мультипротокольная группа «{}» удалена с сервера {} (контейнер {})!",
            group_name, creds.host, target.id
        ),
    );
    Ok(())
}

fn rand_u32_from_uuid() -> u32 {
    let b = *Uuid::new_v4().as_bytes();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn is_exit_wg_family(protocol: &str) -> bool {
    matches!(
        protocol.trim().to_lowercase().as_str(),
        "amneziawg" | "awg" | "awg2" | "wireguard" | "wg" | "awg_vless"
    )
}

/// Deploys an isolated sing-box Bridge Gateway on Server 1 (Entry VPS) that accepts a standard
/// single-protocol client connection (WireGuard / AmneziaWG 1.0 / AmneziaWG 3.1 / VLESS Reality /
/// Shadowsocks / Hysteria 2) from a Router, Phone, TV, or PC and transparently routes 100% of
/// its traffic into `exit_profile` (Server 2 — Exit VPS).
pub async fn deploy_bridge_client_gateway(
    app: &AppHandle,
    req: &CreateBridgeClientRequest,
    exit_profile: &VpnProfile,
) -> Result<BridgeClientConfig, String> {
    let emit_gw = |msg: &str| {
        let _ = app.emit("bridge-gateway-log", msg.to_string());
    };

    let entry_host = req.entry_host.trim().to_string();
    let entry_port = if req.entry_ssh_port == 0 {
        22
    } else {
        req.entry_ssh_port
    };
    let entry_user = if req.entry_username.trim().is_empty() {
        "root".to_string()
    } else {
        req.entry_username.trim().to_string()
    };
    let proto = req.protocol.trim().to_lowercase();

    if entry_host.is_empty() || req.entry_password.trim().is_empty() {
        return Err("Укажите IP-адрес и SSH-пароль Входного моста (Сервер 1).".to_string());
    }
    if exit_profile.protocol == "enigma" {
        return Err("Протокол Enigma не может быть Выходным сервером для шлюза моста.".to_string());
    }
    if !matches!(
        proto.as_str(),
        "wireguard" | "amneziawg_router" | "amneziawg" | "vless" | "shadowsocks" | "hysteria2"
    ) {
        return Err(format!("Неподдерживаемый протокол шлюза моста: {}", proto));
    }

    let device_name = if req.name.trim().is_empty() {
        match proto.as_str() {
            "wireguard" => "Роутер (WireGuard)".to_string(),
            "amneziawg_router" => "Роутер (AmneziaWG 1.0)".to_string(),
            "amneziawg" => "Устройство (AmneziaWG 3.1)".to_string(),
            "vless" => "Устройство (VLESS Reality)".to_string(),
            "shadowsocks" => "Роутер (Shadowsocks)".to_string(),
            "hysteria2" => "Устройство (Hysteria 2)".to_string(),
            _ => "Устройство через мост".to_string(),
        }
    } else {
        req.name.trim().to_string()
    };

    emit_gw(&format!(
        "1/5 Подключение по SSH к Входному мосту {}:{}...",
        entry_host, entry_port
    ));
    let sess = connect_ssh_endpoint(
        app,
        &entry_host,
        entry_port,
        &entry_user,
        &req.entry_password,
        "",
    )
    .await?;

    let gw_short = Uuid::new_v4().simple().to_string()[..8].to_string();
    let gw_id = format!("bgw-{}", gw_short);
    let service_name = format!("neuroveil-{}", gw_id);
    let gw_dir = format!("/opt/neuroveil/bridge-gateways/{}", gw_id);

    // Ensure sing-box is installed & detect Docker/systemd + architecture
    emit_gw("2/5 Проверка ядра sing-box и свободных портов на Входном мосте...");
    let (_, arch_raw) = run_remote_cmd(&sess, "uname -m").await?;
    let sb_arch = match arch_raw.trim() {
        "aarch64" | "arm64" => "arm64",
        "armv7l" => "armv7",
        _ => "amd64",
    };

    let prep_cmd = format!(
        r#"
set -e
mkdir -p "{gdir}"
if [ ! -x /usr/local/bin/sing-box ]; then
    SB_TAG=$(curl -fsSL https://api.github.com/repos/Leadaxe/sing-box-lx/releases/latest 2>/dev/null | grep '"tag_name"' | head -n 1 | cut -d '"' -f 4 || true)
    if [ -n "$SB_TAG" ]; then
        SB_VER="${{SB_TAG#v}}"
        URL="https://github.com/Leadaxe/sing-box-lx/releases/download/${{SB_TAG}}/sing-box-${{SB_VER}}-linux-{arch}.tar.gz"
    else
        SB_VER_UP=$(curl -fsSL https://api.github.com/repos/SagerNet/sing-box/releases/latest | grep '"tag_name"' | head -n 1 | cut -d '"' -f 4 | sed 's/^v//')
        URL="https://github.com/SagerNet/sing-box/releases/download/v${{SB_VER_UP}}/sing-box-${{SB_VER_UP}}-linux-{arch}.tar.gz"
    fi
    rm -rf /tmp/sb-bgw-install && mkdir -p /tmp/sb-bgw-install
    curl -fsSL "$URL" -o /tmp/sb-bgw-install/sb.tar.gz
    tar -xzf /tmp/sb-bgw-install/sb.tar.gz -C /tmp/sb-bgw-install
    mv -f /tmp/sb-bgw-install/sing-box-*/sing-box /usr/local/bin/sing-box.tmp
    chmod +x /usr/local/bin/sing-box.tmp
    mv -f /usr/local/bin/sing-box.tmp /usr/local/bin/sing-box
    rm -rf /tmp/sb-bgw-install
fi
DOCKER_OK=0
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    if docker image inspect debian:bookworm-slim >/dev/null 2>&1; then
        DOCKER_OK=1
    fi
fi
echo "DOCKER_OK=$DOCKER_OK"
echo "===PORTS==="
ss -tulnH 2>/dev/null | awk '{{print $5}}' | rev | cut -d: -f1 | rev | sort -un || true
grep -hoE '"(listen_port|external_port|wg_port)"\s*:\s*[0-9]+' /etc/sing-box/config.json /opt/neuroveil/containers/*/config.json /opt/neuroveil/bridge-gateways/*/config.json 2>/dev/null | grep -oE '[0-9]+' || true
echo "===SUBNETS99==="
grep -hoE '10\.99\.[0-9]+' /opt/neuroveil/bridge-gateways/*/config.json 2>/dev/null | sort -u || true
"#,
        gdir = gw_dir,
        arch = sb_arch
    );

    let (prep_code, prep_out) = run_remote_cmd(&sess, &prep_cmd).await?;
    if prep_code != 0 {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "prep failed", "en")
            .await;
        return Err(format!(
            "Ошибка подготовки Входного моста ({}): {}",
            entry_host, prep_out
        ));
    }

    let use_docker = prep_out.lines().any(|l| l.trim() == "DOCKER_OK=1");
    let mut used_ports: HashSet<u16> = HashSet::new();
    let mut used_octets: HashSet<u8> = HashSet::new();
    let mut sec = "";
    for line in prep_out.lines() {
        let t = line.trim();
        if t == "===PORTS===" {
            sec = "ports";
            continue;
        }
        if t == "===SUBNETS99===" {
            sec = "subnets";
            continue;
        }
        if sec == "ports" {
            if let Ok(p) = t.parse::<u16>() {
                if p > 0 {
                    used_ports.insert(p);
                }
            }
        } else if sec == "subnets" {
            if let Some(oct) = t.strip_prefix("10.99.").and_then(|s| s.parse::<u8>().ok()) {
                used_octets.insert(oct);
            }
        }
    }

    // Choose listen port
    let listen_port: u16 = if req.custom_port > 0 {
        if used_ports.contains(&req.custom_port) {
            let _ = sess
                .disconnect(russh::Disconnect::ByApplication, "port busy", "en")
                .await;
            return Err(format!(
                "Порт {} уже занят на Входном мосте {}. Укажите другой порт или оставьте 0 для автоподбора.",
                req.custom_port, entry_host
            ));
        }
        req.custom_port
    } else {
        let candidates: Vec<u16> = match proto.as_str() {
            "wireguard" => (51830..=52999).collect(),
            "amneziawg_router" => (52100..=53999).collect(),
            "amneziawg" => (53100..=54999).collect(),
            "vless" => {
                let mut v = vec![8443, 9443, 2053, 2083, 2087, 2096];
                v.extend(10443..=19999);
                v
            }
            "shadowsocks" => {
                let mut v = vec![8388, 9388, 10388];
                v.extend(18388..=28388);
                v
            }
            "hysteria2" => {
                let mut v = vec![18443, 19443, 24443];
                v.extend(25000..=35000);
                v
            }
            _ => (20000..=30000).collect(),
        };
        candidates
            .into_iter()
            .find(|p| !used_ports.contains(p))
            .ok_or_else(|| "Не удалось найти свободный порт на Входном мосте.".to_string())?
    };

    let subnet_octet: u8 = (1u8..=250u8)
        .find(|o| !used_octets.contains(o))
        .unwrap_or(1);

    let is_wg_client = matches!(proto.as_str(), "wireguard" | "amneziawg_router" | "amneziawg");
    let exit_is_wg = is_exit_wg_family(&exit_profile.protocol);
    let wg_mtu: u16 = if exit_is_wg { 1220 } else { 1280 };
    let nested_exit_mtu: Option<u16> = if is_wg_client && exit_is_wg {
        Some(1220)
    } else if exit_is_wg {
        Some(1280)
    } else {
        None
    };

    // Build Exit outbound/endpoint ("bridge-exit") pointing from Server 1 -> Server 2
    let (mut endpoints, mut outbounds) = build_bridge_exit_nodes(exit_profile, nested_exit_mtu);
    outbounds.push(json!({
        "type": "direct",
        "tag": "direct"
    }));

    let mut inbounds: Vec<serde_json::Value> = Vec::new();
    let mut route_rules: Vec<serde_json::Value> = Vec::new();
    let mut fw_rules = String::new();
    let mut extra_pre_cmd = String::new();

    let cfg_cert_path = if use_docker {
        "/etc/sing-box/cert.pem".to_string()
    } else {
        format!("{}/cert.pem", gw_dir)
    };
    let cfg_key_path = if use_docker {
        "/etc/sing-box/key.pem".to_string()
    } else {
        format!("{}/key.pem", gw_dir)
    };

    emit_gw(&format!(
        "3/5 Генерация ключей для {} на порту {} (выход: {} → {})...",
        proto.to_uppercase(),
        listen_port,
        exit_profile.name,
        exit_profile.server
    ));

    let config_format: String;
    let config_text: String;
    let uri: String;

    match proto.as_str() {
        "wireguard" => {
            let (s_priv, s_pub) = generate_wg_keypair_remote(&sess).await?;
            let (c_priv, c_pub) = generate_wg_keypair_remote(&sess).await?;
            let (_, psk_raw) = run_remote_cmd(&sess, "openssl rand -base64 32").await?;
            let psk = psk_raw.trim().lines().last().unwrap_or("").trim().to_string();

            let srv_ip = format!("10.99.{}.1/24", subnet_octet);
            let cli_ip = format!("10.99.{}.2/32", subnet_octet);
            let cidr = format!("10.99.{}.0/24", subnet_octet);

            let mut peer = json!({
                "public_key": c_pub,
                "allowed_ips": [cli_ip]
            });
            if !psk.is_empty() {
                peer["pre_shared_key"] = json!(psk);
            }

            endpoints.push(json!({
                "type": "wireguard",
                "tag": "gw-in",
                "listen_port": listen_port,
                "address": [srv_ip],
                "private_key": s_priv,
                "mtu": wg_mtu,
                "peers": [peer]
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));
            route_rules.push(json!({
                "source_ip_cidr": [cidr],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let mut conf = String::new();
            conf.push_str("[Interface]\n");
            conf.push_str(&format!("PrivateKey = {}\n", c_priv));
            conf.push_str(&format!("Address = {}\n", cli_ip));
            conf.push_str("DNS = 1.1.1.1, 1.0.0.1\n");
            conf.push_str(&format!("MTU = {}\n", wg_mtu));
            conf.push_str("\n[Peer]\n");
            conf.push_str(&format!("PublicKey = {}\n", s_pub));
            if !psk.is_empty() {
                conf.push_str(&format!("PresharedKey = {}\n", psk));
            }
            conf.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
            conf.push_str(&format!("Endpoint = {}:{}\n", entry_host, listen_port));
            conf.push_str("PersistentKeepalive = 25\n");

            config_format = "conf".to_string();
            config_text = conf.clone();
            uri = conf;
        }
        "amneziawg_router" => {
            let (s_priv, s_pub) = generate_wg_keypair_remote(&sess).await?;
            let (c_priv, c_pub) = generate_wg_keypair_remote(&sess).await?;
            let (_, psk_raw) = run_remote_cmd(&sess, "openssl rand -base64 32").await?;
            let psk = psk_raw.trim().lines().last().unwrap_or("").trim().to_string();

            let srv_ip = format!("10.99.{}.1/24", subnet_octet);
            let cli_ip = format!("10.99.{}.2/32", subnet_octet);
            let cidr = format!("10.99.{}.0/24", subnet_octet);

            let jc = 4 + (rand_u32_from_uuid() % 3);
            let jmin = 40u32;
            let jmax = 70u32;
            let s1 = 15 + (rand_u32_from_uuid() % 65);
            let mut s2 = 15 + (rand_u32_from_uuid() % 65);
            if s1 + 56 == s2 {
                s2 = (s2 + 7) % 100 + 15;
            }
            let h1 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000);
            let h2 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x1111;
            let h3 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x2222;
            let h4 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x3333;

            let mut peer = json!({
                "public_key": c_pub,
                "allowed_ips": [cli_ip]
            });
            if !psk.is_empty() {
                peer["pre_shared_key"] = json!(psk);
            }

            endpoints.push(json!({
                "type": "wireguard",
                "tag": "gw-in",
                "listen_port": listen_port,
                "address": [srv_ip],
                "private_key": s_priv,
                "mtu": wg_mtu,
                "jc": jc,
                "jmin": jmin,
                "jmax": jmax,
                "s1": s1,
                "s2": s2,
                "h1": h1,
                "h2": h2,
                "h3": h3,
                "h4": h4,
                "peers": [peer]
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));
            route_rules.push(json!({
                "source_ip_cidr": [cidr],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let mut conf = String::new();
            conf.push_str("[Interface]\n");
            conf.push_str(&format!("PrivateKey = {}\n", c_priv));
            conf.push_str(&format!("Address = {}\n", cli_ip));
            conf.push_str("DNS = 1.1.1.1, 1.0.0.1\n");
            conf.push_str(&format!("MTU = {}\n", wg_mtu));
            conf.push_str(&format!("Jc = {}\n", jc));
            conf.push_str(&format!("Jmin = {}\n", jmin));
            conf.push_str(&format!("Jmax = {}\n", jmax));
            conf.push_str(&format!("S1 = {}\n", s1));
            conf.push_str(&format!("S2 = {}\n", s2));
            conf.push_str(&format!("H1 = {}\n", h1));
            conf.push_str(&format!("H2 = {}\n", h2));
            conf.push_str(&format!("H3 = {}\n", h3));
            conf.push_str(&format!("H4 = {}\n", h4));
            conf.push_str("\n[Peer]\n");
            conf.push_str(&format!("PublicKey = {}\n", s_pub));
            if !psk.is_empty() {
                conf.push_str(&format!("PresharedKey = {}\n", psk));
            }
            conf.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
            conf.push_str(&format!("Endpoint = {}:{}\n", entry_host, listen_port));
            conf.push_str("PersistentKeepalive = 25\n");

            config_format = "conf".to_string();
            config_text = conf.clone();
            uri = conf;
        }
        "amneziawg" => {
            let (s_priv, s_pub) = generate_wg_keypair_remote(&sess).await?;
            let (c_priv, c_pub) = generate_wg_keypair_remote(&sess).await?;
            let (_, psk_raw) = run_remote_cmd(&sess, "openssl rand -base64 32").await?;
            let psk = psk_raw.trim().lines().last().unwrap_or("").trim().to_string();
            let (_, hpk_raw) = run_remote_cmd(&sess, "openssl rand -base64 32").await?;
            let hpk = hpk_raw.trim().lines().last().unwrap_or("").trim().to_string();

            let srv_ip = format!("10.99.{}.1/24", subnet_octet);
            let cli_ip = format!("10.99.{}.2/32", subnet_octet);
            let cidr = format!("10.99.{}.0/24", subnet_octet);

            let jc = 4 + (rand_u32_from_uuid() % 3);
            let jmin = 40u32;
            let jmax = 70u32;
            let s1 = 15 + (rand_u32_from_uuid() % 65);
            let mut s2 = 15 + (rand_u32_from_uuid() % 65);
            if s1 + 56 == s2 {
                s2 = (s2 + 7) % 100 + 15;
            }
            let s3 = 12 + (rand_u32_from_uuid() % 30);
            let s4 = 12 + (rand_u32_from_uuid() % 30);
            let h1 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000);
            let h2 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x1111;
            let h3 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x2222;
            let h4 = 100_000 + (rand_u32_from_uuid() % 1_900_000_000) ^ 0x3333;

            let mut peer = json!({
                "public_key": c_pub,
                "allowed_ips": [cli_ip]
            });
            if !psk.is_empty() {
                peer["pre_shared_key"] = json!(psk);
            }

            endpoints.push(json!({
                "type": "wireguard",
                "tag": "gw-in",
                "listen_port": listen_port,
                "address": [srv_ip],
                "private_key": s_priv,
                "mtu": wg_mtu,
                "header_protection_key": hpk,
                "random_trailers": true,
                "disable_cookies": true,
                "rekey_after_time": "100-120",
                "rekey_timeout": "3-7",
                "reject_after_time": "150-180",
                "keepalive_timeout": "5-15",
                "max_handshake_attempts": "15-20",
                "jc": jc,
                "jmin": jmin,
                "jmax": jmax,
                "s1": s1,
                "s2": s2,
                "s3": s3,
                "s4": s4,
                "h1": h1,
                "h2": h2,
                "h3": h3,
                "h4": h4,
                "peers": [peer]
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));
            route_rules.push(json!({
                "source_ip_cidr": [cidr],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let mut temp_prof = VpnProfile::default();
            temp_prof.id = gw_id.clone();
            temp_prof.name = device_name.clone();
            temp_prof.protocol = "amneziawg".to_string();
            temp_prof.server = entry_host.clone();
            temp_prof.port = listen_port;
            temp_prof.awg_version = Some("3.1".to_string());
            temp_prof.awg_private_key = Some(c_priv);
            temp_prof.awg_public_key = Some(s_pub);
            temp_prof.awg_preshared_key = if psk.is_empty() { None } else { Some(psk) };
            temp_prof.awg_local_address = Some(cli_ip);
            temp_prof.awg_jc = Some(jc as u16);
            temp_prof.awg_jmin = Some(jmin as u16);
            temp_prof.awg_jmax = Some(jmax as u16);
            temp_prof.awg_s1 = Some(s1 as u16);
            temp_prof.awg_s2 = Some(s2 as u16);
            temp_prof.awg_s3 = Some(s3 as u16);
            temp_prof.awg_s4 = Some(s4 as u16);
            temp_prof.awg_h1 = Some(h1.to_string());
            temp_prof.awg_h2 = Some(h2.to_string());
            temp_prof.awg_h3 = Some(h3.to_string());
            temp_prof.awg_h4 = Some(h4.to_string());
            temp_prof.awg_header_protection_key = Some(hpk);
            temp_prof.awg_random_trailers = Some(true);
            temp_prof.awg_disable_cookies = Some(true);
            temp_prof.awg_persistent_keepalive = Some("25".to_string());

            let conf = build_awg_conf(&temp_prof);
            let awg_uri = build_profile_uri(&temp_prof);
            config_format = "conf".to_string();
            config_text = conf;
            uri = awg_uri;
        }
        "vless" => {
            let (r_priv, r_pub, r_sid) = generate_reality_keypair_remote(&sess).await?;
            let client_uuid = Uuid::new_v4().to_string();
            let sni = if req.reality_sni.trim().is_empty() {
                "www.microsoft.com".to_string()
            } else {
                req.reality_sni.trim().to_string()
            };

            inbounds.push(json!({
                "type": "vless",
                "tag": "gw-in",
                "listen": "::",
                "listen_port": listen_port,
                "users": [
                    {
                        "name": device_name,
                        "uuid": client_uuid,
                        "flow": "xtls-rprx-vision"
                    }
                ],
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "reality": {
                        "enabled": true,
                        "handshake": {
                            "server": sni,
                            "server_port": 443
                        },
                        "private_key": r_priv,
                        "short_id": [r_sid]
                    }
                }
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; fi\n\
                 iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let built_uri = format!(
                "vless://{}@{}:{}?encryption=none&flow=xtls-rprx-vision&security=reality&sni={}&fp=chrome&pbk={}&sid={}&type=tcp#{}",
                urlencoding::encode(&client_uuid),
                entry_host,
                listen_port,
                urlencoding::encode(&sni),
                urlencoding::encode(&r_pub),
                urlencoding::encode(&r_sid),
                urlencoding::encode(&device_name),
            );
            config_format = "uri".to_string();
            config_text = built_uri.clone();
            uri = built_uri;
        }
        "shadowsocks" => {
            use base64::Engine;
            let ss_method = "aes-256-gcm";
            let ss_pass = Uuid::new_v4().simple().to_string()[..24].to_string();

            inbounds.push(json!({
                "type": "shadowsocks",
                "tag": "gw-in",
                "listen": "::",
                "listen_port": listen_port,
                "method": ss_method,
                "password": ss_pass
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/tcp >/dev/null 2>&1 || true; ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/tcp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p tcp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p tcp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p tcp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p tcp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let userinfo = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(format!("{}:{}", ss_method, ss_pass));
            let built_uri = format!(
                "ss://{}@{}:{}#{}",
                userinfo,
                entry_host,
                listen_port,
                urlencoding::encode(&device_name),
            );
            config_format = "uri".to_string();
            config_text = built_uri.clone();
            uri = built_uri;
        }
        "hysteria2" => {
            let hy2_pass = Uuid::new_v4().simple().to_string()[..24].to_string();
            let sni = if req.reality_sni.trim().is_empty() {
                "www.bing.com".to_string()
            } else {
                req.reality_sni.trim().to_string()
            };

            extra_pre_cmd.push_str(&format!(
                r#"
openssl ecparam -genkey -name prime256v1 -out "{gdir}/key.pem" >/dev/null 2>&1
openssl req -new -x509 -key "{gdir}/key.pem" -out "{gdir}/cert.pem" -subj "/CN={sni}" -days 3650 >/dev/null 2>&1
chmod 600 "{gdir}/key.pem"
"#,
                gdir = gw_dir,
                sni = sni
            ));

            inbounds.push(json!({
                "type": "hysteria2",
                "tag": "gw-in",
                "listen": "::",
                "listen_port": listen_port,
                "users": [
                    {
                        "name": device_name,
                        "password": hy2_pass
                    }
                ],
                "masquerade": format!("https://{}", sni),
                "tls": {
                    "enabled": true,
                    "server_name": sni,
                    "alpn": ["h3"],
                    "certificate_path": cfg_cert_path,
                    "key_path": cfg_key_path
                }
            }));

            route_rules.push(json!({
                "inbound": ["gw-in"],
                "outbound": "bridge-exit"
            }));

            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -C PREROUTING -p udp --dport {p} -j RETURN 2>/dev/null || iptables -t nat -I PREROUTING 1 -p udp --dport {p} -j RETURN >/dev/null 2>&1 || true\n\
                 iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT 1 -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n",
                p = listen_port
            ));

            let built_uri = format!(
                "hysteria2://{}@{}:{}/?sni={}&insecure=1#{}",
                urlencoding::encode(&hy2_pass),
                entry_host,
                listen_port,
                urlencoding::encode(&sni),
                urlencoding::encode(&device_name),
            );
            config_format = "uri".to_string();
            config_text = built_uri.clone();
            uri = built_uri;
        }
        _ => unreachable!(),
    }

    let mut gw_config = json!({
        "log": {
            "level": "info",
            "timestamp": true
        },
        "dns": {
            "servers": [
                {
                    "type": "udp",
                    "tag": "dns-direct",
                    "server": "1.1.1.1"
                }
            ],
            "final": "dns-direct",
            "strategy": "ipv4_only"
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "final": "bridge-exit",
            "default_domain_resolver": "dns-direct"
        }
    });
    if !endpoints.is_empty() {
        gw_config["endpoints"] = json!(endpoints);
    }

    let gw_config_str = serde_json::to_string_pretty(&gw_config).map_err(|e| e.to_string())?;
    let runtime_name = if use_docker { "docker" } else { "systemd" };
    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let meta_json = json!({
        "id": gw_id,
        "name": device_name,
        "protocol": proto,
        "listen_port": listen_port,
        "exit_server": exit_profile.server,
        "exit_protocol": exit_profile.protocol,
        "runtime": runtime_name,
        "service_name": service_name,
        "created_at": created_at
    });
    let meta_str = serde_json::to_string_pretty(&meta_json).unwrap_or_default();

    let start_cmd = if use_docker {
        format!(
            r#"
docker rm -f "{svc}" >/dev/null 2>&1 || true
TUN_FLAG=""
if [ -e /dev/net/tun ]; then TUN_FLAG="--device=/dev/net/tun"; fi
docker run -d \
  --name "{svc}" \
  --restart=always \
  --network=host \
  --cap-add=NET_ADMIN \
  $TUN_FLAG \
  -e ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true \
  -v /usr/local/bin/sing-box:/usr/local/bin/sing-box:ro \
  -v "{gdir}":/etc/sing-box \
  debian:bookworm-slim \
  /usr/local/bin/sing-box run -c /etc/sing-box/config.json
sleep 1
STATE=$(docker inspect -f '{{{{.State.Running}}}}' "{svc}" 2>/dev/null || echo false)
if [ "$STATE" != "true" ]; then
    echo "ERR_START_FAILED"
    docker logs --tail 25 "{svc}" 2>&1 || true
    exit 1
fi
"#,
            svc = service_name,
            gdir = gw_dir
        )
    } else {
        format!(
            r#"
cat << 'EOF_BGW_SVC' > "/etc/systemd/system/{svc}.service"
[Unit]
Description=NeuroVeil Bridge Gateway {gid} ({dname})
After=network.target nss-lookup.target

[Service]
Type=simple
Environment=ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true
ExecStart=/usr/local/bin/sing-box run -c {gdir}/config.json
Restart=on-failure
RestartSec=3
OOMScoreAdjust=-800
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF_BGW_SVC
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable "{svc}" >/dev/null 2>&1 || true
systemctl restart "{svc}"
sleep 1
if ! systemctl is-active --quiet "{svc}"; then
    echo "ERR_START_FAILED"
    journalctl -u "{svc}" -n 25 --no-pager 2>&1 || true
    exit 1
fi
"#,
            svc = service_name,
            gid = gw_id,
            dname = device_name,
            gdir = gw_dir
        )
    };

    // For config check: if use_docker, test with host paths or inside docker run --rm check
    let check_cmd = if use_docker {
        format!(
            r#"docker run --rm -e ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true -v /usr/local/bin/sing-box:/usr/local/bin/sing-box:ro -v "{gdir}":/etc/sing-box debian:bookworm-slim /usr/local/bin/sing-box check -c /etc/sing-box/config.json"#,
            gdir = gw_dir
        )
    } else {
        format!(
            r#"ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true /usr/local/bin/sing-box check -c "{gdir}/config.json""#,
            gdir = gw_dir
        )
    };

    emit_gw("4/5 Загрузка конфигурации шлюза моста и проверка sing-box check...");
    let deploy_script = format!(
        r#"
set -e
mkdir -p "{gdir}"
{extra_pre}
cat << 'EOF_BGW_CFG' > "{gdir}/config.json"
{cfg_str}
EOF_BGW_CFG
cat << 'EOF_BGW_META' > "{gdir}/meta.json"
{meta_str}
EOF_BGW_META
{check_cmd}
{fw_rules}
{start_cmd}
echo "BGW_OK"
"#,
        gdir = gw_dir,
        extra_pre = extra_pre_cmd,
        cfg_str = gw_config_str,
        meta_str = meta_str,
        check_cmd = check_cmd,
        fw_rules = fw_rules,
        start_cmd = start_cmd
    );

    let (dep_code, dep_out) = run_remote_cmd(&sess, &deploy_script).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await;

    if dep_code != 0 || !dep_out.contains("BGW_OK") {
        return Err(format!(
            "Ошибка запуска шлюза моста на {}: {}",
            entry_host, dep_out
        ));
    }

    emit_gw(&format!(
        "5/5 Готово! Шлюз «{}» запущен на {}:{} → {} ({})",
        device_name,
        entry_host,
        listen_port,
        exit_profile.server,
        exit_profile.protocol.to_uppercase()
    ));

    Ok(BridgeClientConfig {
        id: gw_id,
        name: device_name,
        protocol: proto,
        entry_host,
        entry_ssh_port: entry_port,
        entry_username: entry_user,
        listen_port,
        exit_profile_id: exit_profile.id.clone(),
        exit_profile_name: exit_profile.name.clone(),
        exit_server: exit_profile.server.clone(),
        exit_protocol: exit_profile.protocol.clone(),
        config_format,
        config_text,
        uri,
        service_name,
        runtime: runtime_name.to_string(),
        created_at,
    })
}

/// Stops and removes a Bridge Gateway container/service from Entry VPS via SSH.
pub async fn remove_bridge_client_gateway(
    app: &AppHandle,
    cfg: &BridgeClientConfig,
    password: &str,
) -> Result<String, String> {
    let sess = connect_ssh_endpoint(
        app,
        &cfg.entry_host,
        if cfg.entry_ssh_port == 0 {
            22
        } else {
            cfg.entry_ssh_port
        },
        if cfg.entry_username.trim().is_empty() {
            "root"
        } else {
            &cfg.entry_username
        },
        password,
        "",
    )
    .await?;

    let gw_dir = format!("/opt/neuroveil/bridge-gateways/{}", cfg.id);
    let svc = cfg.service_name.trim();
    let rm_cmd = format!(
        r#"
if command -v docker >/dev/null 2>&1; then
    docker rm -f "{svc}" >/dev/null 2>&1 || true
fi
if [ -f "/etc/systemd/system/{svc}.service" ]; then
    systemctl disable --now "{svc}" >/dev/null 2>&1 || true
    rm -f "/etc/systemd/system/{svc}.service"
    systemctl daemon-reload >/dev/null 2>&1 || true
fi
rm -rf "{gdir}"
echo "BGW_REMOVED"
"#,
        svc = svc,
        gdir = gw_dir
    );

    let (code, out) = run_remote_cmd(&sess, &rm_cmd).await?;
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "removed", "en")
        .await;

    if code != 0 && !out.contains("BGW_REMOVED") {
        return Err(format!("Ошибка удаления шлюза с {}: {}", cfg.entry_host, out));
    }

    Ok(format!(
        "Шлюз «{}» остановлен и удалён с Входного моста {}",
        cfg.name, cfg.entry_host
    ))
}


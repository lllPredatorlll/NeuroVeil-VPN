use crate::models::{DeployResult, VpnProfile, VpsCredentials};
use crate::singbox::build_profile_uri;
use russh::client::{self, Handle, KeyboardInteractiveAuthResponse};
use russh::{ChannelMsg, Preferred};
use serde_json::json;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

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

        let s = tokio::time::timeout(Duration::from_secs(6), TcpStream::connect(addr))
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
        Duration::from_secs(12),
        client::connect_stream(config, tcp_stream, SshClientHandler),
    )
    .await
    .map_err(|_| {
        format!(
            "Таймаут SSH-рукопожатия с {}:{} (сервер не ответил вовремя).",
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
    if !creds.deploy_vless && !creds.deploy_hy2 && !creds.deploy_tuic && !creds.deploy_ss && !creds.deploy_awg && !creds.deploy_ssh {
        return Err("Выберите хотя бы один протокол для развертывания (VLESS Reality, Hysteria 2, TUIC v5, Shadowsocks, Amnezia 3.1 или SSH Tunnel)".to_string());
    }

    let mut creds = creds.clone();
    if creds.deploy_awg && (creds.awg_port == 0 || creds.awg_port == 51820) {
        creds.awg_port = random_u32_in_range(20000, 55000) as u16;
        emit_log(app, &format!("Авто-рандомизация: выбран порт Amnezia 3.1 UDP {}", creds.awg_port));
    }

    let ssh_port = if creds.ssh_port == 0 { 22 } else { creds.ssh_port };

    emit_log(
        app,
        &format!(
            "=== Старт автоматического развертывания VPN на {} ===",
            creds.host
        ),
    );
    let sess = connect_ssh(app, &creds).await?;
    emit_log(app, "SSH-соединение установлено успешно.");

    // 1. Check OS and install curl / openssl / tar if needed
    emit_log(
        app,
        "Шаг 1/6: Проверка зависимостей (curl, openssl, tar, ca-certificates)...",
    );
    let prep_cmd = format!(
        r#"
export DEBIAN_FRONTEND=noninteractive
mkdir -p /etc/ssh/sshd_config.d
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
    apt-get install -y curl openssl tar ca-certificates iptables >/dev/null 2>&1 || true
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y curl openssl tar ca-certificates iptables >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y curl openssl tar ca-certificates iptables >/dev/null 2>&1 || true
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

    // 2. Download and install sing-box binary (sing-box-lx with native AmneziaWG support)
    emit_log(
        app,
        "Шаг 2/6: Загрузка и установка последней версии ядра sing-box...",
    );
    let install_sb_cmd = format!(
        r#"
set -e
systemctl stop sing-box >/dev/null 2>&1 || true
mkdir -p /etc/sing-box /tmp/sb-install
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
rm -f /usr/local/bin/sing-box
mv -f /tmp/sb-install/sing-box-*/sing-box /usr/local/bin/sing-box
chmod +x /usr/local/bin/sing-box
rm -rf /tmp/sb-install
/usr/local/bin/sing-box version | head -n 1
"#,
        arch = sb_arch
    );
    let (code, sb_ver_out) = run_remote_cmd(&sess, &install_sb_cmd).await?;
    if code != 0 {
        return Err(format!("Ошибка установки sing-box на сервере: {}", sb_ver_out));
    }
    emit_log(
        app,
        &format!(
            "Ядро установлено: {}",
            sb_ver_out.lines().last().unwrap_or("sing-box")
        ),
    );

    // 3. Generate TLS Certificate (for Hysteria 2 / TUIC) and Reality Keypair (for VLESS)
    let sni = if creds.sni.trim().is_empty() {
        "www.bing.com".to_string()
    } else {
        creds.sni.trim().to_string()
    };

    let (vless_uuid, vless_priv, vless_pub, vless_sid) = if creds.deploy_vless {
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
            r#"cat << 'EOF_REAL' > /etc/sing-box/reality.json
{{
  "private_key": "{}",
  "public_key": "{}",
  "short_id": "{}"
}}
EOF_REAL
chmod 600 /etc/sing-box/reality.json
"#,
            priv_key, pub_key, sid
        );
        let _ = run_remote_cmd(&sess, &reality_save_cmd).await;
        emit_log(app, "Ключи VLESS Reality созданы и сохранены в /etc/sing-box/reality.json.");
        (uuid, priv_key, pub_key, sid)
    } else {
        (String::new(), String::new(), String::new(), String::new())
    };

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
    ) = if creds.deploy_awg {
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
        let s1 = creds.awg_s1.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 18) as u16);
        let s2 = creds.awg_s2.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 18) as u16);
        let s3 = creds.awg_s3.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 18) as u16);
        let s4 = creds.awg_s4.map(|v| v.max(12)).unwrap_or_else(|| random_u32_in_range(12, 18) as u16);
        let h1 = creds.awg_h1.clone().unwrap_or_else(|| "1".to_string());
        let h2 = creds.awg_h2.clone().unwrap_or_else(|| "2".to_string());
        let h3 = creds.awg_h3.clone().unwrap_or_else(|| "3".to_string());
        let h4 = creds.awg_h4.clone().unwrap_or_else(|| "4".to_string());

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
            r#"cat << 'EOF_AWG' > /etc/sing-box/awg.json
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
      "ip": "10.8.0.2/32"
    }}
  ]
}}
EOF_AWG
chmod 600 /etc/sing-box/awg.json
"#,
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
            persistent_ka = persistent_ka
        );
        let _ = run_remote_cmd(&sess, &awg_save_cmd).await?;
        (
            s_priv,
            s_pub,
            c_priv,
            c_pub,
            hpk,
            psk,
            jc,
            jmin,
            jmax,
            s1,
            s2,
            s3,
            s4,
            h1,
            h2,
            h3,
            h4,
            rekey_after,
            rekey_to,
            reject_after,
            keepalive_to,
            max_handshake,
            persistent_ka,
            random_trailers,
            disable_cookies,
        )
    } else {
        (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            true,
            true,
        )
    };

    if creds.deploy_hy2 || creds.deploy_tuic {
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
openssl ecparam -genkey -name prime256v1 -out /etc/sing-box/key.pem
openssl req -new -x509 -key /etc/sing-box/key.pem -out /etc/sing-box/cert.pem -subj "/CN={sni}" -days 3650
chmod 600 /etc/sing-box/key.pem /etc/sing-box/cert.pem
"#,
            sni = sni
        );
        let (cert_code, cert_out) = run_remote_cmd(&sess, &cert_cmd).await?;
        if cert_code != 0 {
            return Err(format!("Ошибка генерации сертификата: {}", cert_out));
        }
        emit_log(app, "TLS-сертификаты созданы в /etc/sing-box/.");
    }

    // 4. Optimize Kernel UDP buffers & BBR for QUIC & TCP
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

    // 5. Construct server config for chosen protocols
    emit_log(
        app,
        "Шаг 5/6: Формирование конфигурации сервера sing-box...",
    );
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

    if creds.deploy_vless {
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
                "certificate_path": "/etc/sing-box/cert.pem",
                "key_path": "/etc/sing-box/key.pem"
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
        if creds.enable_port_hopping {
            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow 20000:50000/udp >/dev/null 2>&1 || true; ufw allow 53/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port=20000-50000/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-port=53/udp >/dev/null 2>&1 || true; firewall-cmd --reload >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -A PREROUTING -p udp --dport 20000:50000 -j REDIRECT --to-ports {p} >/dev/null 2>&1 || true\n\
                 iptables -t nat -A PREROUTING -p udp --dport 53 -j REDIRECT --to-ports {p} >/dev/null 2>&1 || true\n\
                 iptables -I INPUT -p udp --dport 20000:50000 -j ACCEPT >/dev/null 2>&1 || true\n\
                 iptables -I INPUT -p udp --dport 53 -j ACCEPT >/dev/null 2>&1 || true\n",
                p = creds.hy2_port
            ));
        } else {
            fw_rules.push_str(&format!(
                "if command -v ufw >/dev/null 2>&1; then ufw allow 53/udp >/dev/null 2>&1 || true; fi\n\
                 if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port=53/udp >/dev/null 2>&1 || true; firewall-cmd --reload >/dev/null 2>&1 || true; fi\n\
                 iptables -t nat -A PREROUTING -p udp --dport 53 -j REDIRECT --to-ports {p} >/dev/null 2>&1 || true\n\
                 iptables -I INPUT -p udp --dport 53 -j ACCEPT >/dev/null 2>&1 || true\n",
                p = creds.hy2_port
            ));
        }
    }

    if creds.deploy_tuic {
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
                "certificate_path": "/etc/sing-box/cert.pem",
                "key_path": "/etc/sing-box/key.pem"
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

    if creds.deploy_ss {
        let ss_in = json!({
            "type": "shadowsocks",
            "tag": "ss-in",
            "listen": "::",
            "listen_port": creds.ss_port,
            "method": creds.ss_method,
            "password": ss_password
        });
        inbounds.push(ss_in);
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

    let mut endpoints = Vec::new();
    let mut route_rules = Vec::new();

    if creds.deploy_awg {
        let mut awg_ep = json!({
            "type": "wireguard",
            "tag": "awg-in",
            "listen_port": creds.awg_port,
            "address": ["10.8.0.1/24"],
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
                    "allowed_ips": ["10.8.0.2/32"]
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
            "source_ip_cidr": ["10.8.0.0/24"],
            "outbound": "direct"
        }));

        fw_rules.push_str(&format!(
            "if command -v ufw >/dev/null 2>&1; then ufw allow {p}/udp >/dev/null 2>&1 || true; ufw allow 20000:50000/udp >/dev/null 2>&1 || true; ufw default allow routed >/dev/null 2>&1 || true; fi\n\
             if command -v firewall-cmd >/dev/null 2>&1; then firewall-cmd --permanent --add-port={p}/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-port=20000-50000/udp >/dev/null 2>&1 || true; firewall-cmd --permanent --add-masquerade >/dev/null 2>&1 || true; fi\n\
             iptables -C INPUT -p udp --dport {p} -j ACCEPT 2>/dev/null || iptables -I INPUT -p udp --dport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -C OUTPUT -p udp --sport {p} -j ACCEPT 2>/dev/null || iptables -I OUTPUT -p udp --sport {p} -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -I INPUT -p udp --dport 20000:50000 -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -A PREROUTING -p udp --dport 20000:50000 -j REDIRECT --to-ports {p} >/dev/null 2>&1 || true\n\
             iptables -I FORWARD -s 10.8.0.0/24 -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -I FORWARD -d 10.8.0.0/24 -j ACCEPT >/dev/null 2>&1 || true\n\
             iptables -t nat -A POSTROUTING -s 10.8.0.0/24 -j MASQUERADE >/dev/null 2>&1 || true\n",
            p = creds.awg_port
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

    let service_and_fw_cmd = format!(
        r#"
cat << 'EOF_CFG' > /etc/sing-box/config.json
{config_str}
EOF_CFG

cat << 'EOF_SVC' > /etc/systemd/system/sing-box.service
[Unit]
Description=NeuroVeil VPN sing-box Server
After=network.target nss-lookup.target

[Service]
Type=simple
Environment=ENABLE_DEPRECATED_LEGACY_DOMAIN_STRATEGY_OPTIONS=true
ExecStart=/usr/local/bin/sing-box run -c /etc/sing-box/config.json
Restart=on-failure
RestartSec=3
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF_SVC

if command -v ufw >/dev/null 2>&1; then
    ufw allow 22/tcp >/dev/null 2>&1 || true
    if [ "{port}" != "22" ] && [ -n "{port}" ]; then ufw allow {port}/tcp >/dev/null 2>&1 || true; fi
fi
iptables -I INPUT -p tcp --dport 22 -j ACCEPT >/dev/null 2>&1 || true
if [ "{port}" != "22" ] && [ -n "{port}" ]; then iptables -I INPUT -p tcp --dport {port} -j ACCEPT >/dev/null 2>&1 || true; fi

{fw_rules}

rm -f /etc/systemd/system/neuroveil-iptables.service
systemctl daemon-reload >/dev/null 2>&1 || true

systemctl enable sing-box >/dev/null 2>&1
systemctl restart sing-box
sleep 1
systemctl is-active sing-box
"#,
        config_str = config_str,
        port = ssh_port,
        fw_rules = fw_rules
    );

    emit_log(
        app,
        "Шаг 6/6: Применение правил фаервола и запуск сервиса sing-box...",
    );
    let (svc_code, svc_out) = run_remote_cmd(&sess, &service_and_fw_cmd).await?;
    if svc_code != 0 || !svc_out.contains("active") {
        let (_, journal) = run_remote_cmd(&sess, "journalctl -u sing-box -n 20 --no-pager").await?;
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "error", "en")
            .await;
        return Err(format!(
            "Сервис sing-box не запустился ({}):\n{}",
            svc_out, journal
        ));
    }
    let _ = sess
        .disconnect(russh::Disconnect::ByApplication, "deploy completed", "en")
        .await;

    let connect_host = creds.host.trim().to_string();

    let mut created_profiles = Vec::new();

    if creds.deploy_vless {
        let reality_sni = if creds.reality_sni.trim().is_empty() {
            "www.microsoft.com".to_string()
        } else {
            creds.reality_sni.trim().to_string()
        };
        let vless_name = format!("VLESS Reality ({})", creds.host.trim());
        let mut vless_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: vless_name,
            protocol: "vless".to_string(),
            server: connect_host.clone(),
            port: creds.vless_port,
            sni: reality_sni,
            insecure: false,
            password: vless_uuid.clone(),
            uuid: Some(vless_uuid),
            up_mbps: 100,
            down_mbps: 100,
            reality_public_key: Some(vless_pub),
            reality_short_id: Some(vless_sid),
            reality_flow: Some("xtls-rprx-vision".to_string()),
            tls_fragment: Some(true),
            ..Default::default()
        };
        vless_profile.uri = build_profile_uri(&vless_profile);
        created_profiles.push(vless_profile);
    }

    if creds.deploy_hy2 {
        let hy2_name = format!("Hysteria 2 ({})", creds.host.trim());
        let mut hy2_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: hy2_name,
            protocol: "hysteria2".to_string(),
            server: connect_host.clone(),
            port: creds.hy2_port,
            sni: sni.clone(),
            insecure: true,
            password: hy2_password,
            obfs_password: hy2_obfs,
            up_mbps: 100,
            down_mbps: 100,
            hop_ports: if creds.enable_port_hopping {
                Some("20000:50000".to_string())
            } else {
                None
            },
            hop_interval: if creds.enable_port_hopping {
                Some("30s".to_string())
            } else {
                None
            },
            ..Default::default()
        };
        hy2_profile.uri = build_profile_uri(&hy2_profile);
        created_profiles.push(hy2_profile);
    }

    if creds.deploy_tuic {
        let tuic_name = format!("TUIC v5 ({})", creds.host.trim());
        let mut tuic_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: tuic_name,
            protocol: "tuic".to_string(),
            server: connect_host.clone(),
            port: creds.tuic_port,
            sni,
            insecure: true,
            password: tuic_password,
            uuid: Some(tuic_uuid),
            up_mbps: 100,
            down_mbps: 100,
            pinned_on_pc: Some(true),
            ..Default::default()
        };
        tuic_profile.uri = build_profile_uri(&tuic_profile);
        created_profiles.push(tuic_profile);
    }

    if creds.deploy_ss {
        let ss_name = format!("Shadowsocks ({})", creds.host.trim());
        let mut ss_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: ss_name,
            protocol: "shadowsocks".to_string(),
            server: connect_host.clone(),
            port: creds.ss_port,
            insecure: false,
            password: ss_password,
            up_mbps: 100,
            down_mbps: 100,
            ss_method: Some(creds.ss_method.clone()),
            ..Default::default()
        };
        ss_profile.uri = build_profile_uri(&ss_profile);
        created_profiles.push(ss_profile);
    }

    if creds.deploy_awg {
        let awg_name = format!("Amnezia 3.1 ({})", creds.host.trim());
        let mut awg_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: awg_name,
            protocol: "amneziawg".to_string(),
            server: connect_host.clone(),
            port: creds.awg_port,
            password: awg_client_priv.clone(),
            awg_private_key: Some(awg_client_priv),
            awg_public_key: Some(awg_server_pub),
            awg_preshared_key: Some(awg_psk),
            awg_local_address: Some("10.8.0.2/32".to_string()),
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
            ..Default::default()
        };
        awg_profile.uri = build_profile_uri(&awg_profile);
        created_profiles.push(awg_profile);
    }

    if creds.deploy_ssh {
        let ssh_name = format!("SSH Tunnel ({}:{})", creds.host.trim(), ssh_port);
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
        created_profiles.push(ssh_profile);
    }

    emit_log(
        app,
        "=== Готово! Сервер развернут, выбранные профили добавлены в клиент ===",
    );

    Ok(DeployResult {
        success: true,
        message: "Сервер успешно настроен! Профили готовы к подключению.".to_string(),
        created_profiles,
        new_ssh_port: None,
    })
}

pub async fn clean_vps(app: &AppHandle, creds: &VpsCredentials) -> Result<String, String> {
    emit_log(
        app,
        &format!("=== Старт очистки сервера {} ===", creds.host),
    );
    let sess = connect_ssh(app, creds).await?;
    emit_log(
        app,
        "SSH-соединение установлено. Остановка и удаление служб VPN...",
    );

    let clean_cmd = format!(
        r#"
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
        "Удалены: сервис sing-box, бинарный файл, конфигурации /etc/sing-box и правила портов.",
    );

    emit_log(app, "=== Сервер полностью очищен! ===");

    Ok("Сервер успешно очищен от всех компонентов VPN.".to_string())
}

pub async fn create_vps_device_user(
    app: &AppHandle,
    creds: &VpsCredentials,
    device_name: &str,
    protocol: &str,
) -> Result<VpnProfile, String> {
    emit_log(
        app,
        &format!(
            "Создание нового ключа на сервере {} для устройства «{}» ({})...",
            creds.host,
            device_name,
            protocol.to_uppercase()
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let (code, raw_cfg) = run_remote_cmd(&sess, "cat /etc/sing-box/config.json").await?;
    if code != 0 || raw_cfg.trim().is_empty() {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "err", "en")
            .await;
        return Err(
            "На сервере не найден /etc/sing-box/config.json. Сначала разверните VPN во вкладке «VPS Автодеплой»."
                .to_string(),
        );
    }

    let mut cfg: serde_json::Value = serde_json::from_str(&raw_cfg)
        .map_err(|e| format!("Ошибка чтения серверного config.json: {}", e))?;

    let connect_host = creds.host.trim().to_string();

    if protocol == "amneziawg" || protocol == "awg2" {
        let is_awg2 = protocol == "awg2";
        let awg_file = if is_awg2 { "/etc/sing-box/awg2.json" } else { "/etc/sing-box/awg.json" };
        let target_tag = if is_awg2 { "awg2-in" } else { "awg-in" };
        let default_port = if is_awg2 { creds.awg2_port } else { creds.awg_port };
        let subnet_prefix = if is_awg2 { "10.8.1." } else { "10.8.0." };

        let (_, awg_raw) = run_remote_cmd(&sess, &format!("cat {} 2>/dev/null", awg_file)).await?;
        let mut awg_json: serde_json::Value = serde_json::from_str(&awg_raw)
            .map_err(|_| format!("На сервере не найден файл {}. Разверните выбранный протокол на сервере перед добавлением устройств.", awg_file))?;

        let (k_code, k_out) = run_remote_cmd(&sess, "/usr/local/bin/sing-box generate wg-keypair").await?;
        if k_code != 0 {
            return Err(format!("Ошибка генерации ключей WireGuard для устройства: {}", k_out));
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

        let endpoints = cfg["endpoints"]
            .as_array_mut()
            .ok_or_else(|| "В серверном конфиге отсутствует секция endpoints".to_string())?;

        let mut wg_idx = None;
        for (idx, ep) in endpoints.iter().enumerate() {
            if ep["type"].as_str() == Some("wireguard") {
                if ep["tag"].as_str() == Some(target_tag) || ep["listen_port"].as_u64() == Some(default_port as u64) {
                    wg_idx = Some(idx);
                    break;
                }
            }
        }
        if wg_idx.is_none() {
            // fallback: any wireguard endpoint
            for (idx, ep) in endpoints.iter().enumerate() {
                if ep["type"].as_str() == Some("wireguard") {
                    wg_idx = Some(idx);
                    break;
                }
            }
        }
        let wg_idx = wg_idx.ok_or_else(|| format!("На сервере не найден WireGuard endpoint для {}. Разверните протокол на сервере.", protocol))?;
        let wg_ep = &mut endpoints[wg_idx];

        let port = wg_ep["listen_port"].as_u64().unwrap_or(default_port as u64) as u16;
        let psk = awg_json["preshared_key"].as_str().map(|s| s.to_string());
        let peers = wg_ep["peers"].as_array_mut().ok_or_else(|| "В endpoint wireguard отсутствует массив peers".to_string())?;
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
        if let Some(devs) = awg_json["devices"].as_array_mut() {
            let mut dev_entry = json!({
                "name": device_name.trim(),
                "client_private_key": dev_priv,
                "client_public_key": dev_pub,
                "ip": peer_ip
            });
            if let Some(ref p) = psk {
                if !p.trim().is_empty() {
                    dev_entry["preshared_key"] = json!(p.trim());
                }
            }
            devs.push(dev_entry);
        }

        let new_cfg_str = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        let new_awg_str = serde_json::to_string_pretty(&awg_json).unwrap_or_default();
        let update_cmd = format!(
            r#"
cat << 'EOF_CFG' > /etc/sing-box/config.json
{new_cfg_str}
EOF_CFG
cat << 'EOF_AWG' > {awg_file}
{new_awg_str}
EOF_AWG
chmod 600 {awg_file}
systemctl restart sing-box
systemctl is-active sing-box
"#,
            new_cfg_str = new_cfg_str,
            awg_file = awg_file,
            new_awg_str = new_awg_str
        );

        let (restart_code, restart_out) = run_remote_cmd(&sess, &update_cmd).await?;
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;

        if restart_code != 0 || !restart_out.contains("active") {
            return Err(format!("Ошибка перезапуска sing-box: {}", restart_out));
        }

        let prof_name = device_name.trim().to_string();

        let server_pub = awg_json["server_public_key"].as_str().unwrap_or("").to_string();
        let hop_ports = awg_json["hop_ports"].as_str().unwrap_or("20000:50000").to_string();
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

        let mut awg_profile = VpnProfile {
            id: Uuid::new_v4().to_string(),
            name: prof_name,
            protocol: protocol.to_string(),
            server: connect_host,
            port,
            password: dev_priv.clone(),
            awg_private_key: Some(dev_priv),
            awg_public_key: Some(server_pub),
            awg_preshared_key: psk,
            awg_local_address: Some(peer_ip),
            awg_version: Some(if is_awg2 { "2.0".to_string() } else { "3.1".to_string() }),
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
            ..Default::default()
        };
        awg_profile.uri = build_profile_uri(&awg_profile);
        emit_log(
            app,
            &format!(
                "Ключ для устройства «{}» успешно создан и активирован на сервере!",
                device_name.trim()
            ),
        );
        return Ok(awg_profile);
    }

    let inbounds = cfg["inbounds"]
        .as_array_mut()
        .ok_or_else(|| "В серверном конфиге отсутствует секция inbounds".to_string())?;

    let new_password = Uuid::new_v4().simple().to_string();
    let new_uuid = Uuid::new_v4().to_string();
    let mut ss_user_pw = String::new();
    let mut inbound_ss_method = None;
    let mut found_port = if protocol == "vless" {
        creds.vless_port
    } else if protocol == "tuic" {
        creds.tuic_port
    } else if protocol == "shadowsocks" {
        creds.ss_port
    } else {
        creds.hy2_port
    };
    let mut found_sni = if protocol == "vless" {
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
    let mut updated = false;

    for inbound in inbounds.iter_mut() {
        let ib_type = inbound["type"].as_str().unwrap_or("");
        if ib_type == protocol {
            if let Some(p) = inbound["listen_port"].as_u64() {
                found_port = p as u16;
            }
            if let Some(s) = inbound["tls"]["server_name"].as_str() {
                found_sni = s.to_string();
            }
            if let Some(obfs_pw) = inbound["obfs"]["password"].as_str() {
                found_obfs = Some(obfs_pw.to_string());
            }
            if ib_type == "shadowsocks" {
                if let Some(m) = inbound["method"].as_str() {
                    inbound_ss_method = Some(m.to_string());
                }
                ss_user_pw = inbound["password"].as_str().unwrap_or("").to_string();
                updated = true;
            } else if let Some(users) = inbound["users"].as_array_mut() {
                if protocol == "vless" {
                    users.push(json!({
                        "name": device_name.trim(),
                        "uuid": new_uuid,
                        "flow": "xtls-rprx-vision"
                    }));
                } else if protocol == "tuic" {
                    users.push(json!({
                        "name": device_name.trim(),
                        "uuid": new_uuid,
                        "password": new_password
                    }));
                } else {
                    users.push(json!({
                        "name": device_name.trim(),
                        "password": new_password
                    }));
                }
                updated = true;
            }
            break;
        }
    }

    if !updated {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "err", "en")
            .await;
        return Err(format!(
            "На сервере не найден активный инбаунд для протокола {}. Переразверните сервер с включенным этим протоколом.",
            protocol
        ));
    }

    let (reality_pub, reality_sid) = if protocol == "vless" {
        let (_, real_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/reality.json 2>/dev/null").await?;
        if let Ok(real_json) = serde_json::from_str::<serde_json::Value>(&real_raw) {
            let pk = real_json["public_key"].as_str().map(|s| s.to_string());
            let sid = real_json["short_id"].as_str().map(|s| s.to_string());
            (pk, sid)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    if protocol != "shadowsocks" {
        let new_cfg_str = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        let update_cmd = format!(
            r#"
cat << 'EOF_CFG' > /etc/sing-box/config.json
{new_cfg_str}
EOF_CFG
systemctl restart sing-box
systemctl is-active sing-box
"#,
            new_cfg_str = new_cfg_str
        );

        let (restart_code, restart_out) = run_remote_cmd(&sess, &update_cmd).await?;
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;

        if restart_code != 0 || !restart_out.contains("active") {
            return Err(format!(
                "Ошибка перезапуска sing-box после добавления устройства: {}",
                restart_out
            ));
        }
    } else {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;
    }

    let prof_name = device_name.trim().to_string();

    let mut profile = VpnProfile {
        id: Uuid::new_v4().to_string(),
        name: prof_name,
        protocol: protocol.to_string(),
        server: connect_host,
        port: found_port,
        sni: found_sni,
        insecure: protocol != "vless",
        password: if protocol == "vless" {
            new_uuid.clone()
        } else if protocol == "shadowsocks" {
            ss_user_pw.clone()
        } else {
            new_password
        },
        uuid: if protocol == "tuic" || protocol == "vless" {
            Some(new_uuid)
        } else {
            None
        },
        obfs_password: found_obfs,
        up_mbps: 100,
        down_mbps: 100,
        hop_ports: if protocol == "hysteria2" && creds.enable_port_hopping {
            Some("20000:50000".to_string())
        } else {
            None
        },
        hop_interval: if protocol == "hysteria2" && creds.enable_port_hopping {
            Some("30s".to_string())
        } else {
            None
        },
        reality_public_key: reality_pub,
        reality_short_id: reality_sid,
        reality_flow: if protocol == "vless" { Some("xtls-rprx-vision".to_string()) } else { None },
        ss_method: if protocol == "shadowsocks" {
            inbound_ss_method.clone().or_else(|| Some(creds.ss_method.clone()))
        } else {
            None
        },
        pinned_on_pc: Some(false),
        ..Default::default()
    };
    profile.uri = build_profile_uri(&profile);

    emit_log(
        app,
        &format!(
            "Ключ для устройства «{}» успешно создан и активирован на сервере!",
            device_name.trim()
        ),
    );

    Ok(profile)
}

pub async fn fetch_vps_profiles(
    app: &AppHandle,
    creds: &VpsCredentials,
) -> Result<Vec<VpnProfile>, String> {
    emit_log(
        app,
        &format!(
            "Авторизация на VPS {} и получение списка устройств...",
            creds.host
        ),
    );
    let sess = connect_ssh(app, creds).await?;

    let (code, raw_cfg) = run_remote_cmd(&sess, "cat /etc/sing-box/config.json").await?;
    let (_, real_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/reality.json 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, awg_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg.json 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, awg2_raw) = run_remote_cmd(&sess, "cat /etc/sing-box/awg2.json 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, ipt_in_raw) = run_remote_cmd(&sess, "iptables -nvxL INPUT 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, ipt_out_raw) = run_remote_cmd(&sess, "iptables -nvxL OUTPUT 2>/dev/null").await.unwrap_or((1, String::new()));
    let (_, ipt_fwd_raw) = run_remote_cmd(&sess, "iptables -nvxL FORWARD 2>/dev/null").await.unwrap_or((1, String::new()));

    if code != 0 || raw_cfg.trim().is_empty() {
        let _ = sess
            .disconnect(russh::Disconnect::ByApplication, "done", "en")
            .await;
        return Err(
            "На сервере не найден /etc/sing-box/config.json. Сначала настройте VPN во вкладке «VPS Автодеплой»."
                .to_string(),
        );
    }

    let (cached_real_pub, cached_real_sid) = if let Ok(real_json) = serde_json::from_str::<serde_json::Value>(&real_raw) {
        (
            real_json["public_key"].as_str().map(|s| s.to_string()),
            real_json["short_id"].as_str().map(|s| s.to_string()),
        )
    } else {
        (None, None)
    };

    let cfg: serde_json::Value = serde_json::from_str(&raw_cfg)
        .map_err(|e| format!("Ошибка разбора серверного config.json: {}", e))?;

    let default_inbounds = Vec::new();
    let inbounds = cfg["inbounds"].as_array().unwrap_or(&default_inbounds);

    let mut profiles: Vec<VpnProfile> = Vec::new();

    let connect_host = creds.host.trim().to_string();

    for inbound in inbounds {
        let ib_type = inbound["type"].as_str().unwrap_or("");
        if ib_type != "hysteria2" && ib_type != "tuic" && ib_type != "vless" && ib_type != "shadowsocks" {
            continue;
        }

        let port = inbound["listen_port"]
            .as_u64()
            .unwrap_or(if ib_type == "vless" { 443 } else if ib_type == "tuic" { 9443 } else if ib_type == "shadowsocks" { 8388 } else { 8443 }) as u16;
        let sni = inbound["tls"]["server_name"]
            .as_str()
            .unwrap_or(if ib_type == "vless" { "www.microsoft.com" } else { "www.bing.com" })
            .to_string();
        let obfs_password = inbound["obfs"]["password"].as_str().map(|s| s.to_string());

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

                let proto_label = if ib_type == "vless" {
                    "VLESS Reality"
                } else if ib_type == "tuic" {
                    "TUIC v5"
                } else if ib_type == "shadowsocks" {
                    "Shadowsocks"
                } else {
                    "Hysteria 2"
                };
                let is_base = user_name == "neuroveil-user" || user_name == "default";
                let prof_name = if is_base {
                    format!("{} ({})", proto_label, connect_host)
                } else {
                    user_name.to_string()
                };
                let mut prof = VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: prof_name,
                    protocol: ib_type.to_string(),
                    server: connect_host.clone(),
                    port,
                    sni: sni.clone(),
                    insecure: ib_type != "vless",
                    password,
                    uuid,
                    obfs_password: obfs_password.clone(),
                    up_mbps: 100,
                    down_mbps: 100,
                    hop_ports: if ib_type == "hysteria2" && creds.enable_port_hopping {
                        Some("20000:50000".to_string())
                    } else {
                        None
                    },
                    hop_interval: if ib_type == "hysteria2" && creds.enable_port_hopping {
                        Some("30s".to_string())
                    } else {
                        None
                    },
                    reality_public_key: if ib_type == "vless" { cached_real_pub.clone() } else { None },
                    reality_short_id: if ib_type == "vless" { cached_real_sid.clone() } else { None },
                    reality_flow: if ib_type == "vless" { Some("xtls-rprx-vision".to_string()) } else { None },
                    ss_method: if ib_type == "shadowsocks" { inbound["method"].as_str().map(|s| s.to_string()) } else { None },
                    pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                    ..Default::default()
                };
                prof.uri = build_profile_uri(&prof);
                profiles.push(prof);
            }
        } else if ib_type == "shadowsocks" {
            let password = inbound["password"].as_str().unwrap_or("").to_string();
            if !password.is_empty() {
                let prof_name = format!("Shadowsocks ({})", creds.host.trim());
                let mut prof = VpnProfile {
                    id: Uuid::new_v4().to_string(),
                    name: prof_name,
                    protocol: "shadowsocks".to_string(),
                    server: connect_host.clone(),
                    port,
                    sni: sni.clone(),
                    insecure: false,
                    password,
                    up_mbps: 100,
                    down_mbps: 100,
                    ss_method: inbound["method"].as_str().map(|s| s.to_string()),
                    ..Default::default()
                };
                prof.uri = build_profile_uri(&prof);
                profiles.push(prof);
            }
        }
    }

    // Check endpoints for Amnezia 3.1 and AmneziaWG 2.0
    if let Some(endpoints) = cfg["endpoints"].as_array() {
        let awg_json = serde_json::from_str::<serde_json::Value>(&awg_raw).ok();
        let awg2_json = serde_json::from_str::<serde_json::Value>(&awg2_raw).ok();

        for ep in endpoints {
            if ep["type"].as_str() == Some("wireguard") {
                let tag = ep["tag"].as_str().unwrap_or("");
                let ep_port = ep["listen_port"].as_u64().unwrap_or(51820) as u16;
                let is_awg2 = tag == "awg2-in" || ep_port == creds.awg2_port || (awg2_json.is_some() && tag.contains("awg2"));
                let active_json = if is_awg2 { awg2_json.as_ref() } else { awg_json.as_ref() };
                let proto_str = if is_awg2 { "awg2" } else { "amneziawg" };
                let proto_ver = if is_awg2 { "2.0" } else { "3.1" };
                let proto_name = if is_awg2 { "AmneziaWG 2.0" } else { "Amnezia 3.1" };

                let port = ep_port;
                let server_pub = active_json.and_then(|j| j["server_public_key"].as_str()).unwrap_or("").to_string();
                let client_priv = active_json.and_then(|j| j["client_private_key"].as_str()).unwrap_or("").to_string();
                let psk = active_json.and_then(|j| j["preshared_key"].as_str()).map(|s| s.to_string());
                let hpk = active_json.and_then(|j| j["header_protection_key"].as_str()).map(|s| s.to_string());
                let hop_ports = active_json.and_then(|j| j["hop_ports"].as_str()).unwrap_or("20000:50000").to_string();
                let jc = ep["jc"].as_u64().map(|v| v as u16).or(Some(4));
                let jmin = ep["jmin"].as_u64().map(|v| v as u16).or(Some(40));
                let jmax = ep["jmax"].as_u64().map(|v| v as u16).or(Some(70));
                let s1 = ep["s1"].as_u64().map(|v| v as u16).or(Some(15));
                let s2 = ep["s2"].as_u64().map(|v| v as u16).or(Some(25));
                let s3 = ep["s3"].as_u64().map(|v| v as u16);
                let s4 = ep["s4"].as_u64().map(|v| v as u16);
                let h1 = ep["h1"].as_str().map(|s| s.to_string()).or_else(|| ep["h1"].as_u64().map(|u| u.to_string()));
                let h2 = ep["h2"].as_str().map(|s| s.to_string()).or_else(|| ep["h2"].as_u64().map(|u| u.to_string()));
                let h3 = ep["h3"].as_str().map(|s| s.to_string()).or_else(|| ep["h3"].as_u64().map(|u| u.to_string()));
                let h4 = ep["h4"].as_str().map(|s| s.to_string()).or_else(|| ep["h4"].as_u64().map(|u| u.to_string()));

                if let Some(devices) = active_json.and_then(|j| j["devices"].as_array()) {
                    for d in devices {
                        let peer_name = d["name"].as_str().unwrap_or(proto_name);
                        let is_base = peer_name == "default" || peer_name == "pc" || peer_name == "neuroveil-user";
                        let prof_name = if is_base {
                            format!("{} ({})", proto_name, connect_host)
                        } else {
                            peer_name.to_string()
                        };
                        let c_priv = d["client_private_key"].as_str().unwrap_or(&client_priv).to_string();
                        if c_priv.trim().is_empty() {
                            continue;
                        }
                        let peer_addr = d["ip"].as_str().unwrap_or(if is_awg2 { "10.8.1.2/32" } else { "10.8.0.2/32" }).to_string();
                        let peer_psk = d["preshared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                        let mut prof = VpnProfile {
                            id: Uuid::new_v4().to_string(),
                            name: prof_name,
                            protocol: proto_str.to_string(),
                            server: connect_host.clone(),
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
                            awg_rekey_after_time: active_json.and_then(|j| j["rekey_after_time"].as_str()).map(|s| s.to_string()),
                            awg_rekey_timeout: active_json.and_then(|j| j["rekey_timeout"].as_str()).map(|s| s.to_string()),
                            awg_reject_after_time: active_json.and_then(|j| j["reject_after_time"].as_str()).map(|s| s.to_string()),
                            awg_keepalive_timeout: active_json.and_then(|j| j["keepalive_timeout"].as_str()).map(|s| s.to_string()),
                            awg_max_handshake_attempts: active_json.and_then(|j| j["max_handshake_attempts"].as_str()).map(|s| s.to_string()),
                            awg_random_trailers: active_json.and_then(|j| j["random_trailers"].as_bool()),
                            awg_disable_cookies: active_json.and_then(|j| j["disable_cookies"].as_bool()),
                            awg_persistent_keepalive: active_json.and_then(|j| j["persistent_keepalive"].as_str()).map(|s| s.to_string()),
                            awg_hop_ports: Some(hop_ports.clone()),
                            awg_port_probing: Some(true),
                            pinned_on_pc: if is_base { Some(true) } else { Some(false) },
                            ..Default::default()
                        };
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
                                format!("{} ({})", proto_name, connect_host)
                            } else {
                                peer_name.to_string()
                            };
                            let default_ip = if is_awg2 { "10.8.1.2/32" } else { "10.8.0.2/32" };
                            let peer_addr = p["allowed_ips"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()).unwrap_or(default_ip).to_string();
                            let peer_psk = p["pre_shared_key"].as_str().map(|s| s.to_string()).or_else(|| psk.clone());
                            let mut prof = VpnProfile {
                                id: Uuid::new_v4().to_string(),
                                name: prof_name,
                                protocol: proto_str.to_string(),
                                server: connect_host.clone(),
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
                                awg_rekey_after_time: active_json.and_then(|j| j["rekey_after_time"].as_str()).map(|s| s.to_string()),
                                awg_rekey_timeout: active_json.and_then(|j| j["rekey_timeout"].as_str()).map(|s| s.to_string()),
                                awg_reject_after_time: active_json.and_then(|j| j["reject_after_time"].as_str()).map(|s| s.to_string()),
                                awg_keepalive_timeout: active_json.and_then(|j| j["keepalive_timeout"].as_str()).map(|s| s.to_string()),
                                awg_max_handshake_attempts: active_json.and_then(|j| j["max_handshake_attempts"].as_str()).map(|s| s.to_string()),
                                awg_random_trailers: active_json.and_then(|j| j["random_trailers"].as_bool()),
                                awg_disable_cookies: active_json.and_then(|j| j["disable_cookies"].as_bool()),
                                awg_persistent_keepalive: active_json.and_then(|j| j["persistent_keepalive"].as_str()).map(|s| s.to_string()),
                                awg_hop_ports: Some(hop_ports.clone()),
                                awg_port_probing: Some(true),
                                pinned_on_pc: if is_base { Some(true) } else { Some(false) },
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

    // Parse iptables byte counters per port and per WireGuard peer IP
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

    // Ensure OUTPUT / FORWARD accounting rules exist on server for future syncs
    let mut acct_setup = String::new();
    for p in & mut profiles {
        let port = p.port;
        let is_tcp = p.protocol == "vless" || p.protocol == "shadowsocks" || p.protocol == "ssh";
        let is_udp = p.protocol != "vless" && p.protocol != "ssh";
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
            "Успешно авторизовано на сервере. Получено устройств/ключей: {}",
            profiles.len()
        ),
    );

    Ok(profiles)
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

    let (code, raw_cfg) = run_remote_cmd(&sess, "cat /etc/sing-box/config.json 2>/dev/null").await?;
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

    if proto == "amneziawg" || proto == "awg2" {
        let is_awg2 = proto == "awg2" || profile.awg_version.as_deref() == Some("2.0");
        let awg_file = if is_awg2 {
            "/etc/sing-box/awg2.json"
        } else {
            "/etc/sing-box/awg.json"
        };
        let (_, awg_raw) = run_remote_cmd(&sess, &format!("cat {} 2>/dev/null", awg_file))
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
                // Single base device without devices array
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
                awg_update_script = format!("rm -f {}\n", awg_file);
            } else {
                let new_awg_str = serde_json::to_string_pretty(&awg_json).unwrap_or_default();
                awg_update_script = format!(
                    "cat << 'EOF_AWG' > {awg_file}\n{new_awg_str}\nEOF_AWG\nchmod 600 {awg_file}\n",
                    awg_file = awg_file,
                    new_awg_str = new_awg_str
                );
            }
        }

        if let Some(endpoints) = cfg["endpoints"].as_array_mut() {
            let mut ep_tags_to_remove = Vec::new();
            endpoints.retain_mut(|ep| {
                if ep["type"].as_str() != Some("wireguard") {
                    return true;
                }
                let ep_port = ep["listen_port"].as_u64().unwrap_or(0) as u16;
                let tag = ep["tag"].as_str().unwrap_or("").to_string();
                let matches_ep = ep_port == profile.port
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
                        if is_awg2 {
                            if r["source_ip_cidr"]
                                .as_array()
                                .map(|a| a.iter().any(|v| v.as_str() == Some("10.8.1.0/24")))
                                .unwrap_or(false)
                            {
                                return false;
                            }
                        } else if r["source_ip_cidr"]
                            .as_array()
                            .map(|a| a.iter().any(|v| v.as_str() == Some("10.8.0.0/24")))
                            .unwrap_or(false)
                        {
                            return false;
                        }
                        true
                    });
                }
            }
        }
    } else if let Some(inbounds) = cfg["inbounds"].as_array_mut() {
        let prof_pw = profile.password.trim();
        let prof_uuid = profile.uuid.as_deref().unwrap_or(prof_pw).trim();
        let prof_name = profile.name.trim();

        inbounds.retain_mut(|ib| {
            let ib_type = ib["type"].as_str().unwrap_or("");
            let ib_port = ib["listen_port"].as_u64().unwrap_or(0) as u16;
            if ib_type != proto || (profile.port > 0 && ib_port != profile.port) {
                return true;
            }
            if let Some(users) = ib["users"].as_array_mut() {
                let before = users.len();
                users.retain(|u| {
                    let u_name = u["name"].as_str().unwrap_or("").trim();
                    let u_uuid = u["uuid"].as_str().unwrap_or("").trim();
                    let u_pw = u["password"].as_str().unwrap_or("").trim();
                    let matched = (!prof_uuid.is_empty() && u_uuid == prof_uuid)
                        || (!prof_pw.is_empty() && u_pw == prof_pw)
                        || (!prof_pw.is_empty() && u_uuid == prof_pw)
                        || u_name == prof_name;
                    !matched
                });
                if users.len() == before && users.len() == 1 {
                    users.clear();
                }
                !users.is_empty()
            } else {
                // Single-user inbound (e.g. shadowsocks)
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
    let service_action = if has_inbounds || has_endpoints {
        "systemctl restart sing-box"
    } else {
        "systemctl stop sing-box || true"
    };

    let update_cmd = format!(
        r#"
cat << 'EOF_CFG' > /etc/sing-box/config.json
{new_cfg_str}
EOF_CFG
{awg_update_script}
{service_action}
"#,
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
            "Конфигурация «{}» успешно удалена с сервера {}!",
            profile.name, creds.host
        ),
    );
    Ok(())
}

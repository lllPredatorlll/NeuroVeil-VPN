use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnProfile {
    pub id: String,
    pub name: String,
    pub protocol: String, // "hysteria2" | "tuic"
    pub server: String,
    pub port: u16,
    #[serde(default)]
    pub original_port: Option<u16>,
    pub sni: String,
    pub insecure: bool,
    pub password: String,
    pub uuid: Option<String>,          // Required for TUIC v5
    pub obfs_password: Option<String>, // Optional Salamander obfuscation for Hysteria 2
    #[serde(default)]
    pub hop_ports: Option<String>,     // Optional Port Hopping range e.g. "20000:50000"
    #[serde(default)]
    pub hop_interval: Option<String>,  // Optional Port Hopping interval e.g. "30s"
    #[serde(default)]
    pub reality_public_key: Option<String>, // VLESS Reality public key
    #[serde(default)]
    pub reality_short_id: Option<String>,   // VLESS Reality short ID (hex)
    #[serde(default)]
    pub reality_flow: Option<String>,       // e.g. "xtls-rprx-vision"
    #[serde(default)]
    pub ss_method: Option<String>,          // Shadowsocks cipher method
    #[serde(default)]
    pub ss_plugin: Option<String>,          // Shadowsocks plugin
    #[serde(default)]
    pub ss_plugin_opts: Option<String>,     // Shadowsocks plugin options
    #[serde(default)]
    pub awg_private_key: Option<String>,    // AmneziaWG client private key
    #[serde(default)]
    pub awg_public_key: Option<String>,     // AmneziaWG server public key
    #[serde(default)]
    pub awg_preshared_key: Option<String>,  // AmneziaWG optional preshared key
    #[serde(default)]
    pub awg_local_address: Option<String>,  // AmneziaWG client address e.g. "10.8.0.2/32"
    #[serde(default)]
    pub awg_jc: Option<u16>,                // Junk packet count
    #[serde(default)]
    pub awg_jmin: Option<u16>,              // Min junk packet size
    #[serde(default)]
    pub awg_jmax: Option<u16>,              // Max junk packet size
    #[serde(default)]
    pub awg_s1: Option<u16>,                // Handshake init padding
    #[serde(default)]
    pub awg_s2: Option<u16>,                // Handshake response padding
    #[serde(default)]
    pub awg_s3: Option<u16>,                // AWG 3.1 cookie under-load padding
    #[serde(default)]
    pub awg_s4: Option<u16>,                // AWG 3.1 transport data padding
    #[serde(default)]
    pub awg_h1: Option<String>,             // Magic header 1
    #[serde(default)]
    pub awg_h2: Option<String>,             // Magic header 2
    #[serde(default)]
    pub awg_h3: Option<String>,             // Magic header 3
    #[serde(default)]
    pub awg_h4: Option<String>,             // Magic header 4
    #[serde(default)]
    pub awg_i1: Option<String>,             // AWG 3.1 init prefix 1
    #[serde(default)]
    pub awg_i2: Option<String>,             // AWG 3.1 init prefix 2
    #[serde(default)]
    pub awg_i3: Option<String>,             // AWG 3.1 init prefix 3
    #[serde(default)]
    pub awg_i4: Option<String>,             // AWG 3.1 init prefix 4
    #[serde(default)]
    pub awg_hop_ports: Option<String>,      // Port hunting range/list e.g. "20000:50000"
    #[serde(default)]
    pub awg_port_probing: Option<bool>,     // Enable automatic port probing / failover
    #[serde(default)]
    pub awg_header_protection_key: Option<String>, // Base64 32-byte header protection key for AWG 3.1
    #[serde(default)]
    pub awg_rekey_after_time: Option<String>,      // AWG 3.1 rekey interval e.g. "100-120"
    #[serde(default)]
    pub awg_rekey_timeout: Option<String>,         // AWG 3.1 rekey timeout e.g. "3-7"
    #[serde(default)]
    pub awg_reject_after_time: Option<String>,     // AWG 3.1 reject interval e.g. "150-180"
    #[serde(default)]
    pub awg_keepalive_timeout: Option<String>,     // AWG 3.1 keepalive timeout e.g. "5-15"
    #[serde(default)]
    pub awg_max_handshake_attempts: Option<String>,// AWG 3.1 max handshake attempts e.g. "15-20"
    #[serde(default)]
    pub awg_random_trailers: Option<bool>,         // AWG 3.1 random trailers toggle
    #[serde(default)]
    pub awg_disable_cookies: Option<bool>,         // AWG 3.1 disable cookies toggle
    #[serde(default)]
    pub awg_persistent_keepalive: Option<String>,  // Persistent keepalive e.g. "25-35" or "25"
    #[serde(default)]
    pub awg_version: Option<String>,               // "3.1" | "2.0"
    #[serde(default)]
    pub tls_fragment: Option<bool>,         // Anti-DPI TLS ClientHello fragmentation
    pub up_mbps: u32,
    pub down_mbps: u32,
    pub uri: String,
    pub ping_ms: Option<u32>,
    #[serde(default)]
    pub pinned_on_pc: Option<bool>,
    #[serde(default)]
    pub routing: Option<RoutingConfig>,
    #[serde(default)]
    pub total_up_bytes: u64,
    #[serde(default)]
    pub total_down_bytes: u64,
}

impl Default for VpnProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            protocol: "amneziawg".to_string(),
            server: String::new(),
            port: 51820,
            original_port: None,
            sni: String::new(),
            insecure: false,
            password: String::new(),
            uuid: None,
            obfs_password: None,
            hop_ports: None,
            hop_interval: None,
            reality_public_key: None,
            reality_short_id: None,
            reality_flow: None,
            ss_method: None,
            ss_plugin: None,
            ss_plugin_opts: None,
            awg_private_key: None,
            awg_public_key: None,
            awg_preshared_key: None,
            awg_local_address: None,
            awg_jc: None,
            awg_jmin: None,
            awg_jmax: None,
            awg_s1: None,
            awg_s2: None,
            awg_s3: None,
            awg_s4: None,
            awg_h1: None,
            awg_h2: None,
            awg_h3: None,
            awg_h4: None,
            awg_i1: None,
            awg_i2: None,
            awg_i3: None,
            awg_i4: None,
            awg_hop_ports: None,
            awg_port_probing: None,
            awg_header_protection_key: None,
            awg_rekey_after_time: None,
            awg_rekey_timeout: None,
            awg_reject_after_time: None,
            awg_keepalive_timeout: None,
            awg_max_handshake_attempts: None,
            awg_random_trailers: None,
            awg_disable_cookies: None,
            awg_persistent_keepalive: None,
            awg_version: None,
            tls_fragment: Some(true),
            up_mbps: 100,
            down_mbps: 100,
            uri: String::new(),
            ping_ms: None,
            pinned_on_pc: None,
            routing: None,
            total_up_bytes: 0,
            total_down_bytes: 0,
        }
    }
}

fn default_outbound_val() -> String {
    "proxy".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub mode: String, // "tun" | "system_proxy"
    pub split_tunneling: bool,
    pub bypass_ru: bool,
    pub bypass_lan: bool,
    #[serde(default = "default_outbound_val")]
    pub default_outbound: String, // "proxy" (all except direct) | "direct" (only proxy list)
    pub direct_domains: Vec<String>,
    pub proxy_domains: Vec<String>,
    #[serde(default)]
    pub direct_ips: Vec<String>,
    #[serde(default)]
    pub proxy_ips: Vec<String>,
    pub direct_processes: Vec<String>,
    #[serde(default)]
    pub proxy_processes: Vec<String>,
    pub remote_dns: String,
    pub direct_dns: String,
    pub local_mixed_port: u16,
    pub clash_api_port: u16,
    #[serde(default = "default_true")]
    pub tls_fragment: bool,
    #[serde(default)]
    pub kill_switch: bool,
    #[serde(default = "default_true")]
    pub block_ipv6: bool,
    #[serde(default = "default_true")]
    pub dns_leak_protect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningProcessInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_outbound_val")]
    pub default_outbound: String, // "proxy" | "direct"
    #[serde(default)]
    pub proxy_processes: Vec<String>,
    #[serde(default)]
    pub direct_processes: Vec<String>,
    #[serde(default)]
    pub is_builtin: bool,
}

pub fn default_routing_presets() -> Vec<RoutingPreset> {
    vec![
        RoutingPreset {
            id: "builtin-games-direct".to_string(),
            name: "Игры мимо VPN (Весь ПК через VPN)".to_string(),
            description: Some("Весь интернет и программы идут через VPN, а игры (Steam, CS2, Dota 2, Riot, Epic) и торренты работают напрямую без пинга.".to_string()),
            default_outbound: "proxy".to_string(),
            proxy_processes: vec![],
            direct_processes: vec![
                "steam.exe".to_string(),
                "steamwebhelper.exe".to_string(),
                "cs2.exe".to_string(),
                "dota2.exe".to_string(),
                "valorant.exe".to_string(),
                "riotclientservices.exe".to_string(),
                "leagueclient.exe".to_string(),
                "epicgameslauncher.exe".to_string(),
                "gta5.exe".to_string(),
                "battle.net.exe".to_string(),
                "genshinimpact.exe".to_string(),
                "qbittorrent.exe".to_string(),
                "utorrent.exe".to_string(),
            ],
            is_builtin: true,
        },
        RoutingPreset {
            id: "builtin-discord-tg".to_string(),
            name: "Discord + Telegram (Только они)".to_string(),
            description: Some("Только мессенджеры Discord и Telegram шифруются через VPN. Весь остальной ПК, браузеры и игры работают через провайдера напрямую.".to_string()),
            default_outbound: "direct".to_string(),
            proxy_processes: vec![
                "Discord.exe".to_string(),
                "Telegram.exe".to_string(),
                "DiscordCanary.exe".to_string(),
                "DiscordPTB.exe".to_string(),
            ],
            direct_processes: vec![],
            is_builtin: true,
        },
        RoutingPreset {
            id: "builtin-browsers".to_string(),
            name: "Только браузеры".to_string(),
            description: Some("Веб-браузеры (Chrome, Edge, Firefox, Brave, Opera, Яндекс) идут через VPN. Любые другие приложения и игры работают напрямую.".to_string()),
            default_outbound: "direct".to_string(),
            proxy_processes: vec![
                "chrome.exe".to_string(),
                "msedge.exe".to_string(),
                "firefox.exe".to_string(),
                "brave.exe".to_string(),
                "browser.exe".to_string(),
                "opera.exe".to_string(),
            ],
            direct_processes: vec![],
            is_builtin: true,
        },
        RoutingPreset {
            id: "builtin-media-social".to_string(),
            name: "Discord + YouTube + Spotify".to_string(),
            description: Some("Разблокировка Discord, YouTube (браузеры) и Spotify, весь остальной ПК работает напрямую.".to_string()),
            default_outbound: "direct".to_string(),
            proxy_processes: vec![
                "Discord.exe".to_string(),
                "Telegram.exe".to_string(),
                "Spotify.exe".to_string(),
                "chrome.exe".to_string(),
                "msedge.exe".to_string(),
                "firefox.exe".to_string(),
            ],
            direct_processes: vec![],
            is_builtin: true,
        },
        RoutingPreset {
            id: "builtin-all-vpn".to_string(),
            name: "Полный VPN (Весь трафик ПК)".to_string(),
            description: Some("Абсолютно весь трафик ПК направляется в защищенный туннель без каких-либо исключений.".to_string()),
            default_outbound: "proxy".to_string(),
            proxy_processes: vec![],
            direct_processes: vec![],
            is_builtin: true,
        },
    ]
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            mode: "system_proxy".to_string(),
            split_tunneling: true,
            bypass_ru: true,
            bypass_lan: true,
            default_outbound: "proxy".to_string(),
            direct_domains: vec![
                ".ru".to_string(),
                ".su".to_string(),
                ".xn--p1ai".to_string(), // .рф
                "yandex.ru".to_string(),
                "yandex.net".to_string(),
                "vk.com".to_string(),
                "gosuslugi.ru".to_string(),
                "sberbank.ru".to_string(),
                "tbank.ru".to_string(),
                "ozon.ru".to_string(),
                "wildberries.ru".to_string(),
                "steamcontent.com".to_string(),
                "steamserver.net".to_string(),
            ],
            proxy_domains: vec![
                "youtube.com".to_string(),
                "googlevideo.com".to_string(),
                "discord.com".to_string(),
                "discordapp.com".to_string(),
                "discord.gg".to_string(),
                "openai.com".to_string(),
                "chatgpt.com".to_string(),
                "instagram.com".to_string(),
                "x.com".to_string(),
                "twitter.com".to_string(),
            ],
            direct_processes: vec![],
            proxy_processes: vec![],
            direct_ips: vec![],
            proxy_ips: vec![],
            remote_dns: "https://1.1.1.1/dns-query".to_string(),
            direct_dns: "77.88.8.8".to_string(),
            local_mixed_port: 2080,
            clash_api_port: 9090,
            tls_fragment: true,
            kill_switch: false,
            block_ipv6: true,
            dns_leak_protect: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_lang() -> String {
    "ru".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPersistentState {
    pub profiles: Vec<VpnProfile>,
    pub selected_profile_id: Option<String>,
    pub routing: RoutingConfig,
    #[serde(default = "default_routing_presets")]
    pub routing_presets: Vec<RoutingPreset>,
    #[serde(default)]
    pub active_preset_id: Option<String>,
    pub saved_vps: Option<VpsCredentials>,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default)]
    pub window_width: Option<f64>,
    #[serde(default)]
    pub window_height: Option<f64>,
    #[serde(default)]
    pub window_maximized: Option<bool>,
}

impl Default for AppPersistentState {
    fn default() -> Self {
        Self {
            profiles: vec![],
            selected_profile_id: None,
            routing: RoutingConfig::default(),
            routing_presets: default_routing_presets(),
            active_preset_id: None,
            saved_vps: None,
            autostart: false,
            start_minimized: false,
            close_to_tray: true,
            auto_connect: false,
            language: "ru".to_string(),
            window_width: None,
            window_height: None,
            window_maximized: None,
        }
    }
}

fn default_vless_port() -> u16 {
    443
}

fn default_reality_sni() -> String {
    "www.microsoft.com".to_string()
}

fn default_ss_port() -> u16 {
    8388
}

fn default_ss_method() -> String {
    "2022-blake3-aes-128-gcm".to_string()
}

fn default_awg_port() -> u16 {
    51820
}

fn default_awg_hop_ports() -> String {
    "20000:50000".to_string()
}

fn default_awg2_port() -> u16 {
    51821
}

pub fn is_awg_protocol(proto: &str) -> bool {
    let p = proto.trim().to_lowercase();
    p == "amneziawg" || p == "awg" || p == "awg2" || p == "amneziawg2" || p == "amnezia31" || p == "awg3"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpsCredentials {
    pub host: String,
    pub ssh_port: u16,
    pub username: String,
    pub password: String,
    #[serde(default = "default_vless_port")]
    pub vless_port: u16,
    pub hy2_port: u16,
    pub tuic_port: u16,
    #[serde(default = "default_ss_port")]
    pub ss_port: u16,
    #[serde(default = "default_ss_method")]
    pub ss_method: String,
    #[serde(default = "default_awg_port")]
    pub awg_port: u16,
    #[serde(default = "default_awg_hop_ports")]
    pub awg_hop_ports: String,
    #[serde(default)]
    pub awg_jc: Option<u16>,
    #[serde(default)]
    pub awg_jmin: Option<u16>,
    #[serde(default)]
    pub awg_jmax: Option<u16>,
    #[serde(default)]
    pub awg_s1: Option<u16>,
    #[serde(default)]
    pub awg_s2: Option<u16>,
    #[serde(default)]
    pub awg_s3: Option<u16>,
    #[serde(default)]
    pub awg_s4: Option<u16>,
    #[serde(default)]
    pub awg_h1: Option<String>,
    #[serde(default)]
    pub awg_h2: Option<String>,
    #[serde(default)]
    pub awg_h3: Option<String>,
    #[serde(default)]
    pub awg_h4: Option<String>,
    #[serde(default)]
    pub awg_i1: Option<String>,
    #[serde(default)]
    pub awg_i2: Option<String>,
    #[serde(default)]
    pub awg_i3: Option<String>,
    #[serde(default)]
    pub awg_i4: Option<String>,
    #[serde(default)]
    pub awg_header_protection_key: Option<String>,
    #[serde(default)]
    pub awg_rekey_after_time: Option<String>,
    #[serde(default)]
    pub awg_rekey_timeout: Option<String>,
    #[serde(default)]
    pub awg_reject_after_time: Option<String>,
    #[serde(default)]
    pub awg_keepalive_timeout: Option<String>,
    #[serde(default)]
    pub awg_max_handshake_attempts: Option<String>,
    #[serde(default)]
    pub awg_random_trailers: Option<bool>,
    #[serde(default)]
    pub awg_disable_cookies: Option<bool>,
    #[serde(default)]
    pub awg_persistent_keepalive: Option<String>,
    #[serde(default)]
    pub awg_port_probing: Option<bool>,
    #[serde(default)]
    pub deploy_awg2: bool,
    #[serde(default = "default_awg2_port")]
    pub awg2_port: u16,
    #[serde(default = "default_awg_hop_ports")]
    pub awg2_hop_ports: String,
    #[serde(default)]
    pub awg2_jc: Option<u16>,
    #[serde(default)]
    pub awg2_jmin: Option<u16>,
    #[serde(default)]
    pub awg2_jmax: Option<u16>,
    #[serde(default)]
    pub awg2_s1: Option<u16>,
    #[serde(default)]
    pub awg2_s2: Option<u16>,
    #[serde(default)]
    pub awg2_h1: Option<String>,
    #[serde(default)]
    pub awg2_h2: Option<String>,
    #[serde(default)]
    pub awg2_h3: Option<String>,
    #[serde(default)]
    pub awg2_h4: Option<String>,
    pub sni: String,
    #[serde(default = "default_reality_sni")]
    pub reality_sni: String,
    #[serde(default = "default_true")]
    pub deploy_vless: bool,
    #[serde(default = "default_true")]
    pub deploy_hy2: bool,
    #[serde(default)]
    pub deploy_tuic: bool,
    #[serde(default)]
    pub deploy_ss: bool,
    #[serde(default)]
    pub deploy_awg: bool,
    #[serde(default = "default_true")]
    pub deploy_ssh: bool,
    pub enable_salamander: bool,
    #[serde(default)]
    pub enable_port_hopping: bool,
    #[serde(default)]
    pub ssh_proxy: String, // Optional local SOCKS5/HTTP proxy e.g. "127.0.0.1:10808"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnStatus {
    pub connected: bool,
    pub active_profile_id: Option<String>,
    pub active_profile_name: Option<String>,
    pub active_protocol: Option<String>,
    pub mode: String,
    pub split_tunneling: bool,
    pub up_bytes: u64,
    pub down_bytes: u64,
    pub up_speed: u64,
    pub down_speed: u64,
    pub singbox_installed: bool,
    pub singbox_version: String,
    pub singbox_path: String,
    pub logs: Vec<String>,
    #[serde(default)]
    pub active_port: Option<u16>,
    #[serde(default)]
    pub port_hunting_status: Option<String>,
    #[serde(default)]
    pub is_handshaking: bool,
    #[serde(default)]
    pub handshake_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProbeResult {
    pub port: u16,
    pub reachable: bool,
    pub latency_ms: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub success: bool,
    pub message: String,
    pub created_profiles: Vec<VpnProfile>,
    pub new_ssh_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub published_at: Option<String>,
    pub html_url: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
    pub repo_url: String,
}

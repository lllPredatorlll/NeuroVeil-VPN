export interface VpnProfile {
  id: string;
  name: string;
  protocol: "hysteria2" | "tuic" | "vless" | "shadowsocks" | "amneziawg" | "awg2" | "ssh";
  server: string;
  port: number;
  original_port?: number | null;
  sni: string;
  insecure: boolean;
  password: string;
  uuid?: string | null;
  obfs_password?: string | null;
  hop_ports?: string | null;
  hop_interval?: string | null;
  reality_public_key?: string | null;
  reality_short_id?: string | null;
  reality_flow?: string | null;
  ss_method?: string | null;
  ss_plugin?: string | null;
  ss_plugin_opts?: string | null;
  awg_private_key?: string | null;
  awg_public_key?: string | null;
  awg_preshared_key?: string | null;
  awg_local_address?: string | null;
  awg_jc?: number | null;
  awg_jmin?: number | null;
  awg_jmax?: number | null;
  awg_s1?: number | null;
  awg_s2?: number | null;
  awg_s3?: number | null;
  awg_s4?: number | null;
  awg_h1?: string | null;
  awg_h2?: string | null;
  awg_h3?: string | null;
  awg_h4?: string | null;
  awg_i1?: string | null;
  awg_i2?: string | null;
  awg_i3?: string | null;
  awg_i4?: string | null;
  awg_hop_ports?: string | null;
  awg_port_probing?: boolean | null;
  awg_header_protection_key?: string | null;
  awg_rekey_after_time?: string | null;
  awg_rekey_timeout?: string | null;
  awg_reject_after_time?: string | null;
  awg_keepalive_timeout?: string | null;
  awg_max_handshake_attempts?: string | null;
  awg_random_trailers?: boolean | null;
  awg_disable_cookies?: boolean | null;
  awg_persistent_keepalive?: string | null;
  awg_version?: string | null;
  tls_fragment?: boolean | null;
  up_mbps: number;
  down_mbps: number;
  uri: string;
  ping_ms?: number | null;
  pinned_on_pc?: boolean | null;
  routing?: RoutingConfig | null;
  total_up_bytes?: number;
  total_down_bytes?: number;
}

export interface RoutingConfig {
  mode: "tun" | "system_proxy";
  split_tunneling: boolean;
  bypass_ru: boolean;
  bypass_lan: boolean;
  default_outbound?: "proxy" | "direct";
  direct_domains: string[];
  proxy_domains: string[];
  direct_ips?: string[];
  proxy_ips?: string[];
  direct_processes: string[];
  proxy_processes?: string[];
  remote_dns: string;
  direct_dns: string;
  local_mixed_port: number;
  clash_api_port: number;
  tls_fragment?: boolean;
  kill_switch?: boolean;
  block_ipv6?: boolean;
  dns_leak_protect?: boolean;
}

export interface RunningProcessInfo {
  name: string;
  path: string;
}

export interface RoutingPreset {
  id: string;
  name: string;
  description?: string | null;
  default_outbound: "proxy" | "direct";
  proxy_processes: string[];
  direct_processes: string[];
  is_builtin?: boolean;
}

export interface VpsCredentials {
  host: string;
  ssh_port: number;
  username: string;
  password: string;
  vless_port: number;
  hy2_port: number;
  tuic_port: number;
  ss_port?: number;
  ss_method?: string;
  awg_port?: number;
  awg_hop_ports?: string;
  awg_jc?: number;
  awg_jmin?: number;
  awg_jmax?: number;
  awg_s1?: number;
  awg_s2?: number;
  awg_s3?: number;
  awg_s4?: number;
  awg_h1?: string;
  awg_h2?: string;
  awg_h3?: string;
  awg_h4?: string;
  awg_i1?: string;
  awg_i2?: string;
  awg_i3?: string;
  awg_i4?: string;
  awg_header_protection_key?: string;
  awg_rekey_after_time?: string;
  awg_rekey_timeout?: string;
  awg_reject_after_time?: string;
  awg_keepalive_timeout?: string;
  awg_max_handshake_attempts?: string;
  awg_random_trailers?: boolean;
  awg_disable_cookies?: boolean;
  awg_persistent_keepalive?: string;
  awg_port_probing?: boolean;
  deploy_awg2?: boolean;
  awg2_port?: number;
  awg2_hop_ports?: string;
  awg2_jc?: number;
  awg2_jmin?: number;
  awg2_jmax?: number;
  awg2_s1?: number;
  awg2_s2?: number;
  awg2_h1?: string;
  awg2_h2?: string;
  awg2_h3?: string;
  awg2_h4?: string;
  sni: string;
  reality_sni: string;
  deploy_vless: boolean;
  deploy_hy2: boolean;
  deploy_tuic: boolean;
  deploy_ss?: boolean;
  deploy_awg?: boolean;
  deploy_ssh?: boolean;
  enable_salamander: boolean;
  enable_port_hopping?: boolean;
  ssh_proxy: string;
}

export interface AppPersistentState {
  profiles: VpnProfile[];
  selected_profile_id?: string | null;
  routing: RoutingConfig;
  routing_presets?: RoutingPreset[];
  active_preset_id?: string | null;
  saved_vps?: VpsCredentials | null;
  autostart?: boolean;
  start_minimized?: boolean;
  close_to_tray?: boolean;
  auto_connect?: boolean;
  language?: "ru" | "en";
  window_width?: number | null;
  window_height?: number | null;
  window_maximized?: boolean | null;
}

export interface VpnStatus {
  connected: boolean;
  active_profile_id?: string | null;
  active_profile_name?: string | null;
  active_protocol?: string | null;
  mode: "tun" | "system_proxy";
  split_tunneling: boolean;
  up_bytes: number;
  down_bytes: number;
  up_speed: number;
  down_speed: number;
  singbox_installed: boolean;
  singbox_version: string;
  singbox_path: string;
  logs: string[];
  active_port?: number | null;
  port_hunting_status?: string | null;
  is_handshaking?: boolean;
  handshake_status?: string | null;
}

export interface PortProbeResult {
  port: number;
  reachable: boolean;
  latency_ms?: number | null;
  message: string;
}

export interface DeployResult {
  success: boolean;
  message: string;
  created_profiles: VpnProfile[];
  new_ssh_port?: number;
}

export interface AppUpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_name: string;
  release_notes: string;
  published_at?: string | null;
  html_url: string;
  download_url?: string | null;
  asset_name?: string | null;
  asset_size?: number | null;
  repo_url: string;
}

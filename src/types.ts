export interface VpnProfile {
  id: string;
  name: string;
  protocol: "hysteria2" | "tuic" | "vless" | "shadowsocks" | "amneziawg" | "awg2" | "awg_quic" | "awg_vless" | "enigma" | "wireguard" | "ssh" | "shadowtls" | "vless_ws" | "webrtc";
  server: string;
  port: number;
  vless_port?: number | null;
  listen_port?: number | null;
  auto_rotate_listen_port?: boolean | null;
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
  shadowtls_password?: string | null;
  shadowtls_sni?: string | null;
  ws_path?: string | null;
  cdn_host?: string | null;
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
  auto_port_hunting?: boolean | null;
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
  container_id?: string | null;
  multi_group_id?: string | null;
  multi_group_name?: string | null;
  enigma_keybook?: string | null;
  enigma_key_path?: string | null;
  enigma_mimic?: string | null;
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
  deploy_shadowtls?: boolean;
  shadowtls_port?: number;
  shadowtls_sni?: string;
  deploy_vless_ws?: boolean;
  vless_ws_port?: number;
  cdn_domain?: string;
  deploy_webrtc?: boolean;
  webrtc_port?: number;
  deploy_awg_quic?: boolean;
  deploy_awg_vless?: boolean;
  deploy_enigma?: boolean;
  enigma_port?: number;
  enigma_hop_ports?: string;
  enable_salamander: boolean;
  enable_port_hopping?: boolean;
  ssh_proxy: string;
}

export interface BridgePairPreset {
  id: string;
  name: string;
  entry_profile_id: string;
  exit_profile_id: string;
}

export interface BridgeClientConfig {
  id: string;
  name: string;
  protocol: string;
  entry_host: string;
  entry_ssh_port: number;
  entry_username: string;
  listen_port: number;
  exit_profile_id: string;
  exit_profile_name: string;
  exit_server: string;
  exit_protocol: string;
  config_format: string;
  config_text: string;
  uri: string;
  service_name: string;
  runtime: string;
  created_at: string;
}

export interface CreateBridgeClientRequest {
  name: string;
  protocol: string;
  entry_host: string;
  entry_ssh_port: number;
  entry_username: string;
  entry_password: string;
  exit_profile_id: string;
  custom_port: number;
  reality_sni: string;
}

export interface DeleteBridgeClientRequest {
  config_id: string;
  remove_from_server: boolean;
  entry_password: string;
}

export interface BridgeConfig {
  enabled: boolean;
  entry_profile_id?: string | null;
  exit_profile_id?: string | null;
  saved_pairs: BridgePairPreset[];
  client_configs?: BridgeClientConfig[];
  last_entry_host?: string | null;
  last_entry_ssh_port?: number | null;
  last_entry_username?: string | null;
  last_entry_protocol?: string | null;
  last_exit_host?: string | null;
  last_exit_ssh_port?: number | null;
  last_exit_username?: string | null;
  last_exit_protocol?: string | null;
}

export interface BridgePairDeployRequest {
  pair_name: string;
  entry_host: string;
  entry_ssh_port: number;
  entry_username: string;
  entry_password: string;
  entry_protocol: string;
  exit_host: string;
  exit_ssh_port: number;
  exit_username: string;
  exit_password: string;
  exit_protocol: string;
  save_pair_preset: boolean;
  auto_connect_after_deploy: boolean;
}

export interface BridgePairDeployResult {
  success: boolean;
  message: string;
  entry_profile: VpnProfile;
  exit_profile: VpnProfile;
  saved_pair?: BridgePairPreset | null;
  entry_new_ssh_port?: number | null;
  exit_new_ssh_port?: number | null;
  auto_connected: boolean;
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
  passive_telemetry_enabled?: boolean;
  bridge?: BridgeConfig;
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
  active_listen_port?: number | null;
  port_hunting_status?: string | null;
  is_handshaking?: boolean;
  handshake_status?: string | null;
  is_connecting?: boolean;
  enigma_rotor_info?: string | null;
  bridge_active?: boolean;
  bridge_entry_profile_id?: string | null;
  bridge_entry_name?: string | null;
  bridge_exit_name?: string | null;
  bridge_entry_protocol?: string | null;
  bridge_exit_protocol?: string | null;
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
  container_id?: string | null;
}

export interface VpsContainerInfo {
  id: string;
  name: string;
  runtime: "docker" | "systemd" | string;
  status: "running" | "stopped" | "unknown" | string;
  created_at: string;
  protocols: string[];
  ports: number[];
  client_count: number;
  is_legacy?: boolean;
}

export interface VpsMultiProtocolGroup {
  id: string;
  name: string;
  container_id: string;
  created_at: string;
  protocols: string[];
  profile_ids: string[];
}

export interface VpsAdminState {
  containers: VpsContainerInfo[];
  profiles: VpnProfile[];
  multi_groups?: VpsMultiProtocolGroup[];
  anti_sleep_enabled?: boolean;
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

export interface SpeedTestServiceCheck {
  name: string;
  host: string;
  port: number;
  reachable: boolean;
  latency_ms?: number | null;
  error?: string | null;
}

export interface SpeedTestProgressEvent {
  phase: "idle" | "ping" | "download" | "upload" | "diagnostics" | "done" | "cancelled" | "error" | string;
  progress_pct: number;
  ping_samples_ms: number[];
  ping_min_ms?: number | null;
  ping_avg_ms?: number | null;
  ping_max_ms?: number | null;
  jitter_ms?: number | null;
  packet_loss_pct?: number | null;
  vps_rtt_ms?: number | null;
  download_samples_mbps: number[];
  download_current_mbps?: number | null;
  download_min_mbps?: number | null;
  download_avg_mbps?: number | null;
  download_max_mbps?: number | null;
  download_bytes: number;
  upload_samples_mbps: number[];
  upload_current_mbps?: number | null;
  upload_min_mbps?: number | null;
  upload_avg_mbps?: number | null;
  upload_max_mbps?: number | null;
  upload_bytes: number;
  external_ip?: string | null;
  country?: string | null;
  city?: string | null;
  isp?: string | null;
  colo?: string | null;
  ipv6_reachable?: boolean | null;
  route_used: string;
  services: SpeedTestServiceCheck[];
  elapsed_ms: number;
  error?: string | null;
}

export interface TelemetrySessionRecord {
  id: string;
  profile_id: string;
  profile_name: string;
  protocol: string;
  server: string;
  port: number;
  mimic_mode?: string | null;
  network_mode: string;
  started_at: string;
  ended_at?: string | null;
  duration_secs: number;
  handshake_ms?: number | null;
  handshake_ok: boolean;
  disconnect_reason: string;
  up_bytes: number;
  down_bytes: number;
  peak_up_bps: number;
  peak_down_bps: number;
  avg_active_up_bps?: number | null;
  avg_active_down_bps?: number | null;
  ping_samples_ms: number[];
  ping_min_ms?: number | null;
  ping_avg_ms?: number | null;
  ping_p95_ms?: number | null;
  ping_max_ms?: number | null;
  jitter_ms?: number | null;
  probes_ok: number;
  probes_fail: number;
  stalls_count: number;
  enigma_handshakes?: number | null;
  enigma_decoys?: number | null;
  enigma_packets_tx?: number | null;
  enigma_packets_rx?: number | null;
}

export interface SpeedTestHistoryRecord {
  id: string;
  timestamp: string;
  profile_id?: string | null;
  profile_name: string;
  protocol: string;
  route_used: string;
  quick_only: boolean;
  ping_min_ms?: number | null;
  ping_avg_ms?: number | null;
  ping_max_ms?: number | null;
  jitter_ms?: number | null;
  packet_loss_pct?: number | null;
  vps_rtt_ms?: number | null;
  download_avg_mbps?: number | null;
  download_max_mbps?: number | null;
  upload_avg_mbps?: number | null;
  upload_max_mbps?: number | null;
  colo?: string | null;
  external_ip?: string | null;
}

export interface TelemetryProtocolSummary {
  protocol: string;
  sessions_count: number;
  total_duration_secs: number;
  handshake_success_pct?: number | null;
  avg_handshake_ms?: number | null;
  total_up_bytes: number;
  total_down_bytes: number;
  peak_down_mbps?: number | null;
  avg_active_down_mbps?: number | null;
  peak_up_mbps?: number | null;
  avg_active_up_mbps?: number | null;
  avg_ping_ms?: number | null;
  p95_ping_ms?: number | null;
  avg_jitter_ms?: number | null;
  packet_loss_pct?: number | null;
  stalls_total: number;
  speedtest_count: number;
  speedtest_avg_down_mbps?: number | null;
  speedtest_avg_up_mbps?: number | null;
}

export interface TelemetryReportResponse {
  enabled: boolean;
  active_session?: TelemetrySessionRecord | null;
  summaries: TelemetryProtocolSummary[];
  sessions: TelemetrySessionRecord[];
  speedtests: SpeedTestHistoryRecord[];
}


import React, { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Server,
  Globe,
  Settings,
  Power,
  Download,
  Upload,
  Terminal,
  Trash2,
  Plus,
  Copy,
  Check,
  Activity,
  RefreshCw,
  AlertTriangle,
  Eye,
  EyeOff,
  Zap,
  Lock,
  Wifi,
  CheckCircle2,
  XCircle,
  Sparkles,
  FolderOpen,
  FileCode,
  Cpu,
  Search,
  QrCode,
  Smartphone,
  FileDown,
  UserPlus,
  Key,
  Minus,
  Square,
  X,
  Palette,
  Languages,
  Info,
  Monitor,
  Shield,
  ShieldCheck,
  Edit3,
  Sliders,
  ShieldAlert,
  ExternalLink,
  MessageSquare,
  Layers,
  Radio,
  Snowflake,
  Cloud,
  Save,
  Gamepad2,
  Bookmark,
  RotateCcw,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import QRCode from "qrcode";
import appIconLogo from "./assets/app-icon.png";
import { THEMES } from "./themes";
import { LOCALES, Language } from "./locales";
import {
  AppPersistentState,
  AppUpdateInfo,
  DeployResult,
  PortProbeResult,
  RoutingConfig,
  RoutingPreset,
  RunningProcessInfo,
  VpnProfile,
  VpnStatus,
  VpsCredentials,
} from "./types";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

function NeuralWiresIcon({ className = "w-6 h-6" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className={className}>
      <defs>
        <linearGradient id="neuralGradCyan" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
          <stop stopColor="#06b6d4" />
          <stop offset="0.5" stopColor="#3b82f6" />
          <stop offset="1" stopColor="#8b5cf6" />
        </linearGradient>
        <radialGradient id="neuralCoreGlow" cx="50%" cy="50%" r="50%">
          <stop stopColor="#22d3ee" stopOpacity="0.8" />
          <stop offset="100%" stopColor="#06b6d4" stopOpacity="0" />
        </radialGradient>
      </defs>
      {/* Central Synapse Core */}
      <circle cx="12" cy="12" r="3.2" fill="#06b6d4" />
      <circle cx="12" cy="12" r="5.5" fill="url(#neuralCoreGlow)" />
      <circle cx="12" cy="12" r="1.3" fill="#ffffff" />
      {/* Neural Wires / Synaptic Pathways */}
      <path d="M12 8.8V3M12 3L9 5M12 3L15 5" stroke="url(#neuralGradCyan)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M12 15.2V21M12 21L9 19M12 21L15 19" stroke="url(#neuralGradCyan)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M8.8 12H3M3 12L5 9M3 12L5 15" stroke="url(#neuralGradCyan)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M15.2 12H21M21 12L19 9M21 12L19 15" stroke="url(#neuralGradCyan)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      {/* Diagonal Axons */}
      <path d="M9.6 9.6L5.5 5.5" stroke="#38bdf8" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M14.4 9.6L18.5 5.5" stroke="#818cf8" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M9.6 14.4L5.5 18.5" stroke="#818cf8" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M14.4 14.4L18.5 18.5" stroke="#c084fc" strokeWidth="1.4" strokeLinecap="round" />
      {/* Synaptic Terminals */}
      <circle cx="5.5" cy="5.5" r="1.5" fill="#38bdf8" />
      <circle cx="18.5" cy="5.5" r="1.5" fill="#818cf8" />
      <circle cx="5.5" cy="18.5" r="1.5" fill="#818cf8" />
      <circle cx="18.5" cy="18.5" r="1.5" fill="#c084fc" />
    </svg>
  );
}

export default function App() {
  const [activeTab, setActiveTab] = useState<
    "dashboard" | "vps" | "profiles" | "routing" | "settings" | "about"
  >("dashboard");

  // Sidebar Collapse state (persisted in localStorage)
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem("neuroveil_sidebar_collapsed") === "true";
    } catch {
      return false;
    }
  });

  const toggleSidebar = () => {
    setSidebarCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("neuroveil_sidebar_collapsed", String(next));
      } catch {}
      return next;
    });
  };

  // Window Opacity State (70% - 100%)
  const [windowOpacity, setWindowOpacity] = useState<number>(() => {
    try {
      const saved =
        localStorage.getItem("neuroveil_window_opacity") ||
        localStorage.getItem("aegis_window_opacity");
      return saved ? Math.max(70, Math.min(100, parseInt(saved, 10))) : 100;
    } catch {
      return 100;
    }
  });

  const handleUpdateOpacity = (val: number) => {
    const clamped = Math.max(70, Math.min(100, val));
    setWindowOpacity(clamped);
    try {
      localStorage.setItem("neuroveil_window_opacity", String(clamped));
    } catch {}
  };

  // Visual Theme State
  const [currentThemeId, setCurrentThemeId] = useState<string>(() => {
    return (
      localStorage.getItem("neuroveil_theme") ||
      localStorage.getItem("aegis_theme") ||
      "cyber_obsidian"
    );
  });
  const currentTheme = THEMES.find((t) => t.id === currentThemeId) || THEMES[0];

  // Language & System Settings State
  const [language, setLanguage] = useState<Language>(() => {
    return (
      (localStorage.getItem("neuroveil_lang") as Language) ||
      (localStorage.getItem("aegis_lang") as Language) ||
      "ru"
    );
  });
  const t = LOCALES[language] || LOCALES.ru;

  const [autostart, setAutostart] = useState(false);
  const [startMinimized, setStartMinimized] = useState(false);
  const [closeToTray, setCloseToTray] = useState(true);
  const [autoConnect, setAutoConnect] = useState(false);

  const handleUpdateSystemSettings = async (
    updates: Partial<{
      autostart: boolean;
      startMinimized: boolean;
      closeToTray: boolean;
      autoConnect: boolean;
      language: Language;
    }>
  ) => {
    const newAutostart = updates.autostart !== undefined ? updates.autostart : autostart;
    const newStartMin =
      updates.startMinimized !== undefined ? updates.startMinimized : startMinimized;
    const newCloseTray =
      updates.closeToTray !== undefined ? updates.closeToTray : closeToTray;
    const newAutoConn =
      updates.autoConnect !== undefined ? updates.autoConnect : autoConnect;
    const newLang = updates.language !== undefined ? updates.language : language;

    if (updates.autostart !== undefined) setAutostart(newAutostart);
    if (updates.startMinimized !== undefined) setStartMinimized(newStartMin);
    if (updates.closeToTray !== undefined) setCloseToTray(newCloseTray);
    if (updates.autoConnect !== undefined) setAutoConnect(newAutoConn);
    if (updates.language !== undefined) {
      setLanguage(newLang);
      localStorage.setItem("neuroveil_lang", newLang);
    }

    try {
      await invoke("save_system_settings", {
        autostart: newAutostart,
        startMinimized: newStartMin,
        closeToTray: newCloseTray,
        autoConnect: newAutoConn,
        language: newLang,
      });
      showNotice(
        "success",
        newLang === "ru" ? "Настройки успешно сохранены!" : "Settings saved successfully!"
      );
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleSelectTheme = (themeId: string) => {
    setCurrentThemeId(themeId);
    localStorage.setItem("neuroveil_theme", themeId);
    const themeName = THEMES.find((t) => t.id === themeId)?.name || themeId;
    showNotice("success", `Тема «${themeName}» успешно применена!`);
  };

  const isLightTheme = currentThemeId === "nordic_frost" || currentThemeId === "cyber_quartz";

  const cssVarsStyle = {
    "--bg-app": currentTheme.vars.bgApp,
    "--bg-sidebar": currentTheme.vars.bgSidebar,
    "--bg-card": currentTheme.vars.bgCard,
    "--bg-card-alt": currentTheme.vars.bgCardAlt,
    "--bg-input": currentTheme.vars.bgInput,
    "--border-card": currentTheme.vars.borderCard,
    "--color-accent": currentTheme.vars.colorAccent,
    "--color-accent-border": currentTheme.vars.colorAccentBorder,
    "--color-accent-bg": currentTheme.vars.colorAccentBg,
    "--color-accent-glow": currentTheme.vars.colorAccentGlow,
    "--btn-grad-from": currentTheme.vars.btnGradFrom,
    "--btn-grad-to": currentTheme.vars.btnGradTo,
    "--btn-grad-from-hover": currentTheme.vars.btnGradFromHover,
    "--btn-grad-to-hover": currentTheme.vars.btnGradToHover,
    "--nav-grad-from": currentTheme.vars.navGradFrom,
    "--nav-grad-to": currentTheme.vars.navGradTo,
  } as React.CSSProperties;

  const handleMouseDownDrag = (e: React.MouseEvent) => {
    if (e.button === 0) {
      invoke("app_start_dragging");
    }
  };

  const handleDoubleClickMaximize = () => {
    invoke("app_toggle_maximize");
  };

  // Persistent & runtime state
  const [profiles, setProfiles] = useState<VpnProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>("");
  const [routing, setRouting] = useState<RoutingConfig>({
    mode: "system_proxy",
    split_tunneling: true,
    bypass_ru: true,
    bypass_lan: true,
    default_outbound: "proxy",
    direct_domains: [".ru", ".su", ".xn--p1ai", "yandex.ru", "vk.com", "gosuslugi.ru"],
    proxy_domains: ["youtube.com", "googlevideo.com", "discord.com", "openai.com"],
    direct_processes: [],
    remote_dns: "https://1.1.1.1/dns-query",
    direct_dns: "77.88.8.8",
    local_mixed_port: 2080,
    clash_api_port: 9090,
  });

  const [status, setStatus] = useState<VpnStatus>({
    connected: false,
    active_profile_id: null,
    active_profile_name: null,
    active_protocol: null,
    mode: "system_proxy",
    split_tunneling: true,
    up_bytes: 0,
    down_bytes: 0,
    up_speed: 0,
    down_speed: 0,
    singbox_installed: false,
    singbox_version: "Проверка...",
    singbox_path: "",
    logs: [],
  });

  function getRandomInt(min: number, max: number): number {
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }

  function getRandomBase64Key(): string {
    const bytes = new Uint8Array(32);
    window.crypto.getRandomValues(bytes);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  function generateRandomHValues(): [string, string, string, string] {
    const set = new Set<number>();
    while (set.size < 4) {
      set.add(getRandomInt(10_000_000, 2_147_483_640));
    }
    const arr = Array.from(set);
    return [arr[0].toString(), arr[1].toString(), arr[2].toString(), arr[3].toString()];
  }

  function generateRandomAwg31Params() {
    return {
      awg_jc: getRandomInt(4, 7),
      awg_jmin: getRandomInt(10, 25),
      awg_jmax: getRandomInt(45, 75),
      awg_s1: getRandomInt(12, 18),
      awg_s2: getRandomInt(12, 18),
      awg_s3: getRandomInt(12, 18),
      awg_s4: getRandomInt(12, 18),
      awg_h1: "1",
      awg_h2: "2",
      awg_h3: "3",
      awg_h4: "4",
      awg_header_protection_key: getRandomBase64Key(),
      awg_preshared_key: getRandomBase64Key(),
      awg_rekey_after_time: `${getRandomInt(90, 110)}-${getRandomInt(115, 140)}`,
      awg_rekey_timeout: `${getRandomInt(2, 4)}-${getRandomInt(5, 8)}`,
      awg_reject_after_time: `${getRandomInt(140, 160)}-${getRandomInt(170, 200)}`,
      awg_keepalive_timeout: `${getRandomInt(4, 7)}-${getRandomInt(12, 18)}`,
      awg_max_handshake_attempts: `${getRandomInt(12, 16)}-${getRandomInt(18, 24)}`,
      awg_persistent_keepalive: `${getRandomInt(20, 25)}-${getRandomInt(30, 40)}`,
      awg_random_trailers: true,
      awg_disable_cookies: true,
    };
  }

  function generateRandomAwg20Params() {
    const [h1, h2, h3, h4] = generateRandomHValues();
    return {
      awg_jc: getRandomInt(3, 8),
      awg_jmin: getRandomInt(15, 35),
      awg_jmax: getRandomInt(50, 95),
      awg_s1: getRandomInt(20, 75),
      awg_s2: getRandomInt(20, 75),
      awg_s3: 0,
      awg_s4: 0,
      awg_h1: h1,
      awg_h2: h2,
      awg_h3: h3,
      awg_h4: h4,
      awg_preshared_key: getRandomBase64Key(),
    };
  }

  // VPS Auto-Deploy state
  const [vpsCreds, setVpsCreds] = useState<VpsCredentials>(() => {
    const autoAwgPort = getRandomInt(21000, 55000);
    const r31 = generateRandomAwg31Params();
    return {
      host: "",
      ssh_port: 22,
      username: "root",
      password: "",
      vless_port: 443,
      hy2_port: 8443,
      tuic_port: 9443,
      ss_port: 8388,
      ss_method: "2022-blake3-aes-128-gcm",
      sni: "www.bing.com",
      reality_sni: "www.microsoft.com",
      deploy_vless: true,
      deploy_hy2: true,
      deploy_tuic: false,
      deploy_ss: false,
      deploy_awg: true,
      deploy_awg2: false,
      deploy_ssh: true,
      awg_port: autoAwgPort,
      awg_hop_ports: "20000:50000",
      ...r31,
      enable_salamander: false,
      enable_port_hopping: true,
      ssh_proxy: "",
    };
  });
  const [showVpsPassword, setShowVpsPassword] = useState(false);
  const [mainCheckStatus, setMainCheckStatus] = useState<{
    busy: boolean;
    result?: string;
    error?: string;
  }>({ busy: false });
  const [vpsLogs, setVpsLogs] = useState<string[]>([
    "[Система] Введите IP, логин и пароль от вашего VPS (Ubuntu / Debian / CentOS) для автоматической установки или очистки.",
  ]);
  const [vpsBusy, setVpsBusy] = useState<"idle" | "testing" | "deploying" | "cleaning">("idle");
  const [confirmCleanModal, setConfirmCleanModal] = useState(false);
  const [isVpsAdmin, setIsVpsAdmin] = useState(false);
  const [adminLoginLoading, setAdminLoginLoading] = useState(false);

  // App Update Center State (GitHub: lllPredatorlll/NeuroVeil-VPN)
  const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [lastUpdateCheck, setLastUpdateCheck] = useState<string | null>(null);

  const handleCheckUpdate = async (silent = false) => {
    setCheckingUpdate(true);
    try {
      const info = await invoke<AppUpdateInfo>("check_app_update");
      setUpdateInfo(info);
      setLastUpdateCheck(new Date().toLocaleTimeString("ru-RU"));
      if (!silent) {
        if (info.has_update) {
          showNotice(
            "info",
            `Доступна новая версия NeuroVeil VPN v${info.latest_version} на GitHub!`
          );
        } else {
          showNotice(
            "success",
            `Проверка завершена: установлена актуальная версия NeuroVeil VPN v${info.current_version} (Build 1050)`
          );
        }
      }
    } catch (err: any) {
      if (!silent) {
        showNotice("error", `Ошибка проверки обновлений: ${String(err)}`);
      }
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleInstallUpdate = async () => {
    if (!updateInfo?.download_url) {
      await handleOpenExternalUrl(
        updateInfo?.html_url || "https://github.com/lllPredatorlll/NeuroVeil-VPN"
      );
      return;
    }
    setInstallingUpdate(true);
    showNotice(
      "info",
      `Загрузка обновления NeuroVeil VPN v${updateInfo.latest_version} с GitHub... Приложение перезапустится автоматически.`
    );
    try {
      await invoke<string>("download_and_install_update", {
        downloadUrl: updateInfo.download_url,
        assetName: updateInfo.asset_name ?? null,
      });
    } catch (err: any) {
      setInstallingUpdate(false);
      showNotice("error", `Не удалось установить обновление: ${String(err)}`);
    }
  };

  const handleOpenExternalUrl = async (url: string) => {
    try {
      await invoke("open_external_url", { url });
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  // UI notifications & helpers
  const [toast, setToast] = useState<{ type: "success" | "error" | "info"; text: string } | null>(
    null
  );
  const [connecting, setConnecting] = useState(false);
  const [reconnectCooldown, setReconnectCooldown] = useState(false);
  const [showAwgAdvanced, setShowAwgAdvanced] = useState(false);
  const [downloadingCore, setDownloadingCore] = useState(false);
  const [uriInput, setUriInput] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [pingingId, setPingingId] = useState<string | null>(null);
  const [showNewProfileForm, setShowNewProfileForm] = useState(false);
  const [newProfile, setNewProfile] = useState<VpnProfile>({
    id: "",
    name: "",
    protocol: "hysteria2",
    server: "",
    port: 8443,
    sni: "www.bing.com",
    insecure: true,
    password: "",
    uuid: "",
    obfs_password: "",
    ss_method: "2022-blake3-aes-128-gcm",
    ss_plugin: "",
    ss_plugin_opts: "",
    tls_fragment: true,
    up_mbps: 100,
    down_mbps: 100,
    uri: "",
  });

  // Textareas & lists for routing rules
  const [directDomainsText, setDirectDomainsText] = useState("");
  const [proxyDomainsText, setProxyDomainsText] = useState("");
  const [directIpsText, setDirectIpsText] = useState("");
  const [proxyIpsText, setProxyIpsText] = useState("");
  const [directProcessesList, setDirectProcessesList] = useState<string[]>([]);
  const [proxyProcessesList, setProxyProcessesList] = useState<string[]>([]);
  const [manualDirectProcInput, setManualDirectProcInput] = useState("");
  const [manualProxyProcInput, setManualProxyProcInput] = useState("");

  // Process picker modal state
  const [procModalTarget, setProcModalTarget] = useState<"proxy" | "direct" | null>(null);
  const [runningProcs, setRunningProcs] = useState<RunningProcessInfo[]>([]);
  const [loadingProcs, setLoadingProcs] = useState(false);
  const [procSearch, setProcSearch] = useState("");

  // Device Manager & QR Modal state
  const [syncedServerProfiles, setSyncedServerProfiles] = useState<VpnProfile[]>([]);
  const [deviceNameInput, setDeviceNameInput] = useState("");
  const [deviceProtocol, setDeviceProtocol] = useState<"vless" | "hysteria2" | "tuic" | "shadowsocks" | "amneziawg" | "awg2">("vless");
  const [creatingDevice, setCreatingDevice] = useState(false);
  const [qrModalProfile, setQrModalProfile] = useState<VpnProfile | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string>("");
  const [routingProfileId, setRoutingProfileId] = useState<string>("");
  const [probingAwgId, setProbingAwgId] = useState<string | null>(null);
  const [awgProbeResults, setAwgProbeResults] = useState<Record<string, { port: number; reachable: boolean; latency_ms?: number; message: string }[]>>({});

  // Routing Presets state
  const [routingPresets, setRoutingPresets] = useState<RoutingPreset[]>([]);
  const [activePresetId, setActivePresetId] = useState<string | null>(null);
  const [showSavePresetModal, setShowSavePresetModal] = useState(false);
  const [newPresetName, setNewPresetName] = useState("");
  const [newPresetOutbound, setNewPresetOutbound] = useState<"direct" | "proxy">("direct");
  const [isApplyingPreset, setIsApplyingPreset] = useState(false);

  // Listening Port Detection state
  const [isProbingListeningPorts, setIsProbingListeningPorts] = useState(false);
  const [isApplyingListeningPort, setIsApplyingListeningPort] = useState(false);
  const [listeningProbeResults, setListeningProbeResults] = useState<Record<string, PortProbeResult[]>>({});
  const [probedHistoryPorts, setProbedHistoryPorts] = useState<Record<string, number[]>>({});

  // In-app Config Editor Modal State
  const [editingProfile, setEditingProfile] = useState<VpnProfile | null>(null);
  const [editForm, setEditForm] = useState<Partial<VpnProfile>>({});
  const [editModeRaw, setEditModeRaw] = useState<boolean>(false);
  const [editRawJson, setEditRawJson] = useState<string>("");
  const [editSaving, setEditSaving] = useState<boolean>(false);

  const handleOpenEditModal = (prof: VpnProfile) => {
    setEditingProfile(prof);
    setEditForm({ ...prof });
    setEditRawJson(JSON.stringify(prof, null, 2));
    setEditModeRaw(false);
  };

  const handleSaveEditedProfile = async () => {
    if (!editingProfile) return;
    setEditSaving(true);
    try {
      let updatedProfile: VpnProfile;
      if (editModeRaw) {
        try {
          updatedProfile = JSON.parse(editRawJson);
        } catch (e: any) {
          showNotice("error", "Ошибка синтаксиса JSON: " + e.message);
          setEditSaving(false);
          return;
        }
      } else {
        updatedProfile = {
          ...editingProfile,
          ...editForm,
          port: Number(editForm.port) || editingProfile.port,
        } as VpnProfile;
      }

      if (updatedProfile.protocol === "amneziawg" || updatedProfile.protocol === "awg2") {
        const priv = (updatedProfile.awg_private_key || updatedProfile.password || "").trim();
        updatedProfile.awg_private_key = priv;
        updatedProfile.password = priv;
      }

      const updatedList = await invoke<VpnProfile[]>("add_or_update_profile", {
        profile: updatedProfile,
      });
      setProfiles(updatedList);
      showNotice("success", `Конфигурация «${updatedProfile.name}» успешно сохранена!`);
      setEditingProfile(null);

      // If active profile was updated while connected, reconnect
      if (status.connected && status.active_profile_id === updatedProfile.id) {
        showNotice("info", "Переподключение туннеля с новыми параметрами...");
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: updatedProfile.id,
          mode: routing.mode,
          splitTunneling: routing.split_tunneling,
        });
        setStatus(st);
      }
    } catch (err: any) {
      showNotice("error", "Ошибка сохранения профиля: " + String(err));
    } finally {
      setEditSaving(false);
    }
  };

  const handleToggleKillSwitch = async () => {
    const nextVal = !routing.kill_switch;
    const nextRouting: RoutingConfig = {
      ...routing,
      kill_switch: nextVal,
    };
    setRouting(nextRouting);
    try {
      await invoke("save_routing_config", {
        routing: nextRouting,
        profileId: selectedProfileId || null,
      });
      showNotice(
        "success",
        nextVal
          ? "Kill Switch включен! Трафик будет заблокирован при обрыве VPN"
          : "Kill Switch выключен"
      );
      if (status.connected && selectedProfile) {
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: nextRouting.mode,
          splitTunneling: nextRouting.split_tunneling,
        });
        setStatus(st);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleToggleBlockIpv6 = async () => {
    const nextVal = !(routing.block_ipv6 ?? true);
    const nextRouting: RoutingConfig = {
      ...routing,
      block_ipv6: nextVal,
    };
    setRouting(nextRouting);
    try {
      await invoke("save_routing_config", {
        routing: nextRouting,
        profileId: selectedProfileId || null,
      });
      showNotice(
        "success",
        nextVal
          ? "Блокировка IPv6 включена! Утечки через IPv6 устранены"
          : "Блокировка IPv6 отключена"
      );
      if (status.connected && selectedProfile) {
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: nextRouting.mode,
          splitTunneling: nextRouting.split_tunneling,
        });
        setStatus(st);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleToggleDnsLeakProtect = async () => {
    const nextVal = !(routing.dns_leak_protect ?? true);
    const nextRouting: RoutingConfig = {
      ...routing,
      dns_leak_protect: nextVal,
    };
    setRouting(nextRouting);
    try {
      await invoke("save_routing_config", {
        routing: nextRouting,
        profileId: selectedProfileId || null,
      });
      showNotice(
        "success",
        nextVal
          ? "Защита от утечек DNS включена! Все запросы шифруются через DoH"
          : "Защита от утечек DNS отключена"
      );
      if (status.connected && selectedProfile) {
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: nextRouting.mode,
          splitTunneling: nextRouting.split_tunneling,
        });
        setStatus(st);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const vpsLogEndRef = useRef<HTMLDivElement>(null);
  const coreLogEndRef = useRef<HTMLDivElement>(null);

  const showNotice = (type: "success" | "error" | "info", text: string) => {
    setToast({ type, text });
    setTimeout(() => {
      setToast((prev) => (prev?.text === text ? null : prev));
    }, 5000);
  };

  // Initial load & polling
  useEffect(() => {
    async function init() {
      try {
        const appState = await invoke<AppPersistentState>("get_app_state");
        setProfiles(appState.profiles);
        const initialPid =
          appState.selected_profile_id ||
          (appState.profiles.length > 0 ? appState.profiles[0].id : "");
        if (initialPid) {
          setSelectedProfileId(initialPid);
          setRoutingProfileId(initialPid);
        }
        const initialProf = appState.profiles.find((p) => p.id === initialPid);
        const initialRouting = initialProf?.routing || appState.routing;
        setRouting(initialRouting);
        setDirectDomainsText((initialRouting.direct_domains || []).join("\n"));
        setProxyDomainsText((initialRouting.proxy_domains || []).join("\n"));
        setDirectIpsText((initialRouting.direct_ips || []).join("\n"));
        setProxyIpsText((initialRouting.proxy_ips || []).join("\n"));
        setDirectProcessesList(initialRouting.direct_processes || []);
        setProxyProcessesList(initialRouting.proxy_processes || []);

        if (appState.routing_presets && appState.routing_presets.length > 0) {
          setRoutingPresets(appState.routing_presets);
        }
        if (appState.active_preset_id) {
          setActivePresetId(appState.active_preset_id);
        }

        if (appState.saved_vps) {
          setVpsCreds((prev) => ({
            ...prev,
            ...appState.saved_vps!,
            deploy_vless: appState.saved_vps!.deploy_vless !== undefined ? appState.saved_vps!.deploy_vless : true,
            deploy_hy2: appState.saved_vps!.deploy_hy2 !== undefined ? appState.saved_vps!.deploy_hy2 : true,
            deploy_tuic: appState.saved_vps!.deploy_tuic !== undefined ? appState.saved_vps!.deploy_tuic : false,
            deploy_ss: appState.saved_vps!.deploy_ss !== undefined ? appState.saved_vps!.deploy_ss : false,
            deploy_awg: appState.saved_vps!.deploy_awg !== undefined ? appState.saved_vps!.deploy_awg : true,
            deploy_awg2: appState.saved_vps!.deploy_awg2 !== undefined ? appState.saved_vps!.deploy_awg2 : false,
            deploy_ssh: appState.saved_vps!.deploy_ssh !== undefined ? appState.saved_vps!.deploy_ssh : true,
            awg_port: (appState.saved_vps!.awg_port && appState.saved_vps!.awg_port !== 51820) ? appState.saved_vps!.awg_port : getRandomInt(21000, 55000),
            awg_hop_ports: appState.saved_vps!.awg_hop_ports || "20000:50000",
            awg_jc: appState.saved_vps!.awg_jc ?? 5,
            awg_jmin: appState.saved_vps!.awg_jmin ?? 10,
            awg_jmax: appState.saved_vps!.awg_jmax ?? 50,
            awg_s1: appState.saved_vps!.awg_s1 ?? 12,
            awg_s2: appState.saved_vps!.awg_s2 ?? 12,
            awg_s3: appState.saved_vps!.awg_s3 ?? 12,
            awg_s4: appState.saved_vps!.awg_s4 ?? 12,
            awg_h1: appState.saved_vps!.awg_h1 ?? "1",
            awg_h2: appState.saved_vps!.awg_h2 ?? "2",
            awg_h3: appState.saved_vps!.awg_h3 ?? "3",
            awg_h4: appState.saved_vps!.awg_h4 ?? "4",
            awg2_port: (appState.saved_vps!.awg2_port && appState.saved_vps!.awg2_port !== 51821) ? appState.saved_vps!.awg2_port : getRandomInt(21000, 55000),
            awg2_hop_ports: appState.saved_vps!.awg2_hop_ports || "20000:50000",
            awg2_jc: appState.saved_vps!.awg2_jc ?? 4,
            awg2_jmin: appState.saved_vps!.awg2_jmin ?? 20,
            awg2_jmax: appState.saved_vps!.awg2_jmax ?? 60,
            awg2_s1: appState.saved_vps!.awg2_s1 ?? 30,
            awg2_s2: appState.saved_vps!.awg2_s2 ?? 40,
            awg2_h1: appState.saved_vps!.awg2_h1 ?? "89472914",
            awg2_h2: appState.saved_vps!.awg2_h2 ?? "194820194",
            awg2_h3: appState.saved_vps!.awg2_h3 ?? "492019482",
            awg2_h4: appState.saved_vps!.awg2_h4 ?? "1582910492",
            vless_port: appState.saved_vps!.vless_port || 443,
            hy2_port: appState.saved_vps!.hy2_port || 8443,
            tuic_port: appState.saved_vps!.tuic_port || 9443,
            ss_port: appState.saved_vps!.ss_port || 8388,
            ss_method: appState.saved_vps!.ss_method || "2022-blake3-aes-128-gcm",
            reality_sni: appState.saved_vps!.reality_sni || "www.microsoft.com",
            password: prev.password,
          }));
        }

        if (appState.autostart !== undefined) setAutostart(appState.autostart);
        if (appState.start_minimized !== undefined) setStartMinimized(appState.start_minimized);
        if (appState.close_to_tray !== undefined) setCloseToTray(appState.close_to_tray);
        if (appState.auto_connect !== undefined) setAutoConnect(appState.auto_connect);
        if (appState.language) {
          setLanguage(appState.language as Language);
          localStorage.setItem("neuroveil_lang", appState.language);
        }

        const st = await invoke<VpnStatus>("get_vpn_status");
        setStatus(st);
      } catch (err) {
        console.error("Failed to load initial state:", err);
      }
    }
    init();

    let isPolling = false;
    let lastTrackedPid: string | null = null;
    let lastUp = 0;
    let lastDown = 0;
    const interval = setInterval(async () => {
      if (isPolling) return;
      isPolling = true;
      try {
        const st = await invoke<VpnStatus>("get_vpn_status");
        setStatus(st);
        if (st.connected && st.active_profile_id) {
          if (lastTrackedPid !== st.active_profile_id) {
            lastTrackedPid = st.active_profile_id;
            lastUp = st.up_bytes;
            lastDown = st.down_bytes;
          } else {
            const deltaUp = st.up_bytes >= lastUp ? st.up_bytes - lastUp : st.up_bytes;
            const deltaDown = st.down_bytes >= lastDown ? st.down_bytes - lastDown : st.down_bytes;
            lastUp = st.up_bytes;
            lastDown = st.down_bytes;
            if (deltaUp > 0 || deltaDown > 0) {
              const activePid = st.active_profile_id;
              setProfiles((prev) =>
                prev.map((p) =>
                  p.id === activePid
                    ? {
                        ...p,
                        total_up_bytes: (p.total_up_bytes || 0) + deltaUp,
                        total_down_bytes: (p.total_down_bytes || 0) + deltaDown,
                      }
                    : p
                )
              );
              setSyncedServerProfiles((prev) =>
                prev.map((p) =>
                  p.id === activePid
                    ? {
                        ...p,
                        total_up_bytes: (p.total_up_bytes || 0) + deltaUp,
                        total_down_bytes: (p.total_down_bytes || 0) + deltaDown,
                      }
                    : p
                )
              );
            }
          }
        } else {
          lastTrackedPid = null;
          lastUp = 0;
          lastDown = 0;
        }
      } catch (_) {
      } finally {
        isPolling = false;
      }
    }, 1000);

    const unlistenPromise = listen<string>("vps-log", (event) => {
      setVpsLogs((prev) => [...prev, event.payload]);
    });

    return () => {
      clearInterval(interval);
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    vpsLogEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [vpsLogs]);

  // Profiles display logic:
  // PC dashboard and routing tabs only show profiles designated for this PC
  const dashboardProfiles = profiles.filter((p) => p.pinned_on_pc !== false);
  const effectiveDashboardProfiles = dashboardProfiles.length > 0 ? dashboardProfiles : profiles;

  // Sync block in Tab 3 ONLY shows active server profiles when logged in as VPS admin,
  // and completely clears them upon logout or disconnection.
  const serverProfiles = isVpsAdmin ? syncedServerProfiles : [];
  const effectiveClientProfiles = dashboardProfiles.length > 0 ? dashboardProfiles : profiles;

  const selectedProfile =
    dashboardProfiles.find((p) => p.id === selectedProfileId) ||
    dashboardProfiles[0] ||
    profiles.find((p) => p.id === selectedProfileId) ||
    profiles[0] ||
    null;

  const activeRoutingProfile =
    dashboardProfiles.find((p) => p.id === (routingProfileId || selectedProfileId)) ||
    selectedProfile ||
    dashboardProfiles[0] ||
    profiles[0] ||
    null;

  const handlePinProfileOnPc = async (profileId: string) => {
    const prof =
      syncedServerProfiles.find((p) => p.id === profileId) ||
      profiles.find((p) => p.id === profileId);
    if (!prof) return;
    const updated: VpnProfile = { ...prof, pinned_on_pc: true };
    try {
      const updatedList = await invoke<VpnProfile[]>("add_or_update_profile", {
        profile: updated,
      });
      setProfiles(updatedList);
      showNotice("success", `Профиль «${prof.name}» добавлен на этот ПК`);
    } catch (e: any) {
      console.error(e);
    }
  };

  const switchRoutingProfile = (profileId: string) => {
    setRoutingProfileId(profileId);
    const target = profiles.find((p) => p.id === profileId);
    if (target) {
      const profRouting = target.routing || routing;
      setRouting(profRouting);
      setDirectDomainsText((profRouting.direct_domains || []).join("\n"));
      setProxyDomainsText((profRouting.proxy_domains || []).join("\n"));
      setDirectIpsText((profRouting.direct_ips || []).join("\n"));
      setProxyIpsText((profRouting.proxy_ips || []).join("\n"));
      setDirectProcessesList(profRouting.direct_processes || []);
      setProxyProcessesList(profRouting.proxy_processes || []);
    }
  };

  const handleQuickSwitchProfile = async (profileId: string) => {
    setSelectedProfileId(profileId);
    setRoutingProfileId(profileId);
    const target = profiles.find((p) => p.id === profileId);
    const targetRouting = target?.routing || routing;
    setRouting(targetRouting);
    setDirectDomainsText((targetRouting.direct_domains || []).join("\n"));
    setProxyDomainsText((targetRouting.proxy_domains || []).join("\n"));
    setDirectIpsText((targetRouting.direct_ips || []).join("\n"));
    setProxyIpsText((targetRouting.proxy_ips || []).join("\n"));
    setDirectProcessesList(targetRouting.direct_processes || []);
    setProxyProcessesList(targetRouting.proxy_processes || []);

    if (status.connected && !connecting && status.active_profile_id !== profileId) {
      if (target?.protocol === "amneziawg" || target?.protocol === "awg2") {
        const privKey = (target.awg_private_key || target.password || "").trim();
        if (!privKey) {
          showNotice(
            "error",
            "В профиле Amnezia не указан приватный ключ (Private Key). Введите ключ в настройках профиля (✏️) или выберите файл .conf."
          );
          handleOpenEditModal(target);
          return;
        }
      }
      setConnecting(true);
      try {
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId,
          mode: targetRouting.mode,
          splitTunneling: targetRouting.split_tunneling,
        });
        setStatus(st);
        showNotice("success", `Переключено на ${target?.name || "профиль"}`);
      } catch (err: any) {
        showNotice("error", String(err));
      } finally {
        setConnecting(false);
      }
    }
  };

  const handleQuickSwitchMode = async (newMode: "tun" | "system_proxy") => {
    setRouting((r) => ({ ...r, mode: newMode }));
    if (status.connected && !connecting && selectedProfile) {
      setConnecting(true);
      try {
        const profRouting = selectedProfile.routing || routing;
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: newMode,
          splitTunneling: profRouting.split_tunneling,
        });
        setStatus(st);
        showNotice(
          "info",
          `Режим переключен: ${newMode === "tun" ? "TUN (Весь ПК)" : "System Proxy"}`
        );
      } catch (err: any) {
        showNotice("error", String(err));
      } finally {
        setConnecting(false);
      }
    }
  };

  const handleToggleSplitForProfile = async () => {
    const nextSplit = !routing.split_tunneling;
    const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
    const nextRouting: RoutingConfig = {
      ...routing,
      split_tunneling: nextSplit,
      direct_domains: directDomainsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      proxy_domains: proxyDomainsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      direct_ips: directIpsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      proxy_ips: proxyIpsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean),
      direct_processes: directProcessesList,
      proxy_processes: proxyProcessesList,
    };
    setRouting(nextRouting);
    try {
      const saved = await invoke<RoutingConfig>("save_routing_config", {
        routing: nextRouting,
        profileId: targetPid || null,
      });
      if (targetPid) {
        setProfiles((prev) =>
          prev.map((p) => (p.id === targetPid ? { ...p, routing: saved } : p))
        );
      }
      if (status.connected && selectedProfile && selectedProfile.id === targetPid) {
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: saved.mode,
          splitTunneling: saved.split_tunneling,
        });
        setStatus(st);
        showNotice(
          "info",
          nextSplit
            ? "Раздельное туннелирование включено для этого профиля"
            : "Раздельное туннелирование выключено (весь трафик через VPN)"
        );
      } else {
        showNotice(
          "info",
          nextSplit
            ? "Раздельное туннелирование включено"
            : "Раздельное туннелирование выключено"
        );
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleExportCoreLogs = async () => {
    try {
      const savedPath = await invoke<string | null>("export_core_logs_to_file");
      if (savedPath) {
        showNotice("success", `Журнал ядра успешно сохранен в файл:\n${savedPath}`);
      }
    } catch (err: any) {
      showNotice("error", `Ошибка экспорта журнала: ${String(err)}`);
    }
  };

  // Actions
  const handleToggleConnect = async () => {
    if (connecting || reconnectCooldown) return;
    setConnecting(true);
    try {
      if (status.connected) {
        setReconnectCooldown(true);
        setTimeout(() => setReconnectCooldown(false), 2000);
        const st = await invoke<VpnStatus>("disconnect_vpn");
        setStatus(st);
        showNotice("info", "VPN отключен. Защитная пауза WireGuard (2 сек)...");
      } else {
        if (!selectedProfile) {
          showNotice(
            "error",
            "Сначала добавьте профиль или разверните VPN на своем VPS во вкладке «VPS Автодеплой»"
          );
          setActiveTab("vps");
          setConnecting(false);
          return;
        }
        if (selectedProfile.protocol === "amneziawg" || selectedProfile.protocol === "awg2") {
          const privKey = (selectedProfile.awg_private_key || selectedProfile.password || "").trim();
          if (!privKey) {
            showNotice(
              "error",
              "В профиле Amnezia отсутствует приватный ключ (Private Key). Введите ключ в настройках профиля (✏️) или выберите файл .conf."
            );
            handleOpenEditModal(selectedProfile);
            setConnecting(false);
            return;
          }
        }
        const profRouting = selectedProfile.routing || routing;
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: profRouting.mode,
          splitTunneling: profRouting.split_tunneling,
        });
        setStatus(st);
        if (st.is_handshaking) {
          showNotice("info", `Запуск туннеля ${selectedProfile.name}. Ожидание рукопожатия WireGuard...`);
        } else {
          showNotice("success", `Подключено к ${selectedProfile.name}`);
        }
      }
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setConnecting(false);
    }
  };

  const handleDownloadCore = async () => {
    if (downloadingCore) return;
    setDownloadingCore(true);
    showNotice("info", "Скачивание официального ядра sing-box с GitHub...");
    try {
      const ver = await invoke<string>("download_core");
      const st = await invoke<VpnStatus>("get_vpn_status");
      setStatus(st);
      showNotice("success", `Ядро успешно установлено: ${ver}`);
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setDownloadingCore(false);
    }
  };

  const handleTestMainServer = async () => {
    if (!vpsCreds.host.trim() || !vpsCreds.password) {
      showNotice("error", "Укажите IP-адрес основного сервера и пароль SSH");
      return;
    }
    setMainCheckStatus({ busy: true });
    try {
      const res = await invoke<string>("test_single_vps_ssh", {
        host: vpsCreds.host.trim(),
        port: vpsCreds.ssh_port,
        username: vpsCreds.username.trim(),
        password: vpsCreds.password,
        proxy: vpsCreds.ssh_proxy.trim() || null,
      });
      setMainCheckStatus({ busy: false, result: res });
      showNotice("success", `Основной VPS доступен (${res})`);
    } catch (err: any) {
      setMainCheckStatus({ busy: false, error: String(err) });
      setVpsLogs((prev) => [...prev, `[ОШИБКА ОСНОВНОГО СЕРВЕРА] ${String(err)}`]);
      showNotice("error", String(err));
    }
  };

  const handleTestSsh = async () => {
    if (!vpsCreds.host.trim() || !vpsCreds.password) {
      showNotice("error", "Укажите IP-адрес сервера и пароль SSH");
      return;
    }
    setVpsBusy("testing");
    try {
      const res = await invoke<string>("test_vps_ssh", { creds: vpsCreds });
      showNotice("success", `SSH ОК: ${res}`);
    } catch (err: any) {
      setVpsLogs((prev) => [...prev, `[ОШИБКА] ${String(err)}`]);
      showNotice("error", String(err));
    } finally {
      setVpsBusy("idle");
    }
  };

  const handleDeployVps = async () => {
    if (!vpsCreds.host.trim() || !vpsCreds.password) {
      showNotice("error", "Укажите IP-адрес сервера и пароль SSH");
      return;
    }
    if (!vpsCreds.deploy_vless && !vpsCreds.deploy_hy2 && !vpsCreds.deploy_tuic && !vpsCreds.deploy_ss && !vpsCreds.deploy_awg && !vpsCreds.deploy_ssh) {
      showNotice(
        "error",
        "Выберите хотя бы один протокол для развертывания (SSH Tunnel, VLESS Reality, Hysteria 2, TUIC v5, Shadowsocks или Amnezia 3.1)"
      );
      return;
    }

    // Автоматическая рандомизация при каждом деплое: новые случайные порты и параметры обфускации
    const autoAwgPort = getRandomInt(21000, 55000);
    const r31 = generateRandomAwg31Params();

    const deployedCreds: VpsCredentials = {
      ...vpsCreds,
      deploy_awg2: false,
      ...r31,
      awg_port: autoAwgPort,
      awg_disable_cookies: vpsCreds.awg_disable_cookies ?? true,
      awg_random_trailers: vpsCreds.awg_random_trailers ?? true,
      awg_port_probing: vpsCreds.awg_port_probing ?? true,
    };
    setVpsCreds(deployedCreds);

    setVpsBusy("deploying");
    setVpsLogs((prev) => [
      ...prev,
      `[Авто-рандомизация] Сгенерирован порт Amnezia 3.1 (UDP ${autoAwgPort}) и уникальные параметры обфускации.`,
    ]);
    try {
      const res = await invoke<DeployResult>("deploy_vps_server", { creds: deployedCreds });
      if (res.new_ssh_port) {
        setVpsCreds((prev) => ({
          ...prev,
          port: res.new_ssh_port!,
          ssh_port: res.new_ssh_port!,
        }));
      }
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      if (appState.selected_profile_id) {
        setSelectedProfileId(appState.selected_profile_id);
      }
      showNotice("success", res.message);
    } catch (err: any) {
      setVpsLogs((prev) => [...prev, `[ОШИБКА РАЗВЕРТЫВАНИЯ] ${String(err)}`]);
      showNotice("error", String(err));
    } finally {
      setVpsBusy("idle");
    }
  };

  const handleCleanVps = async () => {
    setConfirmCleanModal(false);
    if (!vpsCreds.host.trim() || !vpsCreds.password) {
      showNotice("error", "Укажите IP-адрес сервера и пароль SSH для очистки");
      return;
    }
    setVpsBusy("cleaning");
    try {
      const msg = await invoke<string>("clean_vps_server", { creds: vpsCreds });
      showNotice("success", msg);
    } catch (err: any) {
      setVpsLogs((prev) => [...prev, `[ОШИБКА ОЧИСТКИ] ${String(err)}`]);
      showNotice("error", String(err));
    } finally {
      setVpsBusy("idle");
    }
  };

  const handleImportUri = async () => {
    if (!uriInput.trim()) return;
    try {
      const created = await invoke<VpnProfile>("import_profile_uri", { uri: uriInput.trim() });
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      setSelectedProfileId(created.id);
      setUriInput("");
      showNotice("success", `Профиль «${created.name}» успешно импортирован`);
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handlePickConfigFile = async () => {
    try {
      const created = await invoke<VpnProfile | null>("pick_config_file_and_import");
      if (created) {
        const appState = await invoke<AppPersistentState>("get_app_state");
        setProfiles(appState.profiles);
        setSelectedProfileId(created.id);
        showNotice(
          "success",
          `Конфигурация «${created.name}» успешно импортирована из файла!`
        );
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleSaveManualProfile = async () => {
    if (!newProfile.server.trim() || !newProfile.password.trim()) {
      showNotice("error", "Заполните адрес сервера и пароль");
      return;
    }
    try {
      const defaultLabel =
        newProfile.protocol === "ssh"
          ? "SSH Tunnel"
          : newProfile.protocol === "vless"
          ? "VLESS Reality"
          : newProfile.protocol === "tuic"
          ? "TUIC v5"
          : newProfile.protocol === "shadowsocks"
          ? "Shadowsocks"
          : newProfile.protocol === "awg2"
          ? "AmneziaWG 2.0"
          : newProfile.protocol === "amneziawg"
          ? "Amnezia 3.1"
          : "Hysteria 2";
      const isProfileAwg = newProfile.protocol === "amneziawg" || newProfile.protocol === "awg2";
      const profToSave: VpnProfile = {
        ...newProfile,
        pinned_on_pc: true,
        awg_version: newProfile.protocol === "awg2" ? "2.0" : newProfile.protocol === "amneziawg" ? "3.1" : newProfile.awg_version,
        awg_private_key: isProfileAwg ? (newProfile.awg_private_key || newProfile.password) : newProfile.awg_private_key,
        awg_port_probing: isProfileAwg ? (newProfile.awg_port_probing ?? true) : newProfile.awg_port_probing,
        name:
          newProfile.name.trim() ||
          `${defaultLabel} (${newProfile.server.trim()})`,
      };
      const updated = await invoke<VpnProfile[]>("add_or_update_profile", {
        profile: profToSave,
      });
      setProfiles(updated);
      if (!selectedProfileId && updated.length > 0) {
        setSelectedProfileId(updated[updated.length - 1].id);
      }
      setShowNewProfileForm(false);
      showNotice("success", "Профиль сохранен");
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleDownloadAwgConf = async (profile: VpnProfile) => {
    try {
      const conf = await invoke<string>("export_awg_conf", {
        profileId: profile.id,
        profile,
      });
      const blob = new Blob([conf], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      // Чистое имя устройства без черточек, дефисов и протоколов для корректного имени интерфейса WireGuard/Amnezia
      const rawName = (profile.name || "vpn").split("—")[0].split("-")[0].trim();
      const cleanName = rawName.replace(/[^\w\d_а-яА-Я]/g, "") || "vpn";
      a.download = `${cleanName}.conf`;
      a.click();
      URL.revokeObjectURL(url);
      showNotice("success", `Файл конфигурации ${cleanName}.conf для Amnezia скачан`);
    } catch (err: any) {
      showNotice("error", `Ошибка экспорта .conf: ${String(err)}`);
    }
  };

  const handleProbeAwgPorts = async (profileId: string) => {
    setProbingAwgId(profileId);
    try {
      const prof =
        syncedServerProfiles.find((p) => p.id === profileId) ||
        profiles.find((p) => p.id === profileId) ||
        null;
      const results = await invoke<{ port: number; reachable: boolean; latency_ms?: number; message: string }[]>("probe_awg_ports", {
        profileId,
        profile: prof,
      });
      setAwgProbeResults((prev) => ({ ...prev, [profileId]: results }));
      showNotice("success", `Проверено ${results.length} портов AmneziaWG`);
    } catch (err: any) {
      showNotice("error", `Ошибка проверки портов: ${String(err)}`);
    } finally {
      setProbingAwgId(null);
    }
  };

  const handleDeleteProfile = async (id: string, fromServerBlock?: boolean) => {
    try {
      const prof =
        syncedServerProfiles.find((p) => p.id === id) ||
        profiles.find((p) => p.id === id);
      const shouldDeleteFromVps =
        prof &&
        isVpsAdmin &&
        vpsCreds.host.trim() &&
        vpsCreds.password.trim() &&
        (fromServerBlock || prof.server === vpsCreds.host.trim());

      let updated: VpnProfile[];
      if (shouldDeleteFromVps && prof) {
        showNotice("info", `Удаление «${prof.name}» с VPS сервера ${vpsCreds.host}...`);
        updated = await invoke<VpnProfile[]>("delete_vps_device_cmd", {
          creds: vpsCreds,
          profile: prof,
        });
      } else {
        updated = await invoke<VpnProfile[]>("delete_profile", { id });
      }
      setProfiles(updated);
      setSyncedServerProfiles((prev) =>
        prev.filter(
          (p) =>
            p.id !== id &&
            !(
              prof &&
              p.server === prof.server &&
              p.port === prof.port &&
              p.password === prof.password &&
              p.protocol === prof.protocol
            )
        )
      );
      if (selectedProfileId === id) {
        setSelectedProfileId(updated[0]?.id || "");
      }
      showNotice(
        "success",
        shouldDeleteFromVps
          ? `Конфигурация «${prof?.name}» удалена на сервере VPS и в приложении`
          : "Профиль удален"
      );
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handlePingProfile = async (id: string) => {
    setPingingId(id);
    try {
      const ms = await invoke<number>("ping_profile", { id });
      setProfiles((prev) =>
        prev.map((p) => (p.id === id ? { ...p, ping_ms: ms } : p))
      );
    } catch (err: any) {
      showNotice("error", `Пинг не удался: ${String(err)}`);
    } finally {
      setPingingId(null);
    }
  };

  const handleCopyUri = (profile: VpnProfile) => {
    navigator.clipboard.writeText(profile.uri);
    setCopiedId(profile.id);
    setTimeout(() => setCopiedId(null), 2000);
    showNotice("info", "Ссылка профиля скопирована в буфер обмена");
  };

  const handleOpenQrModal = async (profile: VpnProfile) => {
    try {
      const dataUrl = await QRCode.toDataURL(profile.uri, {
        width: 280,
        margin: 2,
        color: {
          dark: "#090d16",
          light: "#ffffff",
        },
      });
      setQrDataUrl(dataUrl);
      setQrModalProfile(profile);
    } catch (err: any) {
      showNotice("error", `Ошибка генерации QR-кода: ${String(err)}`);
    }
  };

  const handleExportJson = async (profile: VpnProfile) => {
    try {
      const savedPath = await invoke<string | null>("export_profile_json", {
        profileId: profile.id,
        profile,
      });
      if (savedPath) {
        showNotice("success", `Конфиг сохранен: ${savedPath}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleLoginVpsAdmin = async () => {
    if (!vpsCreds.host.trim() || !vpsCreds.password.trim()) {
      showNotice("error", "Введите IP-адрес сервера и пароль SSH (root)");
      return;
    }
    setAdminLoginLoading(true);
    try {
      const serverProfiles = await invoke<VpnProfile[]>("login_vps_admin", {
        creds: vpsCreds,
      });
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      setSyncedServerProfiles(serverProfiles);
      if (appState.profiles.length > 0 && !selectedProfileId) {
        setSelectedProfileId(appState.profiles[0].id);
      }
      setIsVpsAdmin(true);
      showNotice(
        "success",
        `Авторизация успешна! С сервера подтянуто устройств: ${serverProfiles.length}`
      );
    } catch (err: any) {
      showNotice("error", `Ошибка авторизации на VPS: ${String(err)}`);
    } finally {
      setAdminLoginLoading(false);
    }
  };

  const handleLogoutVpsAdmin = () => {
    setIsVpsAdmin(false);
    setSyncedServerProfiles([]);
    setVpsCreds((prev) => ({ ...prev, password: "" }));
    showNotice("info", "Вы вышли из режима управления VPS");
  };

  const handleCreateDeviceKey = async () => {
    if (!deviceNameInput.trim()) {
      showNotice("error", "Введите название устройства или имя друга");
      return;
    }
    if (!vpsCreds.host.trim()) {
      showNotice(
        "error",
        "Не указан IP-адрес VPS сервера. Выпуск ключей доступен только владельцу VPS."
      );
      return;
    }
    if (!vpsCreds.password.trim()) {
      showNotice(
        "error",
        "Для выпуска ключа на сервере требуется ввести SSH-пароль root. Доступ разрешен только владельцу сервера."
      );
      return;
    }
    setCreatingDevice(true);
    try {
      const created = await invoke<VpnProfile>("create_device_key", {
        creds: vpsCreds,
        deviceName: deviceNameInput.trim(),
        protocol: deviceProtocol,
      });
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      setSyncedServerProfiles((prev) => [...prev, created]);
      setDeviceNameInput("");
      showNotice("success", `Ключ для <${created.name}> успешно создан на VPS!`);
      await handleOpenQrModal(created);
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setCreatingDevice(false);
    }
  };

  const buildCurrentRoutingObject = (
    override?: Partial<RoutingConfig>
  ): RoutingConfig => ({
    ...routing,
    direct_domains: directDomainsText
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean),
    proxy_domains: proxyDomainsText
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean),
    direct_ips: directIpsText
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean),
    proxy_ips: proxyIpsText
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean),
    direct_processes: directProcessesList,
    proxy_processes: proxyProcessesList,
    ...override,
  });

  const syncAndApplyRoutingImmediately = async (nextRouting: RoutingConfig) => {
    const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
    setRouting(nextRouting);
    if (targetPid) {
      setProfiles((prev) =>
        prev.map((p) => (p.id === targetPid ? { ...p, routing: nextRouting } : p))
      );
    }
    try {
      if (status.connected) {
        const newStatus = await invoke<VpnStatus>("apply_routing_on_the_fly", {
          routing: nextRouting,
          profileId: targetPid || null,
          presetId: activePresetId || null,
        });
        setStatus(newStatus);
      } else {
        await invoke<RoutingConfig>("save_routing_config", {
          routing: nextRouting,
          profileId: targetPid || null,
        });
      }
    } catch (err) {
      console.error("Auto-apply routing error:", err);
    }
  };

  const addProcessItem = (target: "proxy" | "direct", value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return;
    const nextProxy =
      target === "proxy"
        ? proxyProcessesList.includes(trimmed)
          ? proxyProcessesList
          : [...proxyProcessesList, trimmed]
        : proxyProcessesList.filter((x) => x !== trimmed);
    const nextDirect =
      target === "direct"
        ? directProcessesList.includes(trimmed)
          ? directProcessesList
          : [...directProcessesList, trimmed]
        : directProcessesList.filter((x) => x !== trimmed);

    setProxyProcessesList(nextProxy);
    setDirectProcessesList(nextDirect);
    const nextRouting = buildCurrentRoutingObject({
      proxy_processes: nextProxy,
      direct_processes: nextDirect,
    });
    void syncAndApplyRoutingImmediately(nextRouting);
  };

  const removeProcessItem = (target: "proxy" | "direct", value: string) => {
    const nextProxy =
      target === "proxy"
        ? proxyProcessesList.filter((x) => x !== value)
        : proxyProcessesList;
    const nextDirect =
      target === "direct"
        ? directProcessesList.filter((x) => x !== value)
        : directProcessesList;

    setProxyProcessesList(nextProxy);
    setDirectProcessesList(nextDirect);
    const nextRouting = buildCurrentRoutingObject({
      proxy_processes: nextProxy,
      direct_processes: nextDirect,
    });
    void syncAndApplyRoutingImmediately(nextRouting);
  };

  const handlePickExeFile = async (target: "proxy" | "direct") => {
    try {
      const path = await invoke<string | null>("pick_exe_file");
      if (path) {
        addProcessItem(target, path);
        showNotice("info", `Добавлен файл: ${path.split("\\").pop()}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handlePickFolder = async (target: "proxy" | "direct") => {
    try {
      const folder = await invoke<string | null>("pick_folder");
      if (folder) {
        addProcessItem(target, folder);
        showNotice("info", `Добавлена папка: ${folder}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleOpenProcessModal = async (target: "proxy" | "direct") => {
    setProcModalTarget(target);
    setProcSearch("");
    setLoadingProcs(true);
    try {
      const list = await invoke<RunningProcessInfo[]>("get_running_processes");
      setRunningProcs(list);
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setLoadingProcs(false);
    }
  };

  const handleSaveRouting = async () => {
    const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
    const updatedRouting = buildCurrentRoutingObject();
    try {
      const saved = await invoke<RoutingConfig>("save_routing_config", {
        routing: updatedRouting,
        profileId: targetPid || null,
      });
      setRouting(saved);
      if (targetPid) {
        setProfiles((prev) =>
          prev.map((p) => (p.id === targetPid ? { ...p, routing: saved } : p))
        );
      }
      if (status.connected && selectedProfile && selectedProfile.id === targetPid) {
        const st = await invoke<VpnStatus>("apply_routing_on_the_fly", {
          routing: saved,
          profileId: targetPid || null,
          presetId: activePresetId || null,
        });
        setStatus(st);
        showNotice("success", "Правила сохранены и мгновенно применены к активному туннелю!");
      } else {
        showNotice("success", "Правила раздельного туннелирования для профиля сохранены");
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleSelectPreset = async (preset: RoutingPreset) => {
    setActivePresetId(preset.id);
    setProxyProcessesList(preset.proxy_processes);
    setDirectProcessesList(preset.direct_processes);
    const updatedRouting = buildCurrentRoutingObject({
      default_outbound: preset.default_outbound,
      proxy_processes: preset.proxy_processes,
      direct_processes: preset.direct_processes,
      split_tunneling: true,
    });
    setRouting(updatedRouting);

    const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
    if (targetPid) {
      setProfiles((prev) =>
        prev.map((p) => (p.id === targetPid ? { ...p, routing: updatedRouting } : p))
      );
    }
    setIsApplyingPreset(true);
    try {
      if (status.connected) {
        const newStatus = await invoke<VpnStatus>("apply_routing_preset", {
          presetId: preset.id,
          profileId: targetPid || null,
        });
        setStatus(newStatus);
        showNotice("success", `⚡ Предустановка «${preset.name}» мгновенно применена на лету!`);
      } else {
        const saved = await invoke<RoutingConfig>("save_routing_config", {
          routing: updatedRouting,
          profileId: targetPid || null,
        });
        setRouting(saved);
        if (targetPid) {
          setProfiles((prev) =>
            prev.map((p) => (p.id === targetPid ? { ...p, routing: saved } : p))
          );
        }
        await invoke("apply_routing_preset", {
          presetId: preset.id,
          profileId: targetPid || null,
        });
        showNotice("info", `Предустановка «${preset.name}» выбрана`);
      }
    } catch (err: any) {
      showNotice("error", `Ошибка применения предустановки: ${String(err)}`);
    } finally {
      setIsApplyingPreset(false);
    }
  };

  const handleSaveAsNewPreset = async () => {
    const trimmed = newPresetName.trim();
    if (!trimmed) {
      showNotice("error", "Введите название новой предустановки");
      return;
    }

    const newPreset: RoutingPreset = {
      id: `custom-preset-${Date.now()}`,
      name: trimmed,
      description: `Пользовательский набор (${proxyProcessesList.length} через VPN, ${directProcessesList.length} мимо VPN)`,
      default_outbound: routing.default_outbound || newPresetOutbound || "direct",
      proxy_processes: [...proxyProcessesList],
      direct_processes: [...directProcessesList],
      is_builtin: false,
    };

    try {
      const updatedList = await invoke<RoutingPreset[]>("save_routing_preset", {
        preset: newPreset,
      });
      setRoutingPresets(updatedList);
      setActivePresetId(newPreset.id);
      setShowSavePresetModal(false);
      setNewPresetName("");

      const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
      const updatedRouting = buildCurrentRoutingObject({
        default_outbound: newPreset.default_outbound,
        proxy_processes: newPreset.proxy_processes,
        direct_processes: newPreset.direct_processes,
      });
      setRouting(updatedRouting);
      if (targetPid) {
        setProfiles((prev) =>
          prev.map((p) => (p.id === targetPid ? { ...p, routing: updatedRouting } : p))
        );
      }

      if (status.connected) {
        const newStatus = await invoke<VpnStatus>("apply_routing_preset", {
          presetId: newPreset.id,
          profileId: targetPid || null,
        });
        setStatus(newStatus);
        showNotice("success", `⚡ Предустановка «${newPreset.name}» сохранена и применена на лету!`);
      } else {
        await invoke<RoutingConfig>("save_routing_config", {
          routing: updatedRouting,
          profileId: targetPid || null,
        });
        showNotice("success", `Предустановка «${newPreset.name}» успешно создана`);
      }
    } catch (err: any) {
      showNotice("error", `Ошибка сохранения предустановки: ${String(err)}`);
    }
  };

  const handleUpdateCurrentPreset = async () => {
    if (!activePresetId) return;
    const current = routingPresets.find((p) => p.id === activePresetId);
    if (!current) return;

    const updatedPreset: RoutingPreset = {
      ...current,
      default_outbound: routing.default_outbound || current.default_outbound,
      proxy_processes: [...proxyProcessesList],
      direct_processes: [...directProcessesList],
      is_builtin: false,
    };

    try {
      const updatedList = await invoke<RoutingPreset[]>("save_routing_preset", {
        preset: updatedPreset,
      });
      setRoutingPresets(updatedList);
      const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
      const updatedRouting = buildCurrentRoutingObject({
        default_outbound: updatedPreset.default_outbound,
        proxy_processes: updatedPreset.proxy_processes,
        direct_processes: updatedPreset.direct_processes,
      });
      setRouting(updatedRouting);
      if (targetPid) {
        setProfiles((prev) =>
          prev.map((p) => (p.id === targetPid ? { ...p, routing: updatedRouting } : p))
        );
      }

      if (status.connected) {
        const newStatus = await invoke<VpnStatus>("apply_routing_preset", {
          presetId: updatedPreset.id,
          profileId: targetPid || null,
        });
        setStatus(newStatus);
        showNotice("success", `⚡ Предустановка «${updatedPreset.name}» обновлена и применена на лету!`);
      } else {
        await invoke<RoutingConfig>("save_routing_config", {
          routing: updatedRouting,
          profileId: targetPid || null,
        });
        showNotice("success", `Предустановка «${updatedPreset.name}» обновлена`);
      }
    } catch (err: any) {
      showNotice("error", `Ошибка обновления: ${String(err)}`);
    }
  };

  const handleDeletePreset = async (presetId: string) => {
    try {
      const updatedList = await invoke<RoutingPreset[]>("delete_routing_preset", {
        presetId,
      });
      setRoutingPresets(updatedList);
      if (activePresetId === presetId) {
        setActivePresetId(null);
      }
      showNotice("info", "Предустановка удалена");
    } catch (err: any) {
      showNotice("error", `Ошибка удаления: ${String(err)}`);
    }
  };

  const handleResetPresets = async () => {
    try {
      const updatedList = await invoke<RoutingPreset[]>("reset_routing_presets");
      setRoutingPresets(updatedList);
      showNotice("success", "Стандартные предустановки восстановлены");
    } catch (err: any) {
      showNotice("error", `Ошибка: ${String(err)}`);
    }
  };

  const handleApplyOnTheFly = async () => {
    const targetPid = routingProfileId || selectedProfileId || profiles[0]?.id;
    const updatedRouting = buildCurrentRoutingObject();
    setRouting(updatedRouting);
    if (targetPid) {
      setProfiles((prev) =>
        prev.map((p) => (p.id === targetPid ? { ...p, routing: updatedRouting } : p))
      );
    }
    setIsApplyingPreset(true);
    try {
      if (status.connected) {
        const newStatus = await invoke<VpnStatus>("apply_routing_on_the_fly", {
          routing: updatedRouting,
          profileId: targetPid || null,
          presetId: activePresetId || null,
        });
        setStatus(newStatus);
        showNotice("success", "⚡ Правила маршрутизации применены к VPN на лету!");
      } else {
        const saved = await invoke<RoutingConfig>("save_routing_config", {
          routing: updatedRouting,
          profileId: targetPid || null,
        });
        setRouting(saved);
        if (targetPid) {
          setProfiles((prev) =>
            prev.map((p) => (p.id === targetPid ? { ...p, routing: saved } : p))
          );
        }
        showNotice("success", "Правила сохранены");
      }
    } catch (err: any) {
      showNotice("error", `Ошибка: ${String(err)}`);
    } finally {
      setIsApplyingPreset(false);
    }
  };

  const handleProbeListeningPorts = async (resetHistory = false) => {
    if (!selectedProfile) {
      showNotice("error", "Профиль подключения не выбран");
      return;
    }
    setIsProbingListeningPorts(true);
    try {
      const prevExcluded = resetHistory ? [] : (probedHistoryPorts[selectedProfile.id] || []);
      showNotice(
        "info",
        prevExcluded.length > 0
          ? `Поиск следующих 28 портов для «${selectedProfile.name}» (ранее проверено ${prevExcluded.length})...`
          : `Поиск 28 рабочих портов на сервере «${selectedProfile.name}» (${selectedProfile.server})...`
      );

      const results = await invoke<PortProbeResult[]>("probe_profile_ports", {
        profileId: selectedProfile.id,
        profile: selectedProfile,
        customPorts: null,
        excludePorts: prevExcluded,
      });

      setListeningProbeResults((prev) => ({
        ...prev,
        [selectedProfile.id]: results,
      }));

      const newHistory = [...prevExcluded, ...results.map((r) => r.port)];
      setProbedHistoryPorts((prev) => ({
        ...prev,
        [selectedProfile.id]: newHistory,
      }));

      const reachableCount = results.filter((r) => r.reachable).length;
      if (reachableCount > 0) {
        showNotice(
          "success",
          `Найдено ${reachableCount} работающих портов из ${results.length}! Нажмите на порт для применения.`
        );
      } else {
        showNotice(
          "info",
          `Проверено ${results.length} портов: ни один не ответил. Если NAT-переадресация на VPS отключена, работает только исходный порт (${selectedProfile.original_port || selectedProfile.port}).`
        );
      }
    } catch (err: any) {
      showNotice("error", `Ошибка проверки портов: ${String(err)}`);
    } finally {
      setIsProbingListeningPorts(false);
    }
  };

  const handleResetPortHistory = async () => {
    if (!selectedProfile) return;
    setProbedHistoryPorts((prev) => ({
      ...prev,
      [selectedProfile.id]: [],
    }));
    await handleProbeListeningPorts(true);
  };

  const handleApplyListeningPort = async (newPort: number) => {
    if (!selectedProfile) {
      showNotice("error", "Профиль подключения не выбран");
      return;
    }
    if (selectedProfile.port === newPort) {
      showNotice("info", `Порт ${newPort} уже установлен в конфиге «${selectedProfile.name}»`);
      return;
    }

    const currentResults = listeningProbeResults[selectedProfile.id] || [];
    const probeItem = currentResults.find((r) => r.port === newPort);
    if (probeItem && !probeItem.reachable) {
      const confirmed = window.confirm(
        `⚠️ Внимание: порт ${newPort} не ответил на проверку (вероятно закрыт или заблокирован).\n\nПодключение через него может зависнуть на этапе рукопожатия. Всё равно применить к «${selectedProfile.name}»?`
      );
      if (!confirmed) {
        return;
      }
    }

    setIsApplyingListeningPort(true);
    const updatedProfile: VpnProfile = {
      ...selectedProfile,
      port: newPort,
      original_port: selectedProfile.original_port || selectedProfile.port,
    };

    try {
      const updatedList = await invoke<VpnProfile[]>("add_or_update_profile", {
        profile: updatedProfile,
      });
      setProfiles(updatedList);

      if (status.connected && status.active_profile_id === selectedProfile.id) {
        showNotice("info", `Переподключение VPN на порт ${newPort}...`);
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: updatedProfile.id,
          mode: routing.mode,
          splitTunneling: routing.split_tunneling,
        });
        setStatus(st);
        showNotice("success", `⚡ Порт ${newPort} успешно применен к «${selectedProfile.name}», VPN переподключен!`);
      } else {
        showNotice("success", `Порт прослушивания ${newPort} применен к конфигу «${selectedProfile.name}»`);
      }
    } catch (err: any) {
      showNotice("error", `Ошибка применения порта: ${String(err)}`);
    } finally {
      setIsApplyingListeningPort(false);
    }
  };

  const renderProtocolBadge = (protocol: string, version?: string | null) => {
    const p = (protocol || "").toLowerCase();
    let label = "PROXY";
    let colorClass = "bg-slate-800 text-slate-100 border-slate-600/80";
    let dotClass = "bg-slate-400";

    if (p === "awg2" || (p === "amneziawg" && version === "2.0")) {
      label = "AWG 2.0";
      colorClass = "bg-teal-950/80 text-teal-300 border-teal-500/40 shadow-sm";
      dotClass = "bg-teal-400";
    } else if (p === "amneziawg") {
      label = "Amnezia 3.1";
      colorClass = "bg-emerald-950/80 text-emerald-300 border-emerald-500/40 shadow-sm";
      dotClass = "bg-emerald-400";
    } else if (p === "vless") {
      label = "VLESS Reality";
      colorClass = "bg-cyan-950/80 text-cyan-300 border-cyan-500/40 shadow-sm";
      dotClass = "bg-cyan-400";
    } else if (p === "hysteria2") {
      label = "Hysteria 2";
      colorClass = "bg-purple-950/80 text-purple-300 border-purple-500/40 shadow-sm";
      dotClass = "bg-purple-400";
    } else if (p === "tuic") {
      label = "TUIC v5";
      colorClass = "bg-indigo-950/80 text-indigo-300 border-indigo-500/40 shadow-sm";
      dotClass = "bg-indigo-400";
    } else if (p === "shadowsocks") {
      label = "Shadowsocks";
      colorClass = "bg-amber-950/80 text-amber-300 border-amber-500/40 shadow-sm";
      dotClass = "bg-amber-400";
    } else if (p === "ssh") {
      label = "SSH Tunnel";
      colorClass = "bg-violet-950/80 text-violet-300 border-violet-500/40 shadow-sm";
      dotClass = "bg-violet-400";
    }

    return (
      <span
        className={`px-2.5 py-1 rounded-lg border font-mono text-[10px] font-bold tracking-wider uppercase inline-flex items-center gap-1.5 shrink-0 ${colorClass}`}
      >
        <span className={`w-1.5 h-1.5 rounded-full ${dotClass}`} />
        {label}
      </span>
    );
  };

  return (
    <div
      data-theme={currentTheme.id}
      style={{
        ...cssVarsStyle,
        opacity: windowOpacity / 100,
      }}
      className={`flex h-screen w-screen overflow-hidden ${
        isLightTheme ? "text-slate-900" : "text-slate-100"
      } select-none bg-[#090d16] transition-opacity duration-150 relative`}
    >
      {/* Sidebar - collapsible (w-64 vs w-[68px]) */}
      <aside
        style={{ borderRightColor: "var(--color-accent-border)" }}
        className={`${
          sidebarCollapsed ? "w-[68px] px-2" : "w-64 px-3.5"
        } bg-[#0d1322] border-r flex flex-col justify-between py-2.5 shrink-0 transition-all duration-200 z-20`}
      >
        <div>
          {/* Brand Header & Collapse Toggle */}
          {sidebarCollapsed ? (
            <div className="flex flex-col items-center gap-2 mb-4">
              <div
                onMouseDown={handleMouseDownDrag}
                onDoubleClick={handleDoubleClickMaximize}
                data-tauri-drag-region
                className="w-10 h-10 rounded-xl overflow-hidden bg-[#0c1220] border border-cyan-500/40 p-1 flex items-center justify-center shadow-lg shrink-0 cursor-default"
                title="NeuroVeil VPN"
              >
                <img
                  src={appIconLogo}
                  alt="NeuroVeil VPN"
                  className="w-full h-full object-cover rounded-lg pointer-events-none"
                />
              </div>
              <button
                type="button"
                onClick={toggleSidebar}
                className="p-1.5 rounded-lg text-slate-400 hover:text-cyan-300 hover:bg-slate-800/80 transition cursor-pointer"
                title="Развернуть меню разделов"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-2 px-1 py-1 mb-4 select-none">
              <div
                onMouseDown={handleMouseDownDrag}
                onDoubleClick={handleDoubleClickMaximize}
                data-tauri-drag-region
                className="flex items-center gap-2.5 min-w-0 flex-1 cursor-default"
              >
                <div className="w-10 h-10 rounded-xl overflow-hidden bg-[#0c1220] border border-cyan-500/40 p-1 flex items-center justify-center shadow-lg shrink-0">
                  <img
                    src={appIconLogo}
                    alt="NeuroVeil VPN"
                    className="w-full h-full object-cover rounded-lg pointer-events-none"
                  />
                </div>
                <div className="pointer-events-none min-w-0">
                  <div className="flex items-center gap-1.5">
                    <h1 className="font-bold text-sm tracking-tight text-white leading-none truncate">
                      NeuroVeil VPN
                    </h1>
                  </div>
                  <span className="text-[9px] px-1.5 py-0.2 rounded font-mono font-bold bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 mt-1 inline-block">
                    v1.0.3
                  </span>
                </div>
              </div>

              <button
                type="button"
                onClick={toggleSidebar}
                className="p-1.5 rounded-lg text-slate-400 hover:text-cyan-300 hover:bg-slate-800/80 transition cursor-pointer shrink-0"
                title="Свернуть меню (оставить только иконки)"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
            </div>
          )}

          {/* Nav Links with Seamless Sliding Tab Connector */}
          <nav className="relative flex flex-col gap-1.5">
            {/* Sliding Active Tab Indicator merging into Main Panel */}
            <div
              style={{
                transform: `translateY(${
                  Math.max(
                    0,
                    ["dashboard", "vps", "profiles", "routing", "settings", "about"].indexOf(
                      activeTab
                    )
                  ) * 48
                }px)`,
                transition: "transform 340ms cubic-bezier(0.22, 1, 0.36, 1)",
                background:
                  "linear-gradient(90deg, var(--nav-grad-from) 0%, var(--bg-app) 76%, var(--bg-app) 100%)",
                borderTop: "1px solid var(--color-accent-border)",
                borderLeft: "1px solid var(--color-accent-border)",
                borderBottom: "1px solid var(--color-accent-border)",
              }}
              className={`pointer-events-none absolute top-0 left-0 ${
                sidebarCollapsed ? "-right-[9px]" : "-right-[15px]"
              } h-[42px] rounded-l-2xl z-0`}
            >
              {/* Top-Right Concave Curve */}
              <svg
                width="14"
                height="14"
                viewBox="0 0 14 14"
                className="absolute -top-[14px] right-0 block overflow-visible pointer-events-none"
              >
                <path
                  d="M 14 0 L 14 14 L 0 14 L 0 13.5 A 13.5 13.5 0 0 0 13.5 0 Z"
                  fill="var(--bg-app)"
                />
                <path
                  d="M 13.5 0 A 13.5 13.5 0 0 1 0 13.5"
                  fill="none"
                  stroke="var(--color-accent-border)"
                  strokeWidth="1.2"
                />
              </svg>

              {/* Bottom-Right Concave Curve */}
              <svg
                width="14"
                height="14"
                viewBox="0 0 14 14"
                className="absolute -bottom-[14px] right-0 block overflow-visible pointer-events-none"
              >
                <path
                  d="M 0 0 L 14 0 L 14 14 L 13.5 14 A 13.5 13.5 0 0 0 0 0.5 Z"
                  fill="var(--bg-app)"
                />
                <path
                  d="M 0 0.5 A 13.5 13.5 0 0 1 13.5 14"
                  fill="none"
                  stroke="var(--color-accent-border)"
                  strokeWidth="1.2"
                />
              </svg>
            </div>

            <button
              onClick={() => setActiveTab("dashboard")}
              title={sidebarCollapsed ? t.navDashboard : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "dashboard"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Power className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navDashboard}</span>}
              </span>
              {!sidebarCollapsed && (
                <span
                  className={`w-2 h-2 rounded-full ${
                    status.connected
                      ? status.is_handshaking
                        ? "bg-amber-400 shadow-[0_0_8px_#fbbf24] animate-ping"
                        : "bg-emerald-400 shadow-[0_0_8px_#34d399]"
                      : "bg-slate-600"
                  }`}
                />
              )}
            </button>

            <button
              onClick={() => setActiveTab("vps")}
              title={sidebarCollapsed ? `${t.navVps} (SSH)` : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "vps"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Server className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navVps}</span>}
              </span>
              {!sidebarCollapsed && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">
                  SSH
                </span>
              )}
            </button>

            <button
              onClick={() => setActiveTab("profiles")}
              title={sidebarCollapsed ? `${t.navProfiles} (${profiles.length})` : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "profiles"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Smartphone className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navProfiles}</span>}
              </span>
              {!sidebarCollapsed && (
                <span className="text-xs px-2 py-0.5 rounded-full bg-slate-800 text-slate-400">
                  {profiles.length}
                </span>
              )}
            </button>

            <button
              onClick={() => setActiveTab("routing")}
              title={sidebarCollapsed ? t.navRouting : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "routing"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Globe className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navRouting}</span>}
              </span>
            </button>

            <button
              onClick={() => setActiveTab("settings")}
              title={sidebarCollapsed ? t.navSettings : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "settings"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Settings className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navSettings}</span>}
              </span>
            </button>

            <button
              onClick={() => setActiveTab("about")}
              title={sidebarCollapsed ? t.navAbout : undefined}
              className={`relative z-10 w-full h-[42px] flex items-center ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between px-3.5"
              } rounded-xl text-sm font-medium transition-colors duration-200 cursor-pointer ${
                activeTab === "about"
                  ? "text-cyan-300 font-semibold"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/40"
              }`}
            >
              <span className={`flex items-center ${sidebarCollapsed ? "justify-center" : "gap-3"}`}>
                <Info className="w-4 h-4 shrink-0" />
                {!sidebarCollapsed && <span>{t.navAbout}</span>}
              </span>
            </button>
          </nav>
        </div>

        {/* Bottom Core Status Card */}
        {!sidebarCollapsed ? (
          <div className="p-3 rounded-xl bg-[#131c31]/80 border border-slate-800 space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <span className="text-slate-400">Ядро sing-box:</span>
              <span
                className={`font-medium flex items-center gap-1 ${
                  status.singbox_installed ? "text-emerald-400" : "text-amber-400"
                }`}
              >
                {status.singbox_installed ? (
                  <>
                    <CheckCircle2 className="w-3.5 h-3.5" /> Готово
                  </>
                ) : (
                  <>
                    <AlertTriangle className="w-3.5 h-3.5" /> Не скачано
                  </>
                )}
              </span>
            </div>
            <div className="text-[11px] text-slate-500 truncate" title={status.singbox_version}>
              {status.singbox_version}
            </div>
            {!status.singbox_installed && (
              <button
                onClick={handleDownloadCore}
                disabled={downloadingCore}
                className="w-full py-1.5 px-3 rounded-lg bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-xs font-semibold text-white transition flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Download className="w-3.5 h-3.5" />
                {downloadingCore ? "Загрузка..." : "Скачать sing-box"}
              </button>
            )}
          </div>
        ) : (
          <div
            className="p-2.5 rounded-xl bg-[#131c31]/80 border border-slate-800 flex items-center justify-center cursor-default"
            title={`Ядро sing-box: ${status.singbox_installed ? "Готово (" + (status.singbox_version || "") + ")" : "Не скачано"}`}
          >
            <span
              className={`w-3 h-3 rounded-full ${
                status.singbox_installed
                  ? "bg-emerald-400 shadow-[0_0_8px_#34d399]"
                  : "bg-amber-400 shadow-[0_0_8px_#fbbf24] animate-pulse"
              }`}
            />
          </div>
        )}
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col h-full overflow-hidden relative bg-[#090d16]">
        {/* Seamless Draggable Header - NO borders, NO divider lines */}
        <header className="h-9 w-full flex items-center justify-end px-3 pt-1 shrink-0 select-none cursor-default z-30">
          {/* Draggable space spanning the entire width */}
          <div
            className="flex-1 h-full"
            onMouseDown={handleMouseDownDrag}
            onDoubleClick={handleDoubleClickMaximize}
            data-tauri-drag-region
          />

          {/* Window Control Buttons */}
          <div
            className="flex items-center space-x-1 shrink-0 -mr-1 z-40 relative"
            data-tauri-drag-region="false"
            onMouseDown={(e) => e.stopPropagation()}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              data-tauri-drag-region="false"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={async (e) => {
                e.stopPropagation();
                try {
                  await invoke("app_minimize");
                } catch (err) {
                  console.error("Minimize error:", err);
                }
              }}
              className="w-8 h-7 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition cursor-pointer"
              title={t.minimize}
            >
              <Minus className="w-3.5 h-3.5 pointer-events-none" />
            </button>
            <button
              type="button"
              data-tauri-drag-region="false"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={async (e) => {
                e.stopPropagation();
                try {
                  await invoke("app_toggle_maximize");
                } catch (err) {
                  console.error("Maximize error:", err);
                }
              }}
              className="w-8 h-7 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition cursor-pointer"
              title={t.maximize}
            >
              <Square className="w-3 h-3 pointer-events-none" />
            </button>
            <button
              type="button"
              data-tauri-drag-region="false"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={async (e) => {
                e.stopPropagation();
                try {
                  await invoke("app_close");
                } catch (err) {
                  console.error("Close error:", err);
                }
              }}
              className="w-8 h-7 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-rose-600 transition cursor-pointer"
              title={t.close}
            >
              <X className="w-3.5 h-3.5 pointer-events-none" />
            </button>
          </div>
        </header>

        {/* Toast Banner - High Contrast Theme-Safe */}
        {toast && (
          <div
            className={`toast-notice absolute top-4 right-6 z-50 flex items-center gap-2.5 px-4 py-3 rounded-xl shadow-2xl border text-sm font-semibold transition-all backdrop-blur-md ${
              toast.type === "success"
                ? "bg-emerald-700/95 border-emerald-400 text-white shadow-emerald-500/30"
                : toast.type === "error"
                ? "bg-rose-700/95 border-rose-400 text-white shadow-rose-500/30"
                : "bg-slate-900/95 border-cyan-400 text-cyan-100 shadow-cyan-500/30"
            }`}
          >
            {toast.type === "success" && <CheckCircle2 className="w-4 h-4 text-emerald-200 shrink-0" />}
            {toast.type === "error" && <XCircle className="w-4 h-4 text-rose-200 shrink-0" />}
            {toast.type === "info" && <Sparkles className="w-4 h-4 text-cyan-300 shrink-0" />}
            <span className="text-white font-medium">{toast.text}</span>
            <button
              onClick={() => setToast(null)}
              className="ml-2 text-white/70 hover:text-white transition p-0.5"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* TAB 1: DASHBOARD */}
        {activeTab === "dashboard" && (
          <div className="flex-1 overflow-y-auto p-6 flex flex-col justify-between h-full min-h-0 space-y-6 animate-tab-enter">
            <div className="space-y-6">
              {/* Warning if sing-box binary is missing */}
              {!status.singbox_installed && (
                <div className="p-4 rounded-2xl bg-amber-500/10 border border-amber-500/30 flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <AlertTriangle className="w-6 h-6 text-amber-400 shrink-0" />
                    <div>
                      <h3 className="text-sm font-semibold text-amber-200">
                        Для запуска VPN требуется ядро sing-box.exe
                      </h3>
                      <p className="text-xs text-amber-300/80">
                        Нажмите кнопку справа, чтобы клиент автоматически скачал последнюю официальную сборку sing-box для Windows с GitHub.
                      </p>
                    </div>
                  </div>
                  <button
                    onClick={handleDownloadCore}
                    disabled={downloadingCore}
                    className="px-4 py-2 rounded-xl bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-slate-950 font-semibold text-xs shrink-0 transition flex items-center gap-2"
                  >
                    <Download className="w-4 h-4" />
                    {downloadingCore ? "Скачивание..." : "Скачать ядро (1 клик)"}
                  </button>
                </div>
              )}

              <div className="grid grid-cols-12 gap-4">
                {/* Left Column: Power Button & Speed Meter */}
                <div className="col-span-5 p-3.5 rounded-2xl bg-[#111827] border border-slate-800/90 flex flex-col justify-between space-y-2.5 relative overflow-hidden">
                  {/* Modern Main Action Button - Fills the top space completely */}
                  <button
                    onClick={handleToggleConnect}
                    disabled={connecting || reconnectCooldown}
                    className={`w-full h-[64px] rounded-xl px-3.5 flex items-center justify-between gap-3 transition-all duration-300 cursor-pointer border relative overflow-hidden group shadow-md keep-white ${
                      status.connected
                        ? status.is_handshaking
                          ? "bg-gradient-to-r from-amber-600 via-orange-600 to-amber-700 text-white border-amber-400/60 shadow-amber-500/25 ring-1 ring-amber-500/30"
                          : "bg-gradient-to-r from-emerald-600 via-teal-600 to-emerald-700 text-white border-emerald-400/60 shadow-emerald-500/25 ring-1 ring-emerald-500/20"
                        : "bg-gradient-to-r from-slate-800 via-[#161f33] to-slate-800 hover:from-cyan-900/40 hover:to-indigo-900/40 text-white border-slate-700 hover:border-cyan-500/50"
                    }`}
                  >
                    {status.connected && (
                      <div
                        className={`absolute inset-0 ${
                          status.is_handshaking ? "bg-amber-400/15" : "bg-emerald-400/10"
                        } animate-pulse pointer-events-none`}
                      />
                    )}

                    <div className="flex items-center gap-3 relative z-10 w-full justify-between">
                      <div className="flex items-center gap-3 min-w-0">
                        <div
                          className={`w-10 h-10 rounded-xl flex items-center justify-center shrink-0 transition-transform group-hover:scale-105 shadow-inner ${
                            status.connected
                              ? status.is_handshaking
                                ? "bg-amber-500 text-white shadow-amber-700/50"
                                : "bg-emerald-500 text-white shadow-emerald-700/50"
                              : "bg-slate-700/80 group-hover:bg-cyan-500 text-white"
                          }`}
                        >
                          {connecting ? (
                            <RefreshCw className="w-5 h-5 animate-spin text-cyan-300" />
                          ) : status.connected ? (
                            status.is_handshaking ? (
                              <Activity className="w-5 h-5 animate-pulse text-amber-100" />
                            ) : (
                              <ShieldCheck className="w-5 h-5" />
                            )
                          ) : (
                            <Shield className="w-5 h-5" />
                          )}
                        </div>
                        <div className="text-left min-w-0">
                          <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-300 group-hover:text-cyan-200 truncate">
                            {connecting
                              ? "Подождите..."
                              : reconnectCooldown
                              ? "Защитная пауза WireGuard"
                              : status.connected
                              ? status.is_handshaking
                                ? "Рукопожатие WireGuard"
                                : status.active_profile_name || "VPN Подключен"
                              : "VPN Выключен"}
                          </div>
                          <div className="text-sm font-bold text-white leading-tight truncate mt-0.5">
                            {connecting
                              ? "ИНИЦИАЛИЗАЦИЯ..."
                              : reconnectCooldown
                              ? "ПАУЗА..."
                              : status.connected
                              ? status.is_handshaking
                                ? "ОТКЛЮЧИТЬ"
                                : "ОТКЛЮЧИТЬ"
                              : "ПОДКЛЮЧИТЬСЯ"}
                          </div>
                        </div>
                      </div>

                      <div className="shrink-0 flex items-center gap-1.5 pl-2">
                        <span
                          className={`w-2.5 h-2.5 rounded-full ${
                            status.connected
                              ? status.is_handshaking
                                ? "bg-amber-300 animate-ping"
                                : "bg-emerald-300 shadow-[0_0_8px_#34d399]"
                              : "bg-slate-500"
                          }`}
                        />
                      </div>
                    </div>
                  </button>

                  {/* Compact Live Speed Meter */}
                  <div className="grid grid-cols-2 gap-2 pt-1.5 border-t border-slate-800/80">
                    <div className="px-2 py-1 rounded-lg bg-[#0b101d] border border-slate-800/70 flex items-center gap-1.5 min-w-0">
                      <Download className="w-3 h-3 text-cyan-400 shrink-0" />
                      <div className="min-w-0">
                        <div className="text-[9px] text-slate-400 leading-none">Входящий</div>
                        <div className="text-[11px] font-bold text-white truncate font-mono mt-0.5">
                          {formatBytes(status.down_speed)}/с
                        </div>
                      </div>
                    </div>

                    <div className="px-2 py-1 rounded-lg bg-[#0b101d] border border-slate-800/70 flex items-center gap-1.5 min-w-0">
                      <Upload className="w-3 h-3 text-indigo-400 shrink-0" />
                      <div className="min-w-0">
                        <div className="text-[9px] text-slate-400 leading-none">Исходящий</div>
                        <div className="text-[11px] font-bold text-white truncate font-mono mt-0.5">
                          {formatBytes(status.up_speed)}/с
                        </div>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Right Column: Server Selector */}
                <div className="col-span-7">
                  {/* Active Profile Selection Card */}
                  <div className="p-3.5 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-2.5">
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-semibold uppercase tracking-wider text-slate-400 flex items-center gap-1.5">
                        <Layers className="w-3.5 h-3.5 text-cyan-400" />
                        Выбранный сервер и протокол
                      </span>
                      <div className="flex items-center gap-3">
                        <button
                          onClick={() => setActiveTab("profiles")}
                          className="text-xs text-slate-400 hover:text-slate-200 font-medium transition cursor-pointer"
                        >
                          Управление ({effectiveDashboardProfiles.length})
                        </button>
                        <button
                          onClick={() => setActiveTab("vps")}
                          className="text-xs text-cyan-400 hover:text-cyan-300 font-medium flex items-center gap-1 cursor-pointer"
                        >
                          <Plus className="w-3.5 h-3.5" /> Добавить VPS
                        </button>
                      </div>
                    </div>

                    {!selectedProfile ? (
                      <div className="p-3 rounded-xl bg-[#0b101d] border border-dashed border-slate-700 text-center space-y-1.5">
                        <p className="text-xs text-slate-400">
                          Нет сохраненных профилей.
                        </p>
                        <div className="flex justify-center gap-2">
                          <button
                            onClick={() => setActiveTab("vps")}
                            className="px-3 py-1 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-xs font-semibold text-white transition"
                          >
                            Автодеплой
                          </button>
                          <button
                            onClick={() => setActiveTab("profiles")}
                            className="px-3 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 transition"
                          >
                            Вручную
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="max-h-[108px] overflow-y-auto space-y-1.5 pr-1 custom-scrollbar">
                        {effectiveDashboardProfiles.map((p) => {
                          const isCurrentSelected = p.id === (status.active_profile_id || selectedProfileId);
                          const isLiveConnected = status.connected && status.active_profile_id === p.id;

                          return (
                            <div
                              key={p.id}
                              onClick={() => handleQuickSwitchProfile(p.id)}
                              className={`p-2 rounded-xl border cursor-pointer transition flex items-center justify-between gap-2.5 group ${
                                isCurrentSelected
                                  ? "bg-cyan-500/10 border-cyan-500/60 shadow-sm ring-1 ring-cyan-500/30"
                                  : "bg-[#0b101d] border-slate-800/80 hover:border-slate-700 hover:bg-slate-800/40"
                              }`}
                            >
                              <div className="flex items-center gap-2.5 min-w-0">
                                {renderProtocolBadge(p.protocol, p.awg_version)}

                                <div className="min-w-0">
                                  <div className="text-xs font-bold text-white flex items-center gap-1.5 truncate">
                                    <span className="truncate">{p.name}</span>
                                    {isLiveConnected && (
                                      <span className="text-[9px] px-1.5 py-0.2 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 animate-pulse font-mono font-medium shrink-0">
                                        В эфире
                                      </span>
                                    )}
                                  </div>
                                  <div className="text-[10px] text-slate-400 font-mono truncate flex items-center gap-1.5">
                                    <span>
                                      {p.server}:{status.active_port && isLiveConnected ? status.active_port : p.port}
                                    </span>
                                    <span
                                      className="text-[9px] px-1 py-0.2 rounded bg-slate-900/90 text-cyan-300/90 border border-slate-700/80 font-mono"
                                      title={`Трафик: Отдано ${formatBytes(p.total_up_bytes || 0)}, Скачано ${formatBytes(p.total_down_bytes || 0)}`}
                                    >
                                      ↓{formatBytes(p.total_down_bytes || 0)} • ↑{formatBytes(p.total_up_bytes || 0)}
                                    </span>
                                  </div>
                                </div>
                              </div>

                              <div className="flex items-center gap-2 shrink-0">
                                {p.ping_ms != null ? (
                                  <span className="text-[10px] font-mono text-emerald-400 font-semibold">
                                    {p.ping_ms} мс
                                  </span>
                                ) : (
                                  <button
                                    type="button"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handlePingProfile(p.id);
                                    }}
                                    disabled={pingingId === p.id}
                                    className="text-[9px] px-1.5 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-slate-200 transition"
                                  >
                                    {pingingId === p.id ? "..." : "Пинг"}
                                  </button>
                                )}

                                <div
                                  className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center transition-colors ${
                                    isCurrentSelected
                                      ? isLiveConnected
                                        ? "border-emerald-400 bg-emerald-500"
                                        : "border-cyan-400 bg-cyan-500"
                                      : "border-slate-600 bg-transparent group-hover:border-slate-500"
                                  }`}
                                >
                                  {isCurrentSelected && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* ROUTING PRESETS QUICK SELECTOR (Dashboard) */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-cyan-500/30 shadow-[0_0_20px_-10px_rgba(6,182,212,0.15)] space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex items-center gap-2.5">
                    <div className="p-2 rounded-xl bg-cyan-500/15 border border-cyan-500/30 text-cyan-400">
                      <Bookmark className="w-4 h-4" />
                    </div>
                    <div>
                      <div className="text-xs font-bold uppercase tracking-wider text-white flex items-center gap-2">
                        <span>Предустановки маршрутизации</span>
                        {status.connected && (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 flex items-center gap-1 font-semibold animate-pulse">
                            <Zap className="w-3 h-3" /> На лету ⚡
                          </span>
                        )}
                      </div>
                      <p className="text-[11px] text-slate-400">
                        Переключайте списки программ в 1 клик без смены конфигов и разрыва соединения
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    {status.connected && (
                      <button
                        type="button"
                        onClick={handleApplyOnTheFly}
                        disabled={isApplyingPreset}
                        className="px-3 py-1.5 rounded-xl bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/40 text-xs font-semibold text-emerald-300 flex items-center gap-1.5 transition cursor-pointer"
                        title="Применить текущие списки программ к работающему VPN"
                      >
                        <Zap className={`w-3.5 h-3.5 ${isApplyingPreset ? "animate-spin" : ""}`} />
                        <span>Применить к VPN ⚡</span>
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => {
                        if (selectedProfileId) switchRoutingProfile(selectedProfileId);
                        setActiveTab("routing");
                      }}
                      className="px-3 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs font-medium text-slate-200 flex items-center gap-1.5 transition cursor-pointer"
                    >
                      <span>Настройка правил →</span>
                    </button>
                  </div>
                </div>

                {/* Preset Chips */}
                <div className="flex flex-wrap items-center gap-2 pt-1">
                  {routingPresets.map((pr) => {
                    const isActive = activePresetId === pr.id;
                    const totalCount =
                      (pr.proxy_processes || []).length +
                      (pr.direct_processes || []).length;
                    return (
                      <div
                        key={pr.id}
                        onClick={() => handleSelectPreset(pr)}
                        className={`group relative flex items-center gap-2 px-3 py-2 rounded-xl border text-xs font-medium transition cursor-pointer select-none ${
                          isActive
                            ? "bg-gradient-to-r from-cyan-600/25 to-indigo-600/25 border-cyan-400 text-white shadow-md shadow-cyan-500/20"
                            : "bg-[#0b101d] border-slate-800 text-slate-300 hover:border-slate-700 hover:text-white"
                        }`}
                      >
                        {isActive ? (
                          <span className="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_8px_#22d3ee] animate-pulse" />
                        ) : (
                          <span className="w-1.5 h-1.5 rounded-full bg-slate-600 group-hover:bg-slate-400" />
                        )}
                        {pr.id.includes("games") ? (
                          <Gamepad2 className="w-3.5 h-3.5 text-emerald-400" />
                        ) : pr.id.includes("discord") ? (
                          <MessageSquare className="w-3.5 h-3.5 text-indigo-400" />
                        ) : pr.id.includes("browser") ? (
                          <Globe className="w-3.5 h-3.5 text-cyan-400" />
                        ) : pr.id.includes("media") ? (
                          <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                        ) : (
                          <Zap className="w-3.5 h-3.5 text-violet-400" />
                        )}
                        <span className="font-semibold">{pr.name}</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-900/80 text-slate-400 border border-slate-800">
                          {pr.default_outbound === "direct"
                            ? "Только выбор"
                            : "Весь трафик"}
                        </span>
                        <span className="text-[10px] font-mono text-cyan-400/90 font-bold">
                          {totalCount}
                        </span>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeletePreset(pr.id);
                          }}
                          className="ml-1 text-slate-500 hover:text-rose-400 p-0.5 opacity-60 group-hover:opacity-100 transition"
                          title="Удалить пресет"
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </div>
                    );
                  })}
                </div>

                {/* Active Preset Description Banner */}
                <div className="pt-1 text-[11px] text-slate-400 flex items-center justify-between border-t border-slate-800/80">
                  <div className="flex items-center gap-1.5 truncate">
                    <span className="text-cyan-400 font-medium">Активный профиль:</span>
                    <span className="text-slate-300">
                      {routingPresets.find((p) => p.id === activePresetId)?.description ||
                        routingPresets.find((p) => p.id === activePresetId)?.name ||
                        "По умолчанию"}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={handleResetPresets}
                    className="text-[10px] text-slate-400 hover:text-cyan-300 transition shrink-0 ml-2"
                    title="Сбросить все предустановки к исходным стандартным"
                  >
                    Восстановить стандартные
                  </button>
                </div>
              </div>

              {/* LISTENING PORT DETECTOR (Dashboard) */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-cyan-500/30 shadow-[0_0_20px_-10px_rgba(6,182,212,0.15)] space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex items-center gap-2.5">
                    <div className="p-2 rounded-xl bg-cyan-500/15 border border-cyan-500/30 text-cyan-400">
                      <Radio className="w-4 h-4" />
                    </div>
                    <div>
                      <div className="text-xs font-bold uppercase tracking-wider text-white flex flex-wrap items-center gap-2">
                        <span>Определение порта прослушивания</span>
                        {selectedProfile && (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-mono font-bold">
                            Порт: {selectedProfile.port}
                          </span>
                        )}
                        {selectedProfile && selectedProfile.original_port && selectedProfile.port !== selectedProfile.original_port && (
                          <button
                            type="button"
                            onClick={() => handleApplyListeningPort(selectedProfile.original_port!)}
                            disabled={isApplyingListeningPort}
                            className="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 font-semibold flex items-center gap-1 transition cursor-pointer select-none"
                            title={`Вернуть исходный рабочий порт сервера (${selectedProfile.original_port})`}
                          >
                            <RotateCcw className="w-2.5 h-2.5" />
                            <span>Вернуть исходный порт ({selectedProfile.original_port})</span>
                          </button>
                        )}
                        {(probedHistoryPorts[selectedProfile?.id || ""] || []).length > 0 && (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/15 text-indigo-300 border border-indigo-500/30 font-mono">
                            Всего проверено: {(probedHistoryPorts[selectedProfile?.id || ""] || []).length}
                          </span>
                        )}
                      </div>
                      <p className="text-[11px] text-slate-400">
                        Поиск работающих портов прослушивания и быстрое применение к выбранному конфигу
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    {(probedHistoryPorts[selectedProfile?.id || ""] || []).length > 0 && (
                      <button
                        type="button"
                        onClick={handleResetPortHistory}
                        disabled={isProbingListeningPorts}
                        className="p-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-cyan-300 border border-slate-700 transition cursor-pointer"
                        title="Сбросить историю и начать поиск портов с начала"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => handleProbeListeningPorts(false)}
                      disabled={isProbingListeningPorts || !selectedProfile}
                      className={`px-3 py-1.5 rounded-xl text-xs font-semibold flex items-center gap-1.5 transition cursor-pointer select-none shadow-md ${
                        isProbingListeningPorts
                          ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 cursor-wait"
                          : "bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-white shadow-cyan-500/20 active:scale-95"
                      }`}
                      title={
                        (probedHistoryPorts[selectedProfile?.id || ""] || []).length > 0
                          ? "Найти следующие 28 уникальных портов, которые еще не отображались"
                          : "Выполнить поиск 28 работающих портов на сервере для выбранного профиля"
                      }
                    >
                      {isProbingListeningPorts ? (
                        <>
                          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                          <span>Поиск новых портов...</span>
                        </>
                      ) : (probedHistoryPorts[selectedProfile?.id || ""] || []).length > 0 ? (
                        <>
                          <Search className="w-3.5 h-3.5" />
                          <span>Определить еще порты ⚡</span>
                        </>
                      ) : (
                        <>
                          <Search className="w-3.5 h-3.5" />
                          <span>Определить рабочий порт</span>
                        </>
                      )}
                    </button>
                  </div>
                </div>

                {/* Render Port Icons */}
                {isProbingListeningPorts ? (
                  <div className="p-4 rounded-xl bg-[#0b101d] border border-cyan-500/20 flex items-center justify-center gap-2.5 text-xs text-cyan-300 animate-pulse">
                    <RefreshCw className="w-4 h-4 animate-spin text-cyan-400" />
                    <span>
                      Тестирование 28 портов сервера {selectedProfile ? `«${selectedProfile.name}» (${selectedProfile.server})` : ""}...
                    </span>
                  </div>
                ) : listeningProbeResults[selectedProfile?.id || ""] && listeningProbeResults[selectedProfile?.id || ""].length > 0 ? (
                  <div className="space-y-2 pt-1">
                    {listeningProbeResults[selectedProfile?.id || ""].filter((r) => r.reachable).length === 0 && (
                      <div className="p-3 rounded-xl bg-amber-500/10 border border-amber-500/30 flex items-start gap-2.5 text-xs text-amber-200">
                        <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                        <div className="space-y-1">
                          <div className="font-semibold text-amber-300">Сервер не отвечает на проверенные порты</div>
                          <div className="text-[11px] text-amber-300/80 leading-relaxed">
                            Возможно, сервер выключен или на VPS не настроена переадресация портов (NAT iptables).
                            {selectedProfile?.original_port && selectedProfile.port !== selectedProfile.original_port && (
                              <span> Рекомендуется вернуть исходный порт сервера: <strong className="text-white">{selectedProfile.original_port}</strong>.</span>
                            )}
                          </div>
                        </div>
                      </div>
                    )}

                    <div className="flex items-center justify-between text-[11px] text-slate-400 px-0.5">
                      <span className="flex items-center gap-1.5">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_#10b981]" />
                        Нажмите на иконку порта для применения к конфигу:
                      </span>
                      <span className="text-[10px] text-slate-400 font-mono">
                        Отображено: {listeningProbeResults[selectedProfile?.id || ""].length} • Работает: <span className="text-emerald-400 font-bold">{listeningProbeResults[selectedProfile?.id || ""].filter((r) => r.reachable).length}</span>
                      </span>
                    </div>

                    <div className="flex flex-wrap items-center gap-2 max-h-56 overflow-y-auto pr-1">
                      {listeningProbeResults[selectedProfile?.id || ""].map((res) => {
                        const isCurrent = selectedProfile?.port === res.port;
                        const isReachable = res.reachable;
                        return (
                          <button
                            key={res.port}
                            type="button"
                            onClick={() => handleApplyListeningPort(res.port)}
                            disabled={isApplyingListeningPort}
                            className={`group relative flex items-center gap-2 px-3 py-2 rounded-xl border text-xs font-medium transition cursor-pointer select-none ${
                              isCurrent
                                ? "bg-gradient-to-r from-cyan-600/30 to-indigo-600/30 border-cyan-400 text-white shadow-md shadow-cyan-500/25 ring-1 ring-cyan-400"
                                : isReachable
                                ? "bg-[#0b101d] border-emerald-500/35 hover:border-emerald-400 text-emerald-200 hover:text-white hover:bg-emerald-500/10 hover:shadow-sm"
                                : "bg-[#080d18] border-slate-800/80 text-slate-500 hover:border-slate-700 hover:text-slate-400 opacity-60"
                            }`}
                            title={`${res.port}: ${res.message}${res.latency_ms ? ` (${res.latency_ms} мс)` : ""}. Нажмите для применения к «${selectedProfile?.name}»`}
                          >
                            {isCurrent ? (
                              <div className="w-4 h-4 rounded-full bg-cyan-500/20 border border-cyan-400 flex items-center justify-center shrink-0">
                                <Check className="w-2.5 h-2.5 text-cyan-300" />
                              </div>
                            ) : isReachable ? (
                              <div className="w-4 h-4 rounded-full bg-emerald-500/20 border border-emerald-500/40 flex items-center justify-center shrink-0">
                                <Zap className="w-2.5 h-2.5 text-emerald-400" />
                              </div>
                            ) : (
                              <div className="w-4 h-4 rounded-full bg-slate-800 border border-slate-700 flex items-center justify-center shrink-0">
                                <XCircle className="w-2.5 h-2.5 text-slate-500" />
                              </div>
                            )}

                            <div className="flex flex-col items-start leading-tight">
                              <span className="font-mono font-bold text-[12px] tracking-wide">
                                {res.port}
                              </span>
                              {res.latency_ms != null && isReachable && (
                                <span className="text-[9px] text-emerald-400 font-mono">
                                  {res.latency_ms} мс
                                </span>
                              )}
                            </div>

                            {isCurrent && (
                              <span className="text-[9px] px-1.5 py-0.5 rounded bg-cyan-500/20 text-cyan-300 font-semibold uppercase tracking-wider ml-0.5">
                                Активен
                              </span>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ) : (
                  <div className="p-3 rounded-xl bg-[#0b101d]/60 border border-slate-800/60 flex items-center justify-between text-xs text-slate-400">
                    <div className="flex items-center gap-2">
                      <Radio className="w-3.5 h-3.5 text-slate-500 shrink-0" />
                      <span>
                        {selectedProfile
                          ? `Текущий порт конфига «${selectedProfile.name}»: ${selectedProfile.port}. Нажмите кнопку «Определить рабочий порт» для поиска открытых портов.`
                          : "Выберите профиль подключения для определения портов"}
                      </span>
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Bottom Live Core Logs - Anchored to bottom edge */}
            <div className="p-4 rounded-2xl bg-[#0d1322] border border-slate-800/90 space-y-2 mt-auto shrink-0 w-full">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-slate-400 flex items-center gap-2">
                  <Terminal className="w-4 h-4 text-cyan-400" /> Журнал ядра sing-box
                </span>
                <div className="flex items-center gap-3">
                  <button
                    type="button"
                    onClick={handleExportCoreLogs}
                    className="flex items-center gap-1.5 text-[11px] text-cyan-400 hover:text-cyan-300 font-medium transition cursor-pointer"
                    title="Сформировать файл логов и выбрать куда сохранить"
                  >
                    <FileDown className="w-3.5 h-3.5" />
                    {t.exportLogsBtn}
                  </button>
                  <button
                    type="button"
                    onClick={() => invoke("clear_core_logs")}
                    className="text-[11px] text-slate-500 hover:text-slate-300 transition cursor-pointer"
                  >
                    {t.clearLogsBtn}
                  </button>
                </div>
              </div>
              <div className="h-32 overflow-y-auto font-mono text-[11px] text-slate-300 bg-[#070a12] p-3 rounded-xl border border-slate-800/60 space-y-1 select-text">
                {status.logs.length === 0 ? (
                  <div className="text-slate-600">Логи пусты...</div>
                ) : (
                  status.logs.map((line, idx) => (
                    <div key={idx} className="leading-relaxed break-all">
                      {line}
                    </div>
                  ))
                )}
                <div ref={coreLogEndRef} />
              </div>
            </div>

          </div>
        )}

        {/* TAB 2: VPS AUTO-DEPLOY & CLEANER */}
        {activeTab === "vps" && (
          <div className="flex-1 overflow-y-auto p-6 flex flex-col justify-between h-full min-h-0 space-y-6 animate-tab-enter">
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                    <Server className="w-6 h-6 text-cyan-400" />
                    Управление VPS-сервером по SSH
                  </h2>
                  <p className="text-xs text-slate-400 mt-1">
                    Автоматическая установка и настройка защищенных протоколов (SSH Tunnel, VLESS Reality, Amnezia 3.1, Hysteria 2, TUIC) на Linux VPS и полная очистка в 1 клик.
                  </p>
                </div>
              </div>

              <div className="grid grid-cols-12 gap-6">
                {/* Left Column: Server Credentials, Actions & Automation Info (5 cols) */}
                <div className="col-span-5 space-y-5">
                  {/* Card 1: Server SSH Parameters */}
                  <div className="p-5 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-1.5">
                    <Server className="w-3.5 h-3.5 text-cyan-400" />
                    1. Параметры сервера VPS (SSH)
                  </h3>
                  <button
                    type="button"
                    onClick={handleTestMainServer}
                    disabled={mainCheckStatus.busy || vpsBusy !== "idle"}
                    className="px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 border border-amber-400/50 text-[11px] font-semibold text-amber-300 disabled:opacity-50 transition flex items-center gap-1.5 shadow-sm shadow-amber-500/10 cursor-pointer"
                  >
                    <Wifi className="w-3 h-3 text-amber-400" />
                    {mainCheckStatus.busy ? "Проверка..." : "Проверить доступность"}
                  </button>
                </div>

                <div className="grid grid-cols-3 gap-3">
                  <div className="col-span-2">
                    <label className="block text-xs text-slate-400 mb-1">
                      IP-адрес или домен VPS
                    </label>
                    <input
                      type="text"
                      placeholder="Например: 185.22.14.99 (Финляндия)"
                      value={vpsCreds.host}
                      onChange={(e) =>
                        setVpsCreds({ ...vpsCreds, host: e.target.value })
                      }
                      className="w-full px-3.5 py-2 rounded-xl bg-[#0b101d] border border-slate-700/80 text-sm text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">
                      SSH Порт
                    </label>
                    <input
                      type="number"
                      value={vpsCreds.ssh_port}
                      onChange={(e) =>
                        setVpsCreds({
                          ...vpsCreds,
                          ssh_port: Number(e.target.value) || 22,
                        })
                      }
                      className="w-full px-3.5 py-2 rounded-xl bg-[#0b101d] border border-slate-700/80 text-sm text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">
                      Логин SSH
                    </label>
                    <input
                      type="text"
                      value={vpsCreds.username}
                      onChange={(e) =>
                        setVpsCreds({ ...vpsCreds, username: e.target.value })
                      }
                      className="w-full px-3.5 py-2 rounded-xl bg-[#0b101d] border border-slate-700/80 text-sm text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">
                      Пароль root / SSH
                    </label>
                    <div className="relative">
                      <input
                        type={showVpsPassword ? "text" : "password"}
                        placeholder="••••••••••••"
                        value={vpsCreds.password}
                        onChange={(e) =>
                          setVpsCreds({ ...vpsCreds, password: e.target.value })
                        }
                        className="w-full pl-3.5 pr-9 py-2 rounded-xl bg-[#0b101d] border border-slate-700/80 text-sm text-white focus:border-cyan-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => setShowVpsPassword(!showVpsPassword)}
                        className="absolute right-2.5 top-2.5 text-slate-400 hover:text-slate-200"
                      >
                        {showVpsPassword ? (
                          <EyeOff className="w-4 h-4" />
                        ) : (
                          <Eye className="w-4 h-4" />
                        )}
                      </button>
                    </div>
                  </div>
                </div>

                <div>
                  <label className="block text-xs text-slate-400 mb-1">
                    Локальный прокси для SSH (опционально, если ТСПУ блокирует 22 порт)
                  </label>
                  <input
                    type="text"
                    placeholder="Оставьте пустым или укажите напр. 127.0.0.1:10808"
                    value={vpsCreds.ssh_proxy || ""}
                    onChange={(e) =>
                      setVpsCreds({ ...vpsCreds, ssh_proxy: e.target.value })
                    }
                    className="w-full px-3.5 py-2 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-white focus:border-cyan-500 focus:outline-none"
                  />
                </div>

                {mainCheckStatus.result && (
                  <div className="flex items-center gap-2 p-2.5 rounded-xl bg-emerald-950/40 border border-emerald-500/30 text-emerald-300 text-[11px] animate-fadeIn">
                    <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                    <span className="truncate"><b>Основной VPS доступен:</b> {mainCheckStatus.result}</span>
                  </div>
                )}
                    {mainCheckStatus.error && (
                      <div className="flex items-center gap-2 p-2.5 rounded-xl bg-rose-950/40 border border-rose-500/30 text-rose-300 text-[11px] animate-fadeIn">
                        <XCircle className="w-3.5 h-3.5 text-rose-400 shrink-0" />
                        <span className="truncate"><b>Основной VPS недоступен:</b> {mainCheckStatus.error}</span>
                      </div>
                    )}
                  </div>

                  {/* Card 2: Control and Deploy Action Buttons */}
                  <div className="p-5 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-3">
                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-400">
                      Управление и выполнение
                    </h3>
                    <div className="flex gap-2.5">
                      <button
                        onClick={handleTestSsh}
                        disabled={vpsBusy !== "idle"}
                        style={{ color: "#ffffff" }}
                        className="px-4 py-2.5 rounded-xl bg-gradient-to-r from-amber-500 to-yellow-600 hover:from-amber-400 hover:to-yellow-500 border border-amber-400/50 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-amber-500/25 transition flex items-center justify-center gap-2 cursor-pointer"
                      >
                        <Wifi className="w-4 h-4 text-white" />
                        {vpsBusy === "testing" ? "Проверка..." : "Проверить SSH"}
                      </button>

                      <button
                        onClick={handleDeployVps}
                        disabled={vpsBusy !== "idle"}
                        style={{ color: "#ffffff" }}
                        className="flex-1 py-2.5 px-4 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 border border-emerald-400/40 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-emerald-600/25 transition flex items-center justify-center gap-2 cursor-pointer"
                      >
                        <Zap className="w-4 h-4 text-white" />
                        {vpsBusy === "deploying"
                          ? "Развертывание на VPS..."
                          : "Развернуть VPN на сервере"}
                      </button>
                    </div>

                    <button
                      onClick={() => setConfirmCleanModal(true)}
                      disabled={vpsBusy !== "idle"}
                      className="w-full py-2.5 px-4 rounded-xl bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 disabled:opacity-50 text-xs font-semibold text-rose-300 transition flex items-center justify-center gap-2 cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4 text-rose-400" />
                      {vpsBusy === "cleaning"
                        ? "Очистка сервера..."
                        : "Очистить сервер (удалить VPN и конфиги с VPS)"}
                    </button>
                  </div>

                  {/* Card 3: What Happens during Deploy & Clean */}
                  <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 text-xs text-slate-300 space-y-3">
                    <div className="flex items-center justify-between border-b border-slate-800/80 pb-2">
                      <span className="font-bold text-white flex items-center gap-2">
                        <ShieldCheck className="w-4 h-4 text-cyan-400" />
                        Что происходит при развертывании и очистке?
                      </span>
                      <span className="text-[10px] px-2 py-0.5 rounded bg-cyan-500/10 text-cyan-300 border border-cyan-500/20 font-mono">
                        Автоматизация в 1 клик
                      </span>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-[11px] leading-relaxed text-slate-300">
                      <div className="space-y-1 p-2.5 rounded-lg bg-[#0a0f1d] border border-slate-800/60">
                        <div className="font-semibold text-cyan-300 flex items-center gap-1.5">
                          <span className="w-1.5 h-1.5 rounded-full bg-cyan-400" />
                          1. Ядро sing-box
                        </div>
                        <p className="text-slate-400 text-[10px]">
                          Установка последнего ядра с поддержкой Amnezia 3.1 и Reality.
                        </p>
                      </div>

                      <div className="space-y-1 p-2.5 rounded-lg bg-[#0a0f1d] border border-slate-800/60">
                        <div className="font-semibold text-emerald-300 flex items-center gap-1.5">
                          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                          2. BBR + QUIC
                        </div>
                        <p className="text-slate-400 text-[10px]">
                          Включение Google BBR, ip_forward=1 и сокетных буферов 2.5 МБ.
                        </p>
                      </div>

                      <div className="space-y-1 p-2.5 rounded-lg bg-[#0a0f1d] border border-slate-800/60">
                        <div className="font-semibold text-purple-300 flex items-center gap-1.5">
                          <span className="w-1.5 h-1.5 rounded-full bg-purple-400" />
                          3. Ключи и обфускация
                        </div>
                        <p className="text-slate-400 text-[10px]">
                          Генерация ключей Reality, параметров AWG (Jc, S1-S4, H1-H4).
                        </p>
                      </div>

                      <div className="space-y-1 p-2.5 rounded-lg bg-[#0a0f1d] border border-slate-800/60">
                        <div className="font-semibold text-rose-300 flex items-center gap-1.5">
                          <span className="w-1.5 h-1.5 rounded-full bg-rose-400" />
                          4. Очистка сервера
                        </div>
                        <p className="text-slate-400 text-[10px]">
                          Кнопка «Очистить сервер» удаляет службы, ключи и конфиги.
                        </p>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Right Column: Protocol Selection & Detailed Parameters (7 cols) */}
                <div className="col-span-7 p-5 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-xs font-bold uppercase tracking-wider text-indigo-400 flex items-center gap-1.5">
                        <Layers className="w-3.5 h-3.5 text-indigo-400" />
                        2. Выбор протоколов для развертывания (Обход блокировок)
                      </h3>
                      <p className="text-[11px] text-slate-400 mt-0.5">
                        Отметьте нужные протоколы. Для 100% стабильной работы на кабельном интернете рекомендуются <b>SSH Tunnel</b>, <b>VLESS Reality</b> и <b>Amnezia 3.1</b>.
                      </p>
                    </div>
                  </div>

                  <div className="space-y-3.5">

                  {/* Protocol 0: SSH Tunnel */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_ssh
                        ? "bg-violet-950/20 border-violet-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_ssh ?? true}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_ssh: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-violet-500 focus:ring-violet-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>SSH Tunnel (порт 22 • Direct TCP/IP)</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-violet-500/20 text-violet-300 font-semibold border border-violet-500/30">
                              ⚡ 100% обход кабельного DPI
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            Прямой туннель к порту 22 VPS. Российский ТСПУ никогда не трогает SSH. Вся веб-активность и DNS шифруются внутри SSH.
                          </p>
                        </div>
                      </label>
                    </div>
                  </div>

                  {/* Protocol 1: VLESS Reality */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_vless
                        ? "bg-emerald-950/20 border-emerald-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_vless}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_vless: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>VLESS Reality + Anti-DPI TLS Фрагментация</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-emerald-500/20 text-emerald-300 font-semibold border border-emerald-500/30">
                              🛡️ Обход ТСПУ на кабельных провайдерах
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            TCP порт 443. Маскируется под браузер Chrome и сайт Microsoft. Пакет ClientHello разбивается на фрагменты (RFC 5246/8446), обходя проверку SNI/IP на кабельном интернете (Ростелеком, Дом.ru).
                          </p>
                        </div>
                      </label>
                    </div>

                    {vpsCreds.deploy_vless && (
                      <div className="grid grid-cols-2 gap-3 mt-3 pt-3 border-t border-emerald-500/20">
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-1">
                            Порт VLESS Reality (TCP)
                          </label>
                          <input
                            type="number"
                            value={vpsCreds.vless_port || 443}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                vless_port: Number(e.target.value) || 443,
                              })
                            }
                            className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-emerald-500 focus:outline-none"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-1">
                            SNI маскировки Reality
                          </label>
                          <input
                            type="text"
                            value={vpsCreds.reality_sni || "www.microsoft.com"}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                reality_sni: e.target.value,
                              })
                            }
                            placeholder="www.microsoft.com"
                            className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-emerald-500 focus:outline-none"
                          />
                        </div>
                      </div>
                    )}
                  </div>

                  {/* Protocol 2: Hysteria 2 */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_hy2
                        ? "bg-cyan-950/20 border-cyan-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_hy2}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_hy2: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-cyan-500 focus:ring-cyan-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>Hysteria 2</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-cyan-500/20 text-cyan-300 font-semibold border border-cyan-500/30">
                              ⚡ Сверхскорость по UDP
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            Кастомный QUIC-протокол. Максимальная скорость и устойчивость к потере пакетов на мобильных сетях.
                          </p>
                        </div>
                      </label>
                    </div>

                    {vpsCreds.deploy_hy2 && (
                      <div className="space-y-2.5 mt-3 pt-3 border-t border-cyan-500/20">
                        <div className="grid grid-cols-2 gap-3">
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">
                              Порт Hysteria 2 (UDP)
                            </label>
                            <input
                              type="number"
                              value={vpsCreds.hy2_port}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  hy2_port: Number(e.target.value) || 8443,
                                })
                              }
                              className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-cyan-500 focus:outline-none"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">
                              SNI маскировки TLS
                            </label>
                            <input
                              type="text"
                              value={vpsCreds.sni}
                              onChange={(e) =>
                                setVpsCreds({ ...vpsCreds, sni: e.target.value })
                              }
                              className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-cyan-500 focus:outline-none"
                            />
                          </div>
                        </div>

                        <label className="flex items-center gap-2 text-[11px] text-slate-300 cursor-pointer pt-1">
                          <input
                            type="checkbox"
                            checked={vpsCreds.enable_salamander}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                enable_salamander: e.target.checked,
                              })
                            }
                            className="rounded border-slate-700 bg-slate-900 text-cyan-500 focus:ring-cyan-500"
                          />
                          <span>
                            Включить обфускацию <b>Salamander</b> (скрывает QUIC-заголовки)
                          </span>
                        </label>

                        <label className="flex items-center gap-2 text-[11px] text-slate-300 cursor-pointer">
                          <input
                            type="checkbox"
                            checked={vpsCreds.enable_port_hopping ?? true}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                enable_port_hopping: e.target.checked,
                              })
                            }
                            className="rounded border-slate-700 bg-slate-900 text-cyan-500 focus:ring-cyan-500"
                          />
                          <span>
                            Включить <b>Port Hopping (20000-50000) + Порт 53 DNS</b> (прыжки по портам)
                          </span>
                        </label>
                      </div>
                    )}
                  </div>

                  {/* Protocol 3: TUIC v5 */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_tuic
                        ? "bg-purple-950/20 border-purple-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_tuic}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_tuic: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-purple-500 focus:ring-purple-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>TUIC v5</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-purple-500/20 text-purple-300 font-semibold border border-purple-500/30">
                              Альтернативный QUIC
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            Оптимален для мобильных клиентов Hiddify / v2rayNG на смартфонах.
                          </p>
                        </div>
                      </label>
                    </div>

                    {vpsCreds.deploy_tuic && (
                      <div className="mt-3 pt-3 border-t border-purple-500/20">
                        <label className="block text-[11px] text-slate-400 mb-1">
                          Порт TUIC v5 (UDP)
                        </label>
                        <input
                          type="number"
                          value={vpsCreds.tuic_port}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              tuic_port: Number(e.target.value) || 9443,
                            })
                          }
                          className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-purple-500 focus:outline-none"
                        />
                      </div>
                    )}
                  </div>

                  {/* Protocol 4: Shadowsocks */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_ss
                        ? "bg-amber-950/20 border-amber-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_ss}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_ss: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-amber-500 focus:ring-amber-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>Shadowsocks</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-amber-500/20 text-amber-300 font-semibold border border-amber-500/30">
                              Классический TCP/UDP прокси
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            Поддерживает Shadowsocks-2022 и классические AEAD шифры (Outline, sing-box, Shadowrocket, Clash).
                          </p>
                        </div>
                      </label>
                    </div>

                    {vpsCreds.deploy_ss && (
                      <div className="mt-3 pt-3 border-t border-amber-500/20 grid grid-cols-2 gap-3">
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-1">
                            Порт Shadowsocks (TCP/UDP)
                          </label>
                          <input
                            type="number"
                            value={vpsCreds.ss_port}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                ss_port: Number(e.target.value) || 8388,
                              })
                            }
                            className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-amber-500 focus:outline-none"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-1">
                            Метод шифрования
                          </label>
                          <select
                            value={vpsCreds.ss_method}
                            onChange={(e) =>
                              setVpsCreds({
                                ...vpsCreds,
                                ss_method: e.target.value,
                              })
                            }
                            className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-amber-500 focus:outline-none"
                          >
                            <option value="2022-blake3-aes-128-gcm">2022-blake3-aes-128-gcm (SS 2022)</option>
                            <option value="2022-blake3-aes-256-gcm">2022-blake3-aes-256-gcm (SS 2022)</option>
                            <option value="aes-256-gcm">aes-256-gcm (AEAD классик)</option>
                            <option value="chacha20-ietf-poly1305">chacha20-ietf-poly1305</option>
                          </select>
                        </div>
                      </div>
                    )}
                  </div>

                  {/* Protocol 5: Amnezia 3.1 (AWG 3.1) */}
                  <div
                    className={`p-3.5 rounded-xl border transition ${
                      vpsCreds.deploy_awg
                        ? "bg-emerald-950/20 border-emerald-500/40"
                        : "bg-[#0b101d] border-slate-800 opacity-60"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <label className="flex items-center gap-2.5 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={vpsCreds.deploy_awg}
                          onChange={(e) =>
                            setVpsCreds({
                              ...vpsCreds,
                              deploy_awg: e.target.checked,
                            })
                          }
                          className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                        />
                        <div>
                          <div className="text-xs font-bold text-white flex items-center gap-2">
                            <span>Amnezia 3.1 (AWG 3.1)</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-md bg-emerald-500/20 text-emerald-300 font-semibold border border-emerald-500/30">
                              Header Protection + Jitter + Port Hunting
                            </span>
                          </div>
                          <p className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                            Новейшая спецификация Amnezia 3.1: шифрование заголовков ChaCha20 (HeaderProtectionKey), мусорные пакеты (Jc), случайные сдвиги (S1-S4 ≥ 12), случайные интервалы Rekey и обход DPI по белому списку WireGuard.
                          </p>
                        </div>
                      </label>
                    </div>

                    {vpsCreds.deploy_awg && (
                      <div className="mt-3 pt-3 border-t border-emerald-500/20 space-y-3.5">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <span className="text-xs font-bold text-emerald-300">Настройки и функции Amnezia 3.1</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 font-mono">
                              Спецификация v3.1 + Anti-DPI
                            </span>
                          </div>
                          <button
                            type="button"
                            onClick={() => {
                              const r = generateRandomAwg31Params();
                              const newPort = getRandomInt(21000, 55000);
                              setVpsCreds({
                                ...vpsCreds,
                                ...r,
                                awg_port: newPort,
                              });
                              showNotice("success", `Сгенерированы параметры и порт UDP ${newPort}!`);
                            }}
                            className="px-2.5 py-1 rounded-lg bg-emerald-600/30 hover:bg-emerald-600/50 border border-emerald-500/40 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                            title="Сгенерировать случайные значения прямо сейчас"
                          >
                            🎲 Сгенерировать параметры и порт
                          </button>
                        </div>

                        <div className="p-2.5 rounded-lg bg-emerald-950/25 border border-emerald-500/20 text-[11px] text-emerald-300/90 flex items-center gap-2">
                          <Sparkles className="w-3.5 h-3.5 flex-shrink-0 text-emerald-400" />
                          <span>При каждом нажатии <b>«Развернуть VPN на сервере»</b> порт (не 51820) и все параметры обфускации генерируются автоматически для обхода фильтров провайдера.</span>
                        </div>

                        {/* Feature Toggles (Галочки функций) */}
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-2.5 pt-1">
                          <label className="flex items-start gap-2.5 p-2.5 rounded-xl bg-[#070b14] border border-slate-800 hover:border-emerald-500/40 cursor-pointer transition">
                            <input
                              type="checkbox"
                              checked={vpsCreds.awg_disable_cookies ?? true}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  awg_disable_cookies: e.target.checked,
                                })
                              }
                              className="mt-0.5 w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                            />
                            <div className="text-left min-w-0">
                              <div className="text-xs font-semibold text-white flex items-center gap-1.5 flex-wrap">
                                <span>Отключить Cookies</span>
                                <span className="text-[9px] px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 font-mono">Anti-DPI</span>
                              </div>
                              <p className="text-[10px] text-slate-400 mt-0.5 leading-tight">
                                Запрещает серверу слать Type 3 Cookie Reply. Блокирует активное сканирование DPI РКН.
                              </p>
                            </div>
                          </label>

                          <label className="flex items-start gap-2.5 p-2.5 rounded-xl bg-[#070b14] border border-slate-800 hover:border-emerald-500/40 cursor-pointer transition">
                            <input
                              type="checkbox"
                              checked={vpsCreds.awg_random_trailers ?? true}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  awg_random_trailers: e.target.checked,
                                })
                              }
                              className="mt-0.5 w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                            />
                            <div className="text-left min-w-0">
                              <div className="text-xs font-semibold text-white flex items-center gap-1.5 flex-wrap">
                                <span>Случайный паддинг</span>
                                <span className="text-[9px] px-1.5 py-0.2 rounded bg-emerald-500/20 text-emerald-300 font-mono">Trailers</span>
                              </div>
                              <p className="text-[10px] text-slate-400 mt-0.5 leading-tight">
                                Добавляет случайные энтропийные хвосты, маскируя размеры пакетов от эвристического анализа DPI.
                              </p>
                            </div>
                          </label>

                          <label className="flex items-start gap-2.5 p-2.5 rounded-xl bg-[#070b14] border border-slate-800 hover:border-emerald-500/40 cursor-pointer transition">
                            <input
                              type="checkbox"
                              checked={vpsCreds.awg_port_probing ?? true}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  awg_port_probing: e.target.checked,
                                })
                              }
                              className="mt-0.5 w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                            />
                            <div className="text-left min-w-0">
                              <div className="text-xs font-semibold text-white flex items-center gap-1.5 flex-wrap">
                                <span>Авто-пробинг портов</span>
                                <span className="text-[9px] px-1.5 py-0.2 rounded bg-cyan-500/20 text-cyan-300 font-mono">Port Probing</span>
                              </div>
                              <p className="text-[10px] text-slate-400 mt-0.5 leading-tight">
                                Клиент автоматически зондирует и переключает порты из пула Port Hunting при глушении трафика.
                              </p>
                            </div>
                          </label>
                        </div>

                        {/* Ports */}
                        <div className="grid grid-cols-2 gap-3">
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">
                              Основной порт Amnezia 3.1 (UDP)
                            </label>
                            <input
                              type="number"
                              value={vpsCreds.awg_port}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  awg_port: Number(e.target.value) || 0,
                                })
                              }
                              placeholder="Случайный порт UDP (20000-55000)"
                              className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-emerald-500 focus:outline-none"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">
                              Диапазон авто-подбора портов (Port Hunting)
                            </label>
                            <input
                              type="text"
                              value={vpsCreds.awg_hop_ports}
                              onChange={(e) =>
                                setVpsCreds({
                                  ...vpsCreds,
                                  awg_hop_ports: e.target.value,
                                })
                              }
                              placeholder="20000:50000"
                              className="w-full px-3 py-1.5 rounded-lg bg-[#070b14] border border-slate-700 text-xs text-white focus:border-emerald-500 focus:outline-none"
                            />
                          </div>
                        </div>

                        {/* Obfuscation parameters */}
                        <div className="grid grid-cols-4 gap-2 pt-1 text-[11px]">
                          <div>
                            <span className="text-slate-400 block mb-0.5">Jc (Мусорные пакеты)</span>
                            <input
                              type="number"
                              value={vpsCreds.awg_jc ?? 5}
                              onChange={(e) => setVpsCreds({ ...vpsCreds, awg_jc: Number(e.target.value) || 5 })}
                              className="w-full px-2 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-xs"
                            />
                          </div>
                          <div>
                            <span className="text-slate-400 block mb-0.5">Jmin / Jmax (размер)</span>
                            <div className="flex items-center gap-1">
                              <input
                                type="number"
                                value={vpsCreds.awg_jmin ?? 10}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_jmin: Number(e.target.value) || 10 })}
                                className="w-full px-1.5 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-xs"
                              />
                              <input
                                type="number"
                                value={vpsCreds.awg_jmax ?? 50}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_jmax: Number(e.target.value) || 50 })}
                                className="w-full px-1.5 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-xs"
                              />
                            </div>
                          </div>
                          <div>
                            <span className="text-slate-400 block mb-0.5">S1-S4 (≥ 12 байт)</span>
                            <div className="grid grid-cols-4 gap-0.5">
                              <input
                                type="number"
                                value={vpsCreds.awg_s1 ?? 12}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_s1: Math.max(12, Number(e.target.value) || 12) })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="number"
                                value={vpsCreds.awg_s2 ?? 12}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_s2: Math.max(12, Number(e.target.value) || 12) })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="number"
                                value={vpsCreds.awg_s3 ?? 12}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_s3: Math.max(12, Number(e.target.value) || 12) })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="number"
                                value={vpsCreds.awg_s4 ?? 12}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_s4: Math.max(12, Number(e.target.value) || 12) })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                            </div>
                          </div>
                          <div>
                            <span className="text-slate-400 block mb-0.5">H1-H4 (1-4)</span>
                            <div className="grid grid-cols-4 gap-0.5">
                              <input
                                type="text"
                                value={vpsCreds.awg_h1 ?? "1"}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_h1: e.target.value })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="text"
                                value={vpsCreds.awg_h2 ?? "2"}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_h2: e.target.value })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="text"
                                value={vpsCreds.awg_h3 ?? "3"}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_h3: e.target.value })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                              <input
                                type="text"
                                value={vpsCreds.awg_h4 ?? "4"}
                                onChange={(e) => setVpsCreds({ ...vpsCreds, awg_h4: e.target.value })}
                                className="w-full px-1 py-1 rounded bg-[#070b14] border border-slate-700 text-white text-[11px] text-center"
                              />
                            </div>
                          </div>
                        </div>

                        {/* Collapsible Advanced Session Timings & HPK */}
                        <div className="pt-2 border-t border-slate-800/80">
                          <button
                            type="button"
                            onClick={() => setShowAwgAdvanced(!showAwgAdvanced)}
                            className="text-xs font-semibold text-emerald-400 hover:text-emerald-300 flex items-center gap-1.5 transition cursor-pointer"
                          >
                            <span>{showAwgAdvanced ? "▲ Скрыть расширенные параметры сессии" : "▼ Расширенные параметры сессии и HPK (v3.1)"}</span>
                            <span className="text-[10px] text-slate-500">
                              (Rekey, Keepalive, Reject Time, HPK)
                            </span>
                          </button>

                          {showAwgAdvanced && (
                            <div className="mt-3 p-3 rounded-xl bg-[#060c18] border border-slate-800 space-y-3">
                              <div className="grid grid-cols-2 md:grid-cols-4 gap-2.5 text-[11px]">
                                <div>
                                  <label className="text-slate-400 block mb-1">
                                    Rekey After Time
                                  </label>
                                  <input
                                    type="text"
                                    value={vpsCreds.awg_rekey_after_time ?? "90-120"}
                                    onChange={(e) => setVpsCreds({ ...vpsCreds, awg_rekey_after_time: e.target.value })}
                                    placeholder="90-120"
                                    className="w-full px-2 py-1.5 rounded bg-[#070b14] border border-slate-700 text-white text-xs font-mono"
                                  />
                                </div>
                                <div>
                                  <label className="text-slate-400 block mb-1">
                                    Rekey Timeout
                                  </label>
                                  <input
                                    type="text"
                                    value={vpsCreds.awg_rekey_timeout ?? "3-6"}
                                    onChange={(e) => setVpsCreds({ ...vpsCreds, awg_rekey_timeout: e.target.value })}
                                    placeholder="3-6"
                                    className="w-full px-2 py-1.5 rounded bg-[#070b14] border border-slate-700 text-white text-xs font-mono"
                                  />
                                </div>
                                <div>
                                  <label className="text-slate-400 block mb-1">
                                    Reject After Time
                                  </label>
                                  <input
                                    type="text"
                                    value={vpsCreds.awg_reject_after_time ?? "150-180"}
                                    onChange={(e) => setVpsCreds({ ...vpsCreds, awg_reject_after_time: e.target.value })}
                                    placeholder="150-180"
                                    className="w-full px-2 py-1.5 rounded bg-[#070b14] border border-slate-700 text-white text-xs font-mono"
                                  />
                                </div>
                                <div>
                                  <label className="text-slate-400 block mb-1">
                                    Keepalive Timeout
                                  </label>
                                  <input
                                    type="text"
                                    value={vpsCreds.awg_keepalive_timeout ?? "15-25"}
                                    onChange={(e) => setVpsCreds({ ...vpsCreds, awg_keepalive_timeout: e.target.value })}
                                    placeholder="15-25"
                                    className="w-full px-2 py-1.5 rounded bg-[#070b14] border border-slate-700 text-white text-xs font-mono"
                                  />
                                </div>
                              </div>

                              <div>
                                <label className="text-slate-400 block mb-1 text-[11px]">
                                  Header Protection Key (HPK — 32 байта base64)
                                </label>
                                <div className="flex gap-2">
                                  <input
                                    type="text"
                                    value={vpsCreds.awg_header_protection_key ?? ""}
                                    onChange={(e) => setVpsCreds({ ...vpsCreds, awg_header_protection_key: e.target.value })}
                                    placeholder="Автоматически генерируется при деплое"
                                    className="flex-1 px-2.5 py-1.5 rounded bg-[#070b14] border border-slate-700 text-white text-xs font-mono"
                                  />
                                  <button
                                    type="button"
                                    onClick={() => setVpsCreds({ ...vpsCreds, awg_header_protection_key: getRandomBase64Key() })}
                                    className="px-2.5 py-1.5 rounded bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 font-semibold transition"
                                  >
                                    🎲 Сгенерировать
                                  </button>
                                </div>
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Bottom Live SSH Deployment Terminal (Anchored to bottom edge - like Dashboard!) */}
          <div className="p-4 rounded-2xl bg-[#0d1322] border border-slate-800/90 space-y-2 mt-auto shrink-0 w-full">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-slate-400 flex items-center gap-2">
                <Terminal className="w-4 h-4 text-cyan-400" />
                Терминал выполнения SSH и журнал развертывания
              </span>
              <div className="flex items-center gap-3">
                {vpsBusy !== "idle" && (
                  <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 flex items-center gap-1 font-semibold animate-pulse font-mono">
                    <RefreshCw className="w-3 h-3 animate-spin" />
                    {vpsBusy === "deploying" ? "Развертывание..." : vpsBusy === "cleaning" ? "Очистка..." : "Тестирование..."}
                  </span>
                )}
                <button
                  type="button"
                  onClick={() => setVpsLogs([])}
                  className="text-[11px] text-slate-500 hover:text-slate-300 transition cursor-pointer"
                >
                  Очистить вывод
                </button>
              </div>
            </div>

            <div className="h-36 overflow-y-auto font-mono text-[11px] bg-[#060911] p-3 rounded-xl border border-slate-800/80 space-y-1 select-text custom-scrollbar">
              {vpsLogs.length === 0 ? (
                <div className="text-slate-600">Терминал готов к выполнению команд...</div>
              ) : (
                vpsLogs.map((log, index) => (
                  <div
                    key={index}
                    className={`leading-relaxed break-all ${
                      log.includes("ОШИБКА")
                        ? "text-rose-400 font-semibold"
                        : log.includes("Готово!") || log.includes("успешно") || log.includes("CLEAN_OK")
                        ? "text-emerald-400 font-semibold"
                        : log.includes("===")
                        ? "text-cyan-300 font-semibold"
                        : "text-slate-300"
                    }`}
                  >
                    {log}
                  </div>
                ))
              )}
              <div ref={vpsLogEndRef} />
            </div>
          </div>

          {/* Confirmation Modal for Server Clean */}
            {confirmCleanModal && (
              <div className="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4">
                <div className="max-w-md w-full p-6 rounded-2xl bg-[#111827] border border-rose-500/40 shadow-2xl space-y-4">
                  <div className="flex items-center gap-3 text-rose-400">
                    <AlertTriangle className="w-6 h-6 shrink-0" />
                    <h3 className="text-base font-bold text-white">
                      Подтвердите очистку сервера
                    </h3>
                  </div>
                  <p className="text-xs text-slate-300 leading-relaxed">
                    Вы уверены, что хотите полностью удалить <b>sing-box</b>, все ключи, сертификаты (<code>/etc/sing-box</code>) и правила портов с сервера <b>{vpsCreds.host || "VPS"}</b>?
                  </p>
                  <div className="flex justify-end gap-3 pt-2">
                    <button
                      onClick={() => setConfirmCleanModal(false)}
                      className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200"
                    >
                      Отмена
                    </button>
                    <button
                      onClick={handleCleanVps}
                      className="px-4 py-2 rounded-xl bg-rose-600 hover:bg-rose-500 text-xs font-bold text-white"
                    >
                      Да, очистить сервер
                    </button>
                  </div>
                </div>
              </div>
            )}

          </div>
        )}

        {/* TAB 3: PROFILES & DEVICES MANAGER */}
        {activeTab === "profiles" && (
          <div className="flex-1 overflow-y-auto p-6 space-y-5 animate-tab-enter">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                  <Smartphone className="w-6 h-6 text-cyan-400" />
                  Менеджер туннелей и устройств
                </h2>
                <p className="text-xs text-slate-400 mt-1">
                  Управление туннелями для текущего ПК и синхронизация отдельных ключей для мобильных устройств (Телефон, Ноутбук, Друзья).
                </p>
              </div>
            </div>

            {/* ========================================================================= */}
            {/* БЛОК 1: ТУННЕЛИ, УСТАНОВЛЕННЫЕ В ПРИЛОЖЕНИИ (ЭТОТ ПК)                       */}
            {/* ========================================================================= */}
            <div className="p-5 rounded-2xl bg-[#0e1526] border border-slate-800/90 space-y-4 shadow-sm">
              <div className="flex items-center justify-between flex-wrap gap-3">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-sm shrink-0">
                    <Monitor className="w-5 h-5" />
                  </div>
                  <div>
                    <h3 className="text-sm font-bold text-white flex items-center gap-2">
                      Туннели, установленные в приложении (Этот ПК)
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold font-mono">
                        {effectiveClientProfiles.length}
                      </span>
                    </h3>
                    <p className="text-[11px] text-slate-400">
                      Конфигурации для защиты и маршрутизации трафика на этом компьютере.
                    </p>
                  </div>
                </div>

                {/* Column layout for manual buttons */}
                <div className="flex flex-col gap-2 shrink-0 items-stretch sm:items-end min-w-[175px]">
                  <button
                    onClick={handlePickConfigFile}
                    className="w-full px-3.5 py-1.5 rounded-xl bg-cyan-600/20 hover:bg-cyan-600/30 border border-cyan-500/40 text-xs font-semibold text-cyan-300 flex items-center justify-center gap-1.5 transition shrink-0"
                    title="Выбрать файл конфигурации на компьютере (.conf, .json, .txt)"
                  >
                    <FolderOpen className="w-3.5 h-3.5 text-cyan-400" />
                    Выбрать файл конфига
                  </button>
                  <button
                    onClick={() => {
                      setNewProfile({
                        id: "",
                        name: `SSH Tunnel (${vpsCreds.host || "VPS"})`,
                        protocol: "ssh",
                        server: vpsCreds.host || "",
                        port: vpsCreds.ssh_port || 22,
                        sni: vpsCreds.username || "root",
                        insecure: false,
                        password: vpsCreds.password || "",
                        uuid: "",
                        obfs_password: "",
                        ss_method: "2022-blake3-aes-128-gcm",
                        ss_plugin: "",
                        ss_plugin_opts: "",
                        tls_fragment: false,
                        up_mbps: 100,
                        down_mbps: 100,
                        uri: "",
                      });
                      setShowNewProfileForm(true);
                    }}
                    className="w-full px-3.5 py-1.5 rounded-xl bg-violet-600/20 hover:bg-violet-600/30 border border-violet-500/40 text-xs font-semibold text-violet-300 flex items-center justify-center gap-1.5 transition shrink-0"
                  >
                    <Key className="w-3.5 h-3.5 text-violet-400" />
                    ⚡ Быстрый SSH-туннель
                  </button>
                  <button
                    onClick={() => setShowNewProfileForm(!showNewProfileForm)}
                    className="w-full px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 flex items-center justify-center gap-1.5 transition shrink-0"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    Добавить вручную
                  </button>
                </div>
              </div>

            {/* Quick URI Import Bar & File Picker */}
            <div className="p-3.5 rounded-2xl bg-[#111827] border border-slate-800/90 flex flex-wrap sm:flex-nowrap gap-3 items-center">
              <input
                type="text"
                placeholder="Или вставьте готовую ссылку vless://, ss://, hy2:// или tuic:// для импорта"
                value={uriInput}
                onChange={(e) => setUriInput(e.target.value)}
                className="flex-1 px-3.5 py-1.5 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none min-w-[200px]"
              />
              <button
                onClick={handleImportUri}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 shrink-0 transition"
              >
                Импортировать ссылку
              </button>
              <button
                onClick={handlePickConfigFile}
                className="px-4 py-1.5 rounded-xl bg-cyan-950/60 hover:bg-cyan-900/60 border border-cyan-500/40 text-xs font-semibold text-cyan-300 flex items-center gap-1.5 shrink-0 transition"
                title="Выбрать файл конфигурации на компьютере (.conf, .json, .txt)"
              >
                <FolderOpen className="w-3.5 h-3.5 text-cyan-400" />
                Выбрать файл (.conf / .json)
              </button>
            </div>

            {/* Manual Profile Form Drawer */}
            {showNewProfileForm && (
              <div className="p-5 rounded-2xl bg-[#111827] border border-cyan-500/40 space-y-4">
                <h3 className="text-sm font-bold text-cyan-300">
                  Новый профиль VPN
                </h3>
                <div className="grid grid-cols-4 gap-3">
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Протокол</label>
                    <select
                      value={newProfile.protocol}
                      onChange={(e) => {
                        const proto = e.target.value as "vless" | "hysteria2" | "tuic" | "shadowsocks" | "amneziawg" | "awg2" | "ssh";
                        const isAwg31 = proto === "amneziawg";
                        const isAwg20 = proto === "awg2";
                        setNewProfile({
                          ...newProfile,
                          protocol: proto,
                          port: proto === "ssh" ? 22 : proto === "shadowsocks" ? 8388 : proto === "vless" ? 443 : proto === "tuic" ? 9443 : (isAwg20 || isAwg31) ? getRandomInt(21000, 55000) : 8443,
                          sni: proto === "ssh" ? (newProfile.sni || "root") : newProfile.sni,
                          tls_fragment: proto === "ssh" ? false : true,
                          awg_version: isAwg20 ? "2.0" : isAwg31 ? "3.1" : newProfile.awg_version,
                          awg_local_address: isAwg20 ? (newProfile.awg_local_address || "10.8.1.2/32") : isAwg31 ? (newProfile.awg_local_address || "10.8.0.2/32") : newProfile.awg_local_address,
                          awg_hop_ports: (isAwg31 || isAwg20) ? (newProfile.awg_hop_ports || "20000:50000") : newProfile.awg_hop_ports,
                          awg_port_probing: (isAwg31 || isAwg20) ? (newProfile.awg_port_probing ?? true) : newProfile.awg_port_probing,
                        });
                      }}
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    >
                      <option value="ssh">SSH Tunnel (порт 22 • 100% обход блокировок)</option>
                      <option value="vless">VLESS Reality (порт 443 + Anti-DPI)</option>
                      <option value="amneziawg">Amnezia 3.1 (Header Protection + S1-S4 ≥ 12)</option>
                      <option value="awg2">AmneziaWG 2.0 (AWG 2.0 • Магические заголовки H1-H4)</option>
                      <option value="hysteria2">Hysteria 2 (QUIC + Salamander)</option>
                      <option value="tuic">TUIC v5 (QUIC BBR)</option>
                      <option value="shadowsocks">Shadowsocks 2022</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Название</label>
                    <input
                      type="text"
                      placeholder="Мой сервер"
                      value={newProfile.name}
                      onChange={(e) =>
                        setNewProfile({ ...newProfile, name: e.target.value })
                      }
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">IP / Хост</label>
                    <input
                      type="text"
                      placeholder="185.x.x.x"
                      value={newProfile.server}
                      onChange={(e) =>
                        setNewProfile({ ...newProfile, server: e.target.value })
                      }
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Порт</label>
                    <input
                      type="number"
                      value={newProfile.port}
                      onChange={(e) =>
                        setNewProfile({
                          ...newProfile,
                          port: Number(e.target.value) || (newProfile.protocol === "ssh" ? 22 : newProfile.protocol === "shadowsocks" ? 8388 : (newProfile.protocol === "awg2" || newProfile.protocol === "amneziawg") ? getRandomInt(21000, 55000) : 8443),
                        })
                      }
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    />
                  </div>
                </div>

                {newProfile.protocol === "amneziawg" ? (
                  <div className="space-y-3 pt-1">
                    <div className="flex items-center justify-between p-3 rounded-xl bg-emerald-950/25 border border-emerald-500/30">
                      <div>
                        <div className="text-xs font-bold text-emerald-300">
                          Параметры протокола Amnezia 3.1
                        </div>
                        <div className="text-[11px] text-slate-400">
                          Защита заголовков HeaderProtectionKey, сдвиги S1-S4 ≥ 12 и таймеры как в официальном клиенте
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          const r = generateRandomAwg31Params();
                          setNewProfile({ ...newProfile, ...r });
                          showNotice("success", "Параметры Amnezia 3.1 рандомизированы!");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-emerald-600/30 hover:bg-emerald-600/50 border border-emerald-500/40 text-xs font-semibold text-emerald-300 flex items-center gap-1.5 transition"
                      >
                        🎲 Рандомизировать параметры 3.1
                      </button>
                    </div>

                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Приватный ключ клиента (Base64)</label>
                        <input
                          type="text"
                          placeholder="Приватный ключ"
                          value={newProfile.password}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, password: e.target.value, awg_private_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Публичный ключ сервера (Base64)</label>
                        <input
                          type="text"
                          placeholder="Публичный ключ сервера"
                          value={newProfile.awg_public_key || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_public_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">IP клиента (Address)</label>
                        <input
                          type="text"
                          placeholder="10.8.0.2/32"
                          value={newProfile.awg_local_address || "10.8.0.2/32"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_local_address: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">HeaderProtectionKey (Base64 32 байта)</label>
                        <input
                          type="text"
                          placeholder="Base64 32 байта"
                          value={newProfile.awg_header_protection_key || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_header_protection_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">PresharedKey (Base64 32 байта, опционально)</label>
                        <input
                          type="text"
                          placeholder="Base64 32 байта"
                          value={newProfile.awg_preshared_key || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_preshared_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Диапазон авто-подбора портов (Port Hunting)</label>
                        <input
                          type="text"
                          placeholder="20000:50000"
                          value={newProfile.awg_hop_ports || "20000:50000"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_hop_ports: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div className="flex items-center pt-5">
                        <label className="flex items-center gap-2 cursor-pointer text-xs text-slate-300">
                          <input
                            type="checkbox"
                            checked={newProfile.awg_port_probing ?? true}
                            onChange={(e) =>
                              setNewProfile({ ...newProfile, awg_port_probing: e.target.checked })
                            }
                            className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                          />
                          <span>Автоматически перебирать порты при DPI блокировках</span>
                        </label>
                      </div>
                    </div>

                    {/* AWG 3.1 Obfuscation params */}
                    <div className="p-3 rounded-xl bg-[#070b14] border border-slate-800 grid grid-cols-4 gap-2 text-xs">
                      <div>
                        <span className="text-slate-400 block mb-1">Jc (Мусорные пакеты)</span>
                        <input
                          type="number"
                          value={newProfile.awg_jc ?? 5}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_jc: Number(e.target.value) || 5 })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">Jmin / Jmax</span>
                        <div className="flex items-center gap-1">
                          <input
                            type="number"
                            value={newProfile.awg_jmin ?? 10}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_jmin: Number(e.target.value) || 10 })}
                            className="w-full px-1.5 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_jmax ?? 50}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_jmax: Number(e.target.value) || 50 })}
                            className="w-full px-1.5 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                          />
                        </div>
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">S1-S4 (≥ 12 байт)</span>
                        <div className="grid grid-cols-4 gap-0.5">
                          <input
                            type="number"
                            value={newProfile.awg_s1 ?? 12}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s1: Number(e.target.value) || 12 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_s2 ?? 12}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s2: Number(e.target.value) || 12 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_s3 ?? 12}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s3: Number(e.target.value) || 12 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_s4 ?? 12}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s4: Number(e.target.value) || 12 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                        </div>
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">H1-H4 (1-4)</span>
                        <div className="grid grid-cols-4 gap-0.5">
                          <input
                            type="text"
                            value={newProfile.awg_h1 ?? "1"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h1: e.target.value })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-center"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h2 ?? "2"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h2: e.target.value })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-center"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h3 ?? "3"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h3: e.target.value })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-center"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h4 ?? "4"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h4: e.target.value })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-center"
                          />
                        </div>
                      </div>
                    </div>

                    {/* Amnezia 3.1 Timers */}
                    <div className="p-3 rounded-xl bg-[#070b14] border border-slate-800 grid grid-cols-6 gap-2 text-xs">
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">RekeyAfterTime</span>
                        <input
                          type="text"
                          value={newProfile.awg_rekey_after_time || "100-120"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_rekey_after_time: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">RekeyTimeout</span>
                        <input
                          type="text"
                          value={newProfile.awg_rekey_timeout || "3-7"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_rekey_timeout: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">RejectAfterTime</span>
                        <input
                          type="text"
                          value={newProfile.awg_reject_after_time || "150-180"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_reject_after_time: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">KeepaliveTimeout</span>
                        <input
                          type="text"
                          value={newProfile.awg_keepalive_timeout || "5-15"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_keepalive_timeout: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">MaxAttempts</span>
                        <input
                          type="text"
                          value={newProfile.awg_max_handshake_attempts || "15-20"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_max_handshake_attempts: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1 text-[11px]">PersistentKeepalive</span>
                        <input
                          type="text"
                          value={newProfile.awg_persistent_keepalive || "25-35"}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_persistent_keepalive: e.target.value })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px]"
                        />
                      </div>
                    </div>
                  </div>
                ) : newProfile.protocol === "awg2" ? (
                  <div className="space-y-3 pt-1">
                    <div className="flex items-center justify-between p-3 rounded-xl bg-teal-950/25 border border-teal-500/30">
                      <div>
                        <div className="text-xs font-bold text-teal-300">
                          Параметры протокола AmneziaWG 2.0
                        </div>
                        <div className="text-[11px] text-slate-400">
                          Классическая обфускация WireGuard со случайными 32-битными заголовками H1-H4
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          const r = generateRandomAwg20Params();
                          setNewProfile({ ...newProfile, ...r });
                          showNotice("success", "Параметры AWG 2.0 рандомизированы!");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-teal-600/30 hover:bg-teal-600/50 border border-teal-500/40 text-xs font-semibold text-teal-300 flex items-center gap-1.5 transition"
                      >
                        🎲 Рандомизировать параметры 2.0
                      </button>
                    </div>

                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Приватный ключ клиента (Base64)</label>
                        <input
                          type="text"
                          placeholder="Приватный ключ"
                          value={newProfile.password}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, password: e.target.value, awg_private_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Публичный ключ сервера (Base64)</label>
                        <input
                          type="text"
                          placeholder="Публичный ключ сервера"
                          value={newProfile.awg_public_key || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_public_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">IP клиента (Address)</label>
                        <input
                          type="text"
                          placeholder="10.8.1.2/32"
                          value={newProfile.awg_local_address || "10.8.1.2/32"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_local_address: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">PresharedKey (Base64 32 байта, опционально)</label>
                        <input
                          type="text"
                          placeholder="Base64 32 байта"
                          value={newProfile.awg_preshared_key || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_preshared_key: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Диапазон авто-подбора портов (Port Hunting)</label>
                        <input
                          type="text"
                          placeholder="20000:50000"
                          value={newProfile.awg_hop_ports || "20000:50000"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_hop_ports: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>

                    {/* AWG 2.0 Obfuscation params */}
                    <div className="p-3 rounded-xl bg-[#070b14] border border-slate-800 grid grid-cols-4 gap-2 text-xs">
                      <div>
                        <span className="text-slate-400 block mb-1">Jc (Мусорные пакеты)</span>
                        <input
                          type="number"
                          value={newProfile.awg_jc ?? 4}
                          onChange={(e) => setNewProfile({ ...newProfile, awg_jc: Number(e.target.value) || 4 })}
                          className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                        />
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">Jmin / Jmax</span>
                        <div className="flex items-center gap-1">
                          <input
                            type="number"
                            value={newProfile.awg_jmin ?? 20}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_jmin: Number(e.target.value) || 20 })}
                            className="w-full px-1.5 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_jmax ?? 65}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_jmax: Number(e.target.value) || 65 })}
                            className="w-full px-1.5 py-1 rounded bg-[#0b101d] border border-slate-700 text-white"
                          />
                        </div>
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">S1 / S2</span>
                        <div className="grid grid-cols-2 gap-1">
                          <input
                            type="number"
                            value={newProfile.awg_s1 ?? 30}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s1: Number(e.target.value) || 30 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                          <input
                            type="number"
                            value={newProfile.awg_s2 ?? 30}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_s2: Number(e.target.value) || 30 })}
                            className="w-full px-1 py-1 rounded bg-[#0b101d] border border-slate-700 text-white text-[11px] text-center"
                          />
                        </div>
                      </div>
                      <div>
                        <span className="text-slate-400 block mb-1">H1-H4 (32-бит числа)</span>
                        <div className="grid grid-cols-2 gap-1">
                          <input
                            type="text"
                            value={newProfile.awg_h1 ?? "10000001"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h1: e.target.value })}
                            className="w-full px-1 py-0.5 rounded bg-[#0b101d] border border-slate-700 text-white text-[10px] text-center font-mono"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h2 ?? "20000002"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h2: e.target.value })}
                            className="w-full px-1 py-0.5 rounded bg-[#0b101d] border border-slate-700 text-white text-[10px] text-center font-mono"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h3 ?? "30000003"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h3: e.target.value })}
                            className="w-full px-1 py-0.5 rounded bg-[#0b101d] border border-slate-700 text-white text-[10px] text-center font-mono"
                          />
                          <input
                            type="text"
                            value={newProfile.awg_h4 ?? "40000004"}
                            onChange={(e) => setNewProfile({ ...newProfile, awg_h4: e.target.value })}
                            className="w-full px-1 py-0.5 rounded bg-[#0b101d] border border-slate-700 text-white text-[10px] text-center font-mono"
                          />
                        </div>
                      </div>
                    </div>
                  </div>
                ) : newProfile.protocol === "ssh" ? (
                  <div className="space-y-3">
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Пользователь SSH</label>
                        <input
                          type="text"
                          placeholder="root"
                          value={newProfile.sni || "root"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, sni: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Пароль SSH или приватный ключ</label>
                        <input
                          type="password"
                          placeholder="Пароль root или OpenSSH ключ"
                          value={newProfile.password}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, password: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                        />
                      </div>
                    </div>
                    <div className="p-3 rounded-xl bg-violet-950/30 border border-violet-500/30 text-xs text-violet-200 flex items-start gap-2">
                      <span className="text-base">🛡️</span>
                      <div>
                        <span className="font-bold">SSH-туннель (порт 22)</span> — 100% защита от кабельных блокировок (ТСПУ).
                        Российские провайдеры никогда не блокируют SSH (порт 22). Вся веб-активность и DNS шифруются через TCP-туннель.
                      </div>
                    </div>
                  </div>
                ) : newProfile.protocol === "shadowsocks" ? (
                  <div className="grid grid-cols-3 gap-3">
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Метод шифрования</label>
                      <select
                        value={newProfile.ss_method || "2022-blake3-aes-128-gcm"}
                        onChange={(e) =>
                          setNewProfile({ ...newProfile, ss_method: e.target.value })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                      >
                        <option value="2022-blake3-aes-128-gcm">2022-blake3-aes-128-gcm (SS 2022)</option>
                        <option value="2022-blake3-aes-256-gcm">2022-blake3-aes-256-gcm (SS 2022)</option>
                        <option value="aes-256-gcm">aes-256-gcm (AEAD)</option>
                        <option value="aes-128-gcm">aes-128-gcm</option>
                        <option value="chacha20-ietf-poly1305">chacha20-ietf-poly1305</option>
                      </select>
                    </div>
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Пароль / PSK ключ</label>
                      <input
                        type="text"
                        placeholder="Пароль или base64 ключ"
                        value={newProfile.password}
                        onChange={(e) =>
                          setNewProfile({ ...newProfile, password: e.target.value })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Плагин (опционально)</label>
                      <input
                        type="text"
                        placeholder="Например: v2ray-plugin или obfs-local"
                        value={newProfile.ss_plugin || ""}
                        onChange={(e) =>
                          setNewProfile({ ...newProfile, ss_plugin: e.target.value })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                      />
                    </div>
                  </div>
                ) : (
                  <div className="grid grid-cols-3 gap-3">
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">
                        {newProfile.protocol === "vless" ? "UUID пользователя" : "Пароль"}
                      </label>
                      <input
                        type="text"
                        placeholder={newProfile.protocol === "vless" ? "00000000-0000-0000-0000-000000000000" : "Пароль авторизации"}
                        value={newProfile.password}
                        onChange={(e) =>
                          setNewProfile({ ...newProfile, password: e.target.value })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                      />
                    </div>
                    {newProfile.protocol === "tuic" ? (
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">
                          UUID пользователя (для TUIC)
                        </label>
                        <input
                          type="text"
                          placeholder="00000000-0000-0000-0000-000000000000"
                          value={newProfile.uuid || ""}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, uuid: e.target.value })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    ) : (
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">
                          Пароль Salamander Obfs (опционально)
                        </label>
                        <input
                          type="text"
                          placeholder="Оставьте пустым, если без obfs"
                          value={newProfile.obfs_password || ""}
                          onChange={(e) =>
                            setNewProfile({
                              ...newProfile,
                              obfs_password: e.target.value,
                            })
                          }
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    )}
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">SNI (Server Name)</label>
                      <input
                        type="text"
                        value={newProfile.sni}
                        onChange={(e) =>
                          setNewProfile({ ...newProfile, sni: e.target.value })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                      />
                    </div>
                    {newProfile.protocol === "vless" && (
                      <div className="col-span-3 pt-1">
                        <label className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            checked={newProfile.tls_fragment ?? true}
                            onChange={(e) =>
                              setNewProfile({ ...newProfile, tls_fragment: e.target.checked })
                            }
                            className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-emerald-500 focus:ring-emerald-500"
                          />
                          <span className="text-xs text-slate-300">
                            🛡️ Анти-DPI фрагментация TLS (ClientHello record splitting — обход ТСПУ на кабельном интернете)
                          </span>
                        </label>
                      </div>
                    )}
                  </div>
                )}

                <div className="flex justify-end gap-2">
                  <button
                    onClick={() => setShowNewProfileForm(false)}
                    className="px-4 py-2 rounded-xl bg-slate-800 text-xs text-slate-300"
                  >
                    Отмена
                  </button>
                  <button
                    onClick={handleSaveManualProfile}
                    className="px-5 py-2 rounded-xl bg-cyan-600 hover:bg-cyan-500 text-xs font-bold text-white"
                  >
                    Сохранить профиль
                  </button>
                </div>
              </div>
            )}

              {/* Client Tunnels List */}
              <div className="space-y-2.5 pt-1">
                {effectiveClientProfiles.length === 0 ? (
                  <div className="p-6 rounded-xl bg-[#070b14] border border-slate-800/80 text-center space-y-2">
                    <Monitor className="w-8 h-8 text-slate-500 mx-auto" />
                    <div className="text-xs font-semibold text-slate-300">
                      Нет установленных туннелей в приложении
                    </div>
                    <p className="text-[11px] text-slate-400 max-w-md mx-auto">
                      Создайте быстрый SSH-туннель, добавьте конфигурацию вручную или вставьте готовую ссылку vless:// выше.
                    </p>
                  </div>
                ) : (
                  effectiveClientProfiles.map((prof) => (
                    <div
                      key={prof.id}
                      className={`p-4 rounded-xl border transition flex items-center justify-between gap-3 ${
                        selectedProfileId === prof.id
                          ? "bg-[#131d33] border-cyan-500/50"
                          : "bg-[#070b14] border-slate-800/90 hover:border-slate-700"
                      }`}
                    >
                      <div className="flex items-center gap-3.5 min-w-0">
                        {renderProtocolBadge(prof.protocol, prof.awg_version)}
                        <div className="min-w-0">
                          <div className="text-sm font-bold text-white flex items-center gap-2 truncate">
                            <span>{prof.name}</span>
                            {(prof.protocol === "amneziawg" || prof.protocol === "awg2") && !(prof.awg_private_key || prof.password)?.trim() && (
                              <span
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleOpenEditModal(prof);
                                }}
                                className="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/30 font-medium flex items-center gap-1 cursor-pointer hover:bg-amber-500/30 shrink-0"
                                title="Нажмите для ввода ключа"
                              >
                                <AlertTriangle className="w-3 h-3 text-amber-400" />
                                Нет Private Key
                              </span>
                            )}
                            {selectedProfileId === prof.id && (
                              <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 font-mono font-medium">
                                Активен на ПК
                              </span>
                            )}
                          </div>
                          <div className="text-xs text-slate-400 mt-0.5 font-mono truncate flex items-center gap-2 flex-wrap">
                            <span>
                              {prof.server}:{prof.port}
                              {prof.protocol === "ssh"
                                ? ` • SSH порт ${prof.port} • 100% обход DPI`
                                : prof.protocol === "awg2"
                                ? ` • AWG 2.0 (Jc: ${prof.awg_jc ?? 4}) ${prof.awg_hop_ports ? `• Hunting (${prof.awg_hop_ports})` : ""}`
                                : prof.protocol === "amneziawg"
                                ? ` • Amnezia 3.1 (Jc: ${prof.awg_jc ?? 5}) ${prof.awg_hop_ports ? `• Hunting (${prof.awg_hop_ports})` : ""}`
                                : prof.protocol === "shadowsocks"
                                ? ` • ${prof.ss_method || "2022-blake3-aes-128-gcm"}`
                                : ` • SNI: ${prof.sni}`}
                              {prof.hop_ports ? ` • Port Hopping (${prof.hop_ports})` : ""}
                            </span>
                            <span
                              className="text-[10px] px-2 py-0.5 rounded-md bg-slate-900/90 text-cyan-300 border border-cyan-500/30 font-mono inline-flex items-center gap-1.5"
                              title="Суммарный трафик, прошедший через данную конфигурацию"
                            >
                              <span>↓ {formatBytes(prof.total_down_bytes || 0)}</span>
                              <span className="text-slate-600">•</span>
                              <span>↑ {formatBytes(prof.total_up_bytes || 0)}</span>
                              <span className="text-slate-600">•</span>
                              <span className="text-emerald-300 font-semibold">
                                Σ {formatBytes((prof.total_down_bytes || 0) + (prof.total_up_bytes || 0))}
                              </span>
                            </span>
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-1.5 flex-wrap justify-end shrink-0">
                        {(prof.protocol === "amneziawg" || prof.protocol === "awg2") && (
                          <>
                            <button
                              onClick={() => handleDownloadAwgConf(prof)}
                              className="px-2.5 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-xs font-semibold text-emerald-300 flex items-center gap-1.5"
                              title="Скачать .conf для AmneziaVPN"
                            >
                              <FileDown className="w-3.5 h-3.5" />
                              .conf
                            </button>
                            <button
                              onClick={() => handleProbeAwgPorts(prof.id)}
                              disabled={probingAwgId === prof.id}
                              className="px-2.5 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-xs font-semibold text-cyan-300 flex items-center gap-1"
                              title="Тест портов AWG"
                            >
                              <Zap className="w-3.5 h-3.5 text-cyan-400" />
                              {probingAwgId === prof.id ? "Тест..." : "Тест портов"}
                            </button>
                          </>
                        )}

                        <button
                          onClick={() => handlePingProfile(prof.id)}
                          disabled={pingingId === prof.id}
                          className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 flex items-center gap-1"
                          title="Проверить пинг"
                        >
                          <Activity className="w-3.5 h-3.5 text-emerald-400" />
                          {pingingId === prof.id
                            ? "..."
                            : prof.ping_ms != null
                            ? `${prof.ping_ms} мс`
                            : "Пинг"}
                        </button>

                        <button
                          onClick={async () => {
                            await handlePinProfileOnPc(prof.id);
                            handleQuickSwitchProfile(prof.id);
                            if ((prof.protocol === "amneziawg" || prof.protocol === "awg2") && !(prof.awg_private_key || prof.password)?.trim()) {
                              showNotice("info", "Внимание: в профиле Amnezia не указан приватный ключ (Private Key). Введите ключ в настройках (✏️) или выберите .conf файл.");
                            }
                            setActiveTab("dashboard");
                          }}
                          className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                            selectedProfileId === prof.id
                              ? "bg-cyan-600 text-white"
                              : "bg-slate-800 hover:bg-slate-700 text-slate-200"
                          }`}
                        >
                          На ПК
                        </button>

                        <button
                          onClick={() => handleCopyUri(prof)}
                          className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 flex items-center gap-1"
                          title="Скопировать ссылку"
                        >
                          {copiedId === prof.id ? (
                            <Check className="w-3.5 h-3.5 text-emerald-400" />
                          ) : (
                            <Copy className="w-3.5 h-3.5 text-cyan-400" />
                          )}
                        </button>

                        <button
                          onClick={() => handleOpenEditModal(prof)}
                          className="px-2.5 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-xs font-semibold text-cyan-300 flex items-center gap-1.5 transition"
                          title="Редактировать конфигурацию в приложении"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                          Изменить
                        </button>

                        <button
                          onClick={() => handleDeleteProfile(prof.id)}
                          className="p-1.5 rounded-lg bg-rose-950/40 hover:bg-rose-900/60 text-rose-400"
                          title="Удалить профиль"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* ========================================================================= */}
            {/* БЛОК 2: СИНХРОНИЗАЦИЯ С VPS И КЛЮЧИ УСТРОЙСТВ (ТЕЛЕФОН, ДРУЗЬЯ)            */}
            {/* ========================================================================= */}
            <div className="p-5 rounded-2xl bg-[#0d1424] border border-indigo-500/30 space-y-4 shadow-sm">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400 shadow-sm shrink-0">
                  <Cloud className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-white flex items-center gap-2">
                    Синхронизация с сервером и ключи устройств (Телефон, Ноутбук, Друзья)
                    {serverProfiles.length > 0 && (
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 font-semibold font-mono">
                        {serverProfiles.length}
                      </span>
                    )}
                  </h3>
                  <p className="text-[11px] text-slate-400">
                    Синхронизация со списком на вашем сервере VPS и выпуск отдельных ключей для мобильных устройств и друзей.
                  </p>
                </div>
              </div>

              {/* VPS OWNER AUTHENTICATION & DEVICE KEY ISSUANCE */}
              {!isVpsAdmin ? (
                <div className="p-4 rounded-xl bg-[#090e1c] border border-slate-800 space-y-3 shadow-inner">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2.5">
                      <NeuralWiresIcon className="w-5 h-5" />
                      <div>
                        <h4 className="text-xs font-bold text-white flex items-center gap-2">
                          Вход для владельца VPS (Управление сервером и синхронизация)
                        </h4>
                        <p className="text-[11px] text-slate-400">
                          Авторизуйтесь по SSH, чтобы подтянуть устройства с сервера и разблокировать создание новых ключей.
                        </p>
                      </div>
                    </div>
                    <span className="text-[10px] px-2.5 py-0.5 rounded-full bg-slate-800 text-slate-400 border border-slate-700 font-semibold">
                      Режим пользователя
                    </span>
                  </div>

                  <div className="grid grid-cols-12 gap-3 items-end pt-1">
                    <div className="col-span-4">
                      <label className="block text-[11px] text-slate-400 mb-1">IP-адрес / Хост VPS</label>
                      <input
                        type="text"
                        placeholder="Например: 185.100.85.10"
                        value={vpsCreds.host}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, host: e.target.value.trim() })}
                        className="w-full px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                    </div>

                    <div className="col-span-2">
                      <label className="block text-[11px] text-slate-400 mb-1">Логин SSH</label>
                      <input
                        type="text"
                        placeholder="root"
                        value={vpsCreds.username || "root"}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, username: e.target.value.trim() })}
                        className="w-full px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                    </div>

                    <div className="col-span-3">
                      <label className="block text-[11px] text-slate-400 mb-1">Пароль SSH (root)</label>
                      <input
                        type="password"
                        placeholder="Пароль root от VPS"
                        value={vpsCreds.password}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, password: e.target.value })}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") handleLoginVpsAdmin();
                        }}
                        className="w-full px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                    </div>

                    <div className="col-span-3">
                      <button
                        onClick={handleLoginVpsAdmin}
                        disabled={adminLoginLoading || !vpsCreds.host.trim() || !vpsCreds.password.trim()}
                        className="w-full py-1.5 px-3 rounded-lg bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 disabled:opacity-40 text-xs font-bold text-white shadow-md shadow-cyan-600/20 flex items-center justify-center gap-1.5 transition"
                      >
                        <Key className="w-3.5 h-3.5" />
                        {adminLoginLoading ? "Подключение к VPS..." : "Войти как владелец"}
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="space-y-3">
                  {/* Logged in as VPS Owner Banner */}
                  <div className="p-3.5 rounded-xl bg-emerald-950/25 border border-emerald-500/40 flex items-center justify-between">
                    <div className="flex items-center gap-2.5">
                      <div className="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse" />
                      <div>
                        <span className="text-xs font-bold text-emerald-300 flex items-center gap-2">
                          <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                          Авторизован как Владелец сервера: {vpsCreds.host}
                        </span>
                        <p className="text-[10px] text-slate-400 mt-0.5">
                          Устройства синхронизированы с VPS. Выпуск новых ключей разблокирован.
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={handleLoginVpsAdmin}
                        disabled={adminLoginLoading}
                        className="px-3 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 flex items-center gap-1.5 transition"
                      >
                        <RefreshCw className={`w-3.5 h-3.5 ${adminLoginLoading ? "animate-spin" : ""}`} />
                        Синхронизировать
                      </button>
                      <button
                        onClick={handleLogoutVpsAdmin}
                        className="px-3 py-1 rounded-lg bg-rose-950/40 hover:bg-rose-900/40 border border-rose-500/30 text-xs font-medium text-rose-300 transition"
                      >
                        Выйти
                      </button>
                    </div>
                  </div>

                  {/* Create Key on VPS (UNLOCKED) */}
                  <div className="p-4 rounded-xl bg-[#090e1c] border border-cyan-500/30 space-y-2.5">
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-bold uppercase tracking-wider text-cyan-300 flex items-center gap-2">
                        <UserPlus className="w-4 h-4 text-cyan-400" />
                        Выпустить новый ключ на сервере ({vpsCreds.host})
                      </span>
                      <span className="text-[10px] text-slate-400">
                        Создает отдельного пользователя в /etc/sing-box/config.json на сервере
                      </span>
                    </div>

                    <div className="grid grid-cols-12 gap-3 items-end">
                      <div className="col-span-5">
                        <label className="block text-[11px] text-slate-400 mb-1">
                          Название устройства / имя друга
                        </label>
                        <input
                          type="text"
                          placeholder="Например: Мой iPhone, Ноутбук, Друг..."
                          value={deviceNameInput}
                          onChange={(e) => setDeviceNameInput(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") handleCreateDeviceKey();
                          }}
                          className="w-full px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                        />
                      </div>

                      <div className="col-span-4">
                        <label className="block text-[11px] text-slate-400 mb-1">Протокол</label>
                        <select
                          value={deviceProtocol}
                          onChange={(e) =>
                            setDeviceProtocol(e.target.value as "vless" | "hysteria2" | "tuic" | "shadowsocks" | "amneziawg" | "awg2")
                          }
                          className="w-full px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                        >
                          <option value="vless">VLESS Reality (TCP 443 — для кабельного интернета)</option>
                          <option value="hysteria2">Hysteria 2 (UDP скоростной)</option>
                          <option value="tuic">TUIC v5</option>
                          <option value="shadowsocks">Shadowsocks (TCP/UDP классический прокси)</option>
                          <option value="amneziawg">Amnezia 3.1 (AWG 3.1 — Header Protection + Anti-DPI)</option>
                        </select>
                      </div>

                      <div className="col-span-3">
                        <button
                          onClick={handleCreateDeviceKey}
                          disabled={creatingDevice || !deviceNameInput.trim()}
                          className="w-full py-1.5 px-3 rounded-lg bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 disabled:opacity-40 text-xs font-bold text-white shadow-md shadow-cyan-600/20 flex items-center justify-center gap-1.5 transition"
                        >
                          <QrCode className="w-3.5 h-3.5" />
                          {creatingDevice ? "Создание на VPS..." : "+ Создать ключ и QR"}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Server Synced Devices List */}
              <div className="space-y-2.5 pt-1">
                {serverProfiles.length === 0 ? (
                  <div className="p-6 rounded-xl bg-[#090d18] border border-slate-800/80 text-center space-y-2">
                    <Smartphone className="w-8 h-8 text-slate-500 mx-auto" />
                    <div className="text-xs font-semibold text-slate-300">
                      Отдельные ключи для устройств пока не созданы
                    </div>
                    <p className="text-[11px] text-slate-400 max-w-md mx-auto">
                      Авторизуйтесь на сервере выше, введите имя устройства (например, «Мой iPhone» или «Ноутбук») и нажмите «+ Создать ключ и QR», чтобы сгенерировать конфигурацию и отсканировать QR-код с экрана телефона.
                    </p>
                  </div>
                ) : (
                  serverProfiles.map((prof) => (
                    <div
                      key={prof.id}
                      className="p-3.5 rounded-xl border border-indigo-500/20 bg-[#0c1222] hover:border-indigo-500/40 transition flex items-center justify-between gap-3"
                    >
                      <div className="flex items-center gap-3 min-w-0">
                        <div className="w-9 h-9 rounded-xl bg-indigo-500/15 border border-indigo-500/30 flex items-center justify-center text-indigo-300 shrink-0">
                          <Smartphone className="w-4 h-4" />
                        </div>
                        {renderProtocolBadge(prof.protocol, prof.awg_version)}
                        <div className="min-w-0">
                          <div className="text-sm font-bold text-white flex items-center gap-2 truncate">
                            <span>{prof.name}</span>
                            <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 font-medium">
                              Устройство на сервере
                            </span>
                          </div>
                          <div className="text-xs text-slate-400 mt-0.5 font-mono truncate flex items-center gap-2 flex-wrap">
                            <span>{prof.server}:{prof.port}</span>
                            <span
                              className="text-[10px] px-2 py-0.5 rounded-md bg-slate-900/90 text-indigo-200 border border-indigo-500/30 font-mono inline-flex items-center gap-1.5"
                              title="Трафик по данной конфигурации (по данным сервера VPS и локальной статистики)"
                            >
                              <span>↓ {formatBytes(prof.total_down_bytes || 0)}</span>
                              <span className="text-slate-600">•</span>
                              <span>↑ {formatBytes(prof.total_up_bytes || 0)}</span>
                              <span className="text-slate-600">•</span>
                              <span className="text-emerald-300 font-semibold">
                                Σ {formatBytes((prof.total_down_bytes || 0) + (prof.total_up_bytes || 0))}
                              </span>
                            </span>
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-2 flex-wrap justify-end shrink-0">
                        <button
                          onClick={() => handleOpenQrModal(prof)}
                          className="px-3 py-1.5 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-xs font-bold text-white flex items-center gap-1.5 shadow-md shadow-cyan-600/20 transition"
                          title="Показать QR-код для телефона"
                        >
                          <QrCode className="w-3.5 h-3.5" />
                          QR-код для телефона
                        </button>

                        <button
                          onClick={() => handleExportJson(prof)}
                          className="px-2.5 py-1.5 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-xs font-semibold text-indigo-300 flex items-center gap-1.5"
                          title="Скачать готовый .json конфиг sing-box"
                        >
                          <FileDown className="w-3.5 h-3.5" />
                          .json
                        </button>

                        {(prof.protocol === "amneziawg" || prof.protocol === "awg2") && (
                          <button
                            onClick={() => handleDownloadAwgConf(prof)}
                            className="px-2.5 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-xs font-semibold text-emerald-300 flex items-center gap-1.5"
                            title="Скачать .conf для AmneziaVPN"
                          >
                            <FileDown className="w-3.5 h-3.5" />
                            .conf Amnezia
                          </button>
                        )}

                        <button
                          onClick={() => handleCopyUri(prof)}
                          className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 flex items-center gap-1.5"
                          title="Скопировать ссылку"
                        >
                          {copiedId === prof.id ? (
                            <Check className="w-3.5 h-3.5 text-emerald-400" />
                          ) : (
                            <Copy className="w-3.5 h-3.5 text-cyan-400" />
                          )}
                          Ссылка
                        </button>

                        <button
                          onClick={async () => {
                            await handlePinProfileOnPc(prof.id);
                            handleQuickSwitchProfile(prof.id);
                            setActiveTab("dashboard");
                          }}
                          className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200"
                          title="Использовать на этом ПК"
                        >
                          На ПК
                        </button>

                        <button
                          onClick={() => handleDeleteProfile(prof.id, true)}
                          className="p-1.5 rounded-lg bg-rose-950/40 hover:bg-rose-900/60 text-rose-400 cursor-pointer"
                          title="Удалить ключ с сервера VPS и из приложения"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* QR Code & Mobile Setup Modal */}
            {qrModalProfile && (
              <div className="fixed inset-0 bg-black/75 backdrop-blur-sm z-50 flex items-center justify-center p-4">
                <div className="max-w-xl w-full p-5 rounded-2xl bg-[#111827] border border-cyan-500/40 shadow-2xl space-y-3.5 max-h-[92vh] overflow-y-auto">
                  <div className="flex items-center justify-between border-b border-slate-800 pb-3">
                    <div>
                      <h3 className="text-base font-bold text-white flex items-center gap-2">
                        <Smartphone className="w-5 h-5 text-cyan-400" />
                        Подключение смартфона: {qrModalProfile.name}
                      </h3>
                      <p className="text-xs text-slate-400 mt-0.5">
                        Сервер: {qrModalProfile.server}:{qrModalProfile.port} (
                        {qrModalProfile.protocol === "ssh"
                          ? "SSH Tunnel"
                          : qrModalProfile.protocol === "vless"
                          ? "VLESS Reality"
                          : qrModalProfile.protocol === "tuic"
                          ? "TUIC v5"
                          : qrModalProfile.protocol === "shadowsocks"
                          ? "Shadowsocks"
                          : qrModalProfile.protocol === "awg2"
                          ? "AmneziaWG 2.0"
                          : qrModalProfile.protocol === "amneziawg"
                          ? "Amnezia 3.1"
                          : "Hysteria 2"}
                        )
                      </p>
                    </div>
                    <button
                      onClick={() => setQrModalProfile(null)}
                      className="px-3 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs text-slate-300"
                    >
                      Закрыть
                    </button>
                  </div>

                  {/* Quick Protocol Switcher inside QR Modal */}
                  {profiles.some(
                    (p) =>
                      p.server === qrModalProfile.server &&
                      p.protocol !== qrModalProfile.protocol
                  ) && (
                    <div className="flex flex-wrap items-center justify-between p-2.5 rounded-xl bg-[#0b101d] border border-purple-500/30 gap-2">
                      <span className="text-xs text-slate-300">
                        {qrModalProfile.protocol === "hysteria2"
                          ? "Для Hiddify / Shadowsocks на телефоне можно переключить:"
                          : "Другие протоколы для этого сервера:"}
                      </span>
                      <div className="flex items-center gap-1.5 flex-wrap">
                        {profiles
                          .filter(
                            (p) =>
                              p.server === qrModalProfile.server &&
                              p.protocol !== qrModalProfile.protocol
                          )
                          .map((alt) => (
                            <button
                              key={alt.id}
                              onClick={() => handleOpenQrModal(alt)}
                              className="px-2.5 py-1 rounded-lg bg-purple-600/30 hover:bg-purple-600/50 border border-purple-500/40 text-xs font-semibold text-purple-200"
                            >
                              {alt.protocol === "ssh"
                                ? "SSH Tunnel"
                                : alt.protocol === "vless"
                                ? "VLESS Reality"
                                : alt.protocol === "hysteria2"
                                ? "Hysteria 2"
                                : alt.protocol === "tuic"
                                ? "TUIC v5"
                                : alt.protocol === "awg2"
                                ? "AWG 2.0"
                                : alt.protocol === "amneziawg"
                                ? "Amnezia 3.1"
                                : "Shadowsocks"}
                            </button>
                          ))}
                      </div>
                    </div>
                  )}

                  <div className="flex flex-col items-center justify-center py-1">
                    <div className="p-3 bg-white rounded-2xl shadow-xl">
                      <img
                        src={qrDataUrl}
                        alt="VPN QR Code"
                        className="w-52 h-52 block"
                      />
                    </div>
                  </div>

                  {/* Quick Actions */}
                  <div className={`grid ${(qrModalProfile.protocol === "amneziawg" || qrModalProfile.protocol === "awg2") ? "grid-cols-3" : "grid-cols-2"} gap-2.5`}>
                    <button
                      onClick={() => handleCopyUri(qrModalProfile)}
                      className="py-2.5 px-3 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-cyan-300 flex items-center justify-center gap-1.5"
                    >
                      <Copy className="w-4 h-4" />
                      Ссылка{" "}
                      {qrModalProfile.protocol === "ssh"
                        ? "ssh://"
                        : qrModalProfile.protocol === "vless"
                        ? "vless://"
                        : qrModalProfile.protocol === "tuic"
                        ? "tuic://"
                        : qrModalProfile.protocol === "shadowsocks"
                        ? "ss://"
                        : qrModalProfile.protocol === "awg2"
                        ? "awg2://"
                        : qrModalProfile.protocol === "amneziawg"
                        ? "awg://"
                        : "hysteria2://"}
                    </button>
                    {(qrModalProfile.protocol === "amneziawg" || qrModalProfile.protocol === "awg2") && (
                      <button
                        onClick={() => handleDownloadAwgConf(qrModalProfile)}
                        className="py-2.5 px-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-xs font-bold text-white flex items-center justify-center gap-1.5"
                      >
                        <FileDown className="w-4 h-4" />
                        Скачать .conf (Amnezia)
                      </button>
                    )}
                    <button
                      onClick={() => handleExportJson(qrModalProfile)}
                      className="py-2.5 px-3 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-xs font-bold text-white flex items-center justify-center gap-1.5"
                    >
                      <FileDown className="w-4 h-4" />
                      Скачать .json
                    </button>
                  </div>

                  {/* Troubleshooting Box for Hiddify Timeout / Unknown IP */}
                  <div className="p-3.5 rounded-xl bg-amber-950/30 border border-amber-500/40 text-xs text-amber-100 space-y-1.5">
                    <div className="font-bold text-amber-300">
                      Почему в Hiddify пишет «Тайм-аут» и «Неизвестный IP»? Как исправить за 15 секунд:
                    </div>
                    <div>
                      1. <b>Используйте протокол TUIC v5 (порт 9443)</b>: в мобильном Hiddify есть баг парсера ссылок Hysteria 2 с паролем обфускации <code>obfs=salamander</code> (приложение теряет пароль обфускации и сервер отбрасывает пакеты). Нажмите кнопку <b>«Переключить QR на TUIC v5»</b> выше и отсканируйте его!
                    </div>
                    <div>
                      2. В Hiddify откройте <b>Параметры конфигурации</b> (значок ползунков вверху справа):
                      <br />• <b>Регион</b> → выберите <b>«Другой (other)»</b> (при выборе «Россия» Hiddify ломает проверку соединения).
                      <br />• Включите тумблер <b>«Разрешить небезопасные (Allow Insecure)»</b> (для самоподписанного сертификата VPS).
                    </div>
                    <div>
                      3. <b>Или используйте официальное приложение <code>sing-box</code>, Shadowsocks / v2rayNG / NekoBox</b> (Google Play / App Store) или скачанный <code>.json</code> — там стабильно работают Shadowsocks, TUIC v5 и Hysteria 2.
                    </div>
                  </div>
                </div>
              </div>
            )}

          </div>
        )}

        {/* TAB 4: ROUTING & CORE */}
        {activeTab === "routing" && (
          <div className="flex-1 overflow-y-auto p-6 space-y-5 animate-tab-enter">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                  <Globe className="w-5 h-5 text-cyan-400" />
                  Раздельное туннелирование и маршруты
                </h2>
                <p className="text-xs text-slate-400 mt-1">
                  Индивидуальные правила обхода и списки сайтов для каждого профиля. (Для перехвата трафика любых игр и программ рекомендуется <b>TUN-режим</b>).
                </p>
              </div>
              <button
                type="button"
                onClick={handleSaveRouting}
                className="px-5 py-2.5 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-600/20 shrink-0 cursor-pointer"
              >
                Сохранить и применить
              </button>
            </div>

            {/* Profile Selector for Routing */}
            {effectiveDashboardProfiles.length > 0 && (
              <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-2.5">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold uppercase tracking-wider text-slate-400 flex items-center gap-2">
                    <Smartphone className="w-4 h-4 text-cyan-400" />
                    {t.selectProfileForRouting}
                  </span>
                  <span className="text-[11px] text-slate-400">
                    Правила маршрутизации сохраняются отдельно для выбранного профиля
                  </span>
                </div>
                <div className="flex items-center gap-2.5 overflow-x-auto pb-1">
                  {effectiveDashboardProfiles.map((p) => {
                    const isTarget = p.id === (routingProfileId || selectedProfileId);
                    const isSplitActive = p.routing ? p.routing.split_tunneling : routing.split_tunneling;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => switchRoutingProfile(p.id)}
                        className={`px-3.5 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition shrink-0 border cursor-pointer ${
                          isTarget
                            ? "bg-cyan-500/20 border-cyan-400 text-white shadow-md shadow-cyan-500/20"
                            : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200 hover:border-slate-700"
                        }`}
                      >
                        <span
                          className={`w-2 h-2 rounded-full ${
                            isSplitActive ? "bg-cyan-400 shadow-[0_0_6px_#22d3ee]" : "bg-slate-600"
                          }`}
                        />
                        <span>{p.name}</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-800/80 text-slate-400 border border-slate-700/60 uppercase">
                          {p.protocol}
                        </span>
                        {p.id === selectedProfileId && (
                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-medium">
                            Активен на ПК
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Master Split Tunneling Toggle Card (Top Placement) */}
            <div
              className={`p-5 rounded-2xl border transition-all ${
                routing.split_tunneling
                  ? "bg-[#111827] border-cyan-500/50 shadow-[0_0_30px_-10px_rgba(6,182,212,0.25)]"
                  : "bg-[#111827] border-slate-800/90"
              }`}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3.5">
                  <div
                    className={`p-2.5 rounded-xl border ${
                      routing.split_tunneling
                        ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-400 shadow-[0_0_15px_-3px_rgba(6,182,212,0.4)]"
                        : "bg-slate-800/60 border-slate-700/60 text-slate-400"
                    }`}
                  >
                    <Zap className="w-5 h-5" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2.5">
                      <h3 className="text-sm font-bold text-white">
                        {t.splitTunnelingMasterTitle} «{activeRoutingProfile?.name || "По умолчанию"}»
                      </h3>
                      <span
                        className={`text-[10px] px-2.5 py-0.5 rounded-full font-bold uppercase border ${
                          routing.split_tunneling
                            ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-300"
                            : "bg-slate-800 border-slate-700 text-slate-400"
                        }`}
                      >
                        {routing.split_tunneling
                          ? t.splitTunnelingEnabledBadge
                          : t.splitTunnelingDisabledBadge}
                      </span>
                    </div>
                    <p className="text-xs text-slate-400 mt-1 max-w-2xl leading-relaxed">
                      {t.splitTunnelingMasterDesc}
                    </p>
                  </div>
                </div>

                <button
                  type="button"
                  onClick={handleToggleSplitForProfile}
                  className={`w-14 h-7 rounded-full transition-colors p-0.5 shrink-0 ml-4 cursor-pointer ${
                    routing.split_tunneling
                      ? "bg-cyan-500 shadow-[0_0_15px_rgba(6,182,212,0.5)]"
                      : "bg-slate-700"
                  }`}
                  title={
                    routing.split_tunneling
                      ? "Выключить раздельное туннелирование"
                      : "Включить раздельное туннелирование"
                  }
                >
                  <div
                    className={`w-6 h-6 rounded-full bg-white shadow-md transform transition-transform ${
                      routing.split_tunneling ? "translate-x-7" : "translate-x-0"
                    }`}
                  />
                </button>
              </div>
            </div>

            {/* Anti-DPI TLS Fragmentation Setting */}
            {/* Split Tunneling Inactive Notice */}
            {!routing.split_tunneling && (
              <div className="p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-200 text-xs flex items-center gap-3">
                <AlertTriangle className="w-5 h-5 text-amber-400 shrink-0" />
                <div>
                  <span className="font-bold">Внимание:</span> {t.splitTunnelingDisabledNotice}
                </div>
              </div>
            )}

            {/* Split Tunneling Rules Container */}
            <div
              className={`space-y-5 transition-all duration-300 ${
                !routing.split_tunneling
                  ? "opacity-35 pointer-events-none select-none filter grayscale-[35%]"
                  : ""
              }`}
            >
              {/* 1. ROUTING PRESETS (Перемещен выше режима туннелирования) */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-cyan-500/30 shadow-[0_0_25px_-10px_rgba(6,182,212,0.2)] space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex items-center gap-2.5">
                    <div className="p-2 rounded-xl bg-cyan-500/15 border border-cyan-500/30 text-cyan-400">
                      <Bookmark className="w-4 h-4" />
                    </div>
                    <div>
                      <div className="text-xs font-bold uppercase tracking-wider text-white flex items-center gap-2">
                        <span>Предустановки маршрутизации</span>
                        {status.connected && (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 flex items-center gap-1 font-semibold animate-pulse">
                            <Zap className="w-3 h-3" /> На лету ⚡
                          </span>
                        )}
                      </div>
                      <p className="text-[11px] text-slate-400">
                        Переключайте списки программ в один клик без смены конфигов и разрыва соединения
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    {status.connected && (
                      <button
                        type="button"
                        onClick={handleApplyOnTheFly}
                        disabled={isApplyingPreset}
                        className="px-3 py-1.5 rounded-xl bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/40 text-xs font-semibold text-emerald-300 flex items-center gap-1.5 transition cursor-pointer"
                        title="Применить текущие списки программ к работающему VPN"
                      >
                        <Zap className={`w-3.5 h-3.5 ${isApplyingPreset ? "animate-spin" : ""}`} />
                        <span>Применить к VPN ⚡</span>
                      </button>
                    )}
                    {activePresetId && !routingPresets.find((p) => p.id === activePresetId)?.is_builtin && (
                      <button
                        type="button"
                        onClick={handleUpdateCurrentPreset}
                        className="px-2.5 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs font-semibold text-slate-200 flex items-center gap-1.5 transition cursor-pointer"
                        title="Перезаписать активный пользовательский пресет текущими списками"
                      >
                        <Save className="w-3.5 h-3.5 text-cyan-400" />
                        <span>Обновить пресет</span>
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => {
                        setNewPresetOutbound(routing.default_outbound || "direct");
                        setShowSavePresetModal(true);
                      }}
                      className="px-3 py-1.5 rounded-xl bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/40 text-xs font-semibold text-cyan-200 flex items-center gap-1.5 transition cursor-pointer"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>+ Создать пресет</span>
                    </button>
                  </div>
                </div>

                {/* Preset Chips */}
                <div className="flex flex-wrap items-center gap-2 pt-1">
                  {routingPresets.map((pr) => {
                    const isActive = activePresetId === pr.id;
                    const totalCount = (pr.proxy_processes || []).length + (pr.direct_processes || []).length;
                    return (
                      <div
                        key={pr.id}
                        onClick={() => handleSelectPreset(pr)}
                        className={`group relative flex items-center gap-2 px-3 py-2 rounded-xl border text-xs font-medium transition cursor-pointer select-none ${
                          isActive
                            ? "bg-gradient-to-r from-cyan-600/25 to-indigo-600/25 border-cyan-400 text-white shadow-md shadow-cyan-500/20"
                            : "bg-[#0b101d] border-slate-800 text-slate-300 hover:border-slate-700 hover:text-white"
                        }`}
                      >
                        {isActive ? (
                          <span className="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_8px_#22d3ee] animate-pulse" />
                        ) : (
                          <span className="w-1.5 h-1.5 rounded-full bg-slate-600 group-hover:bg-slate-400" />
                        )}
                        {pr.id.includes("games") ? (
                          <Gamepad2 className="w-3.5 h-3.5 text-emerald-400" />
                        ) : pr.id.includes("discord") ? (
                          <MessageSquare className="w-3.5 h-3.5 text-indigo-400" />
                        ) : pr.id.includes("browser") ? (
                          <Globe className="w-3.5 h-3.5 text-cyan-400" />
                        ) : pr.id.includes("media") ? (
                          <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                        ) : (
                          <Zap className="w-3.5 h-3.5 text-violet-400" />
                        )}
                        <span className="font-semibold">{pr.name}</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-900/80 text-slate-400 border border-slate-800">
                          {pr.default_outbound === "direct" ? "Только выбор" : "Весь трафик"}
                        </span>
                        <span className="text-[10px] font-mono text-cyan-400/90 font-bold">
                          {totalCount}
                        </span>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeletePreset(pr.id);
                          }}
                          className="ml-1 text-slate-500 hover:text-rose-400 p-0.5 opacity-60 group-hover:opacity-100 transition"
                          title="Удалить пресет"
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </div>
                    );
                  })}
                </div>

                {/* Active Preset Description Banner */}
                <div className="pt-1 text-[11px] text-slate-400 flex items-center justify-between border-t border-slate-800/80">
                  <div className="flex items-center gap-1.5 truncate">
                    <span className="text-cyan-400 font-medium">Активный профиль:</span>
                    <span className="text-slate-300">
                      {routingPresets.find((p) => p.id === activePresetId)?.description ||
                        routingPresets.find((p) => p.id === activePresetId)?.name ||
                        "По умолчанию"}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={handleResetPresets}
                    className="text-[10px] text-slate-400 hover:text-cyan-300 transition shrink-0 ml-2"
                    title="Сбросить все предустановки к исходным стандартным"
                  >
                    Восстановить стандартные
                  </button>
                </div>
              </div>

              {/* 2. Default Routing Mode Selector */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-3">
                <div className="text-xs font-bold uppercase tracking-wider text-slate-400">
                  Режим раздельного туннелирования по умолчанию
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <button
                    type="button"
                    onClick={() => {
                      const nextRouting = buildCurrentRoutingObject({
                        default_outbound: "direct",
                      });
                      void syncAndApplyRoutingImmediately(nextRouting);
                    }}
                    className={`p-3.5 rounded-xl border text-left transition flex flex-col justify-between cursor-pointer ${
                      routing.default_outbound === "direct"
                        ? "bg-cyan-500/15 border-cyan-500/60 text-white shadow-[0_0_20px_-5px_rgba(6,182,212,0.25)]"
                        : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200"
                    }`}
                  >
                    <div className="flex flex-wrap items-center justify-between gap-1.5 mb-1.5">
                      <div className="flex items-center gap-2 min-w-0">
                        <Shield className="w-4 h-4 text-cyan-400 shrink-0" />
                        <span className="text-xs font-bold text-cyan-300">
                          Только выбранное через VPN (Белый список)
                        </span>
                      </div>
                      {routing.default_outbound === "direct" && (
                        <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 shrink-0 whitespace-nowrap">
                          Активен (колонка «Через VPN» ↓)
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] text-slate-400 leading-relaxed">
                      Весь ПК и игры работают напрямую без VPN, а через VPN идут ТОЛЬКО программы, папки и сайты из левого мастер-блока «Через VPN» ниже.
                    </div>
                  </button>

                  <button
                    type="button"
                    onClick={() => {
                      const nextRouting = buildCurrentRoutingObject({
                        default_outbound: "proxy",
                      });
                      void syncAndApplyRoutingImmediately(nextRouting);
                    }}
                    className={`p-3.5 rounded-xl border text-left transition flex flex-col justify-between cursor-pointer ${
                      (routing.default_outbound || "proxy") === "proxy"
                        ? "bg-emerald-500/15 border-emerald-500/60 text-white shadow-[0_0_20px_-5px_rgba(16,185,129,0.25)]"
                        : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200"
                    }`}
                  >
                    <div className="flex flex-wrap items-center justify-between gap-1.5 mb-1.5">
                      <div className="flex items-center gap-2 min-w-0">
                        <Zap className="w-4 h-4 text-emerald-400 shrink-0" />
                        <span className="text-xs font-bold text-emerald-300">
                          Весь трафик через VPN (Черный список)
                        </span>
                      </div>
                      {(routing.default_outbound || "proxy") === "proxy" && (
                        <span className="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 shrink-0 whitespace-nowrap">
                          Активен (колонка «Мимо VPN» ↓)
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] text-slate-400 leading-relaxed">
                      Все программы и сайты идут через VPN, а выбранные в правом мастер-блоке «Мимо VPN» ниже и RU-сайты — напрямую.
                    </div>
                  </button>
                </div>
              </div>

              {/* 3. ДВА РАЗДЕЛЬНЫХ МАСТЕР-БЛОКА: ЧЕРЕЗ VPN (Слева) и МИМО VPN (Справа) */}
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
                {/* МАСТЕР-БЛОК 1: ЧЕРЕЗ VPN (Туннелируемые ресурсы) */}
                <div
                  className={`p-5 rounded-2xl bg-[#0b1220]/90 border-2 transition-all space-y-4 ${
                    routing.default_outbound === "direct"
                      ? "border-cyan-500/70 shadow-[0_0_30px_-6px_rgba(6,182,212,0.35)]"
                      : "border-cyan-500/30 opacity-90"
                  }`}
                >
                  {/* Master Header Left */}
                  <div className="flex items-start justify-between gap-3 pb-3.5 border-b border-cyan-500/25">
                    <div className="flex items-start gap-3 min-w-0 flex-1">
                      <div className="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shrink-0 mt-0.5">
                        <Shield className="w-4 h-4 text-cyan-400" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="text-xs font-bold uppercase tracking-wider text-cyan-300 flex items-center gap-2 flex-wrap">
                          <span>Через VPN (Только выбранное)</span>
                          {routing.default_outbound === "direct" && (
                            <span className="text-[9px] px-2 py-0.5 rounded-full bg-cyan-500/25 text-cyan-200 border border-cyan-500/40 font-semibold font-mono shrink-0">
                              Активный режим
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] text-cyan-400/80 mt-1 leading-relaxed">
                          Программы, папки, домены и IP-адреса, направляемые в защищенный туннель
                        </div>
                      </div>
                    </div>
                    <span className="text-[10px] px-2.5 py-1 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold font-mono shrink-0 mt-0.5">
                      Proxy Route
                    </span>
                  </div>

                  {/* 1.1: Программы и папки через VPN */}
                  <div className="p-4 rounded-xl bg-[#080d19] border border-cyan-500/20 space-y-2.5">
                    <div className="flex items-center gap-2">
                      <Cpu className="w-4 h-4 text-cyan-400 shrink-0" />
                      <span className="text-xs font-bold uppercase tracking-wider text-cyan-400">
                        Программы и папки ЧЕРЕЗ VPN ({proxyProcessesList.length})
                      </span>
                    </div>
                    <p className="text-[11px] text-slate-400">
                      Добавьте сюда папку программы (например <code>Discord</code>), <code>.exe</code> файл или процесс, который должен всегда идти через VPN:
                    </p>

                    {/* Action Buttons */}
                    <div className="flex flex-wrap gap-2 pt-1">
                      <button
                        type="button"
                        onClick={() => handlePickExeFile("proxy")}
                        className="px-2.5 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-xs font-semibold text-cyan-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <FileCode className="w-3.5 h-3.5" /> + Файл (.exe)
                      </button>
                      <button
                        type="button"
                        onClick={() => handlePickFolder("proxy")}
                        className="px-2.5 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-xs font-semibold text-cyan-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <FolderOpen className="w-3.5 h-3.5" /> + Папка
                      </button>
                      <button
                        type="button"
                        onClick={() => handleOpenProcessModal("proxy")}
                        className="px-2.5 py-1.5 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-xs font-semibold text-indigo-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <Cpu className="w-3.5 h-3.5" /> + Из запущенных
                      </button>
                    </div>

                    {/* Manual Input */}
                    <div className="flex gap-2 pt-1">
                      <input
                        type="text"
                        placeholder="Имя процесса (напр. Discord.exe) или путь к папке..."
                        value={manualProxyProcInput}
                        onChange={(e) => setManualProxyProcInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            addProcessItem("proxy", manualProxyProcInput);
                            setManualProxyProcInput("");
                          }
                        }}
                        className="flex-1 px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => {
                          addProcessItem("proxy", manualProxyProcInput);
                          setManualProxyProcInput("");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 cursor-pointer"
                      >
                        Добавить
                      </button>
                    </div>

                    {/* List */}
                    <div className="h-44 overflow-y-auto rounded-xl bg-[#050811] border border-slate-800/80 p-2 space-y-1.5">
                      {proxyProcessesList.length === 0 ? (
                        <div className="h-full flex items-center justify-center text-xs text-slate-600 text-center px-4">
                          Список пуст. Нажмите «+ Файл», «+ Папка» или «+ Из запущенных».
                        </div>
                      ) : (
                        proxyProcessesList.map((item, idx) => {
                          const isPath = item.includes("\\") || item.includes("/");
                          const isExe = item.toLowerCase().endsWith(".exe");
                          return (
                            <div
                              key={idx}
                              className="flex items-center justify-between px-2.5 py-1.5 rounded-lg bg-[#0e1628] border border-slate-800 text-xs group"
                            >
                              <div className="flex items-center gap-2 min-w-0 flex-1 pr-2">
                                {isPath && !isExe ? (
                                  <FolderOpen className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                                ) : isPath ? (
                                  <FileCode className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                                ) : (
                                  <Cpu className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
                                )}
                                <div className="truncate font-mono text-[11px] text-slate-200 min-w-0 flex-1" title={item}>
                                  {item}
                                </div>
                              </div>
                              <button
                                type="button"
                                onClick={() => removeProcessItem("proxy", item)}
                                className="text-slate-500 hover:text-rose-400 p-0.5 shrink-0 cursor-pointer"
                                title="Удалить"
                              >
                                <Trash2 className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>

                  {/* 1.2: Домены через VPN */}
                  <div className="p-4 rounded-xl bg-[#080d19] border border-cyan-500/20 space-y-2">
                    <label className="text-xs font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-2">
                      <Globe className="w-4 h-4 text-cyan-400 shrink-0" />
                      <span>Домены всегда ЧЕРЕЗ VPN (Proxy Домены)</span>
                    </label>
                    <p className="text-[11px] text-slate-400">
                      Сайты, которые принудительно идут через VPN (по одному на строку):
                    </p>
                    <textarea
                      rows={4}
                      value={proxyDomainsText}
                      onChange={(e) => setProxyDomainsText(e.target.value)}
                      className="w-full p-3 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>

                  {/* 1.3: IP-адреса через VPN */}
                  <div className="p-4 rounded-xl bg-[#080d19] border border-cyan-500/20 space-y-2">
                    <label className="text-xs font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-2">
                      <Layers className="w-4 h-4 text-cyan-400 shrink-0" />
                      <span>IP-адреса и подсети ЧЕРЕЗ VPN (Proxy IP / CIDR)</span>
                    </label>
                    <p className="text-[11px] text-slate-400">
                      IP-адреса или диапазоны подсетей CIDR для принудительной маршрутизации через VPN (по одному на строку):
                    </p>
                    <textarea
                      rows={4}
                      value={proxyIpsText}
                      placeholder="1.1.1.1&#10;8.8.8.8&#10;91.108.4.0/22&#10;149.154.160.0/20"
                      onChange={(e) => setProxyIpsText(e.target.value)}
                      className="w-full p-3 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </div>

                {/* МАСТЕР-БЛОК 2: МИМО VPN (Прямой интернет / Исключения) */}
                <div
                  className={`p-5 rounded-2xl bg-[#091515]/90 border-2 transition-all space-y-4 ${
                    (routing.default_outbound || "proxy") === "proxy"
                      ? "border-emerald-500/70 shadow-[0_0_30px_-6px_rgba(16,185,129,0.35)]"
                      : "border-emerald-500/30 opacity-90"
                  }`}
                >
                  {/* Master Header Right */}
                  <div className="flex items-start justify-between gap-3 pb-3.5 border-b border-emerald-500/25">
                    <div className="flex items-start gap-3 min-w-0 flex-1">
                      <div className="w-9 h-9 rounded-xl bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shrink-0 mt-0.5">
                        <ExternalLink className="w-4 h-4 text-emerald-400" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="text-xs font-bold uppercase tracking-wider text-emerald-300 flex items-center gap-2 flex-wrap">
                          <span>Мимо VPN (Прямой интернет / Исключения)</span>
                          {(routing.default_outbound || "proxy") === "proxy" && (
                            <span className="text-[9px] px-2 py-0.5 rounded-full bg-emerald-500/25 text-emerald-200 border border-emerald-500/40 font-semibold font-mono shrink-0">
                              Активный режим
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] text-emerald-400/80 mt-1 leading-relaxed">
                          Игры, торренты, российские сайты и прямой трафик без задержек
                        </div>
                      </div>
                    </div>
                    <span className="text-[10px] px-2.5 py-1 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-semibold font-mono shrink-0 mt-0.5">
                      Direct Bypass
                    </span>
                  </div>

                  {/* 2.1: Программы и папки мимо VPN */}
                  <div className="p-4 rounded-xl bg-[#061111] border border-emerald-500/20 space-y-2.5">
                    <div className="flex items-center gap-2">
                      <Cpu className="w-4 h-4 text-emerald-400 shrink-0" />
                      <span className="text-xs font-bold uppercase tracking-wider text-emerald-400">
                        Программы и папки МИМО VPN ({directProcessesList.length})
                      </span>
                    </div>
                    <p className="text-[11px] text-slate-400">
                      Добавьте сюда папки с играми (например <code>Steam</code>, <code>CS2</code>), торренты или программы, которые должны идти напрямую без пинга:
                    </p>

                    {/* Action Buttons */}
                    <div className="flex flex-wrap gap-2 pt-1">
                      <button
                        type="button"
                        onClick={() => handlePickExeFile("direct")}
                        className="px-2.5 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-xs font-semibold text-emerald-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <FileCode className="w-3.5 h-3.5" /> + Файл (.exe)
                      </button>
                      <button
                        type="button"
                        onClick={() => handlePickFolder("direct")}
                        className="px-2.5 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-xs font-semibold text-emerald-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <FolderOpen className="w-3.5 h-3.5" /> + Папка
                      </button>
                      <button
                        type="button"
                        onClick={() => handleOpenProcessModal("direct")}
                        className="px-2.5 py-1.5 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-xs font-semibold text-indigo-300 flex items-center gap-1.5 transition cursor-pointer"
                      >
                        <Cpu className="w-3.5 h-3.5" /> + Из запущенных
                      </button>
                    </div>

                    {/* Manual Input */}
                    <div className="flex gap-2 pt-1">
                      <input
                        type="text"
                        placeholder="Имя процесса (напр. cs2.exe) или путь к папке..."
                        value={manualDirectProcInput}
                        onChange={(e) => setManualDirectProcInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            addProcessItem("direct", manualDirectProcInput);
                            setManualDirectProcInput("");
                          }
                        }}
                        className="flex-1 px-3 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-emerald-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => {
                          addProcessItem("direct", manualDirectProcInput);
                          setManualDirectProcInput("");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 cursor-pointer"
                      >
                        Добавить
                      </button>
                    </div>

                    {/* List */}
                    <div className="h-44 overflow-y-auto rounded-xl bg-[#040b0b] border border-slate-800/80 p-2 space-y-1.5">
                      {directProcessesList.length === 0 ? (
                        <div className="h-full flex items-center justify-center text-xs text-slate-600 text-center px-4">
                          Список пуст. Нажмите «+ Файл», «+ Папка» или «+ Из запущенных».
                        </div>
                      ) : (
                        directProcessesList.map((item, idx) => {
                          const isPath = item.includes("\\") || item.includes("/");
                          const isExe = item.toLowerCase().endsWith(".exe");
                          return (
                            <div
                              key={idx}
                              className="flex items-center justify-between px-2.5 py-1.5 rounded-lg bg-[#091818] border border-slate-800 text-xs group"
                            >
                              <div className="flex items-center gap-2 min-w-0 flex-1 pr-2">
                                {isPath && !isExe ? (
                                  <FolderOpen className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                                ) : isPath ? (
                                  <FileCode className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                                ) : (
                                  <Cpu className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
                                )}
                                <div className="truncate font-mono text-[11px] text-slate-200 min-w-0 flex-1" title={item}>
                                  {item}
                                </div>
                              </div>
                              <button
                                type="button"
                                onClick={() => removeProcessItem("direct", item)}
                                className="text-slate-500 hover:text-rose-400 p-0.5 shrink-0 cursor-pointer"
                                title="Удалить"
                              >
                                <Trash2 className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          );
                        })
                      )}
                    </div>
                  </div>

                  {/* 2.2: Домены мимо VPN */}
                  <div className="p-4 rounded-xl bg-[#061111] border border-emerald-500/20 space-y-2">
                    <label className="text-xs font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-2">
                      <Globe className="w-4 h-4 text-emerald-400 shrink-0" />
                      <span>Домены всегда МИМО VPN (Direct Домены)</span>
                    </label>
                    <p className="text-[11px] text-slate-400">
                      Сайты и доменные зоны напрямую (например <code>.ru</code>, <code>vk.com</code>, <code>steamcontent.com</code>):
                    </p>
                    <textarea
                      rows={4}
                      value={directDomainsText}
                      onChange={(e) => setDirectDomainsText(e.target.value)}
                      className="w-full p-3 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>

                  {/* 2.3: IP-адреса мимо VPN */}
                  <div className="p-4 rounded-xl bg-[#061111] border border-emerald-500/20 space-y-2">
                    <label className="text-xs font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-2">
                      <Layers className="w-4 h-4 text-emerald-400 shrink-0" />
                      <span>IP-адреса и подсети МИМО VPN (Direct IP / CIDR)</span>
                    </label>
                    <p className="text-[11px] text-slate-400">
                      IP-адреса или локальные подсети, которые всегда идут напрямую без VPN:
                    </p>
                    <textarea
                      rows={4}
                      value={directIpsText}
                      placeholder="192.168.0.0/16&#10;10.0.0.0/8&#10;172.16.0.0/12"
                      onChange={(e) => setDirectIpsText(e.target.value)}
                      className="w-full p-3 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </div>
              </div>

            {/* DNS, Local Ports & Core Block */}
            <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-3">
              <div className="flex items-center justify-between pb-2 border-b border-slate-800/80">
                <span className="text-xs font-bold uppercase tracking-wider text-slate-300 flex items-center gap-2">
                  <Activity className="w-4 h-4 text-cyan-400" />
                  DNS-серверы, локальный прокси-порт и Ядро sing-box
                </span>
                <span className="text-[10px] text-slate-500 font-mono">
                  System Core Settings
                </span>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 items-end">
                <div>
                  <label className="text-xs text-slate-400 mb-1 flex items-center gap-1.5">
                    <Shield className="w-3.5 h-3.5 text-cyan-400" />
                    <span>Защищенный DNS (VPN)</span>
                  </label>
                  <input
                    type="text"
                    value={routing.remote_dns}
                    onChange={(e) =>
                      setRouting({ ...routing, remote_dns: e.target.value })
                    }
                    placeholder="tls://1.1.1.1"
                    className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500 focus:outline-none"
                  />
                </div>
                <div>
                  <label className="text-xs text-slate-400 mb-1 flex items-center gap-1.5">
                    <Globe className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Прямой DNS (RU)</span>
                  </label>
                  <input
                    type="text"
                    value={routing.direct_dns}
                    onChange={(e) =>
                      setRouting({ ...routing, direct_dns: e.target.value })
                    }
                    placeholder="77.88.8.8"
                    className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-emerald-500 focus:outline-none"
                  />
                </div>
                <div>
                  <label className="text-xs text-slate-400 mb-1 flex items-center gap-1.5">
                    <Layers className="w-3.5 h-3.5 text-indigo-400" />
                    <span>Локальный прокси-порт</span>
                  </label>
                  <input
                    type="number"
                    value={routing.local_mixed_port}
                    onChange={(e) =>
                      setRouting({
                        ...routing,
                        local_mixed_port: Number(e.target.value) || 2080,
                      })
                    }
                    className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-indigo-500 focus:outline-none"
                  />
                </div>
                <div>
                  <button
                    onClick={handleDownloadCore}
                    disabled={downloadingCore}
                    className="w-full py-2 px-3 rounded-xl bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-xs font-semibold text-cyan-300 border border-cyan-500/30 flex items-center justify-center gap-2 transition cursor-pointer"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${downloadingCore ? "animate-spin" : ""}`} />
                    {downloadingCore ? "Обновление..." : "Обновить sing-box"}
                  </button>
                </div>
              </div>
            </div>


            </div>

            {/* Running Processes Picker Modal */}
            {procModalTarget && (
              <div className="fixed inset-0 bg-black/75 backdrop-blur-sm z-50 flex items-center justify-center p-4">
                <div className="max-w-2xl w-full p-5 rounded-2xl bg-[#111827] border border-slate-700 shadow-2xl flex flex-col max-h-[80vh]">
                  <div className="flex items-center justify-between pb-3 border-b border-slate-800">
                    <div>
                      <h3 className="text-base font-bold text-white flex items-center gap-2">
                        <Cpu className="w-5 h-5 text-cyan-400" />
                        Выбор из запущенных процессов и их папок
                      </h3>
                      <p className="text-xs text-slate-400 mt-0.5">
                        Добавление в список:{" "}
                        <b className={procModalTarget === "proxy" ? "text-cyan-300" : "text-emerald-300"}>
                          {procModalTarget === "proxy" ? "ЧЕРЕЗ VPN" : "МИМО VPN (Напрямую)"}
                        </b>
                      </p>
                    </div>
                    <button
                      onClick={() => setProcModalTarget(null)}
                      className="px-3 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs text-slate-300"
                    >
                      Закрыть
                    </button>
                  </div>

                  <div className="my-3 relative">
                    <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
                    <input
                      type="text"
                      placeholder="Поиск по имени процесса или пути (например discord, steam, chrome)..."
                      value={procSearch}
                      onChange={(e) => setProcSearch(e.target.value)}
                      className="w-full pl-9 pr-4 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>

                  <div className="flex-1 overflow-y-auto space-y-1.5 pr-1">
                    {loadingProcs ? (
                      <div className="py-12 text-center text-xs text-slate-400">
                        Сканирование запущенных процессов...
                      </div>
                    ) : (
                      runningProcs
                        .filter(
                          (p) =>
                            p.name.toLowerCase().includes(procSearch.toLowerCase()) ||
                            p.path.toLowerCase().includes(procSearch.toLowerCase())
                        )
                        .map((proc, i) => {
                          const folderPath = proc.path.includes("\\")
                            ? proc.path.substring(0, proc.path.lastIndexOf("\\"))
                            : "";
                          return (
                            <div
                              key={i}
                              className="p-2.5 rounded-xl bg-[#0b101d] border border-slate-800/80 flex items-center justify-between gap-3 hover:border-slate-700"
                            >
                              <div className="min-w-0 flex-1">
                                <div className="text-xs font-bold text-white">
                                  {proc.name}
                                </div>
                                <div
                                  className="text-[10px] text-slate-400 font-mono truncate"
                                  title={proc.path}
                                >
                                  {proc.path}
                                </div>
                              </div>
                              <div className="flex items-center gap-1.5 shrink-0">
                                <button
                                  type="button"
                                  onClick={() => {
                                    addProcessItem(procModalTarget, proc.name);
                                    showNotice("info", `Добавлен процесс: ${proc.name}`);
                                  }}
                                  className="px-2.5 py-1 rounded-lg bg-indigo-500/20 hover:bg-indigo-500/30 border border-indigo-500/30 text-[11px] font-semibold text-indigo-300"
                                >
                                  + Имя ({proc.name})
                                </button>
                                {folderPath && (
                                  <button
                                    type="button"
                                    onClick={() => {
                                      addProcessItem(procModalTarget, folderPath);
                                      showNotice("info", `Добавлена вся папка: ${folderPath}`);
                                    }}
                                    className="px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/30 text-[11px] font-semibold text-amber-300"
                                    title={`Добавить всю папку: ${folderPath}`}
                                  >
                                    + Папку целиком
                                  </button>
                                )}
                              </div>
                            </div>
                          );
                        })
                    )}
                  </div>
                </div>
              </div>
            )}

            {/* SAVE PRESET MODAL */}
            {showSavePresetModal && (
              <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-fadeIn">
                <div className="w-full max-w-md rounded-2xl bg-[#0f172a] border border-cyan-500/40 shadow-2xl p-6 space-y-4">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Bookmark className="w-5 h-5 text-cyan-400" />
                      <h3 className="text-base font-bold text-white">Новая предустановка маршрутов</h3>
                    </div>
                    <button
                      type="button"
                      onClick={() => setShowSavePresetModal(false)}
                      className="text-slate-400 hover:text-white p-1"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>

                  <p className="text-xs text-slate-300">
                    Текущие списки программ будут сохранены в отдельную предустановку, которую можно будет переключать в один клик прямо во время работы VPN.
                  </p>

                  <div className="space-y-1.5">
                    <label className="text-xs font-semibold text-slate-200">Название предустановки:</label>
                    <input
                      type="text"
                      placeholder="например: Для работы, Стриминг, Игры + Discord"
                      value={newPresetName}
                      onChange={(e) => setNewPresetName(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleSaveAsNewPreset();
                      }}
                      autoFocus
                      className="w-full px-3.5 py-2 rounded-xl bg-[#080c15] border border-slate-700 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-500"
                    />
                  </div>

                  <div className="p-3 rounded-xl bg-[#080c15] border border-slate-800 space-y-2">
                    <div className="text-xs font-semibold text-slate-300">Сохраняемые списки:</div>
                    <div className="flex items-center justify-between text-xs text-slate-400">
                      <span className="text-cyan-300">Через VPN:</span>
                      <span className="font-mono text-white font-bold">{proxyProcessesList.length} программ / папок</span>
                    </div>
                    <div className="flex items-center justify-between text-xs text-slate-400">
                      <span className="text-emerald-300">Мимо VPN:</span>
                      <span className="font-mono text-white font-bold">{directProcessesList.length} программ / папок</span>
                    </div>
                    <div className="flex items-center justify-between text-xs text-slate-400 pt-1 border-t border-slate-800">
                      <span>Режим маршрутизации:</span>
                      <span className="text-white font-semibold">
                        {(routing.default_outbound || "direct") === "direct"
                          ? "Только выбранные через VPN"
                          : "Весь трафик ПК через VPN"}
                      </span>
                    </div>
                  </div>

                  {status.connected && (
                    <div className="p-2.5 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-[11px] text-emerald-300 flex items-center gap-2">
                      <Zap className="w-4 h-4 shrink-0" />
                      <span>Будет мгновенно применено на лету к активному VPN без разрыва связи!</span>
                    </div>
                  )}

                  <div className="flex justify-end gap-2.5 pt-2">
                    <button
                      type="button"
                      onClick={() => setShowSavePresetModal(false)}
                      className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-300 transition cursor-pointer"
                    >
                      Отмена
                    </button>
                    <button
                      type="button"
                      onClick={handleSaveAsNewPreset}
                      disabled={!newPresetName.trim()}
                      className="px-4 py-2 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-600/20 disabled:opacity-50 transition cursor-pointer"
                    >
                      Сохранить пресет
                    </button>
                  </div>
                </div>
              </div>
            )}

          </div>
        )}

        {/* TAB 5: SETTINGS & THEMES */}
        {activeTab === "settings" && (
          <div className="flex-1 overflow-y-auto p-6 space-y-6 animate-tab-enter">
            {/* Header */}
            <div>
              <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                <Settings className={`w-6 h-6 ${currentTheme.styles.accentText}`} />
                {t.settingsTitle}
              </h2>
              <p className="text-xs text-slate-400 mt-1">{t.settingsSubtitle}</p>
            </div>

            {/* NETWORK OPERATING MODE SECTION (Moved from Connection tab) */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <Globe className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                  <div>
                    <h3 className="text-sm font-bold text-white">Режим работы сети</h3>
                    <p className="text-[11px] text-slate-400">
                      Выбор способа перехвата сетевого трафика в Windows
                    </p>
                  </div>
                </div>
                <span className="text-[10px] px-2.5 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-semibold font-mono">
                  {routing.mode === "tun" ? "TUN-режим (Вирт. адаптер)" : "System Proxy (Прокси Windows)"}
                </span>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <button
                  type="button"
                  onClick={() => handleQuickSwitchMode("system_proxy")}
                  className={`p-3.5 rounded-xl border text-left transition cursor-pointer ${
                    routing.mode === "system_proxy"
                      ? "bg-cyan-500/15 border-cyan-500/50 text-white shadow-md shadow-cyan-500/10"
                      : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-xs font-bold">System Proxy</span>
                    <Globe className="w-4 h-4 text-cyan-400" />
                  </div>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Легкий режим без прав администратора (браузеры, Telegram, Discord).
                  </p>
                </button>

                <button
                  type="button"
                  onClick={() => handleQuickSwitchMode("tun")}
                  className={`p-3.5 rounded-xl border text-left transition cursor-pointer ${
                    routing.mode === "tun"
                      ? "bg-cyan-500/15 border-cyan-500/50 text-white shadow-md shadow-cyan-500/10"
                      : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-xs font-bold">TUN-режим (Вирт. адаптер)</span>
                    <Zap className="w-4 h-4 text-indigo-400" />
                  </div>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Перехватывает весь трафик ПК, включая игры и любые приложения.
                  </p>
                </button>
              </div>

              {/* Split Tunneling Status & Link to Routing tab */}
              <div className="pt-2.5 flex items-center justify-between border-t border-slate-800/80 text-xs">
                <div className="flex items-center gap-2">
                  <span className="text-slate-400">Раздельное туннелирование:</span>
                  <span
                    className={`font-semibold flex items-center gap-1.5 ${
                      (selectedProfile?.routing?.split_tunneling ?? routing.split_tunneling)
                        ? "text-cyan-400"
                        : "text-slate-400"
                    }`}
                  >
                    {(selectedProfile?.routing?.split_tunneling ?? routing.split_tunneling) ? (
                      <>
                        <span className="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_6px_#22d3ee]" />
                        Включено для «{selectedProfile?.name || "профиля"}»
                      </>
                    ) : (
                      <>
                        <span className="w-2 h-2 rounded-full bg-slate-600" />
                        Выключено (Весь трафик в VPN)
                      </>
                    )}
                  </span>
                </div>
                <button
                  type="button"
                  onClick={() => {
                    if (selectedProfileId) switchRoutingProfile(selectedProfileId);
                    setActiveTab("routing");
                  }}
                  className="text-cyan-400 hover:text-cyan-300 font-medium flex items-center gap-1 transition cursor-pointer"
                >
                  {t.manageRoutingLink}
                </button>
              </div>
            </div>

            {/* SECURITY & KILL SWITCH SECTION */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2.5">
                <ShieldAlert className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-sm font-bold text-white">
                    Сетевая безопасность, Kill Switch и защита от утечек
                  </h3>
                  <p className="text-[11px] text-slate-400">
                    Управление защитой от утечек IP, DNS и аварийной блокировкой сетевого трафика
                  </p>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3.5 pt-1">
                {/* Kill Switch Toggle */}
                <div
                  onClick={handleToggleKillSwitch}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                        Kill Switch (Аварийная блокировка)
                      </span>
                      <span className="text-[9px] px-1.5 py-0.2 rounded bg-rose-500/20 text-rose-300 border border-rose-500/30">
                        PRO
                      </span>
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      Мгновенно блокирует весь исходящий трафик в Windows при обрыве или сбое VPN-соединения
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      routing.kill_switch ? "bg-rose-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        routing.kill_switch ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* DNS Leak Protection Toggle */}
                <div
                  onClick={handleToggleDnsLeakProtect}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                        Защита от утечек DNS (DNS Leak Protection)
                      </span>
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      Перехватывает 53 порт и шифрует все запросы через защищенный Cloudflare DoH (1.1.1.1)
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      (routing.dns_leak_protect ?? true) ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        (routing.dns_leak_protect ?? true) ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* IPv6 Leak Shield Toggle */}
                <div
                  onClick={handleToggleBlockIpv6}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                        Блокировка протокола IPv6 (IPv6 Shield)
                      </span>
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      Предотвращает утечку реального IPv6 провайдера мимо VPN-туннеля
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      (routing.block_ipv6 ?? true) ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        (routing.block_ipv6 ?? true) ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Anti-DPI TLS ClientHello Splitting Full Card */}
                <div
                  onClick={async () => {
                    const nextVal = !(routing.tls_fragment ?? true);
                    const nextR = { ...routing, tls_fragment: nextVal };
                    setRouting(nextR);
                    try {
                      await invoke("save_routing_config", {
                        routing: nextR,
                        profileId: selectedProfileId || null,
                      });
                      showNotice("success", nextVal ? "TLS фрагментация включена" : "TLS фрагментация отключена");
                    } catch (e: any) {
                      showNotice("error", String(e));
                    }
                  }}
                  className={`col-span-2 p-4 rounded-xl border cursor-pointer transition flex items-center justify-between gap-4 group ${
                    (routing.tls_fragment ?? true)
                      ? "bg-emerald-950/20 border-emerald-500/40 shadow-sm"
                      : "bg-slate-900/60 border-slate-800/80 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-start gap-3.5 min-w-0 pr-2">
                    <div
                      className={`p-2.5 rounded-xl border shrink-0 transition-colors ${
                        (routing.tls_fragment ?? true)
                          ? "bg-emerald-500/20 border-emerald-500/40 text-emerald-400 shadow-[0_0_15px_-3px_rgba(16,185,129,0.4)]"
                          : "bg-slate-800/60 border-slate-700/60 text-slate-400"
                      }`}
                    >
                      <ShieldCheck className="w-5 h-5" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                          Анти-DPI фрагментация TLS (ClientHello Record Splitting)
                        </span>
                        <span
                          className={`text-[10px] font-bold px-2 py-0.5 rounded-full border ${
                            (routing.tls_fragment ?? true)
                              ? "bg-emerald-500/20 border-emerald-500/40 text-emerald-300"
                              : "bg-slate-800 border-slate-700 text-slate-400"
                          }`}
                        >
                          {(routing.tls_fragment ?? true) ? "Активно" : "Выключено"}
                        </span>
                      </div>
                      <p className="text-[11px] text-slate-400 mt-1 leading-relaxed">
                        Разбивает TCP пакет и TLS-записи рукопожатия ClientHello на фрагменты (RFC 5246/8446) для протоколов VLESS Reality, TUIC и Hysteria 2. Обходит сигнатурный анализ и проверку соответствия SNI/IP российскими ТСПУ у кабельных провайдеров (Ростелеком, Дом.ru, МТС, Билайн).
                      </p>
                    </div>
                  </div>

                  <div
                    className={`w-12 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      (routing.tls_fragment ?? true)
                        ? "bg-emerald-500 shadow-sm shadow-emerald-500/50"
                        : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        (routing.tls_fragment ?? true) ? "translate-x-6" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* SYSTEM AUTOSTART & TRAY BEHAVIOR SECTION */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2.5">
                <Monitor className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-sm font-bold text-white">{t.systemSectionTitle}</h3>
                  <p className="text-[11px] text-slate-400">{t.systemSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3.5 pt-1">
                {/* Autostart Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ autostart: !autostart })}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                      {t.autostartTitle}
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      {t.autostartDesc}
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      autostart ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        autostart ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Start Minimized Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ startMinimized: !startMinimized })}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                      {t.startMinimizedTitle}
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      {t.startMinimizedDesc}
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      startMinimized ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        startMinimized ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Close to Tray Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ closeToTray: !closeToTray })}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                      {t.closeToTrayTitle}
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      {t.closeToTrayDesc}
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      closeToTray ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        closeToTray ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Auto-connect on startup Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ autoConnect: !autoConnect })}
                  className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-3 transition group"
                >
                  <div className="min-w-0 pr-2">
                    <div className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                      {t.autoConnectTitle}
                    </div>
                    <div className="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
                      {t.autoConnectDesc}
                    </div>
                  </div>
                  <div
                    className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 ${
                      autoConnect ? "bg-cyan-500" : "bg-slate-700"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-full bg-white shadow-md transform transition-transform ${
                        autoConnect ? "translate-x-5" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* INTERFACE LANGUAGE SECTION */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2.5">
                <Languages className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-sm font-bold text-white">{t.languageSectionTitle}</h3>
                  <p className="text-[11px] text-slate-400">{t.languageSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4 pt-1">
                {/* Russian Card */}
                <div
                  onClick={() => handleUpdateSystemSettings({ language: "ru" })}
                  className={`p-4 rounded-xl cursor-pointer border transition-all flex items-center justify-between ${
                    language === "ru"
                      ? "border-cyan-400 bg-cyan-500/10 shadow-md ring-1 ring-cyan-400/50"
                      : "border-slate-800 bg-slate-900/50 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-blue-500/15 border border-blue-500/30 flex items-center justify-center font-black font-mono text-sm text-blue-300 shrink-0 shadow-sm">
                      RU
                    </div>
                    <div>
                      <div className="text-xs font-bold text-white">{t.langRu}</div>
                      <div className="text-[11px] text-slate-400">Русский язык интерфейса</div>
                    </div>
                  </div>
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center ${
                      language === "ru"
                        ? "border-cyan-400 bg-cyan-500"
                        : "border-slate-600 bg-transparent"
                    }`}
                  >
                    {language === "ru" && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                  </div>
                </div>

                {/* English Card */}
                <div
                  onClick={() => handleUpdateSystemSettings({ language: "en" })}
                  className={`p-4 rounded-xl cursor-pointer border transition-all flex items-center justify-between ${
                    language === "en"
                      ? "border-cyan-400 bg-cyan-500/10 shadow-md ring-1 ring-cyan-400/50"
                      : "border-slate-800 bg-slate-900/50 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-amber-500/15 border border-amber-500/30 flex items-center justify-center font-black font-mono text-sm text-amber-300 shrink-0 shadow-sm">
                      EN
                    </div>
                    <div>
                      <div className="text-xs font-bold text-white">{t.langEn}</div>
                      <div className="text-[11px] text-slate-400">English Interface</div>
                    </div>
                  </div>
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center ${
                      language === "en"
                        ? "border-cyan-400 bg-cyan-500"
                        : "border-slate-600 bg-transparent"
                    }`}
                  >
                    {language === "en" && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                  </div>
                </div>
              </div>
            </div>

            {/* THEMES & APPEARANCE SECTION */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-5 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <Palette className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                  <div>
                    <h3 className="text-sm font-bold text-white">{t.themesSectionTitle}</h3>
                    <p className="text-[11px] text-slate-400">{t.themesSectionSubtitle}</p>
                  </div>
                </div>
                <span className="text-[11px] px-3 py-1 rounded-full bg-slate-800 text-slate-300 border border-slate-700 font-medium">
                  {t.activeTheme}: <b className="text-white">{currentTheme.name}</b>
                </span>
              </div>

              {/* Theme Grid Cards */}
              <div className="grid grid-cols-3 gap-4 pt-1">
                {THEMES.map((theme) => {
                  const isSelected = theme.id === currentThemeId;
                  return (
                    <div
                      key={theme.id}
                      onClick={() => handleSelectTheme(theme.id)}
                      className={`p-4 rounded-xl cursor-pointer border transition-all text-left relative group ${
                        isSelected
                          ? `border-cyan-400 shadow-lg ${theme.styles.accentGlow} bg-slate-800/40 ring-1 ring-cyan-400/50`
                          : "border-slate-800 hover:border-slate-600 bg-slate-900/50 hover:bg-slate-800/30"
                      }`}
                    >
                      {/* Swatch Previews */}
                      <div className="flex items-center gap-1.5 mb-3">
                        <div
                          className="w-6 h-6 rounded-md shadow-inner border border-white/10"
                          style={{ backgroundColor: theme.preview.bg }}
                          title="Background"
                        />
                        <div
                          className="w-6 h-6 rounded-md shadow-inner border border-white/10"
                          style={{ backgroundColor: theme.preview.card }}
                          title="Cards"
                        />
                        <div
                          className="w-6 h-6 rounded-md shadow-inner border border-white/10"
                          style={{ backgroundColor: theme.preview.accent }}
                          title="Primary Accent"
                        />
                        <div
                          className="w-6 h-6 rounded-md shadow-inner border border-white/10"
                          style={{ backgroundColor: theme.preview.secondary }}
                          title="Secondary Accent"
                        />
                      </div>

                      <div className="flex items-center justify-between mb-1">
                        <span className="text-xs font-bold text-white group-hover:text-cyan-200 transition">
                          {theme.name}
                        </span>
                        <span className="text-[9px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-400 border border-slate-700">
                          {theme.badge}
                        </span>
                      </div>

                      <p className="text-[11px] text-slate-400 leading-relaxed mb-3 min-h-[32px]">
                        {theme.description}
                      </p>

                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSelectTheme(theme.id);
                        }}
                        className={`w-full py-1.5 px-3 rounded-lg text-xs font-semibold flex items-center justify-center gap-1.5 transition ${
                          isSelected
                            ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40"
                            : "bg-slate-800 text-slate-300 hover:bg-slate-700"
                        }`}
                      >
                        {isSelected ? (
                          <>
                            <Check className="w-3.5 h-3.5 text-cyan-400" />
                            {t.themeSelected}
                          </>
                        ) : (
                          t.applyTheme
                        )}
                      </button>
                    </div>
                  );
                })}
              </div>

              {/* Window Opacity Slider (Внешний вид и прозрачность окна) */}
              <div className="pt-5 border-t border-slate-800/80 space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <Sliders className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                    <div>
                      <h4 className="text-xs font-bold text-white">Прозрачность окна приложения</h4>
                      <p className="text-[11px] text-slate-400">
                        Настройте прозрачность интерфейса NeuroVeil VPN для комфортной работы поверх других окон
                      </p>
                    </div>
                  </div>
                  <span className="text-xs px-2.5 py-1 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono font-bold">
                    {windowOpacity}%
                  </span>
                </div>

                <div className="flex items-center gap-4 pt-1">
                  <span className="text-xs text-slate-400 font-mono">70%</span>
                  <input
                    type="range"
                    min={70}
                    max={100}
                    step={1}
                    value={windowOpacity}
                    onChange={(e) => handleUpdateOpacity(parseInt(e.target.value, 10))}
                    className="flex-1 accent-cyan-500 cursor-pointer h-2 bg-slate-800 rounded-lg appearance-none"
                  />
                  <span className="text-xs text-slate-400 font-mono">100%</span>
                  <button
                    onClick={() => handleUpdateOpacity(100)}
                    className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 transition"
                  >
                    Сбросить (100%)
                  </button>
                </div>
              </div>
            </div>

            {/* SYSTEM & CORE SECTION */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-3.5 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2.5">
                <Cpu className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-sm font-bold text-white">{t.coreSectionTitle}</h3>
                  <p className="text-[11px] text-slate-400">{t.coreSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4 pt-1">
                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1">
                  <span className="text-[11px] text-slate-400">{t.coreVersionLabel}</span>
                  <div className="text-xs font-mono font-semibold text-white flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_#34d399]" />
                    {status.singbox_version || "sing-box (готов)"}
                  </div>
                  <p className="text-[10px] text-slate-500 pt-1">
                    Бинарный файл: <code>data/bin/sing-box.exe</code>
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1">
                  <span className="text-[11px] text-slate-400">{t.coreDriverLabel}</span>
                  <div className="text-xs font-mono font-semibold text-white flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_#34d399]" />
                    wintun.dll (High-performance Layer-3 TUN)
                  </div>
                  <p className="text-[10px] text-slate-500 pt-1">
                    Расположение: <code>data/bin/wintun.dll</code>
                  </p>
                </div>
              </div>

              <div className="flex gap-3 pt-1">
                <button
                  type="button"
                  onClick={handleDownloadCore}
                  disabled={downloadingCore}
                  className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 border border-slate-700 flex items-center gap-2 transition"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${downloadingCore ? "animate-spin" : ""}`} />
                  {downloadingCore ? "Проверка..." : t.reinstallCoreBtn}
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await invoke("clear_core_logs");
                    showNotice("info", "Журнал логов ядра очищен");
                  }}
                  className="px-4 py-2 rounded-xl bg-slate-800/60 hover:bg-slate-700/60 text-xs font-medium text-slate-400 hover:text-slate-200 transition"
                >
                  {t.clearLogsBtn}
                </button>
              </div>
            </div>

            {/* Bottom Footer Bar */}
            <div className="p-4 rounded-xl bg-slate-900/40 border border-slate-800/60 text-[11px] text-slate-400 flex items-center justify-between">
              <div>
                <span className="font-semibold text-slate-300">NeuroVeil VPN v1.0.3 (Build 1051)</span> — Next-Gen High-Speed VPN Client
              </div>
              <div className="text-slate-500 font-mono text-[10px]">
                Port: 8443 (Hy2) • 9443 (TUIC) • Динамический UDP (Amnezia 3.1) • Tray & Autostart Active
              </div>
            </div>

          </div>
        )}

        {/* TAB 6: ABOUT & DEVELOPER (MoDiE) */}
        {activeTab === "about" && (
          <div className="flex-1 overflow-y-auto p-6 space-y-6 animate-tab-enter">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                  <Info className={`w-6 h-6 ${currentTheme.styles.accentText}`} />
                  О программе NeuroVeil VPN
                </h2>
                <p className="text-xs text-slate-400 mt-1">
                  Архитектура безопасности, технологии обхода ТСПУ, информация о разработчике и центр обновлений
                </p>
              </div>
              <span className="text-xs px-3 py-1 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono font-bold">
                Версия: 1.0.3 build 1051
              </span>
            </div>

            {/* Brand Hero Banner */}
            <div
              className={`p-6 rounded-2xl border transition-all duration-300 relative overflow-hidden ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex flex-col md:flex-row items-center gap-6 relative z-10">
                <div className="w-20 h-20 rounded-2xl overflow-hidden bg-[#0c1220] border-2 border-cyan-500/50 p-2 flex items-center justify-center shadow-xl shadow-cyan-500/20 shrink-0">
                  <img
                    src={appIconLogo}
                    alt="NeuroVeil VPN"
                    className="w-full h-full object-cover rounded-xl pointer-events-none"
                  />
                </div>
                <div className="text-center md:text-left flex-1">
                  <div className="flex items-center justify-center md:justify-start gap-2.5 flex-wrap">
                    <h3 className="text-xl font-bold text-white tracking-tight">NeuroVeil VPN Client</h3>
                    <span className="text-[10px] px-2.5 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-semibold">
                      Официальная сборка
                    </span>
                    <span className="text-[10px] px-2.5 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono">
                      sing-box 1.11+ Core
                    </span>
                  </div>
                  <p className="text-xs text-slate-300 mt-2 leading-relaxed">
                    NeuroVeil VPN — высокоскоростной защищенный клиент нового поколения, разработанный специально для беспрепятственного доступа к сети в условиях жестких блокировок, фильтрации трафика и анализа пакетов ТСПУ провайдеров. Включает нативную интеграцию современных криптографических протоколов, раздельное туннелирование на уровне приложений и нулевое логирование данных.
                  </p>
                </div>
              </div>
            </div>

            {/* Architecture & Tech Specs Details */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Cpu className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <h3 className="text-sm font-bold text-white">Технологический стек и протоколы обхода</h3>
              </div>

              <div className="grid grid-cols-3 gap-3.5 pt-1">
                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-emerald-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-emerald-400" />
                    AmneziaWG (AWG 3.1)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Модифицированный WireGuard со случайными мусорными пакетами (Junk packets Jc/Jmin/Jmax), динамическими сдвигами S1-S4, рандомизацией заголовков H1-H4 и Port Hunting против блокировок протокола UDP.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-cyan-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-cyan-400" />
                    VLESS Reality
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Маскировка входящего TLS-рукопожатия под легитимный трафик зарубежных серверов (Bing, Microsoft, Apple, Yahoo) без необходимости арендовать личные домены и сертификаты.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-purple-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-purple-400" />
                    Hysteria 2
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Ультраскоростной протокол на базе UDP QUIC с кастомным алгоритмом контроля перегрузок BBRv3, маскировкой Salamander и гарантированной скоростью даже при 20-30% потере пакетов.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-indigo-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-indigo-400" />
                    TUIC v5
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Протокол поверх чистого IETF QUIC с нулевой задержкой рукопожатия (0-RTT), мультиплексированием множества потоков в единый канал и адаптивным обходом блокировок провайдеров.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-amber-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-amber-400" />
                    Shadowsocks 2022 (AEAD)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Новейший криптографический стандарт AEAD с шифрованием Blake3 AES-128/256-GCM, защитой от активного зондирования ТСПУ и минимальным оверхедом по сетевым ресурсам.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-violet-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-violet-400" />
                    SSH Dynamic Tunnel (SOCKS5)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Прямой шифрованный туннель через стандартный защищенный порт SSH с аппаратной аутентификацией. Подключается к любому чистому VPS без предварительной установки стороннего ПО.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-sky-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-sky-400" />
                    Драйвер ядра WinTun L3
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Высокопроизводительный виртуальный сетевой драйвер уровня Ring 0, обеспечивающий пропускную способность до 10 Гбит/с с нулевыми задержками и минимальной нагрузкой на CPU.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-teal-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-teal-400" />
                    Smart Split Tunneling
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Интеллектуальная маршрутизация по процессам (.exe), папкам, доменам и IP-диапазонам. Банки, Госуслуги, VK, Яндекс и Steam работают напрямую на полной скорости провайдера.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-rose-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-rose-400" />
                    Kill Switch & Zero-Logs
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    100% клиентская архитектура: клиент и сервер не сохраняют логи подключений. Kill Switch блокирует аварийные утечки IP и DNS при разрыве соединения.
                  </p>
                </div>
              </div>
            </div>

            {/* Developer Card (MoDiE) */}
            <div
              className={`p-6 rounded-2xl border transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3.5">
                  <div className="w-12 h-12 rounded-2xl bg-gradient-to-br from-indigo-500 via-purple-600 to-pink-500 flex items-center justify-center text-white font-black text-xl shadow-lg shadow-indigo-500/30 shrink-0">
                    M
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <h3 className="text-base font-bold text-white">Разработчик: MoDiE</h3>
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 font-semibold">
                        Lead Developer & System Architect
                      </span>
                    </div>
                    <p className="text-xs text-slate-400 mt-0.5">
                      Разработка архитектуры клиента, алгоритмов обхода ТСПУ и интерфейса NeuroVeil VPN
                    </p>
                  </div>
                </div>
              </div>

              <div className="p-4 rounded-xl bg-slate-900/60 border border-slate-800/80 flex flex-col md:flex-row items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-xl bg-[#5865F2]/20 border border-[#5865F2]/40 flex items-center justify-center text-[#5865F2] shrink-0">
                    <MessageSquare className="w-5 h-5" />
                  </div>
                  <div>
                    <div className="text-xs font-bold text-white">Discord сообщество проекта</div>
                    <div className="text-[11px] text-slate-400">
                      Обсуждение релизов, помощь с настройкой собственных VPS и обратная связь с разработчиком
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2.5 w-full md:w-auto">
                  <button
                    type="button"
                    onClick={() =>
                      showNotice(
                        "info",
                        "Ссылка на официальный Discord-сервер будет добавлена в ближайшем релизе!"
                      )
                    }
                    className="flex-1 md:flex-initial px-4 py-2 rounded-xl bg-[#5865F2] hover:bg-[#4752C4] text-white font-semibold text-xs transition shadow-lg shadow-[#5865F2]/25 flex items-center justify-center gap-2 cursor-pointer"
                  >
                    <ExternalLink className="w-3.5 h-3.5" />
                    Discord Сообщество
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      navigator.clipboard.writeText("618406757731008515");
                      handleOpenExternalUrl("https://discord.com/users/618406757731008515");
                      showNotice(
                        "success",
                        "Открыт профиль MoDiE в Discord (ID: 618406757731008515 скопирован в буфер обмена)"
                      );
                    }}
                    title="Discord ID: 618406757731008515"
                    className="flex-1 md:flex-initial px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 font-medium text-xs transition flex items-center justify-center gap-1.5 cursor-pointer"
                  >
                    <MessageSquare className="w-3.5 h-3.5 text-indigo-400" />
                    Написать MoDiE
                  </button>
                </div>
              </div>
            </div>

            {/* Updates Section (GitHub Repository: lllPredatorlll/NeuroVeil-VPN) */}
            <div
              className={`p-5 rounded-2xl border transition-all duration-300 space-y-3.5 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2.5">
                  <RefreshCw
                    className={`w-5 h-5 ${currentTheme.styles.accentText} ${
                      checkingUpdate || installingUpdate ? "animate-spin" : ""
                    }`}
                  />
                  <div>
                    <h3 className="text-sm font-bold text-white">Центр обновлений приложения</h3>
                    <p className="text-[11px] text-slate-400">
                      Автоматическая проверка и установка релизов из официального репозитория GitHub
                    </p>
                  </div>
                </div>
                {updateInfo?.has_update ? (
                  <span className="text-[11px] px-2.5 py-1 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/40 font-semibold flex items-center gap-1.5 animate-pulse">
                    <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                    Доступно обновление v{updateInfo.latest_version}
                  </span>
                ) : (
                  <span className="text-[11px] px-2.5 py-1 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-medium flex items-center gap-1.5">
                    <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                    Установлена актуальная версия
                  </span>
                )}
              </div>

              <div className="p-4 rounded-xl bg-slate-900/50 border border-slate-800/80 flex flex-col md:flex-row md:items-center justify-between gap-3">
                <div className="space-y-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-xs font-bold text-white">
                      NeuroVeil VPN v1.0.3 (Build 1051)
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        handleOpenExternalUrl("https://github.com/lllPredatorlll/NeuroVeil-VPN")
                      }
                      className="text-[10px] px-2 py-0.5 rounded-md bg-slate-800/90 hover:bg-slate-700 text-cyan-300 border border-slate-700 transition flex items-center gap-1 cursor-pointer"
                      title="Открыть официальный репозиторий на GitHub"
                    >
                      <ExternalLink className="w-3 h-3" />
                      github.com/lllPredatorlll/NeuroVeil-VPN
                    </button>
                  </div>
                  <div className="text-[11px] text-slate-400">
                    Канал обновлений: GitHub Release / Main Branch
                    {lastUpdateCheck ? ` • Проверено в ${lastUpdateCheck}` : ""}
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    disabled={checkingUpdate || installingUpdate}
                    onClick={() => handleCheckUpdate(false)}
                    className="px-4 py-2 rounded-xl bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-xs font-semibold text-white transition flex items-center gap-1.5 cursor-pointer shadow-md shadow-cyan-600/20"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${checkingUpdate ? "animate-spin" : ""}`} />
                    {checkingUpdate ? "Проверка GitHub..." : "Проверить обновления"}
                  </button>

                  {updateInfo?.has_update && (
                    <button
                      type="button"
                      disabled={installingUpdate}
                      onClick={handleInstallUpdate}
                      className="px-4 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-xs font-bold text-white transition flex items-center gap-1.5 cursor-pointer shadow-md shadow-emerald-600/25"
                    >
                      <Download className={`w-3.5 h-3.5 ${installingUpdate ? "animate-bounce" : ""}`} />
                      {installingUpdate
                        ? "Загрузка и установка..."
                        : `Обновить до v${updateInfo.latest_version}`}
                    </button>
                  )}
                </div>
              </div>

              {updateInfo && (
                <div className="p-4 rounded-xl bg-slate-950/60 border border-slate-800/80 space-y-2.5">
                  <div className="flex flex-wrap items-center justify-between gap-2 border-b border-slate-800/80 pb-2">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-bold text-white">
                        {updateInfo.release_name || `NeuroVeil VPN v${updateInfo.latest_version}`}
                      </span>
                      <span className="text-[10px] px-2 py-0.5 rounded bg-slate-800 text-slate-300 border border-slate-700 font-mono">
                        v{updateInfo.latest_version}
                      </span>
                      {updateInfo.asset_size ? (
                        <span className="text-[10px] text-slate-400">
                          • {formatBytes(updateInfo.asset_size)}
                        </span>
                      ) : null}
                    </div>
                    <div className="flex items-center gap-2">
                      {!updateInfo.has_update && updateInfo.download_url && (
                        <button
                          type="button"
                          disabled={installingUpdate}
                          onClick={handleInstallUpdate}
                          className="text-[11px] px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition flex items-center gap-1 cursor-pointer"
                          title="Скачать заново и переустановить сборку из репозитория GitHub"
                        >
                          <Download className="w-3 h-3 text-cyan-400" />
                          {installingUpdate ? "Установка..." : "Переустановить с GitHub"}
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => handleOpenExternalUrl(updateInfo.html_url)}
                        className="text-[11px] px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-cyan-300 border border-slate-700 transition flex items-center gap-1 cursor-pointer"
                      >
                        <ExternalLink className="w-3 h-3" />
                        Открыть на GitHub
                      </button>
                    </div>
                  </div>
                  <div className="text-[11px] text-slate-300 whitespace-pre-wrap leading-relaxed max-h-40 overflow-y-auto pr-1">
                    {updateInfo.release_notes}
                  </div>
                </div>
              )}
            </div>

          </div>
        )}

        {/* EDIT PROFILE CONFIG MODAL */}
        {editingProfile && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 animate-fadeIn">
            <div className="max-w-2xl w-full p-6 rounded-2xl bg-[#111827] border border-cyan-500/40 shadow-2xl space-y-4 max-h-[90vh] overflow-y-auto">
              <div className="flex items-center justify-between border-b border-slate-800 pb-3">
                <div className="flex items-center gap-2.5">
                  <div className="p-2 rounded-xl bg-cyan-500/10 text-cyan-400 border border-cyan-500/30">
                    <Edit3 className="w-5 h-5" />
                  </div>
                  <div>
                    <h3 className="text-base font-bold text-white flex items-center gap-2">
                      Редактирование профиля: {editingProfile.name}
                    </h3>
                    <p className="text-xs text-slate-400">
                      Протокол: {editingProfile.protocol === "awg2" ? "AMNEZIAWG 2.0" : editingProfile.protocol === "amneziawg" ? "AMNEZIA 3.1" : editingProfile.protocol.toUpperCase()} • ID: {editingProfile.id}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setEditModeRaw(!editModeRaw)}
                    className="px-3 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-cyan-300 border border-slate-700 transition"
                  >
                    {editModeRaw ? "Визуальный редактор" : "Прямой JSON / Raw"}
                  </button>
                  <button
                    onClick={() => setEditingProfile(null)}
                    className="p-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>

              {editModeRaw ? (
                <div className="space-y-2">
                  <label className="text-xs text-slate-300 font-medium">
                    Полный JSON-объект конфигурации профиля:
                  </label>
                  <textarea
                    rows={14}
                    value={editRawJson}
                    onChange={(e) => setEditRawJson(e.target.value)}
                    className="w-full p-3 rounded-xl bg-[#070a12] border border-slate-800 text-xs font-mono text-cyan-200 focus:outline-none focus:border-cyan-500"
                  />
                  <p className="text-[11px] text-slate-500">
                    Вы можете напрямую отредактировать любые поля sing-box / WireGuard ключей.
                  </p>
                </div>
              ) : (
                <div className="space-y-3.5">
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Название профиля</label>
                      <input
                        type="text"
                        value={editForm.name || ""}
                        onChange={(e) => setEditForm({ ...editForm, name: e.target.value })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Сервер / IP</label>
                      <input
                        type="text"
                        value={editForm.server || ""}
                        onChange={(e) => setEditForm({ ...editForm, server: e.target.value })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">Порт</label>
                      <input
                        type="number"
                        value={editForm.port || 0}
                        onChange={(e) => setEditForm({ ...editForm, port: parseInt(e.target.value, 10) || 0 })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-slate-400 mb-1">SNI / Хост маскировки</label>
                      <input
                        type="text"
                        value={editForm.sni || ""}
                        onChange={(e) => setEditForm({ ...editForm, sni: e.target.value })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                      />
                    </div>
                    {editingProfile.protocol !== "amneziawg" && editingProfile.protocol !== "awg2" && (
                      <div className="col-span-2">
                        <label className="block text-xs text-slate-400 mb-1">Пароль / UUID / Секретный ключ</label>
                        <input
                          type="text"
                          value={editForm.password || editForm.uuid || ""}
                          onChange={(e) => setEditForm({ ...editForm, password: e.target.value, uuid: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                        />
                      </div>
                    )}
                  </div>

                  {editingProfile.protocol === "amneziawg" && (
                    <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-3">
                      <div className="flex items-center justify-between">
                        <div>
                          <div className="text-xs font-bold text-emerald-400">Параметры Amnezia 3.1 (Header Protection):</div>
                          <span className="text-[10px] text-slate-400">Стек обфускации, динамические сдвиги S1-S4 ≥ 12 и Port Hunting</span>
                        </div>
                        <button
                          type="button"
                          onClick={() => {
                            const r = generateRandomAwg31Params();
                            setEditForm({ ...editForm, ...r });
                            showNotice("success", "Параметры Amnezia 3.1 рандомизированы!");
                          }}
                          className="px-2.5 py-1 rounded-lg bg-emerald-600/30 hover:bg-emerald-600/50 border border-emerald-500/40 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition"
                        >
                          🎲 Рандомизировать 3.1
                        </button>
                      </div>

                      <div className="grid grid-cols-2 gap-2 text-xs">
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">
                            AWG Private Key {!editForm.awg_private_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
                          </label>
                          <input
                            type="text"
                            value={editForm.awg_private_key || editForm.password || ""}
                            placeholder="Вставьте PrivateKey из .conf"
                            onChange={(e) => setEditForm({ ...editForm, awg_private_key: e.target.value, password: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">
                            AWG Public Key (Сервер) {!editForm.awg_public_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
                          </label>
                          <input
                            type="text"
                            value={editForm.awg_public_key || ""}
                            placeholder="Вставьте PublicKey сервера"
                            onChange={(e) => setEditForm({ ...editForm, awg_public_key: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">Local Address (IP / CIDR)</label>
                          <input
                            type="text"
                            value={editForm.awg_local_address || ""}
                            placeholder="10.8.0.2/32"
                            onChange={(e) => setEditForm({ ...editForm, awg_local_address: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">HeaderProtectionKey (Base64 32 байта)</label>
                          <input
                            type="text"
                            value={editForm.awg_header_protection_key || ""}
                            placeholder="Base64 32 байта"
                            onChange={(e) => setEditForm({ ...editForm, awg_header_protection_key: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div className="col-span-2">
                          <label className="block text-[11px] text-slate-400 mb-0.5">Preshared Key (PSK, опционально)</label>
                          <input
                            type="text"
                            value={editForm.awg_preshared_key || ""}
                            placeholder="Опциональный pre-shared key"
                            onChange={(e) => setEditForm({ ...editForm, awg_preshared_key: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div className="col-span-2">
                          <label className="block text-[11px] text-slate-400 mb-0.5">Диапазон перебора портов (Port Hunting Range)</label>
                          <input
                            type="text"
                            value={editForm.awg_hop_ports || ""}
                            placeholder="20000:50000"
                            onChange={(e) => setEditForm({ ...editForm, awg_hop_ports: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div className="col-span-2 grid grid-cols-3 gap-2">
                          <div>
                            <label className="block text-[10px] text-slate-400">Jc (Мусорные пакеты)</label>
                            <input
                              type="number"
                              value={editForm.awg_jc ?? 5}
                              onChange={(e) => setEditForm({ ...editForm, awg_jc: parseInt(e.target.value, 10) || 5 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                          <div>
                            <label className="block text-[10px] text-slate-400">Jmin (Мин. размер)</label>
                            <input
                              type="number"
                              value={editForm.awg_jmin ?? 10}
                              onChange={(e) => setEditForm({ ...editForm, awg_jmin: parseInt(e.target.value, 10) || 10 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                          <div>
                            <label className="block text-[10px] text-slate-400">Jmax (Макс. размер)</label>
                            <input
                              type="number"
                              value={editForm.awg_jmax ?? 50}
                              onChange={(e) => setEditForm({ ...editForm, awg_jmax: parseInt(e.target.value, 10) || 50 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                        </div>

                        {/* Shifts S1-S4 (≥ 12 bytes) */}
                        <div className="col-span-2">
                          <label className="block text-[10px] text-slate-400 mb-1">Сдвиги пакетов S1 - S4 (Amnezia 3.1: каждый ≥ 12 байт)</label>
                          <div className="grid grid-cols-4 gap-2">
                            <div>
                              <input
                                type="number"
                                placeholder="S1"
                                value={editForm.awg_s1 ?? 12}
                                onChange={(e) => setEditForm({ ...editForm, awg_s1: parseInt(e.target.value, 10) || 12 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S1</span>
                            </div>
                            <div>
                              <input
                                type="number"
                                placeholder="S2"
                                value={editForm.awg_s2 ?? 12}
                                onChange={(e) => setEditForm({ ...editForm, awg_s2: parseInt(e.target.value, 10) || 12 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S2</span>
                            </div>
                            <div>
                              <input
                                type="number"
                                placeholder="S3"
                                value={editForm.awg_s3 ?? 12}
                                onChange={(e) => setEditForm({ ...editForm, awg_s3: parseInt(e.target.value, 10) || 12 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S3</span>
                            </div>
                            <div>
                              <input
                                type="number"
                                placeholder="S4"
                                value={editForm.awg_s4 ?? 12}
                                onChange={(e) => setEditForm({ ...editForm, awg_s4: parseInt(e.target.value, 10) || 12 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S4</span>
                            </div>
                          </div>
                        </div>

                        {/* Magic Headers H1-H4 */}
                        <div className="col-span-2">
                          <label className="block text-[10px] text-slate-400 mb-1">Заголовки H1 - H4 (1, 2, 3, 4)</label>
                          <div className="grid grid-cols-4 gap-2">
                            <div>
                              <input
                                type="text"
                                placeholder="H1"
                                value={editForm.awg_h1 ?? "1"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h1: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H1</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H2"
                                value={editForm.awg_h2 ?? "2"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h2: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H2</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H3"
                                value={editForm.awg_h3 ?? "3"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h3: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H3</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H4"
                                value={editForm.awg_h4 ?? "4"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h4: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H4</span>
                            </div>
                          </div>
                        </div>

                        {/* Amnezia 3.1 Timers */}
                        <div className="col-span-2 pt-1 border-t border-slate-800">
                          <label className="block text-[10px] text-slate-400 mb-1">Таймеры и параметры рукопожатия Amnezia 3.1</label>
                          <div className="grid grid-cols-3 gap-2">
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">RekeyAfterTime</span>
                              <input
                                type="text"
                                value={editForm.awg_rekey_after_time || "100-120"}
                                onChange={(e) => setEditForm({ ...editForm, awg_rekey_after_time: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">RekeyTimeout</span>
                              <input
                                type="text"
                                value={editForm.awg_rekey_timeout || "3-7"}
                                onChange={(e) => setEditForm({ ...editForm, awg_rekey_timeout: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">RejectAfterTime</span>
                              <input
                                type="text"
                                value={editForm.awg_reject_after_time || "150-180"}
                                onChange={(e) => setEditForm({ ...editForm, awg_reject_after_time: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">KeepaliveTimeout</span>
                              <input
                                type="text"
                                value={editForm.awg_keepalive_timeout || "5-15"}
                                onChange={(e) => setEditForm({ ...editForm, awg_keepalive_timeout: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">MaxHandshakeAttempts</span>
                              <input
                                type="text"
                                value={editForm.awg_max_handshake_attempts || "15-20"}
                                onChange={(e) => setEditForm({ ...editForm, awg_max_handshake_attempts: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                            <div>
                              <span className="text-[9px] text-slate-500 block mb-0.5">PersistentKeepalive</span>
                              <input
                                type="text"
                                value={editForm.awg_persistent_keepalive || "25-35"}
                                onChange={(e) => setEditForm({ ...editForm, awg_persistent_keepalive: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                              />
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  )}

                  {editingProfile.protocol === "awg2" && (
                    <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-3">
                      <div className="flex items-center justify-between">
                        <div>
                          <div className="text-xs font-bold text-teal-400">Параметры AmneziaWG 2.0:</div>
                          <span className="text-[10px] text-slate-400">Классическая обфускация WireGuard со случайными 32-битными заголовками H1-H4</span>
                        </div>
                        <button
                          type="button"
                          onClick={() => {
                            const r = generateRandomAwg20Params();
                            setEditForm({ ...editForm, ...r });
                            showNotice("success", "Параметры AWG 2.0 рандомизированы!");
                          }}
                          className="px-2.5 py-1 rounded-lg bg-teal-600/30 hover:bg-teal-600/50 border border-teal-500/40 text-[11px] font-semibold text-teal-300 flex items-center gap-1 transition"
                        >
                          🎲 Рандомизировать 2.0
                        </button>
                      </div>

                      <div className="grid grid-cols-2 gap-2 text-xs">
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">
                            AWG Private Key {!editForm.awg_private_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
                          </label>
                          <input
                            type="text"
                            value={editForm.awg_private_key || editForm.password || ""}
                            placeholder="Вставьте PrivateKey из .conf"
                            onChange={(e) => setEditForm({ ...editForm, awg_private_key: e.target.value, password: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">
                            AWG Public Key (Сервер) {!editForm.awg_public_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
                          </label>
                          <input
                            type="text"
                            value={editForm.awg_public_key || ""}
                            placeholder="Вставьте PublicKey сервера"
                            onChange={(e) => setEditForm({ ...editForm, awg_public_key: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">Local Address (IP / CIDR)</label>
                          <input
                            type="text"
                            value={editForm.awg_local_address || ""}
                            placeholder="10.8.1.2/32"
                            onChange={(e) => setEditForm({ ...editForm, awg_local_address: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">Preshared Key (PSK, опционально)</label>
                          <input
                            type="text"
                            value={editForm.awg_preshared_key || ""}
                            placeholder="Опциональный pre-shared key"
                            onChange={(e) => setEditForm({ ...editForm, awg_preshared_key: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div className="col-span-2">
                          <label className="block text-[11px] text-slate-400 mb-0.5">Диапазон перебора портов (Port Hunting Range)</label>
                          <input
                            type="text"
                            value={editForm.awg_hop_ports || ""}
                            placeholder="20000:50000"
                            onChange={(e) => setEditForm({ ...editForm, awg_hop_ports: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div className="col-span-2 grid grid-cols-3 gap-2">
                          <div>
                            <label className="block text-[10px] text-slate-400">Jc (Мусорные пакеты)</label>
                            <input
                              type="number"
                              value={editForm.awg_jc ?? 4}
                              onChange={(e) => setEditForm({ ...editForm, awg_jc: parseInt(e.target.value, 10) || 4 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                          <div>
                            <label className="block text-[10px] text-slate-400">Jmin (Мин. размер)</label>
                            <input
                              type="number"
                              value={editForm.awg_jmin ?? 20}
                              onChange={(e) => setEditForm({ ...editForm, awg_jmin: parseInt(e.target.value, 10) || 20 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                          <div>
                            <label className="block text-[10px] text-slate-400">Jmax (Макс. размер)</label>
                            <input
                              type="number"
                              value={editForm.awg_jmax ?? 65}
                              onChange={(e) => setEditForm({ ...editForm, awg_jmax: parseInt(e.target.value, 10) || 65 })}
                              className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white"
                            />
                          </div>
                        </div>

                        {/* Shifts S1-S2 */}
                        <div className="col-span-2">
                          <label className="block text-[10px] text-slate-400 mb-1">Сдвиги пакетов S1 - S2</label>
                          <div className="grid grid-cols-2 gap-2">
                            <div>
                              <input
                                type="number"
                                placeholder="S1"
                                value={editForm.awg_s1 ?? 30}
                                onChange={(e) => setEditForm({ ...editForm, awg_s1: parseInt(e.target.value, 10) || 30 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S1</span>
                            </div>
                            <div>
                              <input
                                type="number"
                                placeholder="S2"
                                value={editForm.awg_s2 ?? 30}
                                onChange={(e) => setEditForm({ ...editForm, awg_s2: parseInt(e.target.value, 10) || 30 })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">S2</span>
                            </div>
                          </div>
                        </div>

                        {/* Magic Headers H1-H4 (32-bit integers) */}
                        <div className="col-span-2">
                          <label className="block text-[10px] text-slate-400 mb-1">Случайные заголовки H1 - H4 (32-битные числа)</label>
                          <div className="grid grid-cols-4 gap-2">
                            <div>
                              <input
                                type="text"
                                placeholder="H1"
                                value={editForm.awg_h1 ?? "10000001"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h1: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H1</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H2"
                                value={editForm.awg_h2 ?? "20000002"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h2: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H2</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H3"
                                value={editForm.awg_h3 ?? "30000003"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h3: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H3</span>
                            </div>
                            <div>
                              <input
                                type="text"
                                placeholder="H4"
                                value={editForm.awg_h4 ?? "40000004"}
                                onChange={(e) => setEditForm({ ...editForm, awg_h4: e.target.value })}
                                className="w-full px-2 py-1 rounded bg-[#0b101d] border border-slate-700 text-xs text-white text-center font-mono"
                              />
                              <span className="text-[9px] text-slate-500 block text-center mt-0.5">H4</span>
                            </div>
                          </div>
                        </div>

                        <div className="col-span-2">
                          <label className="block text-[11px] text-slate-400 mb-0.5">PersistentKeepalive (секунды)</label>
                          <input
                            type="text"
                            value={editForm.awg_persistent_keepalive || "25"}
                            placeholder="25"
                            onChange={(e) => setEditForm({ ...editForm, awg_persistent_keepalive: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                      </div>
                    </div>
                  )}

                  {editingProfile.protocol === "hysteria2" && (
                    <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-2">
                      <div className="text-xs font-bold text-cyan-400">Параметры Hysteria 2:</div>
                      <div className="grid grid-cols-2 gap-2 text-xs">
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">Пароль обфускации Salamander</label>
                          <input
                            type="text"
                            value={editForm.obfs_password || ""}
                            onChange={(e) => setEditForm({ ...editForm, obfs_password: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div>
                          <label className="block text-[11px] text-slate-400 mb-0.5">Port Hopping Range</label>
                          <input
                            type="text"
                            value={editForm.hop_ports || ""}
                            placeholder="20000:50000"
                            onChange={(e) => setEditForm({ ...editForm, hop_ports: e.target.value })}
                            className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}

              <div className="flex justify-end gap-3 pt-2 border-t border-slate-800">
                <button
                  type="button"
                  onClick={() => setEditingProfile(null)}
                  className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 transition"
                >
                  Отмена
                </button>
                <button
                  type="button"
                  onClick={handleSaveEditedProfile}
                  disabled={editSaving}
                  className="px-5 py-2 rounded-xl bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-cyan-600/30 transition flex items-center gap-2"
                >
                  {editSaving ? "Сохранение..." : "Сохранить изменения"}
                </button>
              </div>
            </div>

          </div>
        )}
      </main>
    </div>
  );
}

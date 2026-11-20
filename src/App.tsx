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
  FileText,
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
  ChevronDown,
  ChevronUp,
  Archive,
  Crown,
  CreditCard,
  Play,
  LogOut,
  Box,
} from "lucide-react";
import QRCode from "qrcode";
import appIconLogo from "./assets/app-icon.png";
import { THEMES, resolveThemeId } from "./themes";
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
  VpsAdminState,
  VpsContainerInfo,
  VpsCredentials,
  VpsMultiProtocolGroup,
} from "./types";
import pkg from "../package.json";

const APP_VERSION = pkg.version;
const APP_BUILD = "2020";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

interface ProtocolMeta {
  id: string;
  label: string;
  shortBadge: string;
  description: string;
  accent: string;
  accentTo?: string;
  bg: string;
  border: string;
  text: string;
  gradient?: string;
  gradientLine?: string;
  isHybrid?: boolean;
}

const PROTOCOL_CATALOG: Record<string, ProtocolMeta> = {
  vless: {
    id: "vless",
    label: "VLESS Reality",
    shortBadge: "TCP 443 • TLS 1.3",
    description:
      "Маскировка под легитимный TLS 1.3 (Chrome uTLS) на TCP 443 без домена. Оптимален против DPI.",
    accent: "#22d3ee",
    bg: "rgba(6, 182, 212, 0.14)",
    border: "rgba(34, 211, 238, 0.45)",
    text: "#67e8f9",
  },
  amneziawg: {
    id: "amneziawg",
    label: "Amnezia 3.1",
    shortBadge: "UDP • HPK + H1–H4",
    description:
      "Модифицированный WireGuard с шифрованием заголовков (HPK), случайными H1–H4 и мусорными пакетами Jc.",
    accent: "#34d399",
    bg: "rgba(16, 185, 129, 0.14)",
    border: "rgba(52, 211, 153, 0.45)",
    text: "#6ee7b7",
  },
  awg_quic: {
    id: "awg_quic",
    label: "AWG + QUIC Pre-Warm",
    shortBadge: "ГИБРИД • UDP + QUIC",
    description:
      "Гибридный прогрев UDP: за 15 мс до старта AmneziaWG шлёт STUN и QUIC v1 Initial для пробития stateful-фаерволов.",
    accent: "#38bdf8",
    accentTo: "#34d399",
    bg: "rgba(14, 165, 233, 0.16)",
    border: "rgba(45, 212, 191, 0.58)",
    text: "#7dd3fc",
    gradient: "linear-gradient(135deg, rgba(14, 165, 233, 0.30), rgba(16, 185, 129, 0.30))",
    gradientLine: "linear-gradient(180deg, #38bdf8 0%, #2dd4bf 50%, #34d399 100%)",
    isHybrid: true,
  },
  awg_vless: {
    id: "awg_vless",
    label: "Гибрид AWG + Reality",
    shortBadge: "ГИБРИД • AWG + TCP 443",
    description:
      "Двухслойная капсула: трафик AmneziaWG инкапсулируется в TLS 1.3 VLESS Reality (TCP 443) при блокировке UDP.",
    accent: "#34d399",
    accentTo: "#e879f9",
    bg: "rgba(217, 70, 239, 0.16)",
    border: "rgba(232, 121, 249, 0.58)",
    text: "#f0abfc",
    gradient:
      "linear-gradient(135deg, rgba(16, 185, 129, 0.28), rgba(6, 182, 212, 0.24), rgba(217, 70, 239, 0.34))",
    gradientLine: "linear-gradient(180deg, #34d399 0%, #22d3ee 50%, #e879f9 100%)",
    isHybrid: true,
  },
  awg2: {
    id: "awg2",
    label: "AmneziaWG 2.0",
    shortBadge: "UDP • Классик AWG",
    description:
      "Облегчённая совместимая версия AmneziaWG 2.0 без HPK для клиентов AmneziaVPN старых и мобильных версий.",
    accent: "#2dd4bf",
    bg: "rgba(20, 184, 166, 0.14)",
    border: "rgba(45, 212, 191, 0.45)",
    text: "#5eead4",
  },
  hysteria2: {
    id: "hysteria2",
    label: "Hysteria 2",
    shortBadge: "UDP QUIC • BBR",
    description:
      "Скоростной UDP QUIC с обфускацией Salamander и прыжками по портам (Port Hopping) для нестабильных сетей.",
    accent: "#c084fc",
    bg: "rgba(168, 85, 247, 0.14)",
    border: "rgba(192, 132, 252, 0.45)",
    text: "#d8b4fe",
  },
  tuic: {
    id: "tuic",
    label: "TUIC v5",
    shortBadge: "UDP QUIC • 0-RTT",
    description:
      "Лёгкий UDP QUIC-протокол с 0-RTT рукопожатием и BBR — минимальный пинг для игр и потокового видео.",
    accent: "#818cf8",
    bg: "rgba(99, 102, 241, 0.14)",
    border: "rgba(129, 140, 248, 0.45)",
    text: "#a5b4fc",
  },
  shadowtls: {
    id: "shadowtls",
    label: "ShadowTLS v3",
    shortBadge: "TCP 8443 • TLS 1.3",
    description:
      "Верификация рукопожатия L7 TLS 1.3 с защитой от активного зондирования поверх доверенных SNI-доменов.",
    accent: "#f472b6",
    bg: "rgba(236, 72, 153, 0.14)",
    border: "rgba(244, 114, 182, 0.45)",
    text: "#f9a8d4",
  },
  shadowsocks: {
    id: "shadowsocks",
    label: "Shadowsocks 2022",
    shortBadge: "TCP/UDP • AEAD",
    description:
      "Современный стандарт шифрования 2022-blake3-aes-gcm с минимальной нагрузкой на CPU и высокой скоростью.",
    accent: "#fbbf24",
    bg: "rgba(245, 158, 11, 0.14)",
    border: "rgba(251, 191, 36, 0.45)",
    text: "#fcd34d",
  },
  vless_ws: {
    id: "vless_ws",
    label: "Cloudflare CDN WS",
    shortBadge: "WS • CDN Shield",
    description:
      "VLESS поверх WebSocket для проксирования трафика через сеть Cloudflare CDN и обхода блокировок по IP.",
    accent: "#fb923c",
    bg: "rgba(249, 115, 22, 0.14)",
    border: "rgba(251, 146, 60, 0.45)",
    text: "#fdba74",
  },
  webrtc: {
    id: "webrtc",
    label: "WebRTC Steganography",
    shortBadge: "gRPC • Media",
    description:
      "Стеганография трафика через HTTP/2 gRPC с имитацией медиапотоков звонков (WebRTC) в реальном времени.",
    accent: "#a3e635",
    bg: "rgba(132, 204, 22, 0.14)",
    border: "rgba(163, 230, 53, 0.45)",
    text: "#bef264",
  },
  ssh: {
    id: "ssh",
    label: "SSH Tunnel",
    shortBadge: "TCP 22 • Direct",
    description:
      "Прямой зашифрованный SSH-туннель (TCP 22) с SOCKS5/портовым форвардингом без установки сторонних ядер.",
    accent: "#a78bfa",
    bg: "rgba(139, 92, 246, 0.14)",
    border: "rgba(167, 139, 250, 0.45)",
    text: "#c4b5fd",
  },
  wireguard: {
    id: "wireguard",
    label: "WireGuard",
    shortBadge: "UDP • Классик",
    description:
      "Классический высокопроизводительный туннель ядра на базе Curve25519 и ChaCha20Poly1305.",
    accent: "#fb7185",
    bg: "rgba(244, 63, 94, 0.14)",
    border: "rgba(251, 113, 133, 0.45)",
    text: "#fda4af",
  },
};

function getProtocolMeta(protocol: string, version?: string | null): ProtocolMeta {
  const p = (protocol || "").toLowerCase();
  if (p === "wg") return PROTOCOL_CATALOG.wireguard;
  if (p === "awg2" || (p === "amneziawg" && version === "2.0")) return PROTOCOL_CATALOG.awg2;
  if (PROTOCOL_CATALOG[p]) return PROTOCOL_CATALOG[p];
  return {
    id: p || "proxy",
    label: protocol ? protocol.toUpperCase() : "PROXY",
    shortBadge: "PROXY",
    description: "Стандартный прокси-протокол.",
    accent: "#94a3b8",
    bg: "rgba(100, 116, 139, 0.16)",
    border: "rgba(148, 163, 184, 0.4)",
    text: "#cbd5e1",
  };
}

const LIGHT_PROTOCOL_COLORS: Record<string, { text: string; border: string; bg: string; gradient?: string }> = {
  vless: { text: "#0e7490", border: "rgba(14, 116, 144, 0.38)", bg: "rgba(6, 182, 212, 0.14)" },
  amneziawg: { text: "#047857", border: "rgba(4, 120, 87, 0.38)", bg: "rgba(16, 185, 129, 0.14)" },
  awg_quic: {
    text: "#0369a1",
    border: "rgba(13, 148, 136, 0.45)",
    bg: "rgba(14, 165, 233, 0.15)",
    gradient: "linear-gradient(135deg, rgba(14, 165, 233, 0.20), rgba(16, 185, 129, 0.22))",
  },
  awg_vless: {
    text: "#a21caf",
    border: "rgba(162, 28, 175, 0.42)",
    bg: "rgba(217, 70, 239, 0.14)",
    gradient: "linear-gradient(135deg, rgba(16, 185, 129, 0.18), rgba(6, 182, 212, 0.16), rgba(217, 70, 239, 0.22))",
  },
  awg2: { text: "#0f766e", border: "rgba(15, 118, 110, 0.38)", bg: "rgba(20, 184, 166, 0.14)" },
  hysteria2: { text: "#7e22ce", border: "rgba(126, 34, 206, 0.38)", bg: "rgba(168, 85, 247, 0.14)" },
  tuic: { text: "#4338ca", border: "rgba(67, 56, 202, 0.38)", bg: "rgba(99, 102, 241, 0.14)" },
  shadowtls: { text: "#be185d", border: "rgba(190, 24, 93, 0.38)", bg: "rgba(236, 72, 153, 0.14)" },
  shadowsocks: { text: "#b45309", border: "rgba(180, 83, 9, 0.38)", bg: "rgba(245, 158, 11, 0.15)" },
  vless_ws: { text: "#c2410c", border: "rgba(194, 65, 12, 0.38)", bg: "rgba(249, 115, 22, 0.14)" },
  webrtc: { text: "#3f6212", border: "rgba(63, 98, 18, 0.38)", bg: "rgba(132, 204, 22, 0.16)" },
  ssh: { text: "#6d28d9", border: "rgba(109, 40, 217, 0.38)", bg: "rgba(139, 92, 246, 0.14)" },
  wireguard: { text: "#be123c", border: "rgba(190, 18, 60, 0.38)", bg: "rgba(244, 63, 94, 0.14)" },
};

function getProtocolBadgeStyle(meta: ProtocolMeta, lightTheme: boolean): React.CSSProperties {
  if (!lightTheme) {
    return {
      background: meta.isHybrid ? meta.gradient : meta.bg,
      borderColor: meta.border,
      color: meta.text,
    };
  }
  const light = LIGHT_PROTOCOL_COLORS[meta.id];
  if (!light) {
    return {
      background: "rgba(100, 116, 139, 0.14)",
      borderColor: "rgba(71, 85, 105, 0.35)",
      color: "#334155",
    };
  }
  return {
    background: meta.isHybrid ? light.gradient || light.bg : light.bg,
    borderColor: light.border,
    color: light.text,
  };
}

interface ProtocolPowerTheme {
  idleBg: string;
  idleBorder: string;
  idleGlow: string;
  idleIconBg: string;
  connectedBg: string;
  connectedBorder: string;
  connectedGlow: string;
  connectedIconBg: string;
  pulseOverlay: string;
  dotColor: string;
}

const PROTOCOL_POWER_THEMES: Record<string, ProtocolPowerTheme> = {
  vless: {
    idleBg: "linear-gradient(135deg, #0b1628 0%, #0e3a4d 100%)",
    idleBorder: "rgba(34, 211, 238, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(34, 211, 238, 0.30)",
    idleIconBg: "rgba(6, 182, 212, 0.28)",
    connectedBg: "linear-gradient(135deg, #0e7490 0%, #0891b2 50%, #0e7490 100%)",
    connectedBorder: "rgba(103, 232, 249, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(34, 211, 238, 0.45)",
    connectedIconBg: "#06b6d4",
    pulseOverlay: "rgba(34, 211, 238, 0.14)",
    dotColor: "#67e8f9",
  },
  amneziawg: {
    idleBg: "linear-gradient(135deg, #0b1918 0%, #0b4432 100%)",
    idleBorder: "rgba(52, 211, 153, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(52, 211, 153, 0.30)",
    idleIconBg: "rgba(16, 185, 129, 0.28)",
    connectedBg: "linear-gradient(135deg, #047857 0%, #059669 50%, #0d9488 100%)",
    connectedBorder: "rgba(110, 231, 183, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(16, 185, 129, 0.45)",
    connectedIconBg: "#10b981",
    pulseOverlay: "rgba(52, 211, 153, 0.14)",
    dotColor: "#6ee7b7",
  },
  awg_quic: {
    idleBg: "linear-gradient(135deg, #0b1829 0%, #0c4a6e 52%, #065f46 100%)",
    idleBorder: "rgba(45, 212, 191, 0.62)",
    idleGlow: "0 4px 16px -4px rgba(56, 189, 248, 0.32)",
    idleIconBg: "linear-gradient(135deg, rgba(14, 165, 233, 0.36), rgba(16, 185, 129, 0.36))",
    connectedBg: "linear-gradient(135deg, #0369a1 0%, #0d9488 52%, #059669 100%)",
    connectedBorder: "rgba(94, 234, 212, 0.68)",
    connectedGlow: "0 6px 20px -4px rgba(45, 212, 191, 0.45)",
    connectedIconBg: "linear-gradient(135deg, #0ea5e9, #10b981)",
    pulseOverlay: "rgba(45, 212, 191, 0.15)",
    dotColor: "#5eead4",
  },
  awg_vless: {
    idleBg: "linear-gradient(135deg, #064e3b 0%, #164e63 50%, #701a75 100%)",
    idleBorder: "rgba(232, 121, 249, 0.62)",
    idleGlow: "0 4px 16px -4px rgba(232, 121, 249, 0.32)",
    idleIconBg: "linear-gradient(135deg, rgba(16, 185, 129, 0.34), rgba(217, 70, 239, 0.42))",
    connectedBg: "linear-gradient(135deg, #059669 0%, #0891b2 48%, #a21caf 100%)",
    connectedBorder: "rgba(240, 171, 252, 0.68)",
    connectedGlow: "0 6px 20px -4px rgba(232, 121, 249, 0.48)",
    connectedIconBg: "linear-gradient(135deg, #10b981, #d946ef)",
    pulseOverlay: "rgba(232, 121, 249, 0.14)",
    dotColor: "#f0abfc",
  },
  awg2: {
    idleBg: "linear-gradient(135deg, #0a1b1b 0%, #115e59 100%)",
    idleBorder: "rgba(45, 212, 191, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(45, 212, 191, 0.30)",
    idleIconBg: "rgba(20, 184, 166, 0.28)",
    connectedBg: "linear-gradient(135deg, #0f766e 0%, #0d9488 50%, #0f766e 100%)",
    connectedBorder: "rgba(94, 234, 212, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(20, 184, 166, 0.45)",
    connectedIconBg: "#14b8a6",
    pulseOverlay: "rgba(45, 212, 191, 0.14)",
    dotColor: "#5eead4",
  },
  hysteria2: {
    idleBg: "linear-gradient(135deg, #18102a 0%, #4c1d95 100%)",
    idleBorder: "rgba(192, 132, 252, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(192, 132, 252, 0.30)",
    idleIconBg: "rgba(168, 85, 247, 0.30)",
    connectedBg: "linear-gradient(135deg, #6b21a8 0%, #9333ea 50%, #7e22ce 100%)",
    connectedBorder: "rgba(216, 180, 254, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(168, 85, 247, 0.45)",
    connectedIconBg: "#a855f7",
    pulseOverlay: "rgba(192, 132, 252, 0.14)",
    dotColor: "#d8b4fe",
  },
  tuic: {
    idleBg: "linear-gradient(135deg, #11152c 0%, #312e81 100%)",
    idleBorder: "rgba(129, 140, 248, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(129, 140, 248, 0.30)",
    idleIconBg: "rgba(99, 102, 241, 0.30)",
    connectedBg: "linear-gradient(135deg, #3730a3 0%, #4f46e5 50%, #4338ca 100%)",
    connectedBorder: "rgba(165, 180, 252, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(99, 102, 241, 0.45)",
    connectedIconBg: "#6366f1",
    pulseOverlay: "rgba(129, 140, 248, 0.14)",
    dotColor: "#a5b4fc",
  },
  shadowtls: {
    idleBg: "linear-gradient(135deg, #24101e 0%, #831843 100%)",
    idleBorder: "rgba(244, 114, 182, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(244, 114, 182, 0.30)",
    idleIconBg: "rgba(236, 72, 153, 0.30)",
    connectedBg: "linear-gradient(135deg, #9d174d 0%, #db2777 50%, #be185d 100%)",
    connectedBorder: "rgba(249, 168, 212, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(236, 72, 153, 0.45)",
    connectedIconBg: "#ec4899",
    pulseOverlay: "rgba(244, 114, 182, 0.14)",
    dotColor: "#f9a8d4",
  },
  shadowsocks: {
    idleBg: "linear-gradient(135deg, #21170a 0%, #78350f 100%)",
    idleBorder: "rgba(251, 191, 36, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(251, 191, 36, 0.28)",
    idleIconBg: "rgba(245, 158, 11, 0.28)",
    connectedBg: "linear-gradient(135deg, #92400e 0%, #d97706 50%, #b45309 100%)",
    connectedBorder: "rgba(252, 211, 77, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(245, 158, 11, 0.45)",
    connectedIconBg: "#f59e0b",
    pulseOverlay: "rgba(251, 191, 36, 0.14)",
    dotColor: "#fcd34d",
  },
  vless_ws: {
    idleBg: "linear-gradient(135deg, #24140a 0%, #7c2d12 100%)",
    idleBorder: "rgba(251, 146, 60, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(251, 146, 60, 0.28)",
    idleIconBg: "rgba(249, 115, 22, 0.28)",
    connectedBg: "linear-gradient(135deg, #9a3412 0%, #ea580c 50%, #c2410c 100%)",
    connectedBorder: "rgba(253, 186, 116, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(249, 115, 22, 0.45)",
    connectedIconBg: "#f97316",
    pulseOverlay: "rgba(251, 146, 60, 0.14)",
    dotColor: "#fdba74",
  },
  webrtc: {
    idleBg: "linear-gradient(135deg, #161f0a 0%, #365314 100%)",
    idleBorder: "rgba(163, 230, 53, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(163, 230, 53, 0.26)",
    idleIconBg: "rgba(132, 204, 22, 0.26)",
    connectedBg: "linear-gradient(135deg, #3f6212 0%, #65a30d 50%, #4d7c0f 100%)",
    connectedBorder: "rgba(190, 242, 100, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(132, 204, 22, 0.42)",
    connectedIconBg: "#65a30d",
    pulseOverlay: "rgba(163, 230, 53, 0.14)",
    dotColor: "#bef264",
  },
  ssh: {
    idleBg: "linear-gradient(135deg, #191229 0%, #4c1d95 100%)",
    idleBorder: "rgba(167, 139, 250, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(167, 139, 250, 0.28)",
    idleIconBg: "rgba(139, 92, 246, 0.28)",
    connectedBg: "linear-gradient(135deg, #5b21b6 0%, #7c3aed 50%, #6d28d9 100%)",
    connectedBorder: "rgba(196, 181, 253, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(139, 92, 246, 0.45)",
    connectedIconBg: "#8b5cf6",
    pulseOverlay: "rgba(167, 139, 250, 0.14)",
    dotColor: "#c4b5fd",
  },
  wireguard: {
    idleBg: "linear-gradient(135deg, #240f17 0%, #881337 100%)",
    idleBorder: "rgba(251, 113, 133, 0.55)",
    idleGlow: "0 4px 16px -4px rgba(251, 113, 133, 0.28)",
    idleIconBg: "rgba(244, 63, 94, 0.28)",
    connectedBg: "linear-gradient(135deg, #9f1239 0%, #e11d48 50%, #be123c 100%)",
    connectedBorder: "rgba(253, 164, 175, 0.65)",
    connectedGlow: "0 6px 20px -4px rgba(244, 63, 94, 0.45)",
    connectedIconBg: "#f43f5e",
    pulseOverlay: "rgba(251, 113, 133, 0.14)",
    dotColor: "#fda4af",
  },
};

function getProtocolPowerTheme(meta: ProtocolMeta): ProtocolPowerTheme {
  return (
    PROTOCOL_POWER_THEMES[meta.id] || {
      idleBg: "linear-gradient(135deg, #1e293b 0%, #334155 100%)",
      idleBorder: meta.border || "rgba(148, 163, 184, 0.45)",
      idleGlow: "0 4px 14px -4px rgba(148, 163, 184, 0.25)",
      idleIconBg: meta.bg || "rgba(148, 163, 184, 0.24)",
      connectedBg: "linear-gradient(135deg, #047857 0%, #059669 50%, #0d9488 100%)",
      connectedBorder: "rgba(110, 231, 183, 0.6)",
      connectedGlow: "0 6px 18px -4px rgba(16, 185, 129, 0.4)",
      connectedIconBg: "#10b981",
      pulseOverlay: "rgba(52, 211, 153, 0.12)",
      dotColor: "#6ee7b7",
    }
  );
}

type VpsDeviceProtocolId =
  | "vless"
  | "amneziawg"
  | "awg_quic"
  | "awg_vless"
  | "awg2"
  | "hysteria2"
  | "tuic"
  | "shadowtls"
  | "shadowsocks"
  | "vless_ws"
  | "webrtc";

const VPS_DEVICE_PROTOCOLS: VpsDeviceProtocolId[] = [
  "vless",
  "amneziawg",
  "awg_quic",
  "awg_vless",
  "awg2",
  "hysteria2",
  "tuic",
  "shadowtls",
  "shadowsocks",
  "vless_ws",
  "webrtc",
];

function isProtocolAvailableInContainer(
  protoKey: VpsDeviceProtocolId,
  container?: { protocols?: string[] }
): boolean {
  if (!container) return false;
  const labels = container.protocols || [];
  const hasAwg31 = labels.includes("Amnezia 3.1");
  const hasVless = labels.includes("VLESS Reality");
  switch (protoKey) {
    case "vless":
      return hasVless;
    case "amneziawg":
    case "awg_quic":
      return hasAwg31;
    case "awg_vless":
      return hasAwg31 && hasVless;
    case "awg2":
      return labels.includes("AWG 2.0");
    case "hysteria2":
      return labels.includes("Hysteria 2");
    case "tuic":
      return labels.includes("TUIC v5");
    case "shadowtls":
      return labels.includes("ShadowTLS v3");
    case "shadowsocks":
      return labels.includes("Shadowsocks");
    case "vless_ws":
      return labels.includes("VLESS WS");
    case "webrtc":
      return labels.includes("WebRTC");
  }
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

  const [popupParams, setPopupParams] = useState<{
    mode: string | null;
    profileId: string | null;
    submode: string | null;
  }>(() => {
    const sp = new URLSearchParams(window.location.search);
    return {
      mode: sp.get("popup"),
      profileId: sp.get("profileId"),
      submode: sp.get("submode"),
    };
  });
  const popupMode = popupParams.mode;
  const popupParamsRef = useRef(popupParams);
  useEffect(() => {
    popupParamsRef.current = popupParams;
  }, [popupParams]);
  const loadedQrKeyRef = useRef<string | null>(null);
  const loadedEditKeyRef = useRef<string | null>(null);
  const routingLoadedForPidRef = useRef<string | null>(null);

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
    invoke("emit_window_event", { event: "ui-prefs-updated", payload: {} }).catch(() => {});
  };

  // Visual Theme State
  const [currentThemeId, setCurrentThemeId] = useState<string>(() => {
    return resolveThemeId(
      localStorage.getItem("neuroveil_theme") || localStorage.getItem("aegis_theme")
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
      invoke("emit_window_event", { event: "ui-prefs-updated", payload: {} }).catch(() => {});
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
    const resolved = resolveThemeId(themeId);
    setCurrentThemeId(resolved);
    localStorage.setItem("neuroveil_theme", resolved);
    invoke("emit_window_event", { event: "ui-prefs-updated", payload: {} }).catch(() => {});
    const themeName = THEMES.find((t) => t.id === resolved)?.name || resolved;
    showNotice("success", `Тема «${themeName}» успешно применена!`);
  };

  const isLightTheme = Boolean(currentTheme.isLight);

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

  function generateRandomAwg31Params() {
    const s1 = getRandomInt(12, 28);
    let s2 = getRandomInt(12, 28);
    if (s1 + 56 === s2) {
      s2 += 1;
    }
    return {
      awg_jc: getRandomInt(4, 7),
      awg_jmin: getRandomInt(10, 25),
      awg_jmax: getRandomInt(45, 75),
      awg_s1: s1,
      awg_s2: s2,
      awg_s3: getRandomInt(12, 28),
      awg_s4: getRandomInt(12, 28),
      awg_h1: String(getRandomInt(100000000, 499999999)),
      awg_h2: String(getRandomInt(500000000, 999999999)),
      awg_h3: String(getRandomInt(1000000000, 1499999999)),
      awg_h4: String(getRandomInt(1500000000, 2140000000)),
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
      deploy_awg_quic: true,
      deploy_awg_vless: true,
      deploy_awg2: false,
      deploy_ssh: true,
      deploy_shadowtls: true,
      shadowtls_port: 8443,
      shadowtls_sni: "www.microsoft.com",
      deploy_vless_ws: true,
      vless_ws_port: 2096,
      cdn_domain: "",
      deploy_webrtc: true,
      webrtc_port: 3478,
      awg_port: autoAwgPort,
      awg_hop_ports: "20000:65000",
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
  const [isVpsAdmin, setIsVpsAdmin] = useState(false);
  const [adminLoginLoading, setAdminLoginLoading] = useState(false);
  const [vpsContainers, setVpsContainers] = useState<VpsContainerInfo[]>([]);
  const [vpsMultiGroups, setVpsMultiGroups] = useState<VpsMultiProtocolGroup[]>([]);
  const [expandedMultiGroupIds, setExpandedMultiGroupIds] = useState<string[]>([]);
  const [copiedMultiId, setCopiedMultiId] = useState<string | null>(null);
  const [deletingMultiGroupId, setDeletingMultiGroupId] = useState<string | null>(null);
  const [vpsAdminSubTab, setVpsAdminSubTab] = useState<"containers" | "configs" | "new_key">("containers");
  const [selectedAdminContainerId, setSelectedAdminContainerId] = useState<string>("");
  const [configsFilterContainerId, setConfigsFilterContainerId] = useState<string>("all");
  const [containerBusyId, setContainerBusyId] = useState<string | null>(null);
  const [confirmDeleteContainerId, setConfirmDeleteContainerId] = useState<string | null>(null);

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
            `Проверка завершена: установлена актуальная версия NeuroVeil VPN v${info.current_version} (Build ${APP_BUILD})`
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
  const [autoPortHunting, setAutoPortHunting] = useState(false);
  const [autoPortAttempt, setAutoPortAttempt] = useState(0);
  const autoPortStopRef = useRef(false);
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

  // Process picker state (inside popupMode === "processes")
  const [runningProcs, setRunningProcs] = useState<RunningProcessInfo[]>([]);
  const [loadingProcs, setLoadingProcs] = useState(false);
  const [procSearch, setProcSearch] = useState("");

  // Device Manager & QR Popup state
  const [syncedServerProfiles, setSyncedServerProfiles] = useState<VpnProfile[]>([]);
  const [deviceNameInput, setDeviceNameInput] = useState("");
  const [selectedDeviceProtocols, setSelectedDeviceProtocols] = useState<VpsDeviceProtocolId[]>([
    "vless",
    "amneziawg",
  ]);
  const [lastCreatedKeys, setLastCreatedKeys] = useState<{
    created: VpnProfile[];
    skipped: string[];
  } | null>(null);
  const [creatingDevice, setCreatingDevice] = useState(false);
  const [qrModalProfile, setQrModalProfile] = useState<VpnProfile | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string>("");
  const [routingProfileId, setRoutingProfileId] = useState<string>("");
  const [probingAwgId, setProbingAwgId] = useState<string | null>(null);
  const [awgProbeResults, setAwgProbeResults] = useState<Record<string, { port: number; reachable: boolean; latency_ms?: number; message: string }[]>>({});

  // Routing Presets state
  const [routingPresets, setRoutingPresets] = useState<RoutingPreset[]>([]);
  const [activePresetId, setActivePresetId] = useState<string | null>(null);
  const [newPresetName, setNewPresetName] = useState("");
  const [newPresetOutbound, setNewPresetOutbound] = useState<"direct" | "proxy">("direct");
  const [isApplyingPreset, setIsApplyingPreset] = useState(false);

  // Backup & Factory Reset state
  const [backupBusy, setBackupBusy] = useState<"idle" | "export" | "import">("idle");
  const [resetBusy, setResetBusy] = useState(false);
  const [confirmFactoryReset, setConfirmFactoryReset] = useState(false);

  const handleExportBackup = async () => {
    setBackupBusy("export");
    try {
      const uiPrefs = {
        theme: currentThemeId,
        language,
        window_opacity: windowOpacity,
      };
      const savedPath = await invoke<string | null>("export_backup_file", {
        uiPrefs,
      });
      if (savedPath) {
        const fileName = savedPath.split(/[\\/]/).pop() || savedPath;
        showNotice("success", `Резервная копия сохранена: ${fileName}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setBackupBusy("idle");
    }
  };

  const handleImportBackup = async () => {
    setBackupBusy("import");
    try {
      const res = await invoke<{
        path: string;
        state: AppPersistentState;
        ui_prefs?: { theme?: string; language?: string; window_opacity?: number };
      } | null>("import_backup_file");

      if (!res) return;

      const restored = res.state;
      setProfiles(restored.profiles || []);
      const nextPid =
        restored.selected_profile_id ||
        (restored.profiles?.length ? restored.profiles[0].id : "");
      setSelectedProfileId(nextPid);
      setRoutingProfileId(nextPid);

      const nextRouting =
        restored.profiles?.find((p) => p.id === nextPid)?.routing || restored.routing;
      if (nextRouting) {
        setRouting({ ...nextRouting, mode: restored.routing?.mode || nextRouting.mode });
        setDirectDomainsText((nextRouting.direct_domains || []).join("\n"));
        setProxyDomainsText((nextRouting.proxy_domains || []).join("\n"));
        setDirectIpsText((nextRouting.direct_ips || []).join("\n"));
        setProxyIpsText((nextRouting.proxy_ips || []).join("\n"));
        setDirectProcessesList(nextRouting.direct_processes || []);
        setProxyProcessesList(nextRouting.proxy_processes || []);
      }

      if (restored.routing_presets) setRoutingPresets(restored.routing_presets);
      setActivePresetId(restored.active_preset_id || null);
      if (restored.autostart !== undefined) setAutostart(restored.autostart);
      if (restored.start_minimized !== undefined) setStartMinimized(restored.start_minimized);
      if (restored.close_to_tray !== undefined) setCloseToTray(restored.close_to_tray);
      if (restored.auto_connect !== undefined) setAutoConnect(restored.auto_connect);

      if (res.ui_prefs) {
        if (res.ui_prefs.theme) {
          const resolvedTh = resolveThemeId(res.ui_prefs.theme);
          setCurrentThemeId(resolvedTh);
          localStorage.setItem("neuroveil_theme", resolvedTh);
        }
        if (res.ui_prefs.language === "ru" || res.ui_prefs.language === "en") {
          setLanguage(res.ui_prefs.language);
          localStorage.setItem("neuroveil_lang", res.ui_prefs.language);
        }
        if (typeof res.ui_prefs.window_opacity === "number") {
          const op = Math.max(70, Math.min(100, res.ui_prefs.window_opacity));
          setWindowOpacity(op);
          localStorage.setItem("neuroveil_window_opacity", String(op));
        }
        try {
          await invoke("emit_window_event", { event: "ui-prefs-updated" });
        } catch {}
      }

      const st = await invoke<VpnStatus>("get_vpn_status");
      applyStatusUpdate(st);
      setConfirmFactoryReset(false);
      showNotice(
        "success",
        `Резервная копия восстановлена (туннелей: ${restored.profiles?.length || 0})`
      );
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setBackupBusy("idle");
    }
  };

  const handleFactoryReset = async () => {
    setResetBusy(true);
    try {
      const defState = await invoke<AppPersistentState>("factory_reset_app");
      setProfiles([]);
      setSyncedServerProfiles([]);
      setVpsContainers([]);
      setIsVpsAdmin(false);
      setSelectedProfileId("");
      setRoutingProfileId("");
      setRouting(defState.routing);
      setRoutingPresets(defState.routing_presets || []);
      setActivePresetId(null);
      setDirectDomainsText("");
      setProxyDomainsText("");
      setDirectIpsText("");
      setProxyIpsText("");
      setDirectProcessesList([]);
      setProxyProcessesList([]);
      setAutostart(false);
      setStartMinimized(false);
      setCloseToTray(true);
      setAutoConnect(false);
      setVpsCreds((prev) => ({
        ...prev,
        host: "",
        password: "",
        ssh_proxy: "",
        cdn_domain: "",
      }));

      setCurrentThemeId(THEMES[0].id);
      setWindowOpacity(100);
      setLanguage("ru");
      try {
        localStorage.removeItem("neuroveil_theme");
        localStorage.removeItem("aegis_theme");
        localStorage.removeItem("neuroveil_window_opacity");
        localStorage.removeItem("aegis_window_opacity");
        localStorage.removeItem("neuroveil_lang");
        localStorage.removeItem("neuroveil_qr_profile");
        await invoke("emit_window_event", { event: "ui-prefs-updated" });
      } catch {}

      const st = await invoke<VpnStatus>("get_vpn_status");
      applyStatusUpdate(st);
      setConfirmFactoryReset(false);
      setActiveTab("dashboard");
      showNotice("success", "Все настройки и данные сброшены до заводских");
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setResetBusy(false);
    }
  };

  // Config Editor Popup State
  const [editingProfile, setEditingProfile] = useState<VpnProfile | null>(null);
  const [editForm, setEditForm] = useState<Partial<VpnProfile>>({});
  const [editModeRaw, setEditModeRaw] = useState<boolean>(false);
  const [editRawJson, setEditRawJson] = useState<string>("");
  const [editRawConf, setEditRawConf] = useState<string>("");
  const [rawTextFormat, setRawTextFormat] = useState<"conf" | "json">("conf");
  const [editSaving, setEditSaving] = useState<boolean>(false);

  const loadEditProfile = async (prof: VpnProfile, initialMode: "visual" | "text" = "visual") => {
    setEditingProfile(prof);
    setEditForm({ ...prof });
    setEditRawJson(JSON.stringify(prof, null, 2));
    const isAwgOrWg =
      prof.protocol === "amneziawg" ||
      prof.protocol === "awg_quic" ||
      prof.protocol === "awg_vless" ||
      prof.protocol === "awg2" ||
      prof.protocol === "wireguard";
    if (isAwgOrWg) {
      try {
        const conf = await invoke<string>("export_awg_conf", {
          profileId: prof.id,
          profile: prof,
        });
        setEditRawConf(conf);
        setRawTextFormat("conf");
      } catch {
        setRawTextFormat("json");
      }
    } else {
      setRawTextFormat("json");
    }
    setEditModeRaw(initialMode === "text");
  };

  const handleOpenEditModal = async (prof: VpnProfile, initialMode: "visual" | "text" = "visual") => {
    try {
      await invoke("open_popup_window", {
        mode: "edit",
        profileId: prof.id,
        submode: initialMode,
      });
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const applyStatusUpdate = (st: VpnStatus) => {
    setStatus(st);
    if (st.connected && st.active_profile_id && st.active_listen_port) {
      const lp = st.active_listen_port;
      const pid = st.active_profile_id;
      setProfiles((prev) =>
        prev.some((p) => p.id === pid && p.listen_port !== lp)
          ? prev.map((p) => (p.id === pid ? { ...p, listen_port: lp } : p))
          : prev
      );
      setSyncedServerProfiles((prev) =>
        prev.some((p) => p.id === pid && p.listen_port !== lp)
          ? prev.map((p) => (p.id === pid ? { ...p, listen_port: lp } : p))
          : prev
      );
    }
  };

  const handleSaveEditedProfile = async () => {
    if (!editingProfile) return;
    setEditSaving(true);
    try {
      let updatedProfile: VpnProfile;
      if (editModeRaw) {
        try {
          const isConfText = (rawTextFormat === "conf" || editRawConf.includes("[Interface]") || editRawConf.includes("[Peer]"));
          if (isConfText) {
            const parsed = await invoke<VpnProfile>("parse_profile_text", { text: editRawConf });
            const keepAwg2 =
              editingProfile.protocol === "awg2" &&
              parsed.awg_jc == null &&
              !parsed.awg_header_protection_key;
            const preservedProto =
              editingProfile.protocol === "awg_quic" || editingProfile.protocol === "awg_vless"
                ? editingProfile.protocol
                : keepAwg2
                ? "awg2"
                : parsed.protocol;
            updatedProfile = {
              ...editingProfile,
              ...parsed,
              id: editingProfile.id,
              name: editForm.name || parsed.name || editingProfile.name,
              protocol: preservedProto,
              awg_version: keepAwg2 ? "2.0" : parsed.awg_version,
              pinned_on_pc: editingProfile.pinned_on_pc,
              auto_rotate_listen_port: editForm.auto_rotate_listen_port ?? editingProfile.auto_rotate_listen_port,
              listen_port: parsed.listen_port ?? editingProfile.listen_port ?? null,
            };
          } else {
            updatedProfile = JSON.parse(editRawJson);
          }
        } catch (e: any) {
          showNotice("error", "Ошибка синтаксиса текста конфига: " + e.message);
          setEditSaving(false);
          return;
        }
      } else {
        updatedProfile = {
          ...editingProfile,
          ...editForm,
          port: Number(editForm.port) || editingProfile.port,
          listen_port: editForm.listen_port ? Number(editForm.listen_port) || null : null,
        } as VpnProfile;
      }

      if (updatedProfile.protocol === "awg2") {
        const priv = (updatedProfile.awg_private_key || updatedProfile.password || "").trim();
        updatedProfile.awg_private_key = priv;
        updatedProfile.password = priv;
        updatedProfile.awg_version = "2.0";
        updatedProfile.awg_jc = undefined;
        updatedProfile.awg_jmin = undefined;
        updatedProfile.awg_jmax = undefined;
        updatedProfile.awg_s1 = undefined;
        updatedProfile.awg_s2 = undefined;
        updatedProfile.awg_s3 = undefined;
        updatedProfile.awg_s4 = undefined;
        updatedProfile.awg_h1 = undefined;
        updatedProfile.awg_h2 = undefined;
        updatedProfile.awg_h3 = undefined;
        updatedProfile.awg_h4 = undefined;
        updatedProfile.awg_header_protection_key = undefined;
        updatedProfile.awg_hop_ports = undefined;
        updatedProfile.hop_ports = undefined;
        updatedProfile.awg_persistent_keepalive = undefined;
        updatedProfile.awg_port_probing = false;
      } else if (
        updatedProfile.protocol === "amneziawg" ||
        updatedProfile.protocol === "awg_quic" ||
        updatedProfile.protocol === "awg_vless"
      ) {
        const priv = (updatedProfile.awg_private_key || updatedProfile.password || "").trim();
        updatedProfile.awg_private_key = priv;
        updatedProfile.password = priv;
      }

      const updatedList = await invoke<VpnProfile[]>("add_or_update_profile", {
        profile: updatedProfile,
      });
      setProfiles(updatedList);
      showNotice("success", `Конфигурация «${updatedProfile.name}» успешно сохранена!`);

      // If active profile was updated while connected, restart with updated parameters without rotating ListenPort
      if (status.connected && status.active_profile_id === updatedProfile.id) {
        showNotice("info", "Переподключение туннеля с новыми параметрами...");
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: updatedProfile.id,
          mode: routing.mode,
          splitTunneling: routing.split_tunneling,
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
      }
      if (popupParamsRef.current.mode === "edit") {
        await invoke("app_close");
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
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
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
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
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
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
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
        const curParams = popupParamsRef.current;
        const initialPid =
          ((curParams.mode === "routing_rules" || curParams.mode === "save_preset") && curParams.profileId) ||
          appState.selected_profile_id ||
          (appState.profiles.length > 0 ? appState.profiles[0].id : "");
        if (initialPid) {
          setSelectedProfileId(initialPid);
          setRoutingProfileId(initialPid);
        }
        const initialProf = appState.profiles.find((p) => p.id === initialPid);
        const baseRouting = initialProf?.routing || appState.routing;
        const initialRouting: RoutingConfig = {
          ...baseRouting,
          mode: appState.routing?.mode || baseRouting.mode,
        };
        const shouldSkipRoutingOverwrite =
          curParams.mode === "routing_rules" && routingLoadedForPidRef.current === initialPid;
        if (!shouldSkipRoutingOverwrite) {
          if (curParams.mode === "routing_rules") {
            routingLoadedForPidRef.current = initialPid || "__default__";
          }
          setRouting(initialRouting);
          setDirectDomainsText((initialRouting.direct_domains || []).join("\n"));
          setProxyDomainsText((initialRouting.proxy_domains || []).join("\n"));
          setDirectIpsText((initialRouting.direct_ips || []).join("\n"));
          setProxyIpsText((initialRouting.proxy_ips || []).join("\n"));
          setDirectProcessesList(initialRouting.direct_processes || []);
          setProxyProcessesList(initialRouting.proxy_processes || []);
        }

        if (appState.routing_presets && appState.routing_presets.length > 0) {
          setRoutingPresets(appState.routing_presets);
        }
        if (appState.active_preset_id !== undefined) {
          setActivePresetId(appState.active_preset_id || null);
        }

        if (appState.saved_vps) {
          const savedH1 = appState.saved_vps!.awg_h1;
          const savedH2 = appState.saved_vps!.awg_h2;
          const savedH3 = appState.saved_vps!.awg_h3;
          const savedH4 = appState.saved_vps!.awg_h4;
          setVpsCreds((prev) => ({
            ...prev,
            ...appState.saved_vps!,
            deploy_vless: appState.saved_vps!.deploy_vless !== undefined ? appState.saved_vps!.deploy_vless : true,
            deploy_hy2: appState.saved_vps!.deploy_hy2 !== undefined ? appState.saved_vps!.deploy_hy2 : true,
            deploy_tuic: appState.saved_vps!.deploy_tuic !== undefined ? appState.saved_vps!.deploy_tuic : false,
            deploy_ss: appState.saved_vps!.deploy_ss !== undefined ? appState.saved_vps!.deploy_ss : false,
            deploy_awg: appState.saved_vps!.deploy_awg !== undefined ? appState.saved_vps!.deploy_awg : true,
            deploy_awg_quic: appState.saved_vps!.deploy_awg_quic !== undefined ? appState.saved_vps!.deploy_awg_quic : true,
            deploy_awg_vless: appState.saved_vps!.deploy_awg_vless !== undefined ? appState.saved_vps!.deploy_awg_vless : true,
            deploy_awg2: appState.saved_vps!.deploy_awg2 !== undefined ? appState.saved_vps!.deploy_awg2 : false,
            deploy_ssh: appState.saved_vps!.deploy_ssh !== undefined ? appState.saved_vps!.deploy_ssh : true,
            deploy_shadowtls: appState.saved_vps!.deploy_shadowtls !== undefined ? appState.saved_vps!.deploy_shadowtls : true,
            shadowtls_port: appState.saved_vps!.shadowtls_port || 8443,
            shadowtls_sni: appState.saved_vps!.shadowtls_sni || "www.microsoft.com",
            deploy_vless_ws: appState.saved_vps!.deploy_vless_ws !== undefined ? appState.saved_vps!.deploy_vless_ws : true,
            vless_ws_port: appState.saved_vps!.vless_ws_port || 2096,
            cdn_domain: appState.saved_vps!.cdn_domain || "",
            deploy_webrtc: appState.saved_vps!.deploy_webrtc !== undefined ? appState.saved_vps!.deploy_webrtc : true,
            webrtc_port: appState.saved_vps!.webrtc_port || 3478,
            awg_port: (appState.saved_vps!.awg_port && appState.saved_vps!.awg_port !== 51820) ? appState.saved_vps!.awg_port : getRandomInt(21000, 55000),
            awg_hop_ports: appState.saved_vps!.awg_hop_ports || "20000:65000",
            awg_jc: appState.saved_vps!.awg_jc ?? 5,
            awg_jmin: appState.saved_vps!.awg_jmin ?? 10,
            awg_jmax: appState.saved_vps!.awg_jmax ?? 50,
            awg_s1: appState.saved_vps!.awg_s1 ?? 12,
            awg_s2: appState.saved_vps!.awg_s2 ?? 12,
            awg_s3: appState.saved_vps!.awg_s3 ?? 12,
            awg_s4: appState.saved_vps!.awg_s4 ?? 12,
            awg_h1: savedH1 && savedH1 !== "1" ? savedH1 : String(getRandomInt(100000000, 499999999)),
            awg_h2: savedH2 && savedH2 !== "2" ? savedH2 : String(getRandomInt(500000000, 999999999)),
            awg_h3: savedH3 && savedH3 !== "3" ? savedH3 : String(getRandomInt(1000000000, 1499999999)),
            awg_h4: savedH4 && savedH4 !== "4" ? savedH4 : String(getRandomInt(1500000000, 2140000000)),
            awg2_port: (appState.saved_vps!.awg2_port && appState.saved_vps!.awg2_port !== 51821) ? appState.saved_vps!.awg2_port : getRandomInt(21000, 55000),
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
        applyStatusUpdate(st);
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
        applyStatusUpdate(st);
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

    const syncPrefsFromStorage = () => {
      try {
        const th = localStorage.getItem("neuroveil_theme") || localStorage.getItem("aegis_theme");
        if (th) setCurrentThemeId(resolveThemeId(th));
        const lg = localStorage.getItem("neuroveil_lang") as Language | null;
        if (lg) setLanguage(lg);
        const op =
          localStorage.getItem("neuroveil_window_opacity") ||
          localStorage.getItem("aegis_window_opacity");
        if (op) setWindowOpacity(Math.max(70, Math.min(100, parseInt(op, 10))));
      } catch {}
    };
    window.addEventListener("storage", syncPrefsFromStorage);

    const unlistenUiPrefsPromise = listen("ui-prefs-updated", () => {
      syncPrefsFromStorage();
    });

    const unlistenPromise = listen<string>("vps-log", (event) => {
      setVpsLogs((prev) => [...prev, event.payload]);
    });

    const unlistenStatePromise = listen("state-updated", () => {
      init();
    });

    const unlistenPopupParamsPromise = listen<{
      mode?: string;
      profileId?: string | null;
      submode?: string | null;
    }>("popup-params", (event) => {
      loadedQrKeyRef.current = null;
      loadedEditKeyRef.current = null;
      setPopupParams((prev) => ({
        mode: event.payload.mode || prev.mode,
        profileId: event.payload.profileId ?? null,
        submode: event.payload.submode ?? null,
      }));
    });

    const unlistenProcessPickedPromise = listen<{
      target?: "proxy" | "direct";
      item?: string;
    }>("process-picked", (event) => {
      if (popupParamsRef.current.mode === "routing_rules" && event.payload?.item) {
        const targetMode = event.payload.target === "direct" ? "direct" : "proxy";
        addProcessItemRef.current(targetMode, event.payload.item);
      }
    });

    const unlistenConfirmCleanPromise = listen("confirm-clean-vps", () => {
      if (popupParamsRef.current.mode === "vps") {
        void handleCleanVpsRef.current();
      }
    });

    return () => {
      clearInterval(interval);
      window.removeEventListener("storage", syncPrefsFromStorage);
      unlistenUiPrefsPromise.then((unlisten) => unlisten());
      unlistenPromise.then((unlisten) => unlisten());
      unlistenStatePromise.then((unlisten) => unlisten());
      unlistenPopupParamsPromise.then((unlisten) => unlisten());
      unlistenProcessPickedPromise.then((unlisten) => unlisten());
      unlistenConfirmCleanPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    vpsLogEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [vpsLogs]);

  useEffect(() => {
    if (popupMode === "logs") {
      coreLogEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [popupMode, status.logs.length]);

  useEffect(() => {
    if (popupParams.mode !== "qr") return;
    const key = popupParams.profileId || "__first__";
    if (loadedQrKeyRef.current === key) return;
    let target = profiles.find((p) => p.id === popupParams.profileId);
    if (!target) {
      try {
        const raw = localStorage.getItem("neuroveil_qr_profile");
        if (raw) {
          const parsed = JSON.parse(raw) as VpnProfile;
          if (!popupParams.profileId || parsed.id === popupParams.profileId) {
            target = parsed;
          }
        }
      } catch {}
    }
    if (!target && profiles.length > 0 && !popupParams.profileId) {
      target = profiles[0];
    }
    if (target) {
      loadedQrKeyRef.current = key;
      void loadQrForProfile(target);
    }
  }, [popupParams.mode, popupParams.profileId, profiles]);

  useEffect(() => {
    if (popupParams.mode !== "edit" || profiles.length === 0) return;
    const sub = popupParams.submode === "text" ? "text" : "visual";
    const key = `${popupParams.profileId || "__first__"}:${sub}`;
    if (loadedEditKeyRef.current === key) return;
    const target = profiles.find((p) => p.id === popupParams.profileId) || profiles[0];
    if (target) {
      loadedEditKeyRef.current = key;
      void loadEditProfile(target, sub);
    }
  }, [popupParams.mode, popupParams.profileId, popupParams.submode, profiles]);

  useEffect(() => {
    if (popupParams.mode !== "processes") return;
    void loadRunningProcesses();
  }, [popupParams.mode, popupParams.submode]);

  useEffect(() => {
    if (popupParams.mode !== "routing_rules" && popupParams.mode !== "save_preset") return;
    if (!popupParams.profileId || profiles.length === 0) return;
    if (routingLoadedForPidRef.current === popupParams.profileId) return;
    const prof = profiles.find((p) => p.id === popupParams.profileId);
    if (prof) {
      routingLoadedForPidRef.current = prof.id;
      setRoutingProfileId(prof.id);
      if (prof.routing) {
        setRouting(prof.routing);
        setDirectDomainsText((prof.routing.direct_domains || []).join("\n"));
        setProxyDomainsText((prof.routing.proxy_domains || []).join("\n"));
        setDirectIpsText((prof.routing.direct_ips || []).join("\n"));
        setProxyIpsText((prof.routing.proxy_ips || []).join("\n"));
        setDirectProcessesList(prof.routing.direct_processes || []);
        setProxyProcessesList(prof.routing.proxy_processes || []);
      }
    }
  }, [popupParams.mode, popupParams.profileId, profiles]);

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

  const hasTunnelsOrVps =
    effectiveDashboardProfiles.length > 0 ||
    Boolean(vpsCreds.host && vpsCreds.host.trim().length > 0) ||
    isVpsAdmin;

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

  const handlePinMultiGroupOnPc = async (items: VpnProfile[], groupName: string) => {
    try {
      let latestList: VpnProfile[] | null = null;
      for (const item of items) {
        latestList = await invoke<VpnProfile[]>("add_or_update_profile", {
          profile: { ...item, pinned_on_pc: true },
        });
      }
      if (latestList) {
        setProfiles(latestList);
      }
      showNotice(
        "success",
        `Мульти-протокол «${groupName}» (${items.length} шт.) добавлен на этот ПК`
      );
    } catch (e: any) {
      showNotice("error", String(e));
    }
  };

  const switchRoutingProfile = (profileId: string) => {
    setRoutingProfileId(profileId);
    const target = profiles.find((p) => p.id === profileId);
    if (target) {
      const profRouting = { ...(target.routing || routing), mode: routing.mode };
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
    const targetRouting = { ...(target?.routing || routing), mode: routing.mode };
    setRouting(targetRouting);
    setDirectDomainsText((targetRouting.direct_domains || []).join("\n"));
    setProxyDomainsText((targetRouting.proxy_domains || []).join("\n"));
    setDirectIpsText((targetRouting.direct_ips || []).join("\n"));
    setProxyIpsText((targetRouting.proxy_ips || []).join("\n"));
    setDirectProcessesList(targetRouting.direct_processes || []);
    setProxyProcessesList(targetRouting.proxy_processes || []);

    if (status.connected && !connecting && status.active_profile_id !== profileId) {
      if (
        target?.protocol === "amneziawg" ||
        target?.protocol === "awg_quic" ||
        target?.protocol === "awg_vless" ||
        target?.protocol === "awg2"
      ) {
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
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
        showNotice(
          "success",
          st.active_listen_port
            ? `Переключено на ${target?.name || "профиль"} (ListenPort: ${st.active_listen_port})`
            : `Переключено на ${target?.name || "профиль"}`
        );
      } catch (err: any) {
        showNotice("error", String(err));
      } finally {
        setConnecting(false);
      }
    }
  };

  const handleQuickSwitchMode = async (newMode: "tun" | "system_proxy") => {
    setRouting((r) => ({ ...r, mode: newMode }));
    setProfiles((prev) =>
      prev.map((p) =>
        p.routing ? { ...p, routing: { ...p.routing, mode: newMode } } : p
      )
    );
    try {
      await invoke("set_network_mode", { mode: newMode });
    } catch (err: any) {
      console.error("Failed to save network mode:", err);
    }
    if (status.connected && !connecting && selectedProfile) {
      setConnecting(true);
      try {
        const profRouting = selectedProfile.routing || routing;
        const st = await invoke<VpnStatus>("connect_vpn", {
          profileId: selectedProfile.id,
          mode: newMode,
          splitTunneling: profRouting.split_tunneling,
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
        showNotice(
          "info",
          `Режим переключен: ${newMode === "tun" ? "TUN (Весь ПК)" : "System Proxy"}`
        );
      } catch (err: any) {
        showNotice("error", String(err));
      } finally {
        setConnecting(false);
      }
    } else {
      showNotice(
        "info",
        `Режим сети установлен: ${newMode === "tun" ? "TUN (Весь ПК)" : "System Proxy"}`
      );
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
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
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
    if (autoPortHunting) {
      autoPortStopRef.current = true;
      setAutoPortHunting(false);
    }
    if ((connecting && !autoPortHunting) || reconnectCooldown) return;
    setConnecting(true);
    try {
      if (status.connected) {
        setReconnectCooldown(true);
        setTimeout(() => setReconnectCooldown(false), 2000);
        const st = await invoke<VpnStatus>("disconnect_vpn");
        applyStatusUpdate(st);
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
        if (
          selectedProfile.protocol === "amneziawg" ||
          selectedProfile.protocol === "awg_quic" ||
          selectedProfile.protocol === "awg_vless" ||
          selectedProfile.protocol === "awg2"
        ) {
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
          mode: routing.mode || profRouting.mode,
          splitTunneling: profRouting.split_tunneling,
          preserveListenPort: true,
        });
        applyStatusUpdate(st);
        const lpSuffix = st.active_listen_port ? ` (ListenPort: ${st.active_listen_port})` : "";
        if (st.is_handshaking) {
          showNotice("info", `Запуск туннеля ${selectedProfile.name}${lpSuffix}. Ожидание рукопожатия WireGuard...`);
        } else {
          showNotice("success", `Подключено к ${selectedProfile.name}${lpSuffix}`);
        }
      }
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setConnecting(false);
    }
  };

  const handleReconnect = async () => {
    if (connecting || reconnectCooldown || autoPortHunting) return;
    const targetId = status.active_profile_id || selectedProfile?.id;
    if (!targetId) return;
    setConnecting(true);
    try {
      const targetProf = profiles.find((p) => p.id === targetId) || selectedProfile;
      const profRouting = targetProf?.routing || routing;
      const st = await invoke<VpnStatus>("reconnect_vpn", {
        profileId: targetId,
        mode: routing.mode || profRouting.mode,
        splitTunneling: profRouting.split_tunneling,
      });
      applyStatusUpdate(st);
      if (st.active_listen_port) {
        showNotice(
          st.is_handshaking ? "info" : "success",
          `Переподключено • Новый порт прослушивания (ListenPort): ${st.active_listen_port}`
        );
      } else {
        showNotice("success", `Туннель ${st.active_profile_name || ""} переподключен!`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setConnecting(false);
    }
  };

  const handleAutoPortHunt = async () => {
    if (autoPortHunting) {
      autoPortStopRef.current = true;
      setAutoPortHunting(false);
      showNotice("info", "Автоматический перебор портов остановлен");
      return;
    }
    if (connecting || reconnectCooldown) return;
    const targetId = status.active_profile_id || selectedProfile?.id;
    if (!targetId) return;

    autoPortStopRef.current = false;
    setAutoPortHunting(true);
    setAutoPortAttempt(0);

    try {
      let attempt = 1;
      while (!autoPortStopRef.current) {
        setAutoPortAttempt(attempt);
        setConnecting(true);

        const targetProf = profiles.find((p) => p.id === targetId) || selectedProfile;
        const profRouting = targetProf?.routing || routing;
        const st = await invoke<VpnStatus>("reconnect_vpn", {
          profileId: targetId,
          mode: routing.mode || profRouting.mode,
          splitTunneling: profRouting.split_tunneling,
        });
        applyStatusUpdate(st);
        setConnecting(false);

        const triedPort = st.active_listen_port || st.active_port;
        showNotice(
          "info",
          `Автоподбор порта (#${attempt}): проверка ${
            st.active_listen_port ? `ListenPort ${st.active_listen_port}` : `порта ${triedPort || ""}`
          }...`
        );

        let established = false;
        let finalSt = st;

        // Ожидаем до ~4.2 сек подтверждения рукопожатия / входящего трафика на текущем порту
        for (let tick = 0; tick < 12; tick++) {
          if (autoPortStopRef.current) break;
          await new Promise((r) => setTimeout(r, 350));
          if (autoPortStopRef.current) break;

          const pollSt = await invoke<VpnStatus>("get_vpn_status");
          applyStatusUpdate(pollSt);
          finalSt = pollSt;

          if (!pollSt.connected) {
            break;
          }
          if (!pollSt.is_handshaking && (tick >= 2 || pollSt.down_bytes > 0)) {
            established = true;
            break;
          }
        }

        if (autoPortStopRef.current) {
          break;
        }

        if (established) {
          const workingPort = finalSt.active_listen_port || finalSt.active_port;
          showNotice(
            "success",
            `Подключение установлено с попытки #${attempt}! Рабочий порт: ${workingPort || "активен"}`
          );
          break;
        }

        attempt++;
      }
    } catch (err: any) {
      if (!autoPortStopRef.current) {
        showNotice("error", `Ошибка автоподбора порта: ${String(err)}`);
      }
    } finally {
      setAutoPortHunting(false);
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
    const hasAnyProtocol =
      vpsCreds.deploy_vless ||
      vpsCreds.deploy_hy2 ||
      vpsCreds.deploy_tuic ||
      vpsCreds.deploy_ss ||
      vpsCreds.deploy_awg ||
      vpsCreds.deploy_awg_quic ||
      vpsCreds.deploy_awg_vless ||
      vpsCreds.deploy_awg2 ||
      vpsCreds.deploy_ssh ||
      vpsCreds.deploy_shadowtls ||
      vpsCreds.deploy_vless_ws ||
      vpsCreds.deploy_webrtc;

    if (!hasAnyProtocol) {
      showNotice(
        "error",
        "Выберите хотя бы один протокол для развертывания (VLESS, Hysteria 2, TUIC, Shadowsocks, Amnezia 3.1, AWG + QUIC Pre-Warm, Гибрид AWG + Reality, AmneziaWG 2.0, ShadowTLS, Cloudflare WS, WebRTC или SSH)"
      );
      return;
    }

    // Автоматическая рандомизация при каждом деплое: новые случайные порты и параметры обфускации
    const autoAwgPort = getRandomInt(21000, 55000);
    const r31 = generateRandomAwg31Params();

    const autoAwg2Port = (vpsCreds.awg2_port && vpsCreds.awg2_port !== 51821 && vpsCreds.awg2_port !== 51820) ? vpsCreds.awg2_port : getRandomInt(21000, 55000);

    const deployedCreds: VpsCredentials = {
      ...vpsCreds,
      deploy_awg_quic: vpsCreds.deploy_awg_quic ?? true,
      deploy_awg_vless: vpsCreds.deploy_awg_vless ?? true,
      deploy_awg2: vpsCreds.deploy_awg2 ?? false,
      awg2_port: autoAwg2Port,
      ...r31,
      awg_port: autoAwgPort,
      awg_disable_cookies: vpsCreds.awg_disable_cookies ?? true,
      awg_random_trailers: vpsCreds.awg_random_trailers ?? true,
      awg_port_probing: vpsCreds.awg_port_probing ?? true,
    };
    setVpsCreds(deployedCreds);

    setVpsBusy("deploying");
    const logMessages = [
      `[Авто-рандомизация] Сгенерирован порт Amnezia 3.1 (UDP ${autoAwgPort}), 32-битные заголовки H1-H4 (${r31.awg_h1}..${r31.awg_h4}) и непересекающиеся сдвиги S1/S2 (${r31.awg_s1}/${r31.awg_s2}).`,
    ];
    if (vpsCreds.deploy_awg2) {
      logMessages.push(`[Авто-рандомизация] Сгенерирован порт AmneziaWG 2.0 (UDP ${autoAwg2Port}).`);
    }
    setVpsLogs((prev) => [
      ...prev,
      ...logMessages,
    ]);
    try {
      const res = await invoke<DeployResult>("deploy_vps_server", { creds: deployedCreds });
      if (res.new_ssh_port) {
        setVpsCreds((prev) => ({
          ...prev,
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
  const handleCleanVpsRef = useRef(handleCleanVps);
  handleCleanVpsRef.current = handleCleanVps;

  const handleImportUri = async () => {
    if (!uriInput.trim()) return;
    try {
      const created = await invoke<VpnProfile[]>("import_profile_uri", { uri: uriInput.trim() });
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      if (created.length > 0) {
        setSelectedProfileId(created[0].id);
      }
      setUriInput("");
      if (created.length === 1) {
        showNotice("success", `Профиль «${created[0].name}» успешно импортирован`);
      } else if (created.length > 1) {
        const gName = created[0].multi_group_name || "Мульти-протокол";
        showNotice(
          "success",
          `Мульти-протокол «${gName}» (${created.length} шт.) успешно импортирован!`
        );
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handlePickConfigFile = async () => {
    try {
      const created = await invoke<VpnProfile[]>("pick_config_file_and_import");
      if (created && created.length > 0) {
        const appState = await invoke<AppPersistentState>("get_app_state");
        setProfiles(appState.profiles);
        setSelectedProfileId(created[0].id);
        if (created.length === 1) {
          showNotice(
            "success",
            `Конфигурация «${created[0].name}» успешно импортирована из файла!`
          );
        } else {
          const gName = created[0].multi_group_name || "Мульти-протокол";
          showNotice(
            "success",
            `Мульти-протокол «${gName}» (${created.length} шт.) успешно импортирован из файла!`
          );
        }
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
          : newProfile.protocol === "vless_ws"
          ? "VLESS CDN Shield"
          : newProfile.protocol === "webrtc"
          ? "WebRTC Media"
          : newProfile.protocol === "shadowtls"
          ? "ShadowTLS v3"
          : newProfile.protocol === "tuic"
          ? "TUIC v5"
          : newProfile.protocol === "shadowsocks"
          ? "Shadowsocks"
          : newProfile.protocol === "awg2"
          ? "AmneziaWG 2.0"
          : newProfile.protocol === "awg_quic"
          ? "AWG + QUIC Pre-Warm"
          : newProfile.protocol === "awg_vless"
          ? "Hybrid AWG + Reality"
          : newProfile.protocol === "amneziawg"
          ? "Amnezia 3.1"
          : newProfile.protocol === "wireguard"
          ? "WireGuard"
          : "Hysteria 2";
      const isAwg31 =
        newProfile.protocol === "amneziawg" ||
        newProfile.protocol === "awg_quic" ||
        newProfile.protocol === "awg_vless";
      const isAwg20 = newProfile.protocol === "awg2";
      const isProfileAwg = isAwg31 || isAwg20 || newProfile.protocol === "wireguard";
      const isUuidProto =
        newProfile.protocol === "vless" ||
        newProfile.protocol === "vless_ws" ||
        newProfile.protocol === "webrtc";
      const profToSave: VpnProfile = {
        ...newProfile,
        pinned_on_pc: true,
        uuid: isUuidProto ? newProfile.uuid || newProfile.password : newProfile.uuid,
        awg_version: isAwg20 ? "2.0" : isAwg31 ? "3.1" : newProfile.awg_version,
        awg_private_key: isProfileAwg ? (newProfile.awg_private_key || newProfile.password) : newProfile.awg_private_key,
        awg_local_address: isAwg20 ? (newProfile.awg_local_address || "10.8.1.2/24") : newProfile.awg_local_address,
        awg_hop_ports: isAwg31 ? newProfile.awg_hop_ports : undefined,
        hop_ports: isAwg31 ? newProfile.hop_ports : isAwg20 ? undefined : newProfile.hop_ports,
        awg_jc: isAwg20 ? undefined : newProfile.awg_jc,
        awg_jmin: isAwg20 ? undefined : newProfile.awg_jmin,
        awg_jmax: isAwg20 ? undefined : newProfile.awg_jmax,
        awg_s1: isAwg20 ? undefined : newProfile.awg_s1,
        awg_s2: isAwg20 ? undefined : newProfile.awg_s2,
        awg_s3: isAwg20 ? undefined : newProfile.awg_s3,
        awg_s4: isAwg20 ? undefined : newProfile.awg_s4,
        awg_h1: isAwg20 ? undefined : newProfile.awg_h1,
        awg_h2: isAwg20 ? undefined : newProfile.awg_h2,
        awg_h3: isAwg20 ? undefined : newProfile.awg_h3,
        awg_h4: isAwg20 ? undefined : newProfile.awg_h4,
        awg_persistent_keepalive: isAwg20 ? undefined : newProfile.awg_persistent_keepalive,
        awg_port_probing: isAwg31 ? (newProfile.awg_port_probing ?? true) : isAwg20 ? false : newProfile.awg_port_probing,
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

  const loadQrForProfile = async (profile: VpnProfile) => {
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

  const handleOpenQrModal = async (profile: VpnProfile) => {
    try {
      try {
        localStorage.setItem("neuroveil_qr_profile", JSON.stringify(profile));
      } catch {}
      await invoke("open_popup_window", {
        mode: "qr",
        profileId: profile.id,
      });
    } catch (err: any) {
      showNotice("error", String(err));
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

  const handleCopyMultiUris = async (
    items: VpnProfile[],
    groupName: string,
    keyId: string
  ) => {
    const text = items
      .map((p) => `${p.name}: ${p.uri}`)
      .filter((line) => line.trim().length > 0)
      .join("\n");
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopiedMultiId(keyId);
      setTimeout(() => setCopiedMultiId((prev) => (prev === keyId ? null : prev)), 2000);
      showNotice("success", `Скопированы ссылки (${items.length}) для «${groupName}»`);
    } catch {
      showNotice("error", "Не удалось скопировать ссылки в буфер обмена");
    }
  };

  const handleExportMultiJson = async (items: VpnProfile[], groupName: string) => {
    try {
      const savedPath = await invoke<string | null>("export_multi_profile_json", {
        profiles: items,
        groupName,
      });
      if (savedPath) {
        showNotice("success", `Мульти-протокол (.json) сохранен: ${savedPath}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleExportMultiZip = async (items: VpnProfile[], groupName: string) => {
    try {
      const savedPath = await invoke<string | null>("export_multi_profile_zip", {
        profiles: items,
        groupName,
      });
      if (savedPath) {
        showNotice("success", `Архив мульти-протокола (.zip) сохранен: ${savedPath}`);
      }
    } catch (err: any) {
      showNotice("error", String(err));
    }
  };

  const handleDeleteMultiGroup = async (
    groupId: string | null | undefined,
    groupName: string,
    items: VpnProfile[],
    keyId: string
  ) => {
    if (!items.length) return;
    setDeletingMultiGroupId(keyId);
    try {
      const shouldDeleteFromVps =
        isVpsAdmin && vpsCreds.host.trim().length > 0 && vpsCreds.password.trim().length > 0;
      let updated: VpnProfile[];
      if (shouldDeleteFromVps) {
        showNotice(
          "info",
          `Удаление мульти-протокола «${groupName}» (${items.length}) с VPS ${vpsCreds.host}...`
        );
        updated = await invoke<VpnProfile[]>("delete_vps_multi_group_cmd", {
          creds: vpsCreds,
          profiles: items,
          groupId: groupId || null,
          groupName,
        });
      } else {
        updated = profiles;
        for (const item of items) {
          updated = await invoke<VpnProfile[]>("delete_profile", { id: item.id });
        }
      }
      setProfiles(updated);
      const removedIds = new Set(items.map((x) => x.id));
      setSyncedServerProfiles((prev) =>
        prev.filter((p) => {
          if (groupId && p.multi_group_id === groupId) return false;
          if (removedIds.has(p.id)) return false;
          return !items.some(
            (t) =>
              p.server === t.server &&
              p.port === t.port &&
              p.protocol === t.protocol &&
              ((t.password && p.password === t.password) || p.name === t.name)
          );
        })
      );
      if (groupId) {
        setVpsMultiGroups((prev) => prev.filter((g) => g.id !== groupId));
      }
      if (removedIds.has(selectedProfileId)) {
        setSelectedProfileId(updated[0]?.id || "");
      }
      showNotice(
        "success",
        shouldDeleteFromVps
          ? `Мульти-протокол «${groupName}» удален с VPS и в приложении`
          : `Мульти-протокол «${groupName}» удален`
      );
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setDeletingMultiGroupId(null);
    }
  };

  type VpsConfigEntry =
    | {
        kind: "multi";
        keyId: string;
        groupId: string;
        groupName: string;
        containerId?: string | null;
        profiles: VpnProfile[];
      }
    | {
        kind: "single";
        profile: VpnProfile;
      };

  const buildVpsConfigEntries = (list: VpnProfile[]): VpsConfigEntry[] => {
    const buckets = new Map<string, VpnProfile[]>();
    for (const p of list) {
      const gid = (p.multi_group_id || "").trim();
      if (!gid) continue;
      const keyId = `${p.container_id || "legacy"}::${gid}`;
      const arr = buckets.get(keyId);
      if (arr) {
        arr.push(p);
      } else {
        buckets.set(keyId, [p]);
      }
    }
    const seenGroups = new Set<string>();
    const entries: VpsConfigEntry[] = [];
    for (const p of list) {
      const gid = (p.multi_group_id || "").trim();
      const keyId = gid ? `${p.container_id || "legacy"}::${gid}` : "";
      const groupProfiles = keyId ? buckets.get(keyId) : undefined;
      if (gid && groupProfiles && groupProfiles.length >= 2) {
        if (seenGroups.has(keyId)) continue;
        seenGroups.add(keyId);
        const groupMeta = vpsMultiGroups.find((g) => g.id === gid);
        const groupName =
          groupProfiles.find((gp) => gp.multi_group_name)?.multi_group_name ||
          groupMeta?.name ||
          p.name.replace(/\s*\([^)]*\)\s*$/, "").trim() ||
          "Мультипротокол";
        entries.push({
          kind: "multi",
          keyId,
          groupId: gid,
          groupName,
          containerId: p.container_id,
          profiles: groupProfiles,
        });
      } else {
        entries.push({ kind: "single", profile: p });
      }
    }
    return entries;
  };

  const isServerProfilePinnedOnPc = (p: VpnProfile): boolean =>
    profiles.some(
      (lp) =>
        lp.pinned_on_pc !== false &&
        (lp.id === p.id ||
          (lp.server === p.server &&
            lp.port === p.port &&
            lp.protocol === p.protocol &&
            lp.password === p.password))
    );

  const handleLoginVpsAdmin = async (silent = false) => {
    if (!vpsCreds.host.trim() || !vpsCreds.password.trim()) {
      showNotice("error", "Введите IP-адрес сервера и пароль SSH (root)");
      return;
    }
    setAdminLoginLoading(true);
    try {
      const adminState = await invoke<VpsAdminState>("get_vps_admin_state", {
        creds: vpsCreds,
      });
      const appState = await invoke<AppPersistentState>("get_app_state");
      setProfiles(appState.profiles);
      setVpsContainers(adminState.containers);
      setSyncedServerProfiles(adminState.profiles);
      setVpsMultiGroups(adminState.multi_groups || []);
      if (adminState.containers.length > 0) {
        const nextCid =
          selectedAdminContainerId && adminState.containers.some((c) => c.id === selectedAdminContainerId)
            ? selectedAdminContainerId
            : adminState.containers[0].id;
        setSelectedAdminContainerId(nextCid);
        const activeCont = adminState.containers.find((c) => c.id === nextCid);
        if (activeCont) {
          const avail = VPS_DEVICE_PROTOCOLS.filter((p) =>
            isProtocolAvailableInContainer(p, activeCont)
          );
          if (avail.length > 0 && !selectedAdminContainerId) {
            setSelectedDeviceProtocols(avail);
          }
        }
      } else {
        setSelectedAdminContainerId("");
      }
      if (appState.profiles.length > 0 && !selectedProfileId) {
        setSelectedProfileId(appState.profiles[0].id);
      }
      setIsVpsAdmin(true);
      if (!silent) {
        showNotice(
          "success",
          `Авторизация успешна! Контейнеров: ${adminState.containers.length}, ключей: ${adminState.profiles.length}`
        );
      }
    } catch (err: any) {
      showNotice("error", `Ошибка авторизации на VPS: ${String(err)}`);
    } finally {
      setAdminLoginLoading(false);
    }
  };

  const handleLogoutVpsAdmin = () => {
    setIsVpsAdmin(false);
    setVpsContainers([]);
    setSyncedServerProfiles([]);
    setVpsMultiGroups([]);
    setExpandedMultiGroupIds([]);
    setSelectedAdminContainerId("");
    setConfigsFilterContainerId("all");
    setConfirmDeleteContainerId(null);
    setLastCreatedKeys(null);
    setVpsCreds((prev) => ({ ...prev, password: "" }));
    showNotice("info", "Вы вышли из режима управления VPS");
  };

  const handleManageContainer = async (
    containerId: string,
    action: "start" | "stop" | "restart" | "delete"
  ) => {
    if (!vpsCreds.host.trim() || !vpsCreds.password.trim()) {
      showNotice("error", "Введите IP-адрес сервера и пароль SSH (root)");
      return;
    }
    setContainerBusyId(containerId);
    try {
      const msg = await invoke<string>("manage_vps_container_cmd", {
        creds: vpsCreds,
        containerId,
        action,
      });
      setConfirmDeleteContainerId(null);
      if (action === "delete" && configsFilterContainerId === containerId) {
        setConfigsFilterContainerId("all");
      }
      showNotice("success", msg);
      await handleLoginVpsAdmin(true);
    } catch (err: any) {
      showNotice("error", String(err));
    } finally {
      setContainerBusyId(null);
    }
  };

  const toggleDeviceProtocol = (protoKey: VpsDeviceProtocolId) => {
    setSelectedDeviceProtocols((prev) =>
      prev.includes(protoKey) ? prev.filter((p) => p !== protoKey) : [...prev, protoKey]
    );
  };

  const selectContainerAvailableProtocols = (containerId?: string) => {
    const targetId = containerId ?? selectedAdminContainerId;
    const cont = vpsContainers.find((c) => c.id === targetId);
    if (!cont) return;
    const avail = VPS_DEVICE_PROTOCOLS.filter((p) => isProtocolAvailableInContainer(p, cont));
    setSelectedDeviceProtocols(avail);
  };

  const handleCreateDeviceKey = async () => {
    if (!deviceNameInput.trim()) {
      showNotice("error", "Введите название устройства или имя друга");
      return;
    }
    if (selectedDeviceProtocols.length === 0) {
      showNotice("error", "Выберите хотя бы один протокол для выпуска ключа");
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
      const res = await invoke<{ created: VpnProfile[]; skipped: string[] }>("create_device_keys", {
        creds: vpsCreds,
        deviceName: deviceNameInput.trim(),
        protocols: selectedDeviceProtocols,
        containerId: selectedAdminContainerId || null,
      });
      setLastCreatedKeys(res);
      if (res.created.length > 0) {
        setDeviceNameInput("");
        await handleLoginVpsAdmin(true);
        if (res.created.length === 1 && res.skipped.length === 0) {
          showNotice("success", `Ключ для <${res.created[0].name}> успешно создан на VPS!`);
          await handleOpenQrModal(res.created[0]);
        } else {
          const skipSuffix =
            res.skipped.length > 0 ? ` Пропущено: ${res.skipped.length}` : "";
          showNotice(
            res.skipped.length > 0 ? "info" : "success",
            `Создано ключей: ${res.created.length}.${skipSuffix}`
          );
        }
      } else {
        showNotice(
          "error",
          res.skipped.length > 0
            ? res.skipped.join(" ")
            : "Не удалось создать ни одного ключа для выбранных протоколов"
        );
      }
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

  const addProcessItemRef = useRef(addProcessItem);
  addProcessItemRef.current = addProcessItem;

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

  const loadRunningProcesses = async () => {
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

  const handleOpenProcessModal = async (target: "proxy" | "direct") => {
    try {
      await invoke("open_popup_window", {
        mode: "processes",
        profileId: routingProfileId || popupParamsRef.current.profileId || selectedProfileId || null,
        submode: target,
      });
    } catch (err: any) {
      showNotice("error", String(err));
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

    const effectiveOutbound =
      (popupParamsRef.current.submode as "direct" | "proxy" | null) ||
      routing.default_outbound ||
      newPresetOutbound ||
      "direct";

    const newPreset: RoutingPreset = {
      id: `custom-preset-${Date.now()}`,
      name: trimmed,
      description: `Пользовательский набор (${proxyProcessesList.length} через VPN, ${directProcessesList.length} мимо VPN)`,
      default_outbound: effectiveOutbound,
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
      if (popupParamsRef.current.mode === "save_preset") {
        await invoke("app_close");
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



  const renderProtocolBadge = (protocol: string, version?: string | null) => {
    const meta = getProtocolMeta(protocol, version);
    const shortLabel =
      meta.id === "awg2"
        ? "AWG 2.0"
        : meta.id === "awg_quic"
        ? "AWG + QUIC"
        : meta.id === "awg_vless"
        ? "AWG + Reality"
        : meta.id === "vless_ws"
        ? "Cloudflare WS"
        : meta.id === "webrtc"
        ? "WebRTC Стелс"
        : meta.id === "shadowsocks"
        ? "Shadowsocks"
        : meta.label;

    return (
      <span
        style={getProtocolBadgeStyle(meta, isLightTheme)}
        className="px-2.5 py-1 rounded-lg border font-mono text-[10px] font-bold tracking-wider uppercase inline-flex items-center gap-1.5 shrink-0 shadow-sm"
      >
        <span
          style={{
            background: meta.isHybrid ? meta.gradientLine : meta.accent,
          }}
          className="w-1.5 h-1.5 rounded-full shrink-0"
        />
        {shortLabel}
      </span>
    );
  };

  const allPopupModes = [
    "file",
    "key",
    "vps",
    "vps_admin",
    "logs",
    "qr",
    "edit",
    "routing_rules",
    "processes",
    "save_preset",
    "confirm_clean",
  ];

  if (popupMode && allPopupModes.includes(popupMode)) {
    const isPopupMaximizable =
      popupMode === "vps" ||
      popupMode === "vps_admin" ||
      popupMode === "logs" ||
      popupMode === "edit" ||
      popupMode === "routing_rules" ||
      popupMode === "processes";
    const popupRulesTarget = popupParams.submode === "direct" ? "direct" : "proxy";
    const popupProcTarget = popupParams.submode === "direct" ? "direct" : "proxy";
    const popupPresetOutbound =
      (popupParams.submode === "proxy" ? "proxy" : popupParams.submode === "direct" ? "direct" : null) ||
      routing.default_outbound ||
      "direct";

    const popupHeaderTitle =
      popupMode === "file"
        ? "Добавить из файла"
        : popupMode === "key"
        ? "Вставить ключ или создать туннель"
        : popupMode === "vps"
        ? "Добавить VPS — Автодеплой по SSH"
        : popupMode === "vps_admin"
        ? "Управление VPS — Контейнеры и ключи"
        : popupMode === "logs"
        ? "Журнал ядра sing-box"
        : popupMode === "qr"
        ? `QR-код: ${qrModalProfile?.name || "Подключение смартфона"}`
        : popupMode === "edit"
        ? `Редактирование: ${editingProfile?.name || "Конфигурация"}`
        : popupMode === "routing_rules"
        ? popupRulesTarget === "proxy"
          ? "Только выбранное через VPN (Белый список)"
          : "Весь трафик через VPN (Черный список)"
        : popupMode === "processes"
        ? "Выбор из запущенных процессов"
        : popupMode === "save_preset"
        ? "Новая предустановка маршрутов"
        : "Подтвердите очистку сервера";

    return (
      <div
        data-theme={currentTheme.id}
        data-theme-mode={isLightTheme ? "light" : "dark"}
        style={{
          ...cssVarsStyle,
          opacity: windowOpacity / 100,
        }}
        className={`flex flex-col h-screen w-screen overflow-hidden ${
          isLightTheme ? "text-slate-900" : "text-slate-100"
        } select-none bg-[#090d16] border border-cyan-500/40 rounded-xl shadow-2xl relative`}
      >
        {/* Draggable Top Titlebar for Floating Popup Window */}
        <header className="h-9 w-full flex items-center justify-between px-3 pt-0.5 shrink-0 select-none cursor-default z-30 bg-transparent">
          <div
            className="flex-1 h-full flex items-center gap-2 min-w-0 cursor-default"
            onMouseDown={handleMouseDownDrag}
            onDoubleClick={isPopupMaximizable ? handleDoubleClickMaximize : undefined}
            data-tauri-drag-region
          >
            <img
              src={appIconLogo}
              alt="NeuroVeil"
              className="w-4 h-4 rounded object-cover pointer-events-none shrink-0"
            />
            <span className="text-xs font-bold tracking-tight text-white pointer-events-none truncate">
              {popupHeaderTitle}
            </span>
          </div>

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
                } catch {}
              }}
              className="w-7 h-6 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition cursor-pointer"
              title="Свернуть"
            >
              <Minus className="w-3.5 h-3.5 pointer-events-none" />
            </button>
            {isPopupMaximizable && (
              <button
                type="button"
                data-tauri-drag-region="false"
                onMouseDown={(e) => e.stopPropagation()}
                onClick={async (e) => {
                  e.stopPropagation();
                  try {
                    await invoke("app_toggle_maximize");
                  } catch {}
                }}
                className="w-7 h-6 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition cursor-pointer"
                title="Развернуть"
              >
                <Square className="w-3 h-3 pointer-events-none" />
              </button>
            )}
            <button
              type="button"
              data-tauri-drag-region="false"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={async (e) => {
                e.stopPropagation();
                try {
                  await invoke("app_close");
                } catch {}
              }}
              className="w-7 h-6 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-rose-600 transition cursor-pointer"
              title="Закрыть окно"
            >
              <X className="w-3.5 h-3.5 pointer-events-none" />
            </button>
          </div>
        </header>

        {/* Toast Banner inside Popup Window */}
        {toast && (
          <div
            className={`toast-notice absolute top-11 left-2.5 right-2.5 z-[70] flex items-center justify-between gap-2 px-3 py-2.5 rounded-xl shadow-2xl border text-xs font-semibold transition-all backdrop-blur-md ${
              toast.type === "success"
                ? "bg-emerald-700/95 border-emerald-400 text-white shadow-emerald-500/30"
                : toast.type === "error"
                ? "bg-rose-700/95 border-rose-400 text-white shadow-rose-500/30"
                : "bg-slate-900/95 border-cyan-400 text-cyan-100 shadow-cyan-500/30"
            }`}
          >
            <div className="flex items-center gap-2 min-w-0">
              {toast.type === "success" && <CheckCircle2 className="w-4 h-4 text-emerald-200 shrink-0" />}
              {toast.type === "error" && <XCircle className="w-4 h-4 text-rose-200 shrink-0" />}
              {toast.type === "info" && <Sparkles className="w-4 h-4 text-cyan-300 shrink-0" />}
              <span className="text-white font-medium break-words">{toast.text}</span>
            </div>
            <button
              onClick={() => setToast(null)}
              className="text-white/70 hover:text-white transition p-0.5 shrink-0"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* POPUP 1: ДОБАВИТЬ ИЗ ФАЙЛА */}
        {popupMode === "file" && (
          <div className="flex-1 flex flex-col justify-between p-5 overflow-y-auto">
            <div className="space-y-4">
              <div className="flex items-center gap-3.5">
                <div className="w-12 h-12 rounded-2xl bg-cyan-500/15 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0 shadow-lg shadow-cyan-500/10">
                  <FolderOpen className="w-6 h-6" />
                </div>
                <div>
                  <h2 className="text-base font-bold text-white">Импорт конфигурации из файла</h2>
                  <p className="text-xs text-slate-400 mt-0.5">
                    Поддерживаются файлы <b>.conf</b> (AmneziaWG / WireGuard), <b>.json</b> (sing-box) и <b>.txt</b> с ключами.
                  </p>
                </div>
              </div>

              <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800 space-y-3">
                <p className="text-xs text-slate-300 leading-relaxed">
                  Нажмите кнопку ниже, чтобы выбрать файл конфигурации на вашем компьютере. После импорта туннель автоматически появится в главном окне приложения.
                </p>

                <button
                  type="button"
                  onClick={handlePickConfigFile}
                  className="w-full py-3 px-4 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-500/20 flex items-center justify-center gap-2 transition cursor-pointer keep-white"
                >
                  <FolderOpen className="w-4 h-4" />
                  <span>Выбрать файл (.conf / .json / .txt)</span>
                </button>
              </div>

              {profiles.length > 0 && (
                <div className="p-3 rounded-xl bg-emerald-950/30 border border-emerald-500/30 flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                    <span className="text-xs text-emerald-200 truncate">
                      Добавлено туннелей в приложении: <b>{profiles.length}</b>
                    </span>
                  </div>
                </div>
              )}
            </div>

            <div className="flex justify-end pt-3 border-t border-slate-800/80">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
              >
                Закрыть окно
              </button>
            </div>
          </div>
        )}

        {/* POPUP 2: ВСТАВИТЬ КЛЮЧ */}
        {popupMode === "key" && (
          <div className="flex-1 flex flex-col justify-between p-5 overflow-y-auto space-y-4">
            <div className="space-y-3.5">
              <div className="flex items-center gap-3">
                <div className="w-11 h-11 rounded-2xl bg-indigo-500/15 border border-indigo-500/40 flex items-center justify-center text-indigo-400 shrink-0 shadow-lg shadow-indigo-500/10">
                  <Key className="w-5 h-5" />
                </div>
                <div>
                  <h2 className="text-sm font-bold text-white">Вставить ключ или текст конфига</h2>
                  <p className="text-[11px] text-slate-400">
                    Поддерживаются ключи <code>vless://</code>, <code>awg://</code>, <code>hy2://</code>, <code>ss://</code>, <code>tuic://</code> или текст <code>[Interface]</code>
                  </p>
                </div>
              </div>

              <div className="space-y-2">
                <textarea
                  rows={5}
                  placeholder="Вставьте сюда ссылку-ключ (vless://..., awg://..., hy2://..., ss://..., tuic://...) или полный текст .conf / .json"
                  value={uriInput}
                  onChange={(e) => setUriInput(e.target.value)}
                  className="w-full p-3 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-cyan-200 focus:border-cyan-500 focus:outline-none select-text resize-none"
                />

                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={async () => {
                      try {
                        const clip = await navigator.clipboard.readText();
                        if (clip) setUriInput(clip.trim());
                      } catch {}
                    }}
                    className="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
                  >
                    Вставить из буфера
                  </button>
                  <button
                    type="button"
                    onClick={handleImportUri}
                    className="flex-1 py-2 px-4 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-500/20 transition cursor-pointer keep-white"
                  >
                    Импортировать ключ
                  </button>
                </div>
              </div>

              {/* Optional manual creation toggle inside Key popup */}
              <div className="pt-1">
                <button
                  type="button"
                  onClick={() => setShowNewProfileForm(!showNewProfileForm)}
                  className="text-xs font-semibold text-cyan-400 hover:text-cyan-300 flex items-center gap-1.5 transition cursor-pointer"
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>{showNewProfileForm ? "Скрыть ручной ввод параметров" : "Или ввести параметры сервера вручную"}</span>
                </button>

                {showNewProfileForm && (
                  <div className="mt-2.5 p-3 rounded-xl bg-[#111827] border border-cyan-500/40 space-y-2.5">
                    <div className="grid grid-cols-2 gap-2">
                      <div className="col-span-2 space-y-1.5">
                        <label className="block text-[10px] text-slate-400">Выберите протокол</label>
                        <div className="grid grid-cols-2 gap-1.5 max-h-44 overflow-y-auto pr-1 custom-scrollbar">
                          {(
                            [
                              "vless",
                              "amneziawg",
                              "awg_quic",
                              "awg_vless",
                              "awg2",
                              "hysteria2",
                              "tuic",
                              "shadowtls",
                              "shadowsocks",
                              "vless_ws",
                              "webrtc",
                              "ssh",
                            ] as const
                          ).map((protoKey) => {
                            const meta = getProtocolMeta(protoKey);
                            const isSelected = newProfile.protocol === protoKey;
                            return (
                              <button
                                key={protoKey}
                                type="button"
                                onClick={() => {
                                  const isAwg31 =
                                    protoKey === "amneziawg" ||
                                    protoKey === "awg_quic" ||
                                    protoKey === "awg_vless";
                                  const isAwg20 = protoKey === "awg2";
                                  setNewProfile({
                                    ...newProfile,
                                    protocol: protoKey as any,
                                    port:
                                      protoKey === "ssh"
                                        ? 22
                                        : protoKey === "vless"
                                        ? 443
                                        : protoKey === "tuic"
                                        ? 9443
                                        : protoKey === "shadowsocks"
                                        ? 8388
                                        : protoKey === "vless_ws"
                                        ? 2096
                                        : protoKey === "webrtc"
                                        ? 3478
                                        : isAwg31 || isAwg20
                                        ? getRandomInt(21000, 55000)
                                        : 8443,
                                  });
                                }}
                                style={{
                                  background: isSelected
                                    ? "var(--bg-card, #111827)"
                                    : "var(--bg-card-alt, #0b101d)",
                                  borderColor: isSelected
                                    ? isLightTheme
                                      ? LIGHT_PROTOCOL_COLORS[meta.id]?.border || meta.border
                                      : meta.border
                                    : "var(--border-card, rgba(51, 65, 85, 0.55))",
                                }}
                                className="relative overflow-hidden p-2 rounded-xl border text-left transition cursor-pointer flex flex-col gap-1"
                              >
                                <span
                                  style={{
                                    background: meta.isHybrid ? meta.gradientLine : meta.accent,
                                  }}
                                  className="absolute left-0 top-0 bottom-0 w-1"
                                />
                                <div className="flex items-center justify-between gap-1 pl-1.5">
                                  <span className="text-[11px] font-bold text-white truncate flex items-center gap-1.5">
                                    <span
                                      style={{
                                        background: meta.isHybrid ? meta.gradientLine : meta.accent,
                                      }}
                                      className="w-2 h-2 rounded-full shrink-0"
                                    />
                                    <span className="truncate">{meta.label}</span>
                                  </span>
                                  <span
                                    style={getProtocolBadgeStyle(meta, isLightTheme)}
                                    className="text-[8.5px] px-1.5 py-0.5 rounded border font-mono font-bold shrink-0"
                                  >
                                    {meta.isHybrid ? "ГИБРИД" : meta.shortBadge.split("•")[0].trim()}
                                  </span>
                                </div>
                                <p className="protocol-card-desc text-[9.5px] leading-snug pl-1.5 line-clamp-2">
                                  {meta.description}
                                </p>
                              </button>
                            );
                          })}
                        </div>
                      </div>
                      <div>
                        <label className="block text-[10px] text-slate-400 mb-0.5">Название</label>
                        <input
                          type="text"
                          placeholder="Мой туннель"
                          value={newProfile.name}
                          onChange={(e) => setNewProfile({ ...newProfile, name: e.target.value })}
                          className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-[10px] text-slate-400 mb-0.5">IP / Сервер</label>
                        <input
                          type="text"
                          placeholder="185.x.x.x"
                          value={newProfile.server}
                          onChange={(e) => setNewProfile({ ...newProfile, server: e.target.value })}
                          className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-[10px] text-slate-400 mb-0.5">Порт</label>
                        <input
                          type="number"
                          value={newProfile.port}
                          onChange={(e) => setNewProfile({ ...newProfile, port: Number(e.target.value) || 443 })}
                          className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-[10px] text-slate-400 mb-0.5">
                          {newProfile.protocol === "ssh" ? "Логин SSH" : "SNI / Домен"}
                        </label>
                        <input
                          type="text"
                          value={newProfile.sni}
                          onChange={(e) => setNewProfile({ ...newProfile, sni: e.target.value })}
                          className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white"
                        />
                      </div>
                      <div className="col-span-2">
                        <label className="block text-[10px] text-slate-400 mb-0.5">
                          Пароль / UUID / Приватный ключ
                        </label>
                        <input
                          type="text"
                          placeholder="Пароль, UUID или Base64 PrivateKey"
                          value={newProfile.password}
                          onChange={(e) =>
                            setNewProfile({
                              ...newProfile,
                              password: e.target.value,
                              awg_private_key: e.target.value,
                            })
                          }
                          className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                        />
                      </div>
                    </div>
                    <div className="flex justify-end gap-2 pt-1">
                      <button
                        type="button"
                        onClick={handleSaveManualProfile}
                        className="px-3.5 py-1.5 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-xs font-bold text-white cursor-pointer"
                      >
                        Сохранить туннель
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>

            <div className="flex justify-end pt-2 border-t border-slate-800/80">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
              >
                Закрыть окно
              </button>
            </div>
          </div>
        )}

        {/* POPUP 3: ДОБАВИТЬ VPS (АВТОДЕПЛОЙ ПО SSH) */}
        {popupMode === "vps" && (
          <div className="flex-1 overflow-y-auto p-4 flex flex-col justify-between min-h-0 space-y-4">
            <div className="space-y-4">
              {/* Card 1: Server SSH Parameters */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-3.5">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-1.5">
                    <Server className="w-3.5 h-3.5 text-cyan-400" />
                    1. Параметры сервера VPS (SSH)
                  </h3>
                  <button
                    type="button"
                    onClick={handleTestMainServer}
                    disabled={mainCheckStatus.busy || vpsBusy !== "idle"}
                    className="px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 border border-amber-400/50 text-[11px] font-semibold text-amber-300 disabled:opacity-50 transition flex items-center gap-1.5 cursor-pointer"
                  >
                    <Wifi className="w-3 h-3 text-amber-400" />
                    {mainCheckStatus.busy ? "Проверка..." : "Проверить доступность"}
                  </button>
                </div>

                <div className="grid grid-cols-3 gap-3">
                  <div className="col-span-2">
                    <label className="block text-xs text-slate-400 mb-1">IP-адрес или домен VPS</label>
                    <input
                      type="text"
                      placeholder="Например: 185.22.14.99"
                      value={vpsCreds.host}
                      onChange={(e) => setVpsCreds({ ...vpsCreds, host: e.target.value })}
                      className="w-full px-3 py-1.5 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">SSH Порт</label>
                    <input
                      type="number"
                      value={vpsCreds.ssh_port}
                      onChange={(e) =>
                        setVpsCreds({ ...vpsCreds, ssh_port: Number(e.target.value) || 22 })
                      }
                      className="w-full px-3 py-1.5 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Логин SSH</label>
                    <input
                      type="text"
                      value={vpsCreds.username}
                      onChange={(e) => setVpsCreds({ ...vpsCreds, username: e.target.value })}
                      className="w-full px-3 py-1.5 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Пароль root / SSH</label>
                    <div className="relative">
                      <input
                        type={showVpsPassword ? "text" : "password"}
                        placeholder="••••••••••••"
                        value={vpsCreds.password}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, password: e.target.value })}
                        className="w-full pl-3 pr-8 py-1.5 rounded-xl bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => setShowVpsPassword(!showVpsPassword)}
                        className="absolute right-2.5 top-2 text-slate-400 hover:text-slate-200"
                      >
                        {showVpsPassword ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                      </button>
                    </div>
                  </div>
                </div>

                {mainCheckStatus.result && (
                  <div className="flex items-center gap-2 p-2 rounded-xl bg-emerald-950/40 border border-emerald-500/30 text-emerald-300 text-[11px]">
                    <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                    <span className="truncate"><b>VPS доступен:</b> {mainCheckStatus.result}</span>
                  </div>
                )}
                {mainCheckStatus.error && (
                  <div className="flex items-center gap-2 p-2 rounded-xl bg-rose-950/40 border border-rose-500/30 text-rose-300 text-[11px]">
                    <XCircle className="w-3.5 h-3.5 text-rose-400 shrink-0" />
                    <span className="truncate"><b>VPS недоступен:</b> {mainCheckStatus.error}</span>
                  </div>
                )}

                <div className="flex flex-wrap gap-2 pt-1">
                  <button
                    onClick={handleTestSsh}
                    disabled={vpsBusy !== "idle"}
                    className="px-3.5 py-2 rounded-xl bg-gradient-to-r from-amber-500 to-yellow-600 hover:from-amber-400 hover:to-yellow-500 disabled:opacity-50 text-xs font-bold text-white flex items-center justify-center gap-1.5 cursor-pointer keep-white"
                  >
                    <Wifi className="w-3.5 h-3.5" />
                    {vpsBusy === "testing" ? "Проверка..." : "Проверить SSH"}
                  </button>

                  <button
                    onClick={handleDeployVps}
                    disabled={vpsBusy !== "idle"}
                    className="flex-1 py-2 px-4 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 disabled:opacity-50 text-xs font-bold text-white flex items-center justify-center gap-1.5 cursor-pointer keep-white"
                  >
                    <Zap className="w-3.5 h-3.5" />
                    {vpsBusy === "deploying" ? "Развертывание на VPS..." : "Развернуть VPN на сервере"}
                  </button>

                  <button
                    onClick={() =>
                      invoke("open_popup_window", {
                        mode: "confirm_clean",
                        submode: vpsCreds.host.trim() || "VPS",
                      })
                    }
                    disabled={vpsBusy !== "idle"}
                    className="px-3 py-2 rounded-xl bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 disabled:opacity-50 text-xs font-semibold text-rose-300 flex items-center justify-center gap-1.5 cursor-pointer"
                    title="Очистить сервер"
                  >
                    <Trash2 className="w-3.5 h-3.5 text-rose-400" />
                    <span>Очистить</span>
                  </button>
                </div>
              </div>

              {/* Card 2: Protocol Selection */}
              <div className="p-4 rounded-2xl bg-[#111827] border border-slate-800/90 space-y-2.5">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-xs font-bold uppercase tracking-wider text-indigo-400 flex items-center gap-1.5">
                    <Layers className="w-3.5 h-3.5 text-indigo-400" />
                    2. Протоколы для автоматической установки
                  </h3>
                  <span className="text-[10px] text-slate-400">
                    Гибриды отмечены градиентом
                  </span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  {(
                    [
                      { key: "vless", field: "deploy_vless", defaultChecked: true },
                      { key: "amneziawg", field: "deploy_awg", defaultChecked: true },
                      { key: "awg_quic", field: "deploy_awg_quic", defaultChecked: true },
                      { key: "awg_vless", field: "deploy_awg_vless", defaultChecked: true },
                      { key: "awg2", field: "deploy_awg2", defaultChecked: false },
                      { key: "hysteria2", field: "deploy_hy2", defaultChecked: true },
                      { key: "tuic", field: "deploy_tuic", defaultChecked: false },
                      { key: "shadowtls", field: "deploy_shadowtls", defaultChecked: true },
                      { key: "shadowsocks", field: "deploy_ss", defaultChecked: false },
                      { key: "vless_ws", field: "deploy_vless_ws", defaultChecked: true },
                      { key: "webrtc", field: "deploy_webrtc", defaultChecked: true },
                      { key: "ssh", field: "deploy_ssh", defaultChecked: true },
                    ] as const
                  ).map((item) => {
                    const meta = getProtocolMeta(item.key);
                    const checked = Boolean((vpsCreds as any)[item.field] ?? item.defaultChecked);
                    return (
                      <label
                        key={item.key}
                        style={{
                          background: "var(--bg-card-alt, #0b101d)",
                          borderColor: checked
                            ? "var(--color-accent-border, rgba(100, 116, 139, 0.65))"
                            : "var(--border-card, rgba(51, 65, 85, 0.55))",
                        }}
                        className="relative overflow-hidden flex items-start gap-2.5 p-2.5 pl-3.5 rounded-xl border cursor-pointer transition hover:border-slate-600"
                      >
                        <span
                          style={{
                            background: meta.isHybrid ? meta.gradientLine : meta.accent,
                          }}
                          className="absolute left-0 top-0 bottom-0 w-1"
                        />
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={(e) =>
                            setVpsCreds({ ...vpsCreds, [item.field]: e.target.checked } as VpsCredentials)
                          }
                          className="mt-0.5 shrink-0"
                        />
                        <div className="min-w-0 flex-1 space-y-1">
                          <div className="flex items-center justify-between gap-1.5">
                            <span className="text-xs font-bold text-white flex items-center gap-1.5 truncate">
                              <span
                                style={{
                                  background: meta.isHybrid ? meta.gradientLine : meta.accent,
                                }}
                                className="w-2 h-2 rounded-full shrink-0"
                              />
                              <span className="truncate">{meta.label}</span>
                            </span>
                            <span
                              style={getProtocolBadgeStyle(meta, isLightTheme)}
                              className="text-[9px] px-1.5 py-0.5 rounded border font-mono font-bold shrink-0"
                            >
                              {meta.shortBadge}
                            </span>
                          </div>
                          <p className="protocol-card-desc text-[10.5px] leading-snug">
                            {meta.description}
                          </p>
                        </div>
                      </label>
                    );
                  })}
                </div>
              </div>
            </div>

            {/* Bottom Live SSH Deployment Terminal */}
            <div className="p-3.5 rounded-2xl bg-[#0d1322] border border-slate-800/90 space-y-2 shrink-0 w-full">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-slate-400 flex items-center gap-2">
                  <Terminal className="w-3.5 h-3.5 text-cyan-400" />
                  Терминал выполнения SSH и журнал развертывания
                </span>
                <button
                  type="button"
                  onClick={() => setVpsLogs([])}
                  className="text-[11px] text-slate-500 hover:text-slate-300 transition cursor-pointer"
                >
                  Очистить
                </button>
              </div>
              <div className="h-32 overflow-y-auto font-mono text-[11px] bg-[#060911] p-2.5 rounded-xl border border-slate-800/80 space-y-1 select-text">
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
          </div>
        )}

        {/* POPUP 4: ЖУРНАЛ ЯДРА SING-BOX */}
        {popupMode === "logs" && (
          <div className="flex-1 flex flex-col overflow-hidden bg-[#0b101d]">
            <div className="px-4 py-2.5 bg-[#111827] border-b border-slate-800 flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2 text-xs text-slate-300">
                <Terminal className="w-4 h-4 text-cyan-400" />
                <span>
                  Состояние ядра:{" "}
                  <b className={status.connected ? "text-emerald-400" : "text-slate-300"}>
                    {status.connected ? "Активно" : "Остановлено"}
                  </b>{" "}
                  {status.singbox_version ? `(${status.singbox_version})` : ""}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleExportCoreLogs}
                  className="px-2.5 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 flex items-center gap-1 text-[11px] text-cyan-300 font-semibold transition cursor-pointer"
                >
                  <FileDown className="w-3.5 h-3.5" />
                  Экспорт
                </button>
                <button
                  type="button"
                  onClick={() => invoke("clear_core_logs")}
                  className="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] text-slate-300 hover:text-white transition cursor-pointer"
                >
                  Очистить
                </button>
              </div>
            </div>
            <div className="flex-1 overflow-y-auto font-mono text-[11px] text-slate-300 bg-[#070a12] p-3.5 space-y-1 select-text custom-scrollbar">
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
            <div className="px-4 py-2.5 border-t border-slate-800 bg-[#0d1322] flex justify-end shrink-0">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
              >
                Закрыть окно
              </button>
            </div>
          </div>
        )}

        {/* POPUP 5: QR-КОД И ПОДКЛЮЧЕНИЕ СМАРТФОНА */}
        {popupMode === "qr" && (
          <div className="flex-1 flex flex-col overflow-hidden">
            {!qrModalProfile ? (
              <div className="flex-1 flex items-center justify-center p-6 text-xs text-slate-400">
                Загрузка QR-кода конфигурации...
              </div>
            ) : (
              <div className="flex-1 overflow-y-auto p-4 space-y-3.5 custom-scrollbar">
                <div className="flex items-center justify-between border-b border-slate-800 pb-3">
                  <div>
                    <h3 className="text-sm font-bold text-white flex items-center gap-2">
                      <Smartphone className="w-4 h-4 text-cyan-400 shrink-0" />
                      <span>Подключение смартфона: {qrModalProfile.name}</span>
                    </h3>
                    <p className="text-xs text-slate-400 mt-0.5">
                      Сервер: {qrModalProfile.server}:{qrModalProfile.port} (
                      {qrModalProfile.protocol === "ssh"
                        ? "SSH Tunnel"
                        : qrModalProfile.protocol === "vless"
                        ? "VLESS Reality"
                        : qrModalProfile.protocol === "vless_ws"
                        ? "VLESS CDN Shield"
                        : qrModalProfile.protocol === "webrtc"
                        ? "WebRTC Media"
                        : qrModalProfile.protocol === "shadowtls"
                        ? "ShadowTLS v3"
                        : qrModalProfile.protocol === "tuic"
                        ? "TUIC v5"
                        : qrModalProfile.protocol === "shadowsocks"
                        ? "Shadowsocks"
                        : qrModalProfile.protocol === "awg2"
                        ? "AmneziaWG 2.0"
                        : qrModalProfile.protocol === "awg_quic"
                        ? "AWG + QUIC Pre-Warm"
                        : qrModalProfile.protocol === "awg_vless"
                        ? "Hybrid AWG + Reality"
                        : qrModalProfile.protocol === "amneziawg"
                        ? "Amnezia 3.1"
                        : qrModalProfile.protocol === "wireguard"
                        ? "WireGuard"
                        : "Hysteria 2"}
                      )
                    </p>
                  </div>
                </div>

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
                            type="button"
                            onClick={() => loadQrForProfile(alt)}
                            className="px-2.5 py-1 rounded-lg bg-purple-600/30 hover:bg-purple-600/50 border border-purple-500/40 text-xs font-semibold text-purple-200 cursor-pointer"
                          >
                            {alt.protocol === "ssh"
                              ? "SSH Tunnel"
                              : alt.protocol === "vless"
                              ? "VLESS Reality"
                              : alt.protocol === "vless_ws"
                              ? "VLESS CDN"
                              : alt.protocol === "webrtc"
                              ? "WebRTC"
                              : alt.protocol === "shadowtls"
                              ? "ShadowTLS v3"
                              : alt.protocol === "hysteria2"
                              ? "Hysteria 2"
                              : alt.protocol === "tuic"
                              ? "TUIC v5"
                              : alt.protocol === "awg2"
                              ? "AWG 2.0"
                              : alt.protocol === "amneziawg"
                              ? "Amnezia 3.1"
                              : alt.protocol === "wireguard"
                              ? "WireGuard"
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
                      className="w-48 h-48 block"
                    />
                  </div>
                </div>

                <div
                  className={`grid ${
                    qrModalProfile.protocol === "amneziawg" ||
                    qrModalProfile.protocol === "awg2" ||
                    qrModalProfile.protocol === "wireguard"
                      ? "grid-cols-3"
                      : "grid-cols-2"
                  } gap-2`}
                >
                  <button
                    type="button"
                    onClick={() => handleCopyUri(qrModalProfile)}
                    className="py-2 px-3 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-cyan-300 flex items-center justify-center gap-1.5 cursor-pointer"
                  >
                    <Copy className="w-4 h-4 shrink-0" />
                    <span className="truncate">
                      Ссылка{" "}
                      {qrModalProfile.protocol === "ssh"
                        ? "ssh://"
                        : qrModalProfile.protocol === "vless" ||
                          qrModalProfile.protocol === "vless_ws" ||
                          qrModalProfile.protocol === "webrtc"
                        ? "vless://"
                        : qrModalProfile.protocol === "shadowtls"
                        ? "shadowtls://"
                        : qrModalProfile.protocol === "tuic"
                        ? "tuic://"
                        : qrModalProfile.protocol === "shadowsocks"
                        ? "ss://"
                        : qrModalProfile.protocol === "awg2"
                        ? "awg2://"
                        : qrModalProfile.protocol === "amneziawg" ||
                          qrModalProfile.protocol === "wireguard"
                        ? "awg://"
                        : "hysteria2://"}
                    </span>
                  </button>
                  {(qrModalProfile.protocol === "amneziawg" ||
                    qrModalProfile.protocol === "awg2" ||
                    qrModalProfile.protocol === "wireguard") && (
                    <button
                      type="button"
                      onClick={() => handleDownloadAwgConf(qrModalProfile)}
                      className="py-2 px-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-xs font-bold text-white flex items-center justify-center gap-1.5 cursor-pointer keep-white"
                    >
                      <FileDown className="w-4 h-4 shrink-0" />
                      <span className="truncate">.conf (Amnezia)</span>
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => handleExportJson(qrModalProfile)}
                    className="py-2 px-3 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-xs font-bold text-white flex items-center justify-center gap-1.5 cursor-pointer keep-white"
                  >
                    <FileDown className="w-4 h-4 shrink-0" />
                    <span className="truncate">Скачать .json</span>
                  </button>
                </div>

                <div className="p-3 rounded-xl bg-amber-950/30 border border-amber-500/40 text-[11px] text-amber-100 space-y-1.5">
                  <div className="font-bold text-amber-300">
                    Почему в Hiddify пишет «Тайм-аут» и «Неизвестный IP»? Как исправить за 15 секунд:
                  </div>
                  <div>
                    1. <b>Используйте протокол TUIC v5 (порт 9443)</b>: в мобильном Hiddify есть баг парсера ссылок Hysteria 2 с паролем обфускации <code>obfs=salamander</code>.
                  </div>
                  <div>
                    2. В Hiddify откройте <b>Параметры конфигурации</b> → <b>Регион</b> → <b>«Другой (other)»</b> и включите <b>«Разрешить небезопасные (Allow Insecure)»</b>.
                  </div>
                  <div>
                    3. <b>Или используйте официальное приложение <code>sing-box</code>, Shadowsocks / v2rayNG / NekoBox</b> или скачанный <code>.json</code>.
                  </div>
                </div>
              </div>
            )}
            <div className="px-4 py-2.5 border-t border-slate-800 bg-[#0d1322] flex justify-end shrink-0">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
              >
                Закрыть окно
              </button>
            </div>
          </div>
        )}

        {/* POPUP 6: РЕДАКТИРОВАНИЕ КОНФИГУРАЦИИ */}
        {popupMode === "edit" && (
          <div className="flex-1 flex flex-col overflow-hidden">
            {!editingProfile ? (
              <div className="flex-1 flex items-center justify-center p-6 text-xs text-slate-400">
                Загрузка параметров конфигурации...
              </div>
            ) : (
              <>
                <div className="px-4 py-3 border-b border-slate-800 bg-[#0b101d] flex items-center justify-between gap-2 shrink-0">
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className="p-2 rounded-xl bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 shrink-0">
                      <Edit3 className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                      <h3 className="text-sm font-bold text-white truncate">
                        {editingProfile.name}
                      </h3>
                      <p className="text-[11px] text-slate-400 truncate">
                        Протокол:{" "}
                        {editingProfile.protocol === "awg2"
                          ? "AMNEZIAWG 2.0"
                          : editingProfile.protocol === "awg_quic"
                          ? "AWG + QUIC PRE-WARM"
                          : editingProfile.protocol === "awg_vless"
                          ? "ГИБРИД AWG + REALITY"
                          : editingProfile.protocol === "amneziawg"
                          ? "AMNEZIA 3.1"
                          : editingProfile.protocol.toUpperCase()}{" "}
                        • ID: {editingProfile.id}
                      </p>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={async () => {
                      if (!editModeRaw) {
                        const isAwgOrWg =
                          editingProfile.protocol === "amneziawg" ||
                          editingProfile.protocol === "awg_quic" ||
                          editingProfile.protocol === "awg_vless" ||
                          editingProfile.protocol === "awg2" ||
                          editingProfile.protocol === "wireguard";
                        if (isAwgOrWg) {
                          try {
                            const conf = await invoke<string>("export_awg_conf", {
                              profileId: editingProfile.id,
                              profile: { ...editingProfile, ...editForm },
                            });
                            setEditRawConf(conf);
                          } catch {}
                        }
                        setEditRawJson(JSON.stringify({ ...editingProfile, ...editForm }, null, 2));
                      }
                      setEditModeRaw(!editModeRaw);
                    }}
                    className="px-3 py-1.5 rounded-xl bg-cyan-600/20 hover:bg-cyan-600/30 text-xs font-semibold text-cyan-300 border border-cyan-500/40 flex items-center gap-1.5 transition shrink-0 cursor-pointer"
                  >
                    <FileText className="w-3.5 h-3.5" />
                    {editModeRaw ? "Визуальный редактор" : "Редактировать текст конфига"}
                  </button>
                </div>

                <div className="flex-1 overflow-y-auto p-4 space-y-3.5 custom-scrollbar select-text">
                  {editModeRaw ? (
                    <div className="space-y-3">
                      <div className="flex items-center justify-between flex-wrap gap-2">
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-slate-300 font-bold">Текстовый редактор конфигурации:</span>
                          {(editingProfile.protocol === "amneziawg" ||
                            editingProfile.protocol === "awg_quic" ||
                            editingProfile.protocol === "awg_vless" ||
                            editingProfile.protocol === "awg2" ||
                            editingProfile.protocol === "wireguard") && (
                            <div className="flex items-center gap-1 p-0.5 rounded-lg bg-slate-800 border border-slate-700">
                              <button
                                type="button"
                                onClick={() => setRawTextFormat("conf")}
                                className={`px-2 py-0.5 rounded text-[11px] font-semibold transition cursor-pointer ${
                                  rawTextFormat === "conf" ? "bg-cyan-600 text-white" : "text-slate-400 hover:text-white"
                                }`}
                              >
                                .conf (WireGuard / Amnezia)
                              </button>
                              <button
                                type="button"
                                onClick={() => setRawTextFormat("json")}
                                className={`px-2 py-0.5 rounded text-[11px] font-semibold transition cursor-pointer ${
                                  rawTextFormat === "json" ? "bg-cyan-600 text-white" : "text-slate-400 hover:text-white"
                                }`}
                              >
                                JSON (Sing-box)
                              </button>
                            </div>
                          )}
                        </div>
                        <span className="text-[11px] text-slate-400">
                          {rawTextFormat === "conf" ? "Синтаксис .conf [Interface] / [Peer]" : "Синтаксис JSON"}
                        </span>
                      </div>

                      {rawTextFormat === "conf" ? (
                        <textarea
                          rows={16}
                          value={editRawConf}
                          onChange={(e) => setEditRawConf(e.target.value)}
                          placeholder="[Interface]&#10;PrivateKey = ...&#10;Address = ...&#10;ListenPort = ...&#10;&#10;[Peer]&#10;PublicKey = ...&#10;Endpoint = ..."
                          className="w-full p-3 rounded-xl bg-[#070a12] border border-slate-800 text-xs font-mono text-emerald-300 focus:outline-none focus:border-cyan-500 leading-relaxed"
                        />
                      ) : (
                        <textarea
                          rows={16}
                          value={editRawJson}
                          onChange={(e) => setEditRawJson(e.target.value)}
                          className="w-full p-3 rounded-xl bg-[#070a12] border border-slate-800 text-xs font-mono text-cyan-200 focus:outline-none focus:border-cyan-500 leading-relaxed"
                        />
                      )}
                      <p className="text-[11px] text-slate-400">
                        💡 Вы можете напрямую отредактировать или вставить текст конфигурации. При сохранении параметры будут проверены и применены.
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
                          <label className="block text-xs text-slate-400 mb-1">Порт (сервера)</label>
                          <input
                            type="number"
                            value={editForm.port || 0}
                            onChange={(e) => setEditForm({ ...editForm, port: parseInt(e.target.value, 10) || 0 })}
                            className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        <div>
                          <label className="block text-xs text-slate-400 mb-1">Порт прослушивания (Listen Port)</label>
                          <div className="flex items-center gap-1.5">
                            <input
                              type="number"
                              value={editForm.listen_port ?? ""}
                              placeholder="Случайный / не задан"
                              onChange={(e) =>
                                setEditForm({
                                  ...editForm,
                                  listen_port: e.target.value ? parseInt(e.target.value, 10) || null : null,
                                })
                              }
                              className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono focus:border-cyan-500"
                            />
                            <button
                              type="button"
                              onClick={() =>
                                setEditForm({
                                  ...editForm,
                                  listen_port: getRandomInt(10000, 64999),
                                })
                              }
                              className="px-2.5 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs text-cyan-300 shrink-0 transition cursor-pointer"
                              title="Выбрать случайный порт прослушивания"
                            >
                              🎲
                            </button>
                          </div>
                        </div>
                        {(editingProfile.protocol === "amneziawg" ||
                          editingProfile.protocol === "awg_quic" ||
                          editingProfile.protocol === "awg_vless" ||
                          editingProfile.protocol === "awg2" ||
                          editingProfile.protocol === "wireguard") && (
                          <div className="col-span-2">
                            <label className="flex items-center gap-2.5 p-2.5 rounded-xl bg-[#0b101d] border border-slate-800 hover:border-cyan-500/40 cursor-pointer transition">
                              <input
                                type="checkbox"
                                checked={editForm.auto_rotate_listen_port ?? true}
                                onChange={(e) =>
                                  setEditForm({
                                    ...editForm,
                                    auto_rotate_listen_port: e.target.checked,
                                  })
                                }
                                className="w-4 h-4 rounded border-slate-700 bg-slate-900 text-cyan-500 focus:ring-cyan-500"
                              />
                              <div>
                                <div className="text-xs font-semibold text-slate-200">
                                  Менять порт прослушивания (ListenPort) при переподключении
                                </div>
                                <div className="text-[10px] text-slate-400">
                                  При нажатии кнопки «Переподключиться» выбирается новый свободный UDP-порт; основная кнопка подключения сохраняет текущий порт
                                </div>
                              </div>
                            </label>
                          </div>
                        )}
                        <div
                          className={
                            editingProfile.protocol !== "amneziawg" &&
                            editingProfile.protocol !== "awg_quic" &&
                            editingProfile.protocol !== "awg_vless" &&
                            editingProfile.protocol !== "awg2" &&
                            editingProfile.protocol !== "wireguard"
                              ? "col-span-1"
                              : "col-span-2"
                          }
                        >
                          <label className="block text-xs text-slate-400 mb-1">SNI / Хост маскировки</label>
                          <input
                            type="text"
                            value={editForm.sni || ""}
                            onChange={(e) => setEditForm({ ...editForm, sni: e.target.value })}
                            className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                          />
                        </div>
                        {editingProfile.protocol !== "amneziawg" &&
                          editingProfile.protocol !== "awg_quic" &&
                          editingProfile.protocol !== "awg_vless" &&
                          editingProfile.protocol !== "awg2" &&
                          editingProfile.protocol !== "wireguard" && (
                            <div className={editingProfile.protocol === "tuic" ? "col-span-1" : "col-span-2"}>
                              <label className="block text-xs text-slate-400 mb-1">
                                {editingProfile.protocol === "vless" ||
                                editingProfile.protocol === "vless_ws" ||
                                editingProfile.protocol === "webrtc"
                                  ? "UUID пользователя"
                                  : "Пароль / Секретный ключ"}
                              </label>
                              <input
                                type="text"
                                value={editForm.password || editForm.uuid || ""}
                                onChange={(e) => {
                                  const val = e.target.value;
                                  if (
                                    editingProfile.protocol === "vless" ||
                                    editingProfile.protocol === "vless_ws" ||
                                    editingProfile.protocol === "webrtc"
                                  ) {
                                    setEditForm({ ...editForm, password: val, uuid: val });
                                  } else {
                                    setEditForm({ ...editForm, password: val });
                                  }
                                }}
                                className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          )}
                        {editingProfile.protocol === "tuic" && (
                          <div className="col-span-2">
                            <label className="block text-xs text-slate-400 mb-1">TUIC UUID</label>
                            <input
                              type="text"
                              value={editForm.uuid || ""}
                              onChange={(e) => setEditForm({ ...editForm, uuid: e.target.value })}
                              className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                            />
                          </div>
                        )}
                      </div>

                      {editingProfile.protocol === "awg_vless" && (
                        <div className="p-3.5 rounded-xl bg-fuchsia-950/20 border border-fuchsia-500/35 space-y-2.5">
                          <div className="text-xs font-bold text-fuchsia-300">
                            Параметры капсулы VLESS Reality Detour (TCP 443):
                          </div>
                          <div className="grid grid-cols-2 gap-2 text-xs">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">Порт VLESS Reality (TCP)</label>
                              <input
                                type="number"
                                value={editForm.vless_port ?? 443}
                                onChange={(e) => setEditForm({ ...editForm, vless_port: parseInt(e.target.value, 10) || 443 })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">UUID VLESS</label>
                              <input
                                type="text"
                                value={editForm.uuid || ""}
                                placeholder="UUID пользователя VLESS"
                                onChange={(e) => setEditForm({ ...editForm, uuid: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">Reality Public Key</label>
                              <input
                                type="text"
                                value={editForm.reality_public_key || ""}
                                placeholder="Public Key Reality"
                                onChange={(e) => setEditForm({ ...editForm, reality_public_key: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">Reality Short ID (sid)</label>
                              <input
                                type="text"
                                value={editForm.reality_short_id || ""}
                                placeholder="0123456789abcdef"
                                onChange={(e) => setEditForm({ ...editForm, reality_short_id: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          </div>
                        </div>
                      )}

                      {(editingProfile.protocol === "amneziawg" ||
                        editingProfile.protocol === "awg_quic" ||
                        editingProfile.protocol === "awg_vless") && (
                        <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-3">
                          <div className="flex items-center justify-between gap-2">
                            <div>
                              <div className="text-xs font-bold text-emerald-400">
                                Параметры Amnezia 3.1 (Header Protection + 32-bit H1-H4):
                              </div>
                              <span className="text-[10px] text-slate-400">
                                Стек обфускации, 32-битные заголовки H1-H4, динамические сдвиги S1-S4 ≥ 12 и Port Hunting
                              </span>
                            </div>
                            <button
                              type="button"
                              onClick={() => {
                                const r = generateRandomAwg31Params();
                                setEditForm({ ...editForm, ...r });
                                showNotice("success", "Параметры Amnezia 3.1 (включая 32-битные H1-H4) рандомизированы!");
                              }}
                              className="px-2.5 py-1 rounded-lg bg-emerald-600/30 hover:bg-emerald-600/50 border border-emerald-500/40 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition shrink-0 cursor-pointer"
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
                                placeholder="20000:65000"
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
                          <div>
                            <div className="text-xs font-bold text-teal-400">Параметры AmneziaWG 2.0:</div>
                            <span className="text-[10px] text-slate-400">
                              Чистая конфигурация (PrivateKey, Address, DNS 1.1.1.1, MTU 1420, PublicKey, PresharedKey, Endpoint)
                            </span>
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
                                placeholder="10.8.1.2/24"
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
                          </div>
                        </div>
                      )}

                      {editingProfile.protocol === "wireguard" && (
                        <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-3">
                          <div>
                            <div className="text-xs font-bold text-rose-400">Параметры стандартного WireGuard:</div>
                            <span className="text-[10px] text-slate-400">Чистый протокол WireGuard (без обфускации AWG)</span>
                          </div>
                          <div className="grid grid-cols-2 gap-2 text-xs">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">
                                Private Key {!editForm.awg_private_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
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
                                Public Key (Сервер) {!editForm.awg_public_key && <span className="text-amber-400 font-bold ml-1">(обязательно)</span>}
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
                                placeholder="20000:65000"
                                onChange={(e) => setEditForm({ ...editForm, hop_ports: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          </div>
                        </div>
                      )}

                      {editingProfile.protocol === "shadowtls" && (
                        <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-2">
                          <div className="text-xs font-bold text-pink-400">Параметры ShadowTLS v3:</div>
                          <div className="grid grid-cols-2 gap-2 text-xs">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">Пароль ShadowTLS рукопожатия</label>
                              <input
                                type="text"
                                value={editForm.shadowtls_password || ""}
                                onChange={(e) => setEditForm({ ...editForm, shadowtls_password: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">Паразитный SNI (Apple / Microsoft)</label>
                              <input
                                type="text"
                                value={editForm.shadowtls_sni || ""}
                                placeholder="gateway.icloud.com"
                                onChange={(e) => setEditForm({ ...editForm, shadowtls_sni: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          </div>
                        </div>
                      )}

                      {editingProfile.protocol === "vless_ws" && (
                        <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-2">
                          <div className="text-xs font-bold text-orange-400">Параметры VLESS + WebSocket (CDN Shield):</div>
                          <div className="grid grid-cols-2 gap-2 text-xs">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">WebSocket Path</label>
                              <input
                                type="text"
                                value={editForm.ws_path || ""}
                                placeholder="/ws-tunnel"
                                onChange={(e) => setEditForm({ ...editForm, ws_path: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">CDN Domain / Host header (опционально)</label>
                              <input
                                type="text"
                                value={editForm.cdn_host || ""}
                                placeholder="worker.example.workers.dev"
                                onChange={(e) => setEditForm({ ...editForm, cdn_host: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          </div>
                        </div>
                      )}

                      {editingProfile.protocol === "webrtc" && (
                        <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800 space-y-2">
                          <div className="text-xs font-bold text-lime-400">Параметры WebRTC Steganography:</div>
                          <div className="grid grid-cols-2 gap-2 text-xs">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">gRPC Service / Media stream</label>
                              <input
                                type="text"
                                value={editForm.ws_path || "webrtc-media"}
                                placeholder="webrtc-media"
                                onChange={(e) => setEditForm({ ...editForm, ws_path: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-0.5">SNI (Маскировочный хост)</label>
                              <input
                                type="text"
                                value={editForm.sni || ""}
                                placeholder="zoom.us или meet.google.com"
                                onChange={(e) => setEditForm({ ...editForm, sni: e.target.value })}
                                className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-xs text-white font-mono"
                              />
                            </div>
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>

                <div className="flex justify-end gap-2.5 px-4 py-2.5 border-t border-slate-800 bg-[#0d1322] shrink-0">
                  <button
                    type="button"
                    onClick={() => invoke("app_close")}
                    className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 transition cursor-pointer"
                  >
                    Отмена
                  </button>
                  <button
                    type="button"
                    onClick={handleSaveEditedProfile}
                    disabled={editSaving}
                    className="px-5 py-1.5 rounded-xl bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-cyan-600/30 transition flex items-center gap-2 cursor-pointer keep-white"
                  >
                    {editSaving ? "Сохранение..." : "Сохранить изменения"}
                  </button>
                </div>
              </>
            )}
          </div>
        )}

        {/* POPUP 7: НАСТРОЙКА ПРАВИЛ РАЗДЕЛЬНОГО ТУННЕЛИРОВАНИЯ */}
        {popupMode === "routing_rules" && (
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-2.5 border-b border-slate-800 bg-[#0b101d] shrink-0">
              <div className="flex items-center gap-2.5 min-w-0">
                <div
                  className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${
                    popupRulesTarget === "proxy"
                      ? "bg-cyan-500/20 border border-cyan-500/40 text-cyan-300"
                      : "bg-emerald-500/20 border border-emerald-500/40 text-emerald-300"
                  }`}
                >
                  {popupRulesTarget === "proxy" ? (
                    <Shield className="w-4 h-4" />
                  ) : (
                    <Zap className="w-4 h-4" />
                  )}
                </div>
                <div className="min-w-0">
                  <h3 className="text-xs font-bold text-white truncate">
                    {popupRulesTarget === "proxy"
                      ? "Настройка: Только выбранное через VPN (Белый список)"
                      : "Настройка: Весь трафик через VPN (Черный список)"}
                  </h3>
                  <p className="text-[10px] text-slate-400 truncate">
                    {popupRulesTarget === "proxy"
                      ? "Приложения, процессы, домены и IP, идущие ЧЕРЕЗ VPN"
                      : "Приложения, процессы, домены и IP, идущие МИМО VPN (напрямую)"}
                  </p>
                </div>
              </div>
            </div>

            <div className="flex-1 overflow-y-auto p-3.5 space-y-3 custom-scrollbar select-text">
              {popupRulesTarget === "proxy" ? (
                <>
                  <div className="p-3 rounded-xl bg-[#080d19] border border-cyan-500/25 space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-1.5">
                        <Cpu className="w-3.5 h-3.5" />
                        Приложения, процессы и папки ({proxyProcessesList.length})
                      </span>
                    </div>

                    <div className="flex flex-wrap gap-1.5">
                      <button
                        type="button"
                        onClick={() => handlePickExeFile("proxy")}
                        className="px-2.5 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <FileCode className="w-3 h-3" /> + Файл (.exe)
                      </button>
                      <button
                        type="button"
                        onClick={() => handlePickFolder("proxy")}
                        className="px-2.5 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <FolderOpen className="w-3 h-3" /> + Папка
                      </button>
                      <button
                        type="button"
                        onClick={() => handleOpenProcessModal("proxy")}
                        className="px-2.5 py-1 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-[11px] font-semibold text-indigo-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <Cpu className="w-3 h-3" /> + Из запущенных
                      </button>
                    </div>

                    <div className="flex gap-1.5">
                      <input
                        type="text"
                        placeholder="Имя процесса (Discord.exe) или путь к папке..."
                        value={manualProxyProcInput}
                        onChange={(e) => setManualProxyProcInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            addProcessItem("proxy", manualProxyProcInput);
                            setManualProxyProcInput("");
                          }
                        }}
                        className="flex-1 px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-cyan-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => {
                          addProcessItem("proxy", manualProxyProcInput);
                          setManualProxyProcInput("");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 cursor-pointer shrink-0"
                      >
                        Добавить
                      </button>
                    </div>

                    <div className="h-36 overflow-y-auto rounded-lg bg-[#050811] border border-slate-800/80 p-1.5 space-y-1">
                      {proxyProcessesList.length === 0 ? (
                        <div className="h-full flex items-center justify-center text-[11px] text-slate-500 text-center px-3">
                          Список пуст. Добавьте .exe файл, папку или выберите из запущенных процессов.
                        </div>
                      ) : (
                        proxyProcessesList.map((item, idx) => {
                          const isPath = item.includes("\\") || item.includes("/");
                          const isExe = item.toLowerCase().endsWith(".exe");
                          return (
                            <div
                              key={idx}
                              className="flex items-center justify-between px-2 py-1 rounded bg-[#0e1628] border border-slate-800 text-xs"
                            >
                              <div className="flex items-center gap-1.5 min-w-0 flex-1 pr-2">
                                {isPath && !isExe ? (
                                  <FolderOpen className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                                ) : isPath ? (
                                  <FileCode className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                                ) : (
                                  <Cpu className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
                                )}
                                <div className="truncate font-mono text-[11px] text-slate-200" title={item}>
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

                  <div className="p-3 rounded-xl bg-[#080d19] border border-cyan-500/25 space-y-1.5">
                    <label className="text-[11px] font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-1.5">
                      <Globe className="w-3.5 h-3.5" />
                      <span>Домены ЧЕРЕЗ VPN (по одному на строку)</span>
                    </label>
                    <textarea
                      rows={3}
                      value={proxyDomainsText}
                      placeholder="youtube.com&#10;discord.com&#10;openai.com"
                      onChange={(e) => setProxyDomainsText(e.target.value)}
                      className="w-full p-2.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>

                  <div className="p-3 rounded-xl bg-[#080d19] border border-cyan-500/25 space-y-1.5">
                    <label className="text-[11px] font-bold uppercase tracking-wider text-cyan-400 flex items-center gap-1.5">
                      <Layers className="w-3.5 h-3.5" />
                      <span>IP-адреса и подсети CIDR ЧЕРЕЗ VPN</span>
                    </label>
                    <textarea
                      rows={3}
                      value={proxyIpsText}
                      placeholder="1.1.1.1&#10;8.8.8.8&#10;149.154.160.0/20"
                      onChange={(e) => setProxyIpsText(e.target.value)}
                      className="w-full p-2.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-cyan-500 focus:outline-none"
                    />
                  </div>
                </>
              ) : (
                <>
                  <div className="p-3 rounded-xl bg-[#061111] border border-emerald-500/25 space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
                        <Cpu className="w-3.5 h-3.5" />
                        Приложения, процессы и папки МИМО VPN ({directProcessesList.length})
                      </span>
                    </div>

                    <div className="flex flex-wrap gap-1.5">
                      <button
                        type="button"
                        onClick={() => handlePickExeFile("direct")}
                        className="px-2.5 py-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <FileCode className="w-3 h-3" /> + Файл (.exe)
                      </button>
                      <button
                        type="button"
                        onClick={() => handlePickFolder("direct")}
                        className="px-2.5 py-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <FolderOpen className="w-3 h-3" /> + Папка
                      </button>
                      <button
                        type="button"
                        onClick={() => handleOpenProcessModal("direct")}
                        className="px-2.5 py-1 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-[11px] font-semibold text-indigo-300 flex items-center gap-1 transition cursor-pointer"
                      >
                        <Cpu className="w-3 h-3" /> + Из запущенных
                      </button>
                    </div>

                    <div className="flex gap-1.5">
                      <input
                        type="text"
                        placeholder="Имя процесса (cs2.exe) или путь к папке..."
                        value={manualDirectProcInput}
                        onChange={(e) => setManualDirectProcInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            addProcessItem("direct", manualDirectProcInput);
                            setManualDirectProcInput("");
                          }
                        }}
                        className="flex-1 px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs text-white focus:border-emerald-500 focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => {
                          addProcessItem("direct", manualDirectProcInput);
                          setManualDirectProcInput("");
                        }}
                        className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 cursor-pointer shrink-0"
                      >
                        Добавить
                      </button>
                    </div>

                    <div className="h-36 overflow-y-auto rounded-lg bg-[#040b0b] border border-slate-800/80 p-1.5 space-y-1">
                      {directProcessesList.length === 0 ? (
                        <div className="h-full flex items-center justify-center text-[11px] text-slate-500 text-center px-3">
                          Список пуст. Добавьте .exe файл, папку или выберите из запущенных процессов.
                        </div>
                      ) : (
                        directProcessesList.map((item, idx) => {
                          const isPath = item.includes("\\") || item.includes("/");
                          const isExe = item.toLowerCase().endsWith(".exe");
                          return (
                            <div
                              key={idx}
                              className="flex items-center justify-between px-2 py-1 rounded bg-[#091818] border border-slate-800 text-xs"
                            >
                              <div className="flex items-center gap-1.5 min-w-0 flex-1 pr-2">
                                {isPath && !isExe ? (
                                  <FolderOpen className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                                ) : isPath ? (
                                  <FileCode className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                                ) : (
                                  <Cpu className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
                                )}
                                <div className="truncate font-mono text-[11px] text-slate-200" title={item}>
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

                  <div className="p-3 rounded-xl bg-[#061111] border border-emerald-500/25 space-y-1.5">
                    <label className="text-[11px] font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
                      <Globe className="w-3.5 h-3.5" />
                      <span>Домены МИМО VPN (по одному на строку)</span>
                    </label>
                    <textarea
                      rows={3}
                      value={directDomainsText}
                      placeholder=".ru&#10;vk.com&#10;yandex.ru"
                      onChange={(e) => setDirectDomainsText(e.target.value)}
                      className="w-full p-2.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-emerald-500 focus:outline-none"
                    />
                  </div>

                  <div className="p-3 rounded-xl bg-[#061111] border border-emerald-500/25 space-y-1.5">
                    <label className="text-[11px] font-bold uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
                      <Layers className="w-3.5 h-3.5" />
                      <span>IP-адреса и подсети CIDR МИМО VPN</span>
                    </label>
                    <textarea
                      rows={3}
                      value={directIpsText}
                      placeholder="192.168.0.0/16&#10;10.0.0.0/8"
                      onChange={(e) => setDirectIpsText(e.target.value)}
                      className="w-full p-2.5 rounded-lg bg-[#0b101d] border border-slate-700/80 text-xs font-mono text-slate-200 focus:border-emerald-500 focus:outline-none"
                    />
                  </div>
                </>
              )}
            </div>

            <div className="flex items-center justify-between px-4 py-2.5 border-t border-slate-800 bg-[#0d1322] shrink-0">
              <span className="text-[10px] text-slate-400">
                Изменения применяются к текущему профилю
              </span>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => invoke("app_close")}
                  className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-300 cursor-pointer"
                >
                  Закрыть
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await handleSaveRouting();
                    await invoke("app_close");
                  }}
                  className="px-3.5 py-1.5 rounded-lg bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-md shadow-cyan-600/20 cursor-pointer keep-white"
                >
                  Сохранить и закрыть
                </button>
              </div>
            </div>
          </div>
        )}

        {/* POPUP 8: ВЫБОР ИЗ ЗАПУЩЕННЫХ ПРОЦЕССОВ */}
        {popupMode === "processes" && (
          <div className="flex-1 flex flex-col overflow-hidden p-4 space-y-3">
            <div className="flex items-center justify-between pb-2.5 border-b border-slate-800 shrink-0">
              <div>
                <h3 className="text-sm font-bold text-white flex items-center gap-2">
                  <Cpu className="w-4 h-4 text-cyan-400" />
                  Выбор из запущенных процессов и их папок
                </h3>
                <p className="text-xs text-slate-400 mt-0.5">
                  Добавление в список:{" "}
                  <b className={popupProcTarget === "proxy" ? "text-cyan-300" : "text-emerald-300"}>
                    {popupProcTarget === "proxy" ? "ЧЕРЕЗ VPN" : "МИМО VPN (Напрямую)"}
                  </b>
                </p>
              </div>
              <button
                type="button"
                onClick={loadRunningProcesses}
                className="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 cursor-pointer"
              >
                Обновить
              </button>
            </div>

            <div className="relative shrink-0">
              <Search className="w-4 h-4 text-slate-400 absolute left-3 top-2.5" />
              <input
                type="text"
                placeholder="Поиск по имени процесса или пути (например discord, steam, chrome)..."
                value={procSearch}
                onChange={(e) => setProcSearch(e.target.value)}
                className="w-full pl-9 pr-4 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
              />
            </div>

            <div className="flex-1 overflow-y-auto space-y-1.5 pr-1 custom-scrollbar">
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
                          <div className="text-xs font-bold text-white">{proc.name}</div>
                          <div className="text-[10px] text-slate-400 font-mono truncate" title={proc.path}>
                            {proc.path}
                          </div>
                        </div>
                        <div className="flex items-center gap-1.5 shrink-0">
                          <button
                            type="button"
                            onClick={async () => {
                              await invoke("emit_window_event", {
                                event: "process-picked",
                                payload: { target: popupProcTarget, item: proc.name },
                              });
                              showNotice("info", `Добавлен процесс: ${proc.name}`);
                            }}
                            className="px-2.5 py-1 rounded-lg bg-indigo-500/20 hover:bg-indigo-500/30 border border-indigo-500/30 text-[11px] font-semibold text-indigo-300 cursor-pointer"
                          >
                            + Имя ({proc.name})
                          </button>
                          {folderPath && (
                            <button
                              type="button"
                              onClick={async () => {
                                await invoke("emit_window_event", {
                                  event: "process-picked",
                                  payload: { target: popupProcTarget, item: folderPath },
                                });
                                showNotice("info", `Добавлена вся папка: ${folderPath}`);
                              }}
                              className="px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/30 text-[11px] font-semibold text-amber-300 cursor-pointer"
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

            <div className="pt-2 border-t border-slate-800 flex justify-end shrink-0">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 transition cursor-pointer"
              >
                Закрыть окно
              </button>
            </div>
          </div>
        )}

        {/* POPUP 9: НОВАЯ ПРЕДУСТАНОВКА МАРШРУТОВ */}
        {popupMode === "save_preset" && (
          <div className="flex-1 flex flex-col justify-between p-5 overflow-y-auto space-y-4">
            <div className="space-y-3.5">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0">
                  <Bookmark className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-white">Новая предустановка маршрутов</h3>
                  <p className="text-[11px] text-slate-400">
                    Сохранение текущих списков в отдельный пресет
                  </p>
                </div>
              </div>

              <p className="text-xs text-slate-300 leading-relaxed">
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
                  className="w-full px-3.5 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-500 select-text"
                />
              </div>

              <div className="p-3 rounded-xl bg-[#0b101d] border border-slate-800 space-y-2">
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
                    {popupPresetOutbound === "direct"
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
            </div>

            <div className="flex justify-end gap-2.5 pt-3 border-t border-slate-800">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-300 transition cursor-pointer"
              >
                Отмена
              </button>
              <button
                type="button"
                onClick={handleSaveAsNewPreset}
                disabled={!newPresetName.trim()}
                className="px-4 py-1.5 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-600/20 disabled:opacity-50 transition cursor-pointer keep-white"
              >
                Сохранить пресет
              </button>
            </div>
          </div>
        )}

        {/* POPUP 10: ПОДТВЕРЖДЕНИЕ ОЧИСТКИ СЕРВЕРА */}
        {popupMode === "confirm_clean" && (
          <div className="flex-1 flex flex-col justify-between p-5 space-y-3">
            <div className="space-y-3">
              <div className="flex items-center gap-2.5 text-rose-400">
                <AlertTriangle className="w-5 h-5 shrink-0" />
                <h3 className="text-sm font-bold text-white">Подтвердите очистку сервера</h3>
              </div>
              <p className="text-xs text-slate-300 leading-relaxed">
                Удалить <b>sing-box</b>, все ключи и сертификаты с сервера{" "}
                <b>{popupParams.submode || vpsCreds.host || "VPS"}</b>?
              </p>
            </div>
            <div className="flex justify-end gap-2.5 pt-2 border-t border-slate-800">
              <button
                type="button"
                onClick={() => invoke("app_close")}
                className="px-4 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 cursor-pointer"
              >
                Отмена
              </button>
              <button
                type="button"
                onClick={async () => {
                  await invoke("emit_window_event", {
                    event: "confirm-clean-vps",
                    payload: {},
                  });
                  await invoke("app_close");
                }}
                className="px-4 py-1.5 rounded-xl bg-rose-600 hover:bg-rose-500 text-xs font-bold text-white cursor-pointer keep-white"
              >
                Да, очистить
              </button>
            </div>
          </div>
        )}

        {/* POPUP 11: УПРАВЛЕНИЕ VPS (КОНТЕЙНЕРЫ, КОНФИГУРАЦИИ И НОВЫЕ КЛЮЧИ) */}
        {popupMode === "vps_admin" && (
          <div className="flex-1 flex flex-col overflow-hidden p-4 space-y-3 custom-scrollbar">
            {!isVpsAdmin ? (
              <div className="flex-1 flex items-center justify-center overflow-y-auto">
                <div className="w-full max-w-md p-5 rounded-2xl bg-[#111827] border space-y-4">
                  <div className="flex items-center gap-3">
                    <div className="w-11 h-11 rounded-2xl bg-cyan-500/15 border border-cyan-500/35 flex items-center justify-center text-cyan-400 shrink-0">
                      <ShieldCheck className="w-6 h-6" />
                    </div>
                    <div>
                      <h3 className="text-sm font-bold text-white">
                        Авторизация на VPS-сервере
                      </h3>
                      <p className="text-[11px] text-slate-400 mt-0.5 leading-snug">
                        Войдите по SSH для управления контейнерами, существующими ключами и создания новых конфигов.
                      </p>
                    </div>
                  </div>

                  <div className="grid grid-cols-3 gap-2.5">
                    <div className="col-span-2">
                      <label className="block text-[11px] text-slate-400 mb-1">IP-адрес сервера</label>
                      <input
                        type="text"
                        placeholder="185.x.x.x"
                        value={vpsCreds.host}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, host: e.target.value })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] text-slate-400 mb-1">Порт SSH</label>
                      <input
                        type="number"
                        value={vpsCreds.ssh_port}
                        onChange={(e) =>
                          setVpsCreds({ ...vpsCreds, ssh_port: Number(e.target.value) || 22 })
                        }
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
                      />
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2.5">
                    <div>
                      <label className="block text-[11px] text-slate-400 mb-1">Логин SSH</label>
                      <input
                        type="text"
                        value={vpsCreds.username}
                        onChange={(e) => setVpsCreds({ ...vpsCreds, username: e.target.value })}
                        className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] text-slate-400 mb-1">Пароль root / SSH</label>
                      <div className="relative">
                        <input
                          type={showVpsPassword ? "text" : "password"}
                          placeholder="••••••••••••"
                          value={vpsCreds.password}
                          onChange={(e) => setVpsCreds({ ...vpsCreds, password: e.target.value })}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" && !adminLoginLoading) {
                              void handleLoginVpsAdmin();
                            }
                          }}
                          autoFocus
                          className="w-full pl-3 pr-8 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
                        />
                        <button
                          type="button"
                          onClick={() => setShowVpsPassword(!showVpsPassword)}
                          className="absolute right-2.5 top-2.5 text-slate-400 hover:text-slate-200 cursor-pointer"
                        >
                          {showVpsPassword ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                        </button>
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 pt-1">
                    <button
                      type="button"
                      onClick={() => void handleLoginVpsAdmin()}
                      disabled={adminLoginLoading}
                      className="flex-1 py-2.5 px-4 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-cyan-500/20 flex items-center justify-center gap-2 transition cursor-pointer keep-white"
                    >
                      {adminLoginLoading ? (
                        <RefreshCw className="w-4 h-4 animate-spin" />
                      ) : (
                        <Lock className="w-4 h-4" />
                      )}
                      <span>{adminLoginLoading ? "Подключение по SSH..." : "Войти в управление VPS"}</span>
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <>
                {/* Connected VPS Banner */}
                <div className="p-3 rounded-2xl bg-[#111827] border flex flex-wrap items-center justify-between gap-2 shrink-0">
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className="w-9 h-9 rounded-xl bg-emerald-500/15 border border-emerald-500/35 flex items-center justify-center text-emerald-400 shrink-0">
                      <Server className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-xs font-bold text-white flex items-center gap-2 truncate">
                        <span>
                          {vpsCreds.username || "root"}@{vpsCreds.host}:{vpsCreds.ssh_port}
                        </span>
                        <span className="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold">
                          SSH Активен
                        </span>
                      </div>
                      <div className="text-[11px] text-slate-400">
                        Контейнеров: <b className="text-slate-200">{vpsContainers.length}</b> • Конфигураций:{" "}
                        <b className="text-slate-200">{buildVpsConfigEntries(syncedServerProfiles).length}</b>
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-1.5 shrink-0">
                    <button
                      type="button"
                      onClick={() => invoke("open_popup_window", { mode: "vps" })}
                      className="px-2.5 py-1.5 rounded-xl bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                      title="Развернуть новый изолированный контейнер на этом или другом VPS"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>Новый контейнер</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleLoginVpsAdmin()}
                      disabled={adminLoginLoading}
                      className="px-2.5 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 border text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer disabled:opacity-50"
                      title="Обновить состояние контейнеров и ключей с VPS"
                    >
                      <RefreshCw className={`w-3.5 h-3.5 ${adminLoginLoading ? "animate-spin" : ""}`} />
                      <span>Обновить</span>
                    </button>
                    <button
                      type="button"
                      onClick={handleLogoutVpsAdmin}
                      className="px-2.5 py-1.5 rounded-xl bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 text-[11px] font-semibold text-rose-300 flex items-center gap-1 transition cursor-pointer"
                      title="Завершить сессию администрирования"
                    >
                      <LogOut className="w-3.5 h-3.5" />
                      <span>Выйти</span>
                    </button>
                  </div>
                </div>

                {/* Subtabs */}
                <div className="grid grid-cols-3 gap-2 p-1 rounded-xl bg-[#0d1322] border shrink-0">
                  <button
                    type="button"
                    onClick={() => setVpsAdminSubTab("containers")}
                    className={`py-2 px-3 rounded-lg text-xs font-bold flex items-center justify-center gap-1.5 transition cursor-pointer ${
                      vpsAdminSubTab === "containers"
                        ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/35 shadow-sm"
                        : "text-slate-400 hover:text-white"
                    }`}
                  >
                    <Box className="w-3.5 h-3.5" />
                    <span>Контейнеры ({vpsContainers.length})</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => setVpsAdminSubTab("configs")}
                    className={`py-2 px-3 rounded-lg text-xs font-bold flex items-center justify-center gap-1.5 transition cursor-pointer ${
                      vpsAdminSubTab === "configs"
                        ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/35 shadow-sm"
                        : "text-slate-400 hover:text-white"
                    }`}
                  >
                    <Key className="w-3.5 h-3.5" />
                    <span>Конфигурации ({buildVpsConfigEntries(syncedServerProfiles).length})</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => setVpsAdminSubTab("new_key")}
                    className={`py-2 px-3 rounded-lg text-xs font-bold flex items-center justify-center gap-1.5 transition cursor-pointer ${
                      vpsAdminSubTab === "new_key"
                        ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/35 shadow-sm"
                        : "text-slate-400 hover:text-white"
                    }`}
                  >
                    <UserPlus className="w-3.5 h-3.5" />
                    <span>Новый ключ</span>
                  </button>
                </div>

                {/* Subtab Content */}
                <div className="flex-1 overflow-y-auto space-y-2.5 pr-1 custom-scrollbar min-h-0">
                  {/* SUBTAB 1: CONTAINERS */}
                  {vpsAdminSubTab === "containers" && (
                    <>
                      {vpsContainers.length === 0 ? (
                        <div className="p-6 rounded-2xl bg-[#111827] border text-center space-y-3">
                          <Box className="w-8 h-8 text-slate-500 mx-auto" />
                          <div>
                            <div className="text-xs font-bold text-white">На сервере пока нет контейнеров VPN</div>
                            <p className="text-[11px] text-slate-400 mt-1">
                              Разверните первый контейнер в окне «Добавить VPS» — каждый повторный деплой автоматически создаст новый изолированный контейнер со свободными портами.
                            </p>
                          </div>
                          <button
                            type="button"
                            onClick={() => invoke("open_popup_window", { mode: "vps" })}
                            className="px-4 py-2 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-xs font-bold text-white cursor-pointer keep-white"
                          >
                            Развернуть мульти-контейнер на VPS
                          </button>
                        </div>
                      ) : (
                        <>
                          <div className="flex items-center justify-between gap-2 px-1">
                            <span className="text-[11px] text-slate-400">
                              Изолированные контейнеры со своими портами и набором протоколов
                            </span>
                            <button
                              type="button"
                              onClick={() => invoke("open_popup_window", { mode: "vps" })}
                              className="px-3 py-1.5 rounded-xl bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-[11px] font-bold text-white flex items-center gap-1.5 shrink-0 transition cursor-pointer keep-white shadow-sm shadow-emerald-600/20"
                              title="Развернуть новый изолированный мульти-протокольный контейнер на этом VPS"
                            >
                              <Plus className="w-3.5 h-3.5" />
                              <span>Новый мульти-контейнер</span>
                            </button>
                          </div>
                          {vpsContainers.map((c) => {
                            const isBusy = containerBusyId === c.id;
                            const isRunning = c.status === "running";
                            const confirmingDelete = confirmDeleteContainerId === c.id;
                            return (
                              <div
                                key={c.id}
                                className="p-3.5 rounded-2xl bg-[#111827] border space-y-2.5"
                              >
                                <div className="flex flex-wrap items-start justify-between gap-2">
                                  <div className="min-w-0 space-y-1">
                                    <div className="flex items-center gap-2 flex-wrap">
                                      <span className="text-xs font-bold text-white">{c.name}</span>
                                      <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-slate-800 text-slate-300 border">
                                        {c.id}
                                      </span>
                                      <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-indigo-500/15 text-indigo-300 border border-indigo-500/30 uppercase">
                                        {c.runtime}
                                      </span>
                                      <span
                                        className={`text-[10px] px-2 py-0.5 rounded-full font-semibold flex items-center gap-1 border ${
                                          isRunning
                                            ? "bg-emerald-500/15 text-emerald-300 border-emerald-500/35"
                                            : "bg-slate-800 text-slate-400"
                                        }`}
                                      >
                                        <span
                                          className={`w-1.5 h-1.5 rounded-full ${
                                            isRunning ? "bg-emerald-400" : "bg-slate-500"
                                          }`}
                                        />
                                        {isRunning ? "Запущен" : "Остановлен"}
                                      </span>
                                    </div>
                                    <div className="text-[11px] text-slate-400 flex flex-wrap items-center gap-x-3 gap-y-1">
                                      <span>
                                        Ключей: <b className="text-slate-200">{c.client_count}</b>
                                      </span>
                                      {c.ports.length > 0 && (
                                        <span className="font-mono">
                                          Порты: <b className="text-cyan-300">{c.ports.join(", ")}</b>
                                        </span>
                                      )}
                                      {c.created_at && <span>Создан: {c.created_at}</span>}
                                    </div>
                                  </div>

                                  <div className="flex items-center gap-1.5 flex-wrap shrink-0">
                                    <button
                                      type="button"
                                      disabled={isBusy}
                                      onClick={() =>
                                        void handleManageContainer(c.id, isRunning ? "stop" : "start")
                                      }
                                      className={`px-2.5 py-1.5 rounded-lg border text-[11px] font-semibold flex items-center gap-1 transition cursor-pointer disabled:opacity-50 ${
                                        isRunning
                                          ? "bg-amber-500/15 hover:bg-amber-500/25 border-amber-500/30 text-amber-300"
                                          : "bg-emerald-500/15 hover:bg-emerald-500/25 border-emerald-500/30 text-emerald-300"
                                      }`}
                                    >
                                      {isRunning ? (
                                        <>
                                          <Square className="w-3 h-3" />
                                          <span>Стоп</span>
                                        </>
                                      ) : (
                                        <>
                                          <Play className="w-3 h-3" />
                                          <span>Старт</span>
                                        </>
                                      )}
                                    </button>

                                    <button
                                      type="button"
                                      disabled={isBusy}
                                      onClick={() => void handleManageContainer(c.id, "restart")}
                                      className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer disabled:opacity-50"
                                      title="Перезапустить контейнер"
                                    >
                                      <RotateCcw className={`w-3 h-3 ${isBusy ? "animate-spin" : ""}`} />
                                      <span>Рестарт</span>
                                    </button>

                                    <button
                                      type="button"
                                      onClick={() => {
                                        setSelectedAdminContainerId(c.id);
                                        selectContainerAvailableProtocols(c.id);
                                        setVpsAdminSubTab("new_key");
                                      }}
                                      className="px-2.5 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                      title="Выпустить ключи в этом контейнере"
                                    >
                                      <UserPlus className="w-3 h-3" />
                                      <span>+ Ключ</span>
                                    </button>

                                  {confirmingDelete ? (
                                    <div className="flex items-center gap-1">
                                      <button
                                        type="button"
                                        disabled={isBusy}
                                        onClick={() => void handleManageContainer(c.id, "delete")}
                                        className="px-2.5 py-1.5 rounded-lg bg-rose-600 hover:bg-rose-500 text-[11px] font-bold text-white cursor-pointer disabled:opacity-50 keep-white"
                                      >
                                        Подтвердить
                                      </button>
                                      <button
                                        type="button"
                                        disabled={isBusy}
                                        onClick={() => setConfirmDeleteContainerId(null)}
                                        className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] text-slate-300 cursor-pointer"
                                      >
                                        Отмена
                                      </button>
                                    </div>
                                  ) : (
                                    <button
                                      type="button"
                                      disabled={isBusy}
                                      onClick={() => setConfirmDeleteContainerId(c.id)}
                                      className="px-2.5 py-1.5 rounded-lg bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 text-[11px] font-semibold text-rose-300 flex items-center gap-1 transition cursor-pointer disabled:opacity-50"
                                      title="Удалить контейнер с VPS и убрать связанные профили"
                                    >
                                      <Trash2 className="w-3 h-3" />
                                      <span>Удалить</span>
                                    </button>
                                  )}
                                </div>
                              </div>

                              {c.protocols.length > 0 && (
                                <div className="flex flex-wrap gap-1.5 pt-1">
                                  {c.protocols.map((proto, idx) => (
                                    <span
                                      key={idx}
                                      className="px-2 py-0.5 rounded-md bg-[#0b101d] border text-[10px] font-mono text-slate-300"
                                    >
                                      {proto}
                                    </span>
                                  ))}
                                </div>
                              )}
                            </div>
                          );
                        })}
                        </>
                      )}
                    </>
                  )}

                  {/* SUBTAB 2: CONFIGS / CLIENT KEYS */}
                  {vpsAdminSubTab === "configs" && (() => {
                    const allEntries = buildVpsConfigEntries(syncedServerProfiles);
                    const filteredRaw = syncedServerProfiles.filter(
                      (p) =>
                        configsFilterContainerId === "all" ||
                        p.container_id === configsFilterContainerId
                    );
                    const filteredEntries = buildVpsConfigEntries(filteredRaw);

                    return (
                      <div className="space-y-2.5">
                        {vpsContainers.length > 1 && (
                          <div className="flex items-center gap-1.5 overflow-x-auto pb-1 custom-scrollbar">
                            <button
                              type="button"
                              onClick={() => setConfigsFilterContainerId("all")}
                              className={`px-2.5 py-1 rounded-lg border text-[11px] font-semibold shrink-0 cursor-pointer transition ${
                                configsFilterContainerId === "all"
                                  ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-200"
                                  : "bg-[#0d1322] text-slate-400 hover:text-white"
                              }`}
                            >
                              Все ({allEntries.length})
                            </button>
                            {vpsContainers.map((c) => {
                              const count = buildVpsConfigEntries(
                                syncedServerProfiles.filter((p) => p.container_id === c.id)
                              ).length;
                              return (
                                <button
                                  key={c.id}
                                  type="button"
                                  onClick={() => setConfigsFilterContainerId(c.id)}
                                  className={`px-2.5 py-1 rounded-lg border text-[11px] font-semibold shrink-0 cursor-pointer transition ${
                                    configsFilterContainerId === c.id
                                      ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-200"
                                      : "bg-[#0d1322] text-slate-400 hover:text-white"
                                  }`}
                                >
                                  {c.name} ({count})
                                </button>
                              );
                            })}
                          </div>
                        )}

                        {filteredEntries.length === 0 ? (
                          <div className="p-6 rounded-2xl bg-[#111827] border text-center text-xs text-slate-400">
                            Конфигурации не найдены. Создайте новый ключ во вкладке «Новый ключ» или разверните контейнер.
                          </div>
                        ) : (
                          filteredEntries.map((entry) => {
                            if (entry.kind === "multi") {
                              const isExpanded = expandedMultiGroupIds.includes(entry.keyId);
                              const allPinnedOnPc = entry.profiles.every((sub) =>
                                isServerProfilePinnedOnPc(sub)
                              );
                              const totalUp = entry.profiles.reduce(
                                (sum, sub) => sum + (sub.total_up_bytes || 0),
                                0
                              );
                              const totalDown = entry.profiles.reduce(
                                (sum, sub) => sum + (sub.total_down_bytes || 0),
                                0
                              );
                              const uniquePorts = Array.from(
                                new Set(entry.profiles.map((sub) => sub.port))
                              );
                              const containerLabel = vpsContainers.find(
                                (c) => c.id === entry.containerId
                              )?.name;
                              const isDeleting = deletingMultiGroupId === entry.keyId;

                              return (
                                <div
                                  key={entry.keyId}
                                  className="p-3 rounded-2xl bg-[#111827] border space-y-2.5"
                                >
                                  <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-2.5">
                                    <div className="min-w-0 space-y-1.5">
                                      <div className="flex items-center gap-2 flex-wrap">
                                        <span className="text-[10px] px-2 py-0.5 rounded-md bg-indigo-500/20 text-indigo-300 border border-indigo-500/40 font-bold inline-flex items-center gap-1 shrink-0">
                                          <Layers className="w-3 h-3" />
                                          <span>МУЛЬТИ ×{entry.profiles.length}</span>
                                        </span>
                                        <span className="text-xs font-bold text-white truncate">
                                          {entry.groupName}
                                        </span>
                                        {containerLabel && (
                                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-300 border font-mono">
                                            {containerLabel}
                                          </span>
                                        )}
                                        {allPinnedOnPc && (
                                          <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold">
                                            На этом ПК
                                          </span>
                                        )}
                                      </div>

                                      <div className="flex items-center gap-1.5 flex-wrap">
                                        {entry.profiles.map((sub) => (
                                          <span key={sub.id}>
                                            {renderProtocolBadge(sub.protocol, sub.awg_version)}
                                          </span>
                                        ))}
                                      </div>

                                      <div className="text-[11px] text-slate-400 font-mono flex flex-wrap items-center gap-x-3 gap-y-0.5">
                                        <span>{entry.profiles[0]?.server}</span>
                                        <span>Порты: {uniquePorts.join(", ")}</span>
                                        <span>
                                          Трафик: ↑ {formatBytes(totalUp)} / ↓{" "}
                                          {formatBytes(totalDown)}
                                        </span>
                                      </div>
                                    </div>

                                    <div className="flex items-center gap-1.5 flex-wrap shrink-0">
                                      <button
                                        type="button"
                                        onClick={() =>
                                          void handleCopyMultiUris(
                                            entry.profiles,
                                            entry.groupName,
                                            entry.keyId
                                          )
                                        }
                                        className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                        title="Скопировать все URI мульти-протокола"
                                      >
                                        {copiedMultiId === entry.keyId ? (
                                          <Check className="w-3.5 h-3.5 text-emerald-400" />
                                        ) : (
                                          <Copy className="w-3.5 h-3.5" />
                                        )}
                                        <span>URI</span>
                                      </button>

                                      <button
                                        type="button"
                                        onClick={() =>
                                          void handleExportMultiJson(
                                            entry.profiles,
                                            entry.groupName
                                          )
                                        }
                                        className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-slate-300 flex items-center gap-1 transition cursor-pointer"
                                        title="Скачать единый мульти-протокол (.json для sing-box / NeuroVeil)"
                                      >
                                        <FileCode className="w-3.5 h-3.5" />
                                        <span>.json</span>
                                      </button>

                                      <button
                                        type="button"
                                        onClick={() =>
                                          void handleExportMultiZip(
                                            entry.profiles,
                                            entry.groupName
                                          )
                                        }
                                        className="px-2 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                                        title="Скачать полный пакет мульти-протокола (.zip: .conf + .json + URI)"
                                      >
                                        <Archive className="w-3.5 h-3.5" />
                                        <span>.zip</span>
                                      </button>

                                      {!allPinnedOnPc && (
                                        <button
                                          type="button"
                                          onClick={() =>
                                            void handlePinMultiGroupOnPc(
                                              entry.profiles,
                                              entry.groupName
                                            )
                                          }
                                          className="px-2 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                          title="Добавить все ключи мульти-протокола на этот ПК"
                                        >
                                          <Monitor className="w-3.5 h-3.5" />
                                          <span>На ПК</span>
                                        </button>
                                      )}

                                      <button
                                        type="button"
                                        onClick={() =>
                                          setExpandedMultiGroupIds((prev) =>
                                            prev.includes(entry.keyId)
                                              ? prev.filter((id) => id !== entry.keyId)
                                              : [...prev, entry.keyId]
                                          )
                                        }
                                        className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border text-[11px] font-semibold text-slate-200 flex items-center gap-1 transition cursor-pointer"
                                        title="Показать или скрыть отдельные протоколы в составе мульти-ключа"
                                      >
                                        {isExpanded ? (
                                          <ChevronUp className="w-3.5 h-3.5" />
                                        ) : (
                                          <ChevronDown className="w-3.5 h-3.5" />
                                        )}
                                        <span>Состав ({entry.profiles.length})</span>
                                      </button>

                                      <button
                                        type="button"
                                        disabled={isDeleting}
                                        onClick={() =>
                                          void handleDeleteMultiGroup(
                                            entry.groupId,
                                            entry.groupName,
                                            entry.profiles,
                                            entry.keyId
                                          )
                                        }
                                        className="p-1.5 rounded-lg bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 text-rose-300 transition cursor-pointer disabled:opacity-50"
                                        title="Удалить весь мульти-протокол с VPS"
                                      >
                                        <Trash2 className="w-3.5 h-3.5" />
                                      </button>
                                    </div>
                                  </div>

                                  {isExpanded && (
                                    <div className="pt-2 border-t space-y-1.5">
                                      {entry.profiles.map((sub) => {
                                        const subAwgConfSupported =
                                          sub.protocol === "amneziawg" ||
                                          sub.protocol === "awg2" ||
                                          sub.protocol === "awg_quic" ||
                                          sub.protocol === "awg_vless" ||
                                          sub.protocol === "wireguard";
                                        const subPinnedOnPc = isServerProfilePinnedOnPc(sub);

                                        return (
                                          <div
                                            key={sub.id}
                                            className="p-2.5 rounded-xl bg-[#0b101d] border flex flex-col sm:flex-row sm:items-center justify-between gap-2"
                                          >
                                            <div className="min-w-0 space-y-0.5">
                                              <div className="flex items-center gap-2 flex-wrap">
                                                {renderProtocolBadge(sub.protocol, sub.awg_version)}
                                                <span className="text-[11px] font-bold text-white truncate">
                                                  {sub.name}
                                                </span>
                                                {subPinnedOnPc && (
                                                  <span className="text-[10px] px-1.5 py-0.2 rounded bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold">
                                                    На этом ПК
                                                  </span>
                                                )}
                                              </div>
                                              <div className="text-[10.5px] text-slate-400 font-mono flex flex-wrap items-center gap-x-3 gap-y-0.5">
                                                <span>
                                                  {sub.server}:{sub.port}
                                                </span>
                                                {sub.awg_local_address && (
                                                  <span>IP: {sub.awg_local_address}</span>
                                                )}
                                                <span>
                                                  Трафик: ↑ {formatBytes(sub.total_up_bytes || 0)} / ↓{" "}
                                                  {formatBytes(sub.total_down_bytes || 0)}
                                                </span>
                                              </div>
                                            </div>

                                            <div className="flex items-center gap-1 flex-wrap shrink-0">
                                              <button
                                                type="button"
                                                onClick={() => handleCopyUri(sub)}
                                                className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[10.5px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                                title="Скопировать ссылку-ключ"
                                              >
                                                {copiedId === sub.id ? (
                                                  <Check className="w-3 h-3 text-emerald-400" />
                                                ) : (
                                                  <Copy className="w-3 h-3" />
                                                )}
                                                <span>URI</span>
                                              </button>

                                              <button
                                                type="button"
                                                onClick={() => void handleOpenQrModal(sub)}
                                                className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[10.5px] font-semibold text-purple-300 flex items-center gap-1 transition cursor-pointer"
                                                title="Показать QR-код для телефона"
                                              >
                                                <QrCode className="w-3 h-3" />
                                                <span>QR</span>
                                              </button>

                                              {subAwgConfSupported && (
                                                <button
                                                  type="button"
                                                  onClick={() => void handleDownloadAwgConf(sub)}
                                                  className="px-2 py-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[10.5px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                                                  title="Скачать .conf файл"
                                                >
                                                  <FileDown className="w-3 h-3" />
                                                  <span>.conf</span>
                                                </button>
                                              )}

                                              <button
                                                type="button"
                                                onClick={() => void handleExportJson(sub)}
                                                className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[10.5px] font-semibold text-slate-300 flex items-center gap-1 transition cursor-pointer"
                                                title="Сохранить .json конфиг"
                                              >
                                                <FileCode className="w-3 h-3" />
                                                <span>.json</span>
                                              </button>

                                              {!subPinnedOnPc && (
                                                <button
                                                  type="button"
                                                  onClick={() => void handlePinProfileOnPc(sub.id)}
                                                  className="px-2 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[10.5px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                                  title="Добавить этот ключ в список подключений на этом ПК"
                                                >
                                                  <Monitor className="w-3 h-3" />
                                                  <span>На ПК</span>
                                                </button>
                                              )}

                                              <button
                                                type="button"
                                                onClick={() => void handleDeleteProfile(sub.id, true)}
                                                className="p-1 rounded-lg bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 text-rose-300 transition cursor-pointer"
                                                title="Удалить этот протокол с VPS"
                                              >
                                                <Trash2 className="w-3 h-3" />
                                              </button>
                                            </div>
                                          </div>
                                        );
                                      })}
                                    </div>
                                  )}
                                </div>
                              );
                            }

                            const p = entry.profile;
                            const isAwgConfSupported =
                              p.protocol === "amneziawg" ||
                              p.protocol === "awg2" ||
                              p.protocol === "awg_quic" ||
                              p.protocol === "awg_vless" ||
                              p.protocol === "wireguard";
                            const isPinnedOnPc = isServerProfilePinnedOnPc(p);
                            const containerLabel = vpsContainers.find(
                              (c) => c.id === p.container_id
                            )?.name;

                            return (
                              <div
                                key={p.id}
                                className="p-3 rounded-2xl bg-[#111827] border flex flex-col sm:flex-row sm:items-center justify-between gap-2.5"
                              >
                                <div className="min-w-0 space-y-1">
                                  <div className="flex items-center gap-2 flex-wrap">
                                    {renderProtocolBadge(p.protocol, p.awg_version)}
                                    <span className="text-xs font-bold text-white truncate">
                                      {p.name}
                                    </span>
                                    {containerLabel && (
                                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-800 text-slate-300 border font-mono">
                                        {containerLabel}
                                      </span>
                                    )}
                                    {isPinnedOnPc && (
                                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 font-semibold">
                                        На этом ПК
                                      </span>
                                    )}
                                  </div>
                                  <div className="text-[11px] text-slate-400 font-mono flex flex-wrap items-center gap-x-3 gap-y-0.5">
                                    <span>
                                      {p.server}:{p.port}
                                    </span>
                                    {p.awg_local_address && <span>IP: {p.awg_local_address}</span>}
                                    <span>
                                      Трафик: ↑ {formatBytes(p.total_up_bytes || 0)} / ↓{" "}
                                      {formatBytes(p.total_down_bytes || 0)}
                                    </span>
                                  </div>
                                </div>

                                <div className="flex items-center gap-1.5 flex-wrap shrink-0">
                                  <button
                                    type="button"
                                    onClick={() => handleCopyUri(p)}
                                    className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                    title="Скопировать ссылку-ключ"
                                  >
                                    {copiedId === p.id ? (
                                      <Check className="w-3.5 h-3.5 text-emerald-400" />
                                    ) : (
                                      <Copy className="w-3.5 h-3.5" />
                                    )}
                                    <span>URI</span>
                                  </button>

                                  <button
                                    type="button"
                                    onClick={() => void handleOpenQrModal(p)}
                                    className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-purple-300 flex items-center gap-1 transition cursor-pointer"
                                    title="Показать QR-код для телефона"
                                  >
                                    <QrCode className="w-3.5 h-3.5" />
                                    <span>QR</span>
                                  </button>

                                  {isAwgConfSupported && (
                                    <button
                                      type="button"
                                      onClick={() => void handleDownloadAwgConf(p)}
                                      className="px-2 py-1.5 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 transition cursor-pointer"
                                      title="Скачать .conf файл"
                                    >
                                      <FileDown className="w-3.5 h-3.5" />
                                      <span>.conf</span>
                                    </button>
                                  )}

                                  <button
                                    type="button"
                                    onClick={() => void handleExportJson(p)}
                                    className="px-2 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-slate-300 flex items-center gap-1 transition cursor-pointer"
                                    title="Сохранить .json конфиг"
                                  >
                                    <FileCode className="w-3.5 h-3.5" />
                                    <span>.json</span>
                                  </button>

                                  {!isPinnedOnPc && (
                                    <button
                                      type="button"
                                      onClick={() => void handlePinProfileOnPc(p.id)}
                                      className="px-2 py-1.5 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 transition cursor-pointer"
                                      title="Добавить этот ключ в список подключений на этом ПК"
                                    >
                                      <Monitor className="w-3.5 h-3.5" />
                                      <span>На ПК</span>
                                    </button>
                                  )}

                                  <button
                                    type="button"
                                    onClick={() => void handleDeleteProfile(p.id, true)}
                                    className="p-1.5 rounded-lg bg-rose-950/50 hover:bg-rose-900/60 border border-rose-500/30 text-rose-300 transition cursor-pointer"
                                    title="Удалить ключ с VPS"
                                  >
                                    <Trash2 className="w-3.5 h-3.5" />
                                  </button>
                                </div>
                              </div>
                            );
                          })
                        )}
                      </div>
                    );
                  })()}

                  {/* SUBTAB 3: CREATE NEW DEVICE KEY (MULTI-PROTOCOL) */}
                  {vpsAdminSubTab === "new_key" && (() => {
                    const activeContainer = vpsContainers.find(
                      (c) => c.id === selectedAdminContainerId
                    );
                    const availableCount = activeContainer
                      ? VPS_DEVICE_PROTOCOLS.filter((pk) =>
                          isProtocolAvailableInContainer(pk, activeContainer)
                        ).length
                      : 0;
                    const copyAllCreatedUris = async () => {
                      if (!lastCreatedKeys || lastCreatedKeys.created.length === 0) return;
                      const text = lastCreatedKeys.created
                        .map((p) => `${p.name}: ${p.uri}`)
                        .filter((line) => line.trim().length > 0)
                        .join("\n");
                      try {
                        await navigator.clipboard.writeText(text);
                        showNotice(
                          "success",
                          `Скопировано ссылок: ${lastCreatedKeys.created.length}`
                        );
                      } catch {
                        showNotice("error", "Не удалось скопировать ссылки в буфер обмена");
                      }
                    };

                    return (
                      <div className="space-y-3">
                        <div className="p-4 rounded-2xl bg-[#111827] border space-y-3.5">
                          <div className="flex items-center gap-2.5">
                            <div className="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shrink-0">
                              <UserPlus className="w-4 h-4" />
                            </div>
                            <div>
                              <h3 className="text-xs font-bold text-white">
                                Выпуск клиентских мульти-ключей на VPS
                              </h3>
                              <p className="text-[11px] text-slate-400">
                                Отметьте один или сразу несколько протоколов (включая гибриды) — все ключи создадутся за один проход внутри выбранного контейнера.
                              </p>
                            </div>
                          </div>

                          {vpsContainers.length > 0 && (
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-1">
                                Целевой контейнер
                              </label>
                              <select
                                value={selectedAdminContainerId}
                                onChange={(e) => {
                                  const nextId = e.target.value;
                                  setSelectedAdminContainerId(nextId);
                                  selectContainerAvailableProtocols(nextId);
                                }}
                                className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none"
                              >
                                {vpsContainers.map((c) => (
                                  <option key={c.id} value={c.id}>
                                    {c.name} ({c.id}) — {c.protocols.join(", ") || c.runtime}
                                  </option>
                                ))}
                              </select>
                            </div>
                          )}

                          <div className="space-y-3">
                            <div>
                              <label className="block text-[11px] text-slate-400 mb-1">
                                Название устройства / клиента
                              </label>
                              <input
                                type="text"
                                placeholder="Например: iPhone, Планшет, Друг Алексей"
                                value={deviceNameInput}
                                onChange={(e) => setDeviceNameInput(e.target.value)}
                                onKeyDown={(e) => {
                                  if (e.key === "Enter" && !creatingDevice) {
                                    void handleCreateDeviceKey();
                                  }
                                }}
                                className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border text-xs text-white focus:border-cyan-500 focus:outline-none select-text"
                              />
                            </div>

                            <div>
                              <div className="flex flex-wrap items-center justify-between gap-2 mb-1.5">
                                <label className="text-[11px] text-slate-400">
                                  Протоколы ключей (выбрано:{" "}
                                  <b className="text-cyan-300">{selectedDeviceProtocols.length}</b>)
                                </label>
                                <div className="flex items-center gap-1.5 flex-wrap">
                                  {activeContainer && availableCount > 0 && (
                                    <button
                                      type="button"
                                      onClick={() => selectContainerAvailableProtocols()}
                                      className="px-2 py-0.5 rounded-md bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[10px] font-semibold text-cyan-300 cursor-pointer transition"
                                    >
                                      Доступные ({availableCount})
                                    </button>
                                  )}
                                  <button
                                    type="button"
                                    onClick={() =>
                                      setSelectedDeviceProtocols([...VPS_DEVICE_PROTOCOLS])
                                    }
                                    className="px-2 py-0.5 rounded-md bg-slate-800 hover:bg-slate-700 border text-[10px] font-semibold text-slate-300 cursor-pointer transition"
                                  >
                                    Все ({VPS_DEVICE_PROTOCOLS.length})
                                  </button>
                                  <button
                                    type="button"
                                    onClick={() => setSelectedDeviceProtocols([])}
                                    className="px-2 py-0.5 rounded-md bg-slate-800 hover:bg-slate-700 border text-[10px] font-semibold text-slate-400 cursor-pointer transition"
                                  >
                                    Сбросить
                                  </button>
                                </div>
                              </div>

                              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                {VPS_DEVICE_PROTOCOLS.map((protoKey) => {
                                  const meta = getProtocolMeta(protoKey);
                                  const isSelected = selectedDeviceProtocols.includes(protoKey);
                                  const isAvailable = isProtocolAvailableInContainer(
                                    protoKey,
                                    activeContainer
                                  );
                                  return (
                                    <button
                                      key={protoKey}
                                      type="button"
                                      onClick={() => toggleDeviceProtocol(protoKey)}
                                      style={{
                                        background: isSelected
                                          ? "var(--bg-card, #111827)"
                                          : "var(--bg-card-alt, #0b101d)",
                                        borderColor: isSelected
                                          ? isLightTheme
                                            ? LIGHT_PROTOCOL_COLORS[meta.id]?.border || meta.border
                                            : meta.border
                                          : "var(--border-card, rgba(51, 65, 85, 0.55))",
                                      }}
                                      className={`relative overflow-hidden p-2.5 pl-3.5 rounded-xl border text-left transition cursor-pointer flex flex-col gap-1 ${
                                        activeContainer && !isAvailable && !isSelected
                                          ? "opacity-65 hover:opacity-100"
                                          : ""
                                      }`}
                                    >
                                      <span
                                        style={{
                                          background: meta.isHybrid ? meta.gradientLine : meta.accent,
                                        }}
                                        className="absolute left-0 top-0 bottom-0 w-1"
                                      />
                                      <div className="flex items-center justify-between gap-1.5">
                                        <span className="text-xs font-bold text-white flex items-center gap-1.5 min-w-0">
                                          <span
                                            className={`w-3.5 h-3.5 rounded border flex items-center justify-center shrink-0 transition ${
                                              isSelected
                                                ? "bg-cyan-500 border-cyan-400 text-white keep-white"
                                                : "border-slate-600 bg-transparent"
                                            }`}
                                          >
                                            {isSelected && <Check className="w-2.5 h-2.5" />}
                                          </span>
                                          <span
                                            style={{
                                              background: meta.isHybrid
                                                ? meta.gradientLine
                                                : meta.accent,
                                            }}
                                            className="w-2 h-2 rounded-full shrink-0"
                                          />
                                          <span className="truncate">{meta.label}</span>
                                        </span>
                                        <span
                                          style={getProtocolBadgeStyle(meta, isLightTheme)}
                                          className="text-[9px] px-1.5 py-0.5 rounded border font-mono font-bold shrink-0"
                                        >
                                          {meta.shortBadge}
                                        </span>
                                      </div>
                                      <p className="protocol-card-desc text-[10.5px] leading-snug">
                                        {meta.description}
                                      </p>
                                      {activeContainer && (
                                        <div className="flex items-center justify-end">
                                          <span
                                            className={`text-[9px] font-mono px-1.5 py-0.2 rounded ${
                                              isAvailable
                                                ? "text-emerald-400"
                                                : "text-slate-500"
                                            }`}
                                          >
                                            {isAvailable
                                              ? "● активен в контейнере"
                                              : "○ создастся автоматически"}
                                          </span>
                                        </div>
                                      )}
                                    </button>
                                  );
                                })}
                              </div>
                            </div>
                          </div>

                          <button
                            type="button"
                            onClick={() => void handleCreateDeviceKey()}
                            disabled={
                              creatingDevice ||
                              vpsContainers.length === 0 ||
                              selectedDeviceProtocols.length === 0
                            }
                            className="w-full py-2.5 px-4 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 disabled:opacity-50 text-xs font-bold text-white shadow-lg shadow-cyan-500/20 flex items-center justify-center gap-2 transition cursor-pointer keep-white"
                          >
                            {creatingDevice ? (
                              <RefreshCw className="w-4 h-4 animate-spin" />
                            ) : (
                              <QrCode className="w-4 h-4" />
                            )}
                            <span>
                              {creatingDevice
                                ? `Генерация ключей (${selectedDeviceProtocols.length}) и перезапуск контейнера...`
                                : selectedDeviceProtocols.length > 1
                                ? `Создать мульти-ключи (${selectedDeviceProtocols.length})`
                                : "Создать ключ и показать QR"}
                            </span>
                          </button>
                        </div>

                        {lastCreatedKeys &&
                          (lastCreatedKeys.created.length > 0 ||
                            lastCreatedKeys.skipped.length > 0) && (
                            <div className="p-3.5 rounded-2xl bg-[#111827] border space-y-2.5">
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <span className="text-xs font-bold text-white flex items-center gap-1.5 flex-wrap">
                                  <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                                  <span>
                                    Созданные ключи ({lastCreatedKeys.created.length})
                                  </span>
                                  {lastCreatedKeys.created.length > 1 && (
                                    <span className="text-[10px] px-1.5 py-0.5 rounded-md bg-indigo-500/20 text-indigo-300 border border-indigo-500/40 font-bold inline-flex items-center gap-1">
                                      <Layers className="w-3 h-3" />
                                      <span>МУЛЬТИ ×{lastCreatedKeys.created.length}</span>
                                    </span>
                                  )}
                                </span>
                                {lastCreatedKeys.created.length > 1 && (() => {
                                  const multiName =
                                    lastCreatedKeys.created[0]?.multi_group_name ||
                                    "NeuroVeil_Multi";
                                  return (
                                    <div className="flex items-center gap-1.5 flex-wrap">
                                      <button
                                        type="button"
                                        onClick={() => void copyAllCreatedUris()}
                                        className="px-2.5 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 cursor-pointer transition"
                                      >
                                        <Copy className="w-3 h-3" />
                                        <span>Скопировать все ссылки</span>
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() =>
                                          void handleExportMultiJson(
                                            lastCreatedKeys.created,
                                            multiName
                                          )
                                        }
                                        className="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 border text-[11px] font-semibold text-slate-200 flex items-center gap-1 cursor-pointer transition"
                                        title="Скачать единый мульти-протокол (.json для sing-box / NeuroVeil)"
                                      >
                                        <FileCode className="w-3 h-3" />
                                        <span>.json (Мульти)</span>
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() =>
                                          void handleExportMultiZip(
                                            lastCreatedKeys.created,
                                            multiName
                                          )
                                        }
                                        className="px-2.5 py-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 cursor-pointer transition"
                                        title="Скачать пакет мульти-протокола (.zip: .conf + .json + URI)"
                                      >
                                        <Archive className="w-3 h-3" />
                                        <span>.zip (Пакет)</span>
                                      </button>
                                    </div>
                                  );
                                })()}
                              </div>

                              {lastCreatedKeys.created.length > 0 && (
                                <div className="space-y-2">
                                  {lastCreatedKeys.created.map((p) => {
                                    const isAwgConfSupported =
                                      p.protocol === "amneziawg" ||
                                      p.protocol === "awg2" ||
                                      p.protocol === "awg_quic" ||
                                      p.protocol === "awg_vless" ||
                                      p.protocol === "wireguard";
                                    return (
                                      <div
                                        key={p.id}
                                        className="p-2.5 rounded-xl bg-[#0b101d] border flex flex-col sm:flex-row sm:items-center justify-between gap-2"
                                      >
                                        <div className="min-w-0 flex items-center gap-2 flex-wrap">
                                          {renderProtocolBadge(p.protocol, p.awg_version)}
                                          <span className="text-xs font-bold text-white truncate">
                                            {p.name}
                                          </span>
                                          <span className="text-[10px] font-mono text-slate-400">
                                            {p.server}:{p.port}
                                          </span>
                                        </div>
                                        <div className="flex items-center gap-1.5 flex-wrap shrink-0">
                                          <button
                                            type="button"
                                            onClick={() => handleCopyUri(p)}
                                            className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-cyan-300 flex items-center gap-1 cursor-pointer transition"
                                          >
                                            {copiedId === p.id ? (
                                              <Check className="w-3 h-3 text-emerald-400" />
                                            ) : (
                                              <Copy className="w-3 h-3" />
                                            )}
                                            <span>URI</span>
                                          </button>
                                          <button
                                            type="button"
                                            onClick={() => void handleOpenQrModal(p)}
                                            className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-purple-300 flex items-center gap-1 cursor-pointer transition"
                                          >
                                            <QrCode className="w-3 h-3" />
                                            <span>QR</span>
                                          </button>
                                          {isAwgConfSupported && (
                                            <button
                                              type="button"
                                              onClick={() => void handleDownloadAwgConf(p)}
                                              className="px-2 py-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-[11px] font-semibold text-emerald-300 flex items-center gap-1 cursor-pointer transition"
                                            >
                                              <FileDown className="w-3 h-3" />
                                              <span>.conf</span>
                                            </button>
                                          )}
                                          <button
                                            type="button"
                                            onClick={() => void handleExportJson(p)}
                                            className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-slate-300 flex items-center gap-1 cursor-pointer transition"
                                          >
                                            <FileCode className="w-3 h-3" />
                                            <span>.json</span>
                                          </button>
                                        </div>
                                      </div>
                                    );
                                  })}
                                </div>
                              )}

                              {lastCreatedKeys.skipped.length > 0 && (
                                <div className="p-2.5 rounded-xl bg-amber-500/10 border border-amber-500/30 text-[11px] text-amber-300 space-y-1">
                                  <div className="font-bold">Пропущенные протоколы:</div>
                                  {lastCreatedKeys.skipped.map((reason, idx) => (
                                    <div key={idx}>• {reason}</div>
                                  ))}
                                </div>
                              )}
                            </div>
                          )}
                      </div>
                    );
                  })()}
                </div>
              </>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      data-theme={currentTheme.id}
      data-theme-mode={isLightTheme ? "light" : "dark"}
      style={{
        ...cssVarsStyle,
        opacity: windowOpacity / 100,
      }}
      className={`flex h-screen w-screen overflow-hidden ${
        isLightTheme ? "text-slate-900" : "text-slate-100"
      } select-none bg-[#090d16] transition-opacity duration-150 relative`}
    >
      {/* Compact Left Sidebar - AmneziaVPN v2.0 Style Vertical Icon Column */}
      <aside
        style={{ borderRightColor: "var(--color-accent-border)" }}
        className="w-[60px] px-1.5 bg-[#0d1322] border-r flex flex-col justify-between py-2.5 shrink-0 z-30 select-none"
      >
        <div className="flex flex-col items-center gap-2">
          {/* App Brand Icon (Draggable) */}
          <div
            onMouseDown={handleMouseDownDrag}
            data-tauri-drag-region
            onClick={() => {
              setActiveTab("dashboard");
            }}
            className="w-10 h-10 rounded-xl overflow-hidden bg-[#0c1220] border border-cyan-500/40 p-1 flex items-center justify-center shadow-lg shrink-0 cursor-pointer hover:border-cyan-400 transition"
            title="NeuroVeil VPN 2.0 — Главная"
          >
            <img
              src={appIconLogo}
              alt="NeuroVeil VPN"
              className="w-full h-full object-cover rounded-lg pointer-events-none"
            />
          </div>

          <div className="w-7 h-px bg-slate-800/90 my-0.5" />

          {/* Compact Vertical Icon Buttons Column (6 sections) */}
          <nav className="relative flex flex-col items-center gap-1.5 w-full">
            {/* Seamless Sliding Active Tab Connector (merges into Main Window) */}
            <div
              style={{
                transform: `translateY(${
                  Math.max(
                    0,
                    ["dashboard", "profiles", "routing", "vps", "settings", "about"].indexOf(
                      activeTab
                    )
                  ) * 46
                }px)`,
                transition: "transform 340ms cubic-bezier(0.22, 1, 0.36, 1)",
              }}
              className="pointer-events-none absolute top-0 left-0 -right-[7px] h-10 z-0"
            >
              {/* Single seamless SVG silhouette (no overlapping div/SVG borders, larger 18px rounding) */}
              <svg
                className="pointer-events-none absolute -top-[18px] left-0 w-full h-[76px] overflow-visible"
                viewBox="0 0 54 76"
                fill="none"
                shapeRendering="geometricPrecision"
              >
                <path
                  d="M54 0H53.5A18.5 18.5 0 0 1 35 18.5H18.5A18 18 0 0 0 0.5 36.5V39.5A18 18 0 0 0 18.5 57.5H35A18.5 18.5 0 0 1 53.5 76H54V0Z"
                  fill="var(--bg-app)"
                />
                <path
                  d="M53.5 0A18.5 18.5 0 0 1 35 18.5H18.5A18 18 0 0 0 0.5 36.5V39.5A18 18 0 0 0 18.5 57.5H35A18.5 18.5 0 0 1 53.5 76"
                  stroke="var(--color-accent-border)"
                  strokeWidth="1"
                  strokeLinecap="butt"
                />
              </svg>
            </div>

            {/* 1. Подключение (Главная страница) */}
            <button
              type="button"
              onClick={() => {
                setActiveTab("dashboard");
              }}
              title="Подключение (Главная страница)"
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 cursor-pointer ${
                activeTab === "dashboard"
                  ? "text-cyan-300"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50"
              }`}
            >
              <Power className="w-4 h-4 shrink-0" />
              <span
                className={`absolute top-1.5 right-1.5 w-2 h-2 rounded-full ${
                  status.connected
                    ? status.is_handshaking
                      ? "bg-amber-400 shadow-[0_0_6px_#fbbf24] animate-ping"
                      : "bg-emerald-400 shadow-[0_0_6px_#34d399]"
                    : "bg-slate-600"
                }`}
              />
            </button>

            {/* 2. Настройка конфигураций (Неактивна, если нет добавленных туннелей или VPS) */}
            <button
              type="button"
              disabled={!hasTunnelsOrVps}
              onClick={() => {
                if (!hasTunnelsOrVps) return;
                setActiveTab("profiles");
              }}
              title={
                hasTunnelsOrVps
                  ? `Настройка конфигураций (${profiles.length})`
                  : "Настройка конфигураций (Сначала добавьте туннель или VPS)"
              }
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 ${
                !hasTunnelsOrVps
                  ? "text-slate-600 opacity-35 cursor-not-allowed"
                  : activeTab === "profiles"
                  ? "text-cyan-300 cursor-pointer"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50 cursor-pointer"
              }`}
            >
              <Layers className="w-4 h-4 shrink-0" />
              {profiles.length > 0 && (
                <span className="absolute -top-1 -right-1 min-w-[16px] h-[16px] px-1 rounded-full bg-cyan-600 text-[9px] font-bold text-white flex items-center justify-center border border-[#0d1322]">
                  {profiles.length}
                </span>
              )}
            </button>

            {/* 3. Раздельное туннелирование (Неактивна, если нет добавленных туннелей или VPS) */}
            <button
              type="button"
              disabled={!hasTunnelsOrVps}
              onClick={() => {
                if (!hasTunnelsOrVps) return;
                if (selectedProfileId) switchRoutingProfile(selectedProfileId);
                setActiveTab("routing");
              }}
              title={
                hasTunnelsOrVps
                  ? "Раздельное туннелирование"
                  : "Раздельное туннелирование (Сначала добавьте туннель или VPS)"
              }
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 ${
                !hasTunnelsOrVps
                  ? "text-slate-600 opacity-35 cursor-not-allowed"
                  : activeTab === "routing"
                  ? "text-cyan-300 cursor-pointer"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50 cursor-pointer"
              }`}
            >
              <Globe className="w-4 h-4 shrink-0" />
              {hasTunnelsOrVps &&
                (selectedProfile?.routing?.split_tunneling ?? routing.split_tunneling) && (
                  <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-cyan-400 shadow-[0_0_6px_#22d3ee]" />
                )}
            </button>

            {/* 4. Добавление и Автодеплой (Добавление конфиг-файлов, ключей и Автодеплой VPS) */}
            <button
              type="button"
              onClick={() => {
                setActiveTab("vps");
              }}
              title="Добавление конфигураций и Автодеплой VPS"
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 cursor-pointer ${
                activeTab === "vps"
                  ? "text-cyan-300"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50"
              }`}
            >
              <Server className="w-4 h-4 shrink-0" />
            </button>

            {/* 5. Настройки */}
            <button
              type="button"
              onClick={() => {
                setActiveTab("settings");
              }}
              title={t.navSettings}
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 cursor-pointer ${
                activeTab === "settings"
                  ? "text-cyan-300"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50"
              }`}
            >
              <Settings className="w-4 h-4 shrink-0" />
            </button>

            {/* 6. О приложении */}
            <button
              type="button"
              onClick={() => {
                setActiveTab("about");
              }}
              title={t.navAbout}
              className={`relative z-10 w-10 h-10 rounded-xl flex items-center justify-center transition-all duration-150 cursor-pointer ${
                activeTab === "about"
                  ? "text-cyan-300"
                  : "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50"
              }`}
            >
              <Info className="w-4 h-4 shrink-0" />
              {updateInfo?.has_update && (
                <span className="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-amber-400 animate-ping" />
              )}
            </button>
          </nav>
        </div>

        {/* Bottom Core Status Indicator */}
        <div className="flex flex-col items-center gap-1.5">
          <button
            type="button"
            onClick={() => {
              if (!status.singbox_installed) {
                handleDownloadCore();
              } else {
                invoke("open_popup_window", { mode: "logs" });
              }
            }}
            className="w-10 h-10 rounded-xl bg-[#131c31]/80 border border-slate-800 hover:border-slate-700 flex flex-col items-center justify-center cursor-pointer transition"
            title={`Ядро sing-box: ${
              status.singbox_installed
                ? "Готово (" + (status.singbox_version || "") + ") — Нажмите для просмотра логов"
                : "Не скачано — Нажмите для загрузки"
            }`}
          >
            <span
              className={`w-2.5 h-2.5 rounded-full ${
                status.singbox_installed
                  ? "bg-emerald-400 shadow-[0_0_8px_#34d399]"
                  : "bg-amber-400 shadow-[0_0_8px_#fbbf24] animate-pulse"
              }`}
            />
            <span className="text-[8px] font-mono text-slate-400 mt-1">v2.0</span>
          </button>
        </div>
      </aside>

      {/* Main Content Area (Compact AmneziaVPN Window Layout) */}
      <main className="flex-1 flex flex-col h-full overflow-hidden relative bg-[#090d16]">
        {/* Seamless Transparent Drag Bar + Window Controls */}
        <header className="h-7 w-full flex items-center justify-end px-2.5 pt-1 shrink-0 select-none cursor-default z-30 bg-transparent">
          <div
            className="flex-1 h-full min-w-0 cursor-default"
            onMouseDown={handleMouseDownDrag}
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
              className="w-7 h-6 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition cursor-pointer"
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
                  await invoke("app_close");
                } catch (err) {
                  console.error("Close error:", err);
                }
              }}
              className="w-7 h-6 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-rose-600 transition cursor-pointer"
              title={t.close}
            >
              <X className="w-3.5 h-3.5 pointer-events-none" />
            </button>
          </div>
        </header>

        {/* Toast Banner - Compact & High Contrast */}
        {toast && (
          <div
            className={`toast-notice absolute top-11 left-2.5 right-2.5 z-[70] flex items-center justify-between gap-2 px-3 py-2.5 rounded-xl shadow-2xl border text-xs font-semibold transition-all backdrop-blur-md ${
              toast.type === "success"
                ? "bg-emerald-700/95 border-emerald-400 text-white shadow-emerald-500/30"
                : toast.type === "error"
                ? "bg-rose-700/95 border-rose-400 text-white shadow-rose-500/30"
                : "bg-slate-900/95 border-cyan-400 text-cyan-100 shadow-cyan-500/30"
            }`}
          >
            <div className="flex items-center gap-2 min-w-0">
              {toast.type === "success" && <CheckCircle2 className="w-4 h-4 text-emerald-200 shrink-0" />}
              {toast.type === "error" && <XCircle className="w-4 h-4 text-rose-200 shrink-0" />}
              {toast.type === "info" && <Sparkles className="w-4 h-4 text-cyan-300 shrink-0" />}
              <span className="text-white font-medium break-words">{toast.text}</span>
            </div>
            <button
              onClick={() => setToast(null)}
              className="text-white/70 hover:text-white transition p-0.5 shrink-0"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {/* ========================================================================= */}
        {/* MAIN PAGE (ПОДКЛЮЧЕНИЕ): EMPTY STATE (LOGO + TITLE) OR CONNECTION VIEW    */}
        {/* ========================================================================= */}
        {activeTab === "dashboard" && (
          !hasTunnelsOrVps ? (
          <div className="flex-1 flex flex-col items-center justify-center p-6 text-center select-none relative overflow-hidden">
            {/* Subtle background glow behind the logo */}
            <div className="w-56 h-56 rounded-full bg-cyan-500/10 blur-3xl absolute pointer-events-none" />

            <div className="relative z-10 flex flex-col items-center max-w-[300px]">
              <div className="w-28 h-28 rounded-3xl overflow-hidden bg-[#0d1424] border-2 border-cyan-500/40 p-2.5 shadow-2xl shadow-cyan-500/15 mb-5 flex items-center justify-center">
                <img
                  src={appIconLogo}
                  alt="NeuroVeil VPN"
                  className="w-full h-full object-contain rounded-2xl pointer-events-none"
                />
              </div>

              <h1 className="text-2xl font-extrabold tracking-tight text-white">
                NeuroVeil VPN
              </h1>
              <div className="mt-1.5 inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-cyan-500/15 border border-cyan-500/30 text-[11px] font-mono font-bold text-cyan-300">
                <span>Версия {APP_VERSION}</span>
              </div>

              <p className="text-xs text-slate-400 mt-4 leading-relaxed">
                Нет добавленных туннелей или VPS. Добавьте готовый файл конфигурации, ключ или разверните свой сервер в разделе <b className="text-slate-200">«Добавление и Автодеплой»</b>.
              </p>

              <button
                type="button"
                onClick={() => setActiveTab("vps")}
                className="mt-6 w-full py-2.5 px-4 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-xs font-bold text-white shadow-lg shadow-cyan-500/20 flex items-center justify-center gap-2 transition cursor-pointer keep-white"
              >
                <Plus className="w-4 h-4" />
                <span>Добавить туннель или VPS</span>
              </button>
            </div>
          </div>
        ) : (
        <div className="flex-1 flex flex-col p-3.5 gap-3 overflow-hidden h-full min-h-0">
          {/* Missing Core Warning Banner (if sing-box is not downloaded yet) */}
          {!status.singbox_installed && (
            <div className="p-2.5 rounded-xl bg-amber-500/15 border border-amber-500/40 flex items-center justify-between gap-2 shrink-0">
              <div className="flex items-center gap-2 min-w-0">
                <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
                <span className="text-[11px] font-semibold text-amber-200 truncate">
                  Требуется ядро sing-box.exe
                </span>
              </div>
              <button
                onClick={handleDownloadCore}
                disabled={downloadingCore}
                className="px-2.5 py-1 rounded-lg bg-amber-500 hover:bg-amber-400 disabled:opacity-50 text-slate-950 font-bold text-[11px] shrink-0 transition flex items-center gap-1 cursor-pointer"
              >
                <Download className="w-3 h-3" />
                {downloadingCore ? "..." : "Скачать"}
              </button>
            </div>
          )}

          {/* 1. POWER / CONNECT BUTTON CARD */}
          <div className="p-3 rounded-2xl bg-[#111827] border border-slate-800/90 soft-edge-block space-y-2 shrink-0 shadow-md">
            {(() => {
              const powerProfile =
                (status.connected && status.active_profile_id
                  ? profiles.find((p) => p.id === status.active_profile_id)
                  : null) || selectedProfile;
              const powerProtoMeta = powerProfile
                ? getProtocolMeta(powerProfile.protocol, powerProfile.awg_version)
                : null;
              const powerTheme = powerProtoMeta ? getProtocolPowerTheme(powerProtoMeta) : null;
              const isHandshaking = status.connected && status.is_handshaking;

              const buttonStyle: React.CSSProperties | undefined = isHandshaking
                ? undefined
                : powerTheme
                ? status.connected
                  ? {
                      background: powerTheme.connectedBg,
                      borderColor: powerTheme.connectedBorder,
                      boxShadow: powerTheme.connectedGlow,
                    }
                  : {
                      background: powerTheme.idleBg,
                      borderColor: powerTheme.idleBorder,
                      boxShadow: powerTheme.idleGlow,
                    }
                : undefined;

              const iconBoxStyle: React.CSSProperties | undefined = isHandshaking
                ? undefined
                : powerTheme
                ? status.connected
                  ? { background: powerTheme.connectedIconBg }
                  : {
                      background: powerTheme.idleIconBg,
                      border: `1px solid ${powerProtoMeta?.border || "rgba(255,255,255,0.2)"}`,
                    }
                : undefined;

              const dotStyle: React.CSSProperties | undefined = isHandshaking
                ? undefined
                : powerTheme
                ? status.connected
                  ? {
                      backgroundColor: powerTheme.dotColor,
                      boxShadow: `0 0 8px ${powerProtoMeta?.accent || powerTheme.dotColor}`,
                    }
                  : {
                      backgroundColor: powerProtoMeta?.accent || powerTheme.dotColor,
                      boxShadow: `0 0 6px ${powerProtoMeta?.accent || powerTheme.dotColor}`,
                    }
                : undefined;

              return (
                <button
                  onClick={handleToggleConnect}
                  disabled={(connecting && !autoPortHunting) || reconnectCooldown}
                  style={buttonStyle}
                  className={`w-full py-3.5 px-4 rounded-xl flex items-center justify-between gap-3 transition-all duration-300 cursor-pointer border relative overflow-hidden group shadow-md keep-white hover:brightness-110 ${
                    isHandshaking
                      ? "bg-gradient-to-r from-amber-600 via-orange-600 to-amber-700 text-white border-amber-400/60 shadow-amber-500/25"
                      : !powerTheme
                      ? status.connected
                        ? "bg-gradient-to-r from-emerald-600 via-teal-600 to-emerald-700 text-white border-emerald-400/60 shadow-emerald-500/25"
                        : "bg-gradient-to-r from-slate-800 via-[#161f33] to-slate-800 text-white border-slate-700"
                      : "text-white"
                  }`}
                >
                  {status.connected && (
                    <div
                      style={
                        !isHandshaking && powerTheme
                          ? { background: powerTheme.pulseOverlay }
                          : undefined
                      }
                      className={`absolute inset-0 ${
                        isHandshaking ? "bg-amber-400/15" : !powerTheme ? "bg-emerald-400/10" : ""
                      } animate-pulse pointer-events-none`}
                    />
                  )}

                  <div className="flex items-center gap-3 relative z-10 min-w-0">
                    <div
                      style={iconBoxStyle}
                      className={`w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 transition-transform group-hover:scale-105 shadow-inner text-white ${
                        isHandshaking
                          ? "bg-amber-500 shadow-amber-700/50"
                          : !powerTheme
                          ? status.connected
                            ? "bg-emerald-500 shadow-emerald-700/50"
                            : "bg-slate-700/80"
                          : ""
                      }`}
                    >
                      {connecting || autoPortHunting ? (
                        <RefreshCw className="w-6 h-6 animate-spin text-white" />
                      ) : status.connected ? (
                        isHandshaking ? (
                          <Activity className="w-6 h-6 animate-pulse text-amber-100" />
                        ) : (
                          <ShieldCheck className="w-6 h-6" />
                        )
                      ) : (
                        <Power className="w-6 h-6" />
                      )}
                    </div>

                    <div className="text-left min-w-0">
                      <div className="text-[11px] font-semibold uppercase tracking-wider text-white/80 group-hover:text-white truncate">
                        {autoPortHunting
                          ? `Автоподбор порта (#${autoPortAttempt})`
                          : connecting
                          ? "Подождите..."
                          : reconnectCooldown
                          ? "Защитная пауза WireGuard"
                          : status.connected
                          ? status.is_handshaking
                            ? "Рукопожатие туннеля..."
                            : status.active_profile_name || "Туннель активен"
                          : selectedProfile
                          ? selectedProfile.name
                          : "Туннель не выбран"}
                      </div>
                      <div className="text-lg font-extrabold tracking-wide text-white leading-tight truncate mt-0.5">
                        {connecting && !autoPortHunting
                          ? "ПОДКЛЮЧЕНИЕ..."
                          : reconnectCooldown
                          ? "ПАУЗА..."
                          : status.connected
                          ? "ОТКЛЮЧИТЬ"
                          : "ПОДКЛЮЧИТЬ"}
                      </div>
                    </div>
                  </div>

                  <div className="shrink-0 flex flex-col items-end gap-1 relative z-10">
                    <span
                      style={dotStyle}
                      className={`w-3 h-3 rounded-full ${
                        isHandshaking
                          ? "bg-amber-300 animate-ping"
                          : !powerTheme
                          ? status.connected
                            ? "bg-emerald-300 shadow-[0_0_8px_#34d399]"
                            : "bg-slate-500"
                          : ""
                      }`}
                    />
                    {powerProfile && (
                      <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-black/30 text-white/90 border border-white/15">
                        {routing.mode === "tun" ? "TUN" : "Proxy"}
                      </span>
                    )}
                  </div>
                </button>
              );
            })()}

            {/* Quick Reconnect & Auto Port Hunt bar when connected */}
            {(status.connected || autoPortHunting) && (
              <div className="grid grid-cols-2 gap-1.5">
                <button
                  type="button"
                  onClick={handleReconnect}
                  disabled={connecting || reconnectCooldown || autoPortHunting}
                  className="w-full py-1.5 px-2.5 rounded-xl bg-[#0b101d] hover:bg-slate-800/90 border border-cyan-500/30 hover:border-cyan-400/60 text-[11px] font-semibold text-cyan-300 flex items-center justify-between gap-1 transition cursor-pointer disabled:opacity-50 min-w-0"
                  title="Переподключить туннель и сменить локальный порт прослушивания (ListenPort)"
                >
                  <span className="flex items-center gap-1.5 min-w-0">
                    <RotateCcw className={`w-3 h-3 shrink-0 ${connecting && !autoPortHunting ? "animate-spin" : ""}`} />
                    <span className="truncate">Переподкл.</span>
                  </span>
                  {status.active_listen_port && (
                    <span className="text-[10px] font-mono px-1 py-0.2 rounded bg-cyan-500/15 text-cyan-200 border border-cyan-500/30 shrink-0">
                      :{status.active_listen_port}
                    </span>
                  )}
                </button>

                <button
                  type="button"
                  onClick={handleAutoPortHunt}
                  disabled={reconnectCooldown && !autoPortHunting}
                  className={`w-full py-1.5 px-2.5 rounded-xl border text-[11px] font-semibold flex items-center justify-between gap-1 transition cursor-pointer disabled:opacity-50 min-w-0 ${
                    autoPortHunting
                      ? "bg-amber-500/20 hover:bg-rose-500/25 border-amber-400/60 text-amber-200 animate-pulse"
                      : "bg-[#0b101d] hover:bg-slate-800/90 border-emerald-500/35 hover:border-emerald-400/60 text-emerald-300"
                  }`}
                  title={
                    autoPortHunting
                      ? "Остановить автоподбор портов"
                      : "Автоматически перебирать порты (ListenPort) до установления соединения"
                  }
                >
                  <span className="flex items-center gap-1.5 min-w-0">
                    <RefreshCw className={`w-3 h-3 shrink-0 ${autoPortHunting ? "animate-spin text-amber-300" : "text-emerald-400"}`} />
                    <span className="truncate">
                      {autoPortHunting ? `Стоп (#${autoPortAttempt})` : "Автоподбор"}
                    </span>
                  </span>
                  {autoPortHunting && status.active_listen_port && (
                    <span className="text-[10px] font-mono px-1 py-0.2 rounded bg-amber-500/20 text-amber-200 border border-amber-500/40 shrink-0">
                      :{status.active_listen_port}
                    </span>
                  )}
                </button>
              </div>
            )}

            {/* Integrated Metrics Row (Download, Upload, Session/Profile Traffic) */}
            <div className="grid grid-cols-3 gap-1.5 pt-2 border-t border-slate-800/70">
              <div className="px-2 py-1.5 rounded-xl bg-[#0b101d] border border-slate-800/70 flex items-center gap-2 min-w-0">
                <div className="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center shrink-0">
                  <Download className="w-3 h-3 text-cyan-400" />
                </div>
                <div className="min-w-0">
                  <div className="text-[9px] text-slate-400 uppercase tracking-wider leading-none">Входящий</div>
                  <div className="text-[11px] font-bold text-white font-mono truncate mt-0.5">
                    {formatBytes(status.down_speed)}/с
                  </div>
                </div>
              </div>

              <div className="px-2 py-1.5 rounded-xl bg-[#0b101d] border border-slate-800/70 flex items-center gap-2 min-w-0">
                <div className="w-6 h-6 rounded-lg bg-indigo-500/10 border border-indigo-500/25 flex items-center justify-center shrink-0">
                  <Upload className="w-3 h-3 text-indigo-400" />
                </div>
                <div className="min-w-0">
                  <div className="text-[9px] text-slate-400 uppercase tracking-wider leading-none">Исходящий</div>
                  <div className="text-[11px] font-bold text-white font-mono truncate mt-0.5">
                    {formatBytes(status.up_speed)}/с
                  </div>
                </div>
              </div>

              <div
                onClick={() => invoke("open_popup_window", { mode: "logs" })}
                className="px-2 py-1.5 rounded-xl bg-[#0b101d] border border-slate-800/70 hover:border-cyan-500/35 flex items-center gap-2 min-w-0 cursor-pointer transition"
                title="Всего передано трафика по выбранному туннелю (нажмите для открытия журнала ядра)"
              >
                <div className="w-6 h-6 rounded-lg bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center shrink-0">
                  <Activity className="w-3 h-3 text-emerald-400" />
                </div>
                <div className="min-w-0">
                  <div className="text-[9px] text-slate-400 uppercase tracking-wider leading-none">Трафик</div>
                  <div className="text-[11px] font-bold text-emerald-300 font-mono truncate mt-0.5">
                    {formatBytes((selectedProfile?.total_down_bytes || 0) + (selectedProfile?.total_up_bytes || 0))}
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* 3. SPLIT TUNNELING BUTTON & QUICK PRESET BAR (Кнопка раздельного туннелирования) */}
          <div className="p-2.5 rounded-2xl bg-[#111827] border border-slate-800/90 soft-edge-block space-y-2 shrink-0">
            <div className="flex items-center justify-between gap-2">
              <button
                type="button"
                onClick={() => {
                  if (selectedProfileId) switchRoutingProfile(selectedProfileId);
                  setActiveTab("routing");
                }}
                className="split-tunnel-btn flex-1 flex items-center justify-between gap-2.5 px-3 py-2 rounded-xl transition cursor-pointer min-w-0 group"
              >
                <div className="flex items-center gap-2.5 min-w-0">
                  <div className="split-tunnel-btn-icon p-1.5 rounded-lg shrink-0">
                    <Globe className="w-4 h-4" />
                  </div>
                  <div className="text-left min-w-0">
                    <div className="text-xs font-bold text-white flex items-center gap-1.5">
                      <span className="truncate">Раздельное туннелирование</span>
                      {status.connected && (
                        <span
                          className={`text-[9px] px-1.5 py-0.2 rounded-full border font-semibold shrink-0 ${
                            isLightTheme
                              ? "bg-emerald-600/15 text-emerald-700 border-emerald-600/35"
                              : "bg-emerald-500/20 text-emerald-300 border-emerald-500/30"
                          }`}
                        >
                          ⚡ На лету
                        </span>
                      )}
                    </div>
                    <div className="split-tunnel-subtitle text-[10px] text-slate-400 truncate">
                      {(selectedProfile?.routing?.split_tunneling ?? routing.split_tunneling)
                        ? `Активно: ${
                            routingPresets.find((p) => p.id === activePresetId)?.name || "Выборочный обход"
                          }`
                        : "Весь трафик ПК идет через VPN"}
                    </div>
                  </div>
                </div>
                <span className="split-tunnel-btn-cta text-[11px] font-semibold group-hover:translate-x-0.5 transition shrink-0">
                  Настроить →
                </span>
              </button>
            </div>

            {/* Horizontal scrollable quick routing preset chips */}
            <div className="flex items-center gap-1.5 overflow-x-auto pb-0.5 custom-scrollbar">
              {routingPresets.map((pr) => {
                const isActive = activePresetId === pr.id;
                return (
                  <button
                    key={pr.id}
                    type="button"
                    onClick={() => handleSelectPreset(pr)}
                    className={`px-2.5 py-1 rounded-lg border text-[11px] font-medium transition cursor-pointer shrink-0 flex items-center gap-1.5 ${
                      isActive
                        ? "bg-cyan-500/20 border-cyan-400 text-white shadow-sm"
                        : "bg-[#0b101d] border-slate-800 text-slate-300 hover:text-white hover:border-slate-700"
                    }`}
                  >
                    <span
                      style={
                        isActive
                          ? {
                              backgroundColor: "var(--color-accent)",
                              boxShadow: "0 0 6px var(--color-accent-glow)",
                            }
                          : undefined
                      }
                      className={`w-1.5 h-1.5 rounded-full ${
                        isActive ? "" : isLightTheme ? "bg-slate-400" : "bg-slate-600"
                      }`}
                    />
                    <span>{pr.name}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* 4. ADDED TUNNELS LIST (Добавленные туннели) */}
          <div className="flex-1 flex flex-col p-3 rounded-2xl bg-[#111827] border border-slate-800/90 soft-edge-block min-h-0 overflow-hidden">
            <div className="flex items-center justify-between gap-2 pb-2 mb-2 border-b border-slate-800/80 shrink-0">
              <span className="text-xs font-bold uppercase tracking-wider text-slate-300 flex items-center gap-1.5">
                <Layers className="w-3.5 h-3.5 text-cyan-400" />
                Добавленные туннели ({effectiveDashboardProfiles.length})
              </span>
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => setActiveTab("vps")}
                  className="px-2 py-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-[11px] font-semibold text-cyan-300 transition cursor-pointer flex items-center gap-1"
                  title="Добавить конфиг, ссылку или развернуть VPS"
                >
                  <Plus className="w-3 h-3 text-cyan-400" />
                  <span>Добавить / VPS</span>
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab("profiles")}
                  className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-semibold text-slate-200 transition cursor-pointer flex items-center gap-1"
                  title="Настройка конфигураций и устройств"
                >
                  <Sliders className="w-3 h-3 text-cyan-400" />
                  <span>Настроить</span>
                </button>
              </div>
            </div>

            {!selectedProfile ? (
              <div className="flex-1 flex flex-col items-center justify-center p-4 rounded-xl bg-[#0b101d] border border-dashed border-slate-700 text-center space-y-2.5">
                <p className="text-xs text-slate-400">
                  Нет добавленных туннелей на этом ПК. Импортируйте файл/ключ или разверните свой VPS.
                </p>
                <button
                  onClick={() => setActiveTab("vps")}
                  className="px-3 py-1.5 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-xs font-semibold text-white transition cursor-pointer"
                >
                  Добавление и Автодеплой
                </button>
              </div>
            ) : (
              <div className="flex-1 overflow-y-auto space-y-1.5 pr-1 custom-scrollbar min-h-0">
                {effectiveDashboardProfiles.map((p) => {
                  const isCurrentSelected = p.id === (status.active_profile_id || selectedProfileId);
                  const isLiveConnected = status.connected && status.active_profile_id === p.id;
                  const shownListenPort =
                    isLiveConnected && status.active_listen_port
                      ? status.active_listen_port
                      : p.listen_port;

                  return (
                    <div
                      key={p.id}
                      onClick={() => handleQuickSwitchProfile(p.id)}
                      className={`p-2 rounded-xl border cursor-pointer transition flex items-center justify-between gap-2 group ${
                        isCurrentSelected
                          ? "bg-cyan-500/10 border-cyan-500/60 shadow-sm ring-1 ring-cyan-500/30"
                          : "bg-[#0b101d] border-slate-800/80 hover:border-slate-700 hover:bg-slate-800/40"
                      }`}
                    >
                      <div className="flex items-center gap-2 min-w-0 flex-1">
                        {renderProtocolBadge(p.protocol, p.awg_version)}

                        <div className="min-w-0 flex-1">
                          <div className="text-xs font-bold text-white flex items-center gap-1.5 truncate">
                            <span className="truncate">{p.name}</span>
                            {isLiveConnected && (
                              <span
                                className={`text-[9px] px-1.5 py-0.2 rounded-full border animate-pulse font-mono font-medium shrink-0 ${
                                  isLightTheme
                                    ? "bg-emerald-600/15 text-emerald-700 border-emerald-600/35"
                                    : "bg-emerald-500/20 text-emerald-300 border-emerald-500/40"
                                }`}
                              >
                                ON
                              </span>
                            )}
                          </div>
                          <div className="text-[10px] text-slate-400 font-mono truncate flex items-center gap-1.5">
                            <span className="truncate">
                              {p.server}:{status.active_port && isLiveConnected ? status.active_port : p.port}
                            </span>
                            {(p.protocol === "amneziawg" ||
                              p.protocol === "awg_quic" ||
                              p.protocol === "awg2" ||
                              p.protocol === "wireguard") &&
                              shownListenPort && (
                                <span
                                  className={`text-[9px] px-1 py-0.2 rounded border font-mono shrink-0 ${
                                    isLightTheme
                                      ? "bg-emerald-600/10 text-emerald-700 border-emerald-600/35"
                                      : "bg-emerald-500/10 text-emerald-300 border-emerald-500/30"
                                  }`}
                                  title="Локальный порт прослушивания (ListenPort)"
                                >
                                  :{shownListenPort}
                                </span>
                              )}
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        {p.ping_ms != null ? (
                          <span
                            onClick={(e) => {
                              e.stopPropagation();
                              handlePingProfile(p.id);
                            }}
                            className={`text-[10px] font-mono font-semibold hover:underline ${
                              isLightTheme ? "text-emerald-700" : "text-emerald-400"
                            }`}
                            title="Нажмите для повторного пинга"
                          >
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
                            className="tunnel-ping-btn text-[9px] px-1.5 py-0.5 rounded transition cursor-pointer"
                          >
                            {pingingId === p.id ? "..." : "Пинг"}
                          </button>
                        )}

                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleOpenEditModal(p);
                          }}
                          className="p-1 rounded-lg text-slate-400 hover:text-cyan-300 hover:bg-slate-800 transition"
                          title="Настроить параметры туннеля"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                        </button>

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
          )
        )}


        {/* TAB 4: ДОБАВЛЕНИЕ КОНФИГУРАЦИЙ И АВТОДЕПЛОЙ VPS */}
        {activeTab === "vps" && (
          <div className="flex-1 overflow-y-auto p-4 flex flex-col justify-between h-full min-h-0 space-y-3 animate-tab-enter">
            <div className="space-y-3">
              {/* Section Header */}
              <div>
                <h2 className="text-base font-bold text-white flex items-center gap-2">
                  <Server className="w-5 h-5 text-cyan-400" />
                  Добавление конфигураций и Автодеплой
                </h2>
                <p className="text-xs text-slate-400 mt-1 leading-relaxed">
                  Выберите способ добавления туннеля, развертывания или управления сервером.
                </p>
              </div>

              {/* 4 Main Action Buttons */}
              <div className="space-y-2.5 pt-0.5">
                {/* 1. Добавить из файла -> Цветной (Sky) */}
                <button
                  type="button"
                  onClick={handlePickConfigFile}
                  className="w-full p-3.5 rounded-2xl bg-[#111827] hover:bg-sky-950/30 border border-sky-500/40 hover:border-sky-400/75 soft-edge-block transition-all duration-150 flex items-center justify-between gap-3 text-left group cursor-pointer shadow-lg shadow-sky-500/10"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-10 h-10 rounded-xl bg-sky-500/20 border border-sky-400/50 flex items-center justify-center text-sky-400 group-hover:scale-105 transition shrink-0">
                      <FolderOpen className="w-5 h-5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-bold text-white group-hover:text-sky-300 transition">
                        Добавить из файла
                      </div>
                      <div className="text-[11px] text-slate-400 mt-0.5 leading-snug">
                        Выбрать файл конфигурации (.conf, .json, .txt) в Проводнике Windows
                      </div>
                    </div>
                  </div>
                  <div
                    className={`px-2.5 py-1 rounded-lg border text-[11px] font-bold shrink-0 ${
                      isLightTheme
                        ? "bg-sky-600/15 border-sky-600/40 text-sky-700"
                        : "bg-sky-500/20 border-sky-400/40 text-sky-300"
                    }`}
                  >
                    Выбрать
                  </div>
                </button>

                {/* 2. Добавить VPS -> Цветной (Emerald) */}
                <button
                  type="button"
                  onClick={() => invoke("open_popup_window", { mode: "vps" })}
                  className="w-full p-3.5 rounded-2xl bg-[#111827] hover:bg-emerald-950/30 border border-emerald-500/40 hover:border-emerald-400/75 soft-edge-block transition-all duration-150 flex items-center justify-between gap-3 text-left group cursor-pointer shadow-lg shadow-emerald-500/10"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-10 h-10 rounded-xl bg-emerald-500/20 border border-emerald-400/50 flex items-center justify-center text-emerald-400 group-hover:scale-105 transition shrink-0">
                      <Server className="w-5 h-5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-bold text-white group-hover:text-emerald-300 transition">
                        Добавить VPS
                      </div>
                      <div className="text-[11px] text-slate-400 mt-0.5 leading-snug">
                        Автодеплой нового контейнера на Linux VPS по SSH (без перезаписи старых)
                      </div>
                    </div>
                  </div>
                  <div
                    className={`px-2.5 py-1 rounded-lg border text-[11px] font-bold shrink-0 ${
                      isLightTheme
                        ? "bg-emerald-600/15 border-emerald-600/40 text-emerald-700"
                        : "bg-emerald-500/20 border-emerald-400/40 text-emerald-300"
                    }`}
                  >
                    Открыть
                  </div>
                </button>

                {/* 3. Вставить ключ -> Цветной (Violet) */}
                <button
                  type="button"
                  onClick={() => invoke("open_popup_window", { mode: "key" })}
                  className="w-full p-3.5 rounded-2xl bg-[#111827] hover:bg-violet-950/30 border border-violet-500/40 hover:border-violet-400/75 soft-edge-block transition-all duration-150 flex items-center justify-between gap-3 text-left group cursor-pointer shadow-lg shadow-violet-500/10"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-10 h-10 rounded-xl bg-violet-500/20 border border-violet-400/50 flex items-center justify-center text-violet-400 group-hover:scale-105 transition shrink-0">
                      <Key className="w-5 h-5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-bold text-white group-hover:text-violet-300 transition">
                        Вставить ключ
                      </div>
                      <div className="text-[11px] text-slate-400 mt-0.5 leading-snug">
                        Вставить ключ-ссылку (vless://, awg://, hy2://, ss://, tuic://) или текст конфига
                      </div>
                    </div>
                  </div>
                  <div
                    className={`px-2.5 py-1 rounded-lg border text-[11px] font-bold shrink-0 ${
                      isLightTheme
                        ? "bg-violet-600/15 border-violet-600/40 text-violet-700"
                        : "bg-violet-500/20 border-violet-400/40 text-violet-300"
                    }`}
                  >
                    Открыть
                  </div>
                </button>

                {/* 4. Управление VPS -> Цветной */}
                <button
                  type="button"
                  onClick={() => invoke("open_popup_window", { mode: "vps_admin" })}
                  className="w-full p-3.5 rounded-2xl bg-[#111827] hover:bg-amber-950/30 border border-amber-500/40 hover:border-amber-400/75 soft-edge-block transition-all duration-150 flex items-center justify-between gap-3 text-left group cursor-pointer shadow-lg shadow-amber-500/10"
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-10 h-10 rounded-xl bg-amber-500/20 border border-amber-400/50 flex items-center justify-center text-amber-400 group-hover:scale-105 transition shrink-0">
                      <Sliders className="w-5 h-5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-sm font-bold text-white group-hover:text-amber-300 transition">
                        Управление VPS
                      </div>
                      <div className="text-[11px] text-slate-400 mt-0.5 leading-snug">
                        Вход по SSH: управление контейнерами, конфигами и выпуск новых ключей
                      </div>
                    </div>
                  </div>
                  <div
                    className={`px-2.5 py-1 rounded-lg border text-[11px] font-bold shrink-0 ${
                      isLightTheme
                        ? "bg-amber-600/15 border-amber-600/40 text-amber-700"
                        : "bg-amber-500/20 border-amber-400/40 text-amber-300"
                    }`}
                  >
                    Открыть
                  </div>
                </button>
              </div>
            </div>

            {/* Footer status indicator inside section */}
            <div className="p-3 rounded-2xl bg-[#111827] border border-slate-800/90 soft-edge-block flex items-center justify-between gap-2">
              <div className="flex items-center gap-2 min-w-0">
                <Layers className="w-4 h-4 text-cyan-400 shrink-0" />
                <span className="text-xs text-slate-300 truncate">
                  Добавлено туннелей: <b className="text-white">{profiles.length}</b>
                </span>
              </div>
              {profiles.length > 0 && (
                <button
                  type="button"
                  onClick={() => setActiveTab("dashboard")}
                  className="px-3 py-1 rounded-lg bg-cyan-600/20 hover:bg-cyan-600/30 border border-cyan-500/40 text-[11px] font-semibold text-cyan-300 transition cursor-pointer shrink-0"
                >
                  Перейти к подключению
                </button>
              )}
            </div>
          </div>
        )}

        {/* TAB 2: НАСТРОЙКА КОНФИГУРАЦИЙ */}
        {activeTab === "profiles" && (
          <div className="flex-1 overflow-y-auto p-3.5 space-y-3.5 animate-tab-enter">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-sm font-bold text-white flex items-center gap-2">
                  <Sliders className="w-4 h-4 text-cyan-400" />
                  Настройка конфигураций
                </h2>
                <p className="text-[10px] text-slate-400 mt-0.5">
                  Управление установленными туннелями, редактирование параметров и экспорт (.conf / QR).
                </p>
              </div>
            </div>

            {/* ========================================================================= */}
            {/* ТУННЕЛИ, УСТАНОВЛЕННЫЕ В ПРИЛОЖЕНИИ (ЭТОТ ПК)                             */}
            {/* ========================================================================= */}
            <div className="p-3.5 rounded-2xl bg-[#0e1526] border border-slate-800/90 soft-edge-block space-y-3 shadow-sm">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2.5 min-w-0">
                  <div className="w-8 h-8 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-sm shrink-0">
                    <Monitor className="w-4 h-4" />
                  </div>
                  <div className="min-w-0">
                    <h3 className="text-xs font-bold text-white flex items-center gap-2">
                      <span>Установленные туннели</span>
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold font-mono">
                        {effectiveClientProfiles.length}
                      </span>
                    </h3>
                    <p className="text-[10px] text-slate-400 truncate">
                      Конфигурации для защиты и маршрутизации трафика на этом ПК
                    </p>
                  </div>
                </div>
              </div>

            {false && showNewProfileForm && (
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
                        const proto = e.target.value as "vless" | "hysteria2" | "tuic" | "shadowsocks" | "amneziawg" | "awg_quic" | "awg_vless" | "awg2" | "ssh" | "shadowtls" | "vless_ws" | "webrtc";
                        const isAwg31 = proto === "amneziawg" || proto === "awg_quic" || proto === "awg_vless";
                        const isAwg20 = proto === "awg2";
                        const existingAwg = profiles.find((p) => p.protocol === "amneziawg");
                        const existingVless = profiles.find((p) => p.protocol === "vless" && p.reality_public_key);
                        setNewProfile({
                          ...newProfile,
                          protocol: proto,
                          server: newProfile.server || existingAwg?.server || existingVless?.server || "",
                          port: proto === "ssh" ? 22 : proto === "shadowsocks" ? 8388 : proto === "vless" ? 443 : proto === "tuic" ? 9443 : proto === "vless_ws" ? 8080 : proto === "webrtc" ? 2053 : proto === "shadowtls" ? 8443 : isAwg31 ? (existingAwg?.port || getRandomInt(21000, 55000)) : isAwg20 ? getRandomInt(21000, 55000) : 8443,
                          vless_port: proto === "awg_vless" ? (newProfile.vless_port || existingVless?.port || 443) : newProfile.vless_port,
                          uuid: proto === "awg_vless" ? (newProfile.uuid || existingVless?.uuid || existingVless?.password || "") : newProfile.uuid,
                          reality_public_key: proto === "awg_vless" ? (newProfile.reality_public_key || existingVless?.reality_public_key || "") : newProfile.reality_public_key,
                          reality_short_id: proto === "awg_vless" ? (newProfile.reality_short_id || existingVless?.reality_short_id || "") : newProfile.reality_short_id,
                          sni: proto === "ssh" ? (newProfile.sni || "root") : proto === "shadowtls" ? (newProfile.sni || "gateway.icloud.com") : proto === "webrtc" ? (newProfile.sni || "zoom.us") : proto === "awg_vless" ? (existingVless?.sni || newProfile.sni || "www.microsoft.com") : newProfile.sni,
                          shadowtls_sni: proto === "shadowtls" ? (newProfile.shadowtls_sni || "gateway.icloud.com") : newProfile.shadowtls_sni,
                          ws_path: proto === "vless_ws" ? (newProfile.ws_path || "/neuroveil-ws") : proto === "webrtc" ? (newProfile.ws_path || "neuroveil-webrtc") : newProfile.ws_path,
                          tls_fragment: proto === "ssh" ? false : true,
                          password: isAwg31 ? (newProfile.password || existingAwg?.awg_private_key || existingAwg?.password || "") : newProfile.password,
                          awg_private_key: isAwg31 ? (newProfile.awg_private_key || existingAwg?.awg_private_key || existingAwg?.password || "") : newProfile.awg_private_key,
                          awg_public_key: isAwg31 ? (newProfile.awg_public_key || existingAwg?.awg_public_key || "") : newProfile.awg_public_key,
                          awg_preshared_key: isAwg31 ? (newProfile.awg_preshared_key || existingAwg?.awg_preshared_key || "") : newProfile.awg_preshared_key,
                          awg_header_protection_key: isAwg31 ? (newProfile.awg_header_protection_key || existingAwg?.awg_header_protection_key || "") : newProfile.awg_header_protection_key,
                          awg_h1: isAwg31 ? (newProfile.awg_h1 || existingAwg?.awg_h1 || String(getRandomInt(100000000, 499999999))) : newProfile.awg_h1,
                          awg_h2: isAwg31 ? (newProfile.awg_h2 || existingAwg?.awg_h2 || String(getRandomInt(500000000, 999999999))) : newProfile.awg_h2,
                          awg_h3: isAwg31 ? (newProfile.awg_h3 || existingAwg?.awg_h3 || String(getRandomInt(1000000000, 1499999999))) : newProfile.awg_h3,
                          awg_h4: isAwg31 ? (newProfile.awg_h4 || existingAwg?.awg_h4 || String(getRandomInt(1500000000, 2140000000))) : newProfile.awg_h4,
                          awg_version: isAwg20 ? "2.0" : isAwg31 ? "3.1" : newProfile.awg_version,
                          awg_local_address: isAwg20 ? (newProfile.awg_local_address || "10.8.1.2/24") : isAwg31 ? (newProfile.awg_local_address || existingAwg?.awg_local_address || "10.8.0.2/32") : newProfile.awg_local_address,
                          awg_hop_ports: isAwg31 ? (newProfile.awg_hop_ports || existingAwg?.awg_hop_ports || "20000:65000") : isAwg20 ? undefined : newProfile.awg_hop_ports,
                          awg_port_probing: isAwg31 ? (newProfile.awg_port_probing ?? true) : isAwg20 ? false : newProfile.awg_port_probing,
                        });
                      }}
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    >
                      <option value="ssh">SSH Tunnel (порт 22 • Direct TCP/IP)</option>
                      <option value="vless">VLESS Reality (порт 443 + TLS Fragment)</option>
                      <option value="shadowtls">ShadowTLS v3 (L7 TLS 1.3 верификация)</option>
                      <option value="vless_ws">VLESS + WebSocket (Cloudflare CDN Shield)</option>
                      <option value="webrtc">WebRTC Steganography (VLESS over gRPC Media)</option>
                      <option value="amneziawg">Amnezia 3.1 (Header Protection + 32-bit H1-H4)</option>
                      <option value="awg_quic">Спецпротокол: AWG + QUIC Pre-Warm (Прогрев STUN / QUIC)</option>
                      <option value="awg_vless">Спецпротокол: Гибрид AWG + VLESS Reality (Капсула TCP 443)</option>
                      <option value="awg2">AmneziaWG 2.0 (AWG 2.0 • Чистый конфиг)</option>
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
                    <label className="block text-xs text-slate-400 mb-1">Порт (сервера)</label>
                    <input
                      type="number"
                      value={newProfile.port}
                      onChange={(e) =>
                        setNewProfile({
                          ...newProfile,
                          port: Number(e.target.value) || (newProfile.protocol === "ssh" ? 22 : newProfile.protocol === "shadowsocks" ? 8388 : (newProfile.protocol === "awg2" || newProfile.protocol === "amneziawg" || newProfile.protocol === "awg_quic" || newProfile.protocol === "awg_vless") ? getRandomInt(21000, 55000) : 8443),
                        })
                      }
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1">Порт прослушивания (Listen Port)</label>
                    <input
                      type="number"
                      value={newProfile.listen_port || ""}
                      placeholder="Авто / случайный"
                      onChange={(e) =>
                        setNewProfile({
                          ...newProfile,
                          listen_port: e.target.value ? Number(e.target.value) || null : null,
                        })
                      }
                      className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white font-mono"
                    />
                  </div>
                </div>

                {(newProfile.protocol === "amneziawg" || newProfile.protocol === "awg_quic" || newProfile.protocol === "awg_vless") ? (
                  <div className="space-y-3 pt-1">
                    {newProfile.protocol === "awg_quic" && (
                      <div className="p-3 rounded-xl bg-sky-950/30 border border-sky-500/35 text-xs text-sky-200 flex items-start gap-2">
                        <span className="text-base">⚡</span>
                        <div>
                          <span className="font-bold">Спецпротокол AWG + QUIC Pre-Warm:</span> за 15 мс до запуска рукопожатия с того же <code>ListenPort</code> отправляет пакеты RFC 9000 QUIC v1 Initial (1200B) и WebRTC STUN Binding Request. Сетевые маршрутизаторы (NAT/conntrack) инициализируют UDP-поток как стандартную QUIC-сессию перед рукопожатием AmneziaWG.
                        </div>
                      </div>
                    )}
                    {newProfile.protocol === "awg_vless" && (
                      <div className="p-3.5 rounded-xl bg-fuchsia-950/25 border border-fuchsia-500/35 space-y-3">
                        <div className="text-xs text-fuchsia-200 flex items-start gap-2">
                          <span className="text-base">🛡️</span>
                          <div>
                            <span className="font-bold">Спецпротокол Гибрид AWG + VLESS Reality:</span> рукопожатие и пакеты AmneziaWG упаковываются в TLS 1.3 поток VLESS Reality (TCP порт 443, <code>detour</code>). Позволяет установить соединение даже при 100% блокировке UDP у провайдера.
                          </div>
                        </div>
                        <div className="grid grid-cols-5 gap-2.5 text-xs">
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">Порт Reality (TCP)</label>
                            <input
                              type="number"
                              value={newProfile.vless_port ?? 443}
                              onChange={(e) => setNewProfile({ ...newProfile, vless_port: Number(e.target.value) || 443 })}
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-white font-mono"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">UUID VLESS</label>
                            <input
                              type="text"
                              placeholder="Авто из VLESS профиля"
                              value={newProfile.uuid || ""}
                              onChange={(e) => setNewProfile({ ...newProfile, uuid: e.target.value })}
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-white font-mono"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">Reality Public Key</label>
                            <input
                              type="text"
                              placeholder="PublicKey Reality"
                              value={newProfile.reality_public_key || ""}
                              onChange={(e) => setNewProfile({ ...newProfile, reality_public_key: e.target.value })}
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-white font-mono"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">Short ID (sid)</label>
                            <input
                              type="text"
                              placeholder="0123456789abcdef"
                              value={newProfile.reality_short_id || ""}
                              onChange={(e) => setNewProfile({ ...newProfile, reality_short_id: e.target.value })}
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-white font-mono"
                            />
                          </div>
                          <div>
                            <label className="block text-[11px] text-slate-400 mb-1">SNI Reality</label>
                            <input
                              type="text"
                              placeholder="www.microsoft.com"
                              value={newProfile.sni || "www.microsoft.com"}
                              onChange={(e) => setNewProfile({ ...newProfile, sni: e.target.value })}
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#0b101d] border border-slate-700 text-white font-mono"
                            />
                          </div>
                        </div>
                      </div>
                    )}
                    <div className="flex items-center justify-between p-3 rounded-xl bg-emerald-950/25 border border-emerald-500/30">
                      <div>
                        <div className="text-xs font-bold text-emerald-300">
                          Параметры протокола Amnezia 3.1
                        </div>
                        <div className="text-[11px] text-slate-400">
                          Защита заголовков HeaderProtectionKey, 32-битные H1-H4, сдвиги S1-S4 ≥ 12 и таймеры
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          const r = generateRandomAwg31Params();
                          setNewProfile({ ...newProfile, ...r });
                          showNotice("success", "Параметры Amnezia 3.1 (включая 32-битные H1-H4) рандомизированы!");
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
                          placeholder="20000:65000"
                          value={newProfile.awg_hop_ports || "20000:65000"}
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
                          <span>Автоматически перебирать порты при потере связи</span>
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
                    <div className="p-3 rounded-xl bg-teal-950/25 border border-teal-500/30">
                      <div className="text-xs font-bold text-teal-300">
                        Параметры протокола AmneziaWG 2.0
                      </div>
                      <div className="text-[11px] text-slate-400">
                        Чистая WireGuard-совместимая конфигурация (MTU 1420, DNS 1.1.1.1, без лишних параметров)
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
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
                          placeholder="10.8.1.2/24"
                          value={newProfile.awg_local_address || "10.8.1.2/24"}
                          onChange={(e) =>
                            setNewProfile({ ...newProfile, awg_local_address: e.target.value })
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
                        <span className="font-bold">SSH-туннель (порт 22)</span> — надежное шифрованное соединение по стандартному протоколу SSH.
                        Вся веб-активность и DNS шифруются внутри прямого TCP-туннеля.
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
                ) : newProfile.protocol === "shadowtls" ? (
                  <div className="space-y-3">
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Пароль Shadowsocks</label>
                        <input
                          type="text"
                          placeholder="Пароль внутреннего SS"
                          value={newProfile.password}
                          onChange={(e) => setNewProfile({ ...newProfile, password: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Пароль ShadowTLS</label>
                        <input
                          type="text"
                          placeholder="Секрет рукопожатия ShadowTLS v3"
                          value={newProfile.shadowtls_password || ""}
                          onChange={(e) => setNewProfile({ ...newProfile, shadowtls_password: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Целевой SNI</label>
                        <input
                          type="text"
                          placeholder="gateway.icloud.com"
                          value={newProfile.shadowtls_sni || newProfile.sni || "gateway.icloud.com"}
                          onChange={(e) => setNewProfile({ ...newProfile, shadowtls_sni: e.target.value, sni: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>
                    <div className="p-3 rounded-xl bg-pink-950/30 border border-pink-500/30 text-xs text-pink-200 flex items-start gap-2">
                      <span className="text-base">🪞</span>
                      <div>
                        <span className="font-bold">ShadowTLS v3</span> — использует стандартное TLS 1.3 рукопожатие с верификацией валидного сертификата целевого сервера и защитой от внешнего сканирования порта.
                      </div>
                    </div>
                  </div>
                ) : newProfile.protocol === "vless_ws" ? (
                  <div className="space-y-3">
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">UUID пользователя</label>
                        <input
                          type="text"
                          placeholder="00000000-0000-0000-0000-000000000000"
                          value={newProfile.password}
                          onChange={(e) => setNewProfile({ ...newProfile, password: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">WebSocket Path</label>
                        <input
                          type="text"
                          placeholder="/neuroveil-ws"
                          value={newProfile.ws_path || "/neuroveil-ws"}
                          onChange={(e) => setNewProfile({ ...newProfile, ws_path: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">CDN Домен / SNI</label>
                        <input
                          type="text"
                          placeholder="worker.domain.workers.dev"
                          value={newProfile.cdn_host || newProfile.sni || ""}
                          onChange={(e) => setNewProfile({ ...newProfile, cdn_host: e.target.value, sni: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>
                    <div className="p-3 rounded-xl bg-orange-950/30 border border-orange-500/30 text-xs text-orange-200 flex items-start gap-2">
                      <span className="text-base">☁️</span>
                      <div>
                        <span className="font-bold">Cloudflare CDN Shield</span> — туннелирование VLESS трафика через глобальную сеть Cloudflare Edge. Скрывает реальный IP VPS за сетью Cloudflare.
                      </div>
                    </div>
                  </div>
                ) : newProfile.protocol === "webrtc" ? (
                  <div className="space-y-3">
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">UUID пользователя</label>
                        <input
                          type="text"
                          placeholder="00000000-0000-0000-0000-000000000000"
                          value={newProfile.password}
                          onChange={(e) => setNewProfile({ ...newProfile, password: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">Медиа-поток (Service Name)</label>
                        <input
                          type="text"
                          placeholder="neuroveil-webrtc"
                          value={newProfile.ws_path || "neuroveil-webrtc"}
                          onChange={(e) => setNewProfile({ ...newProfile, ws_path: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-slate-400 mb-1">SNI (Маскировка хоста)</label>
                        <input
                          type="text"
                          placeholder="zoom.us"
                          value={newProfile.sni || "zoom.us"}
                          onChange={(e) => setNewProfile({ ...newProfile, sni: e.target.value })}
                          className="w-full px-3 py-2 rounded-xl bg-[#0b101d] border border-slate-700 text-sm text-white"
                        />
                      </div>
                    </div>
                    <div className="p-3 rounded-xl bg-lime-950/30 border border-lime-500/30 text-xs text-lime-200 flex items-start gap-2">
                      <span className="text-base">🎙️</span>
                      <div>
                        <span className="font-bold">WebRTC Steganography</span> — пакеты трафика упаковываются в непрерывный мультимедийный поток с минимальным джиттером и стабильной передачей пакетов без задержек.
                      </div>
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
                            🛡️ Фрагментация TLS-рукопожатия (ClientHello record splitting — RFC 5246/8446)
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
              <div className="space-y-2.5">
                {effectiveClientProfiles.length === 0 ? (
                  <div className="p-5 rounded-xl bg-[#070b14] border border-slate-800/80 text-center space-y-2">
                    <Monitor className="w-7 h-7 text-slate-500 mx-auto" />
                    <div className="text-xs font-semibold text-slate-300">
                      Нет установленных туннелей в приложении
                    </div>
                    <p className="text-[10px] text-slate-400 max-w-xs mx-auto">
                      Добавьте конфигурацию из файла, по ключу или через Автодеплой VPS в соответствующем разделе.
                    </p>
                  </div>
                ) : (
                  effectiveClientProfiles.map((prof) => (
                    <div
                      key={prof.id}
                      style={
                        selectedProfileId === prof.id
                          ? {
                              borderColor: "rgba(16, 185, 129, 0.65)",
                              boxShadow: "0 0 18px rgba(16, 185, 129, 0.30)",
                            }
                          : undefined
                      }
                      className={`p-3 rounded-xl border transition flex flex-col gap-2 ${
                        selectedProfileId === prof.id
                          ? "bg-[#131d33]"
                          : "bg-[#070b14] border-slate-800/90 hover:border-slate-700"
                      }`}
                    >
                      {/* Row 1: Protocol Badge + Name + Ping */}
                      <div className="flex items-center justify-between gap-2 min-w-0">
                        <div className="flex items-center gap-1.5 min-w-0 flex-1">
                          {renderProtocolBadge(prof.protocol, prof.awg_version)}
                          {prof.multi_group_id && (
                            <span
                              className="text-[9.5px] px-1.5 py-0.5 rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/40 font-bold inline-flex items-center gap-1 shrink-0 max-w-[120px]"
                              title={`Мульти-протокол: ${prof.multi_group_name || "Пакет"}`}
                            >
                              <Layers className="w-2.5 h-2.5 shrink-0" />
                              <span className="truncate">
                                {prof.multi_group_name || "МУЛЬТИ"}
                              </span>
                            </span>
                          )}
                          <span
                            className="text-xs font-bold text-white truncate"
                            title={prof.name}
                          >
                            {prof.name}
                          </span>
                        </div>

                        <div className="flex items-center gap-1.5 shrink-0">
                          <button
                            onClick={() => handlePingProfile(prof.id)}
                            disabled={pingingId === prof.id}
                            className="tunnel-ping-btn px-2 py-0.5 rounded-md text-[10px] font-mono font-medium flex items-center gap-1 shrink-0 cursor-pointer transition"
                            title="Проверить пинг"
                          >
                            <Activity
                              className={`w-3 h-3 ${
                                isLightTheme ? "text-emerald-600" : "text-emerald-400"
                              }`}
                            />
                            {pingingId === prof.id
                              ? "..."
                              : prof.ping_ms != null
                              ? `${prof.ping_ms} мс`
                              : "Пинг"}
                          </button>
                        </div>
                      </div>

                      {/* Missing Private Key Warning (if applicable) */}
                      {(prof.protocol === "amneziawg" ||
                        prof.protocol === "awg_quic" ||
                        prof.protocol === "awg_vless" ||
                        prof.protocol === "awg2" ||
                        prof.protocol === "wireguard") &&
                        !(prof.awg_private_key || prof.password)?.trim() && (
                          <div className="flex items-center">
                            <span
                              onClick={(e) => {
                                e.stopPropagation();
                                handleOpenEditModal(prof);
                              }}
                              className={`text-[10px] px-2 py-0.5 rounded-md border font-medium inline-flex items-center gap-1 cursor-pointer hover:bg-amber-500/25 ${
                                isLightTheme
                                  ? "bg-amber-500/15 text-amber-800 border-amber-600/35"
                                  : "bg-amber-500/15 text-amber-300 border-amber-500/30"
                              }`}
                              title="Нажмите для ввода ключа"
                            >
                              <AlertTriangle
                                className={`w-3 h-3 shrink-0 ${
                                  isLightTheme ? "text-amber-600" : "text-amber-400"
                                }`}
                              />
                              Нет Private Key — нажмите, чтобы указать
                            </span>
                          </div>
                        )}

                      {/* Row 2: Server Endpoint & Ports */}
                      <div className="flex items-center gap-1.5 flex-wrap text-[10px] font-mono">
                        <span className="tunnel-meta-chip font-semibold px-2 py-0.5 rounded truncate max-w-[195px]">
                          {prof.server}:{prof.port}
                        </span>
                        {prof.listen_port && (
                          <span className="tunnel-meta-chip px-1.5 py-0.5 rounded shrink-0">
                            :{prof.listen_port}
                          </span>
                        )}
                        {(prof.awg_hop_ports || prof.hop_ports) && (
                          <span
                            className="tunnel-hop-chip px-1.5 py-0.5 rounded truncate max-w-[130px]"
                            title={`Hop: ${prof.awg_hop_ports || prof.hop_ports}`}
                          >
                            Hop: {prof.awg_hop_ports || prof.hop_ports}
                          </span>
                        )}
                      </div>

                      {/* Row 3: Action Toolbar (single row) */}
                      <div className="flex items-center justify-between gap-1.5 pt-2 border-t border-slate-800/80 flex-nowrap">
                        <button
                          onClick={async () => {
                            await handlePinProfileOnPc(prof.id);
                            handleQuickSwitchProfile(prof.id);
                            if (
                              (prof.protocol === "amneziawg" ||
                                prof.protocol === "awg_quic" ||
                                prof.protocol === "awg_vless" ||
                                prof.protocol === "awg2" ||
                                prof.protocol === "wireguard") &&
                              !(prof.awg_private_key || prof.password)?.trim()
                            ) {
                              showNotice(
                                "info",
                                "Внимание: в профиле не указан приватный ключ (Private Key). Введите ключ в настройках (✏️) или выберите .conf файл."
                              );
                            }
                            setActiveTab("dashboard");
                          }}
                          className={`px-2 py-1 rounded-lg text-[10px] font-semibold transition shrink-0 whitespace-nowrap cursor-pointer ${
                            selectedProfileId === prof.id
                              ? "bg-emerald-600 text-white shadow-sm shadow-emerald-600/25 keep-white"
                              : "bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700/70"
                          }`}
                        >
                          {selectedProfileId === prof.id ? "Выбран на ПК" : "Выбрать на ПК"}
                        </button>

                        <div className="flex items-center gap-[3px] flex-nowrap justify-end shrink-0">
                          <button
                            onClick={() => handleOpenQrModal(prof)}
                            className="p-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-cyan-300 flex items-center justify-center transition shrink-0 cursor-pointer"
                            title="Показать QR-код для телефона"
                          >
                            <QrCode className="w-3.5 h-3.5" />
                          </button>

                          <button
                            onClick={() => {
                              if (
                                prof.protocol === "amneziawg" ||
                                prof.protocol === "awg_quic" ||
                                prof.protocol === "awg_vless" ||
                                prof.protocol === "awg2" ||
                                prof.protocol === "wireguard"
                              ) {
                                handleDownloadAwgConf(prof);
                              } else {
                                handleExportJson(prof);
                              }
                            }}
                            className="p-1 rounded-lg bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-emerald-300 flex items-center justify-center transition shrink-0 cursor-pointer"
                            title={
                              prof.protocol === "amneziawg" ||
                              prof.protocol === "awg_quic" ||
                              prof.protocol === "awg_vless" ||
                              prof.protocol === "awg2" ||
                              prof.protocol === "wireguard"
                                ? "Скачать .conf для WireGuard / AmneziaVPN"
                                : "Скачать .json конфиг sing-box"
                            }
                          >
                            <Download className="w-3.5 h-3.5" />
                          </button>

                          {(prof.protocol === "amneziawg" || prof.protocol === "awg_quic") && (
                            <button
                              onClick={() => handleProbeAwgPorts(prof.id)}
                              disabled={probingAwgId === prof.id}
                              className="p-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-cyan-300 flex items-center justify-center transition shrink-0 cursor-pointer"
                              title="Тест портов AWG"
                            >
                              <Zap
                                className={`w-3.5 h-3.5 text-cyan-400 ${
                                  probingAwgId === prof.id ? "animate-pulse" : ""
                                }`}
                              />
                            </button>
                          )}

                          <button
                            onClick={() => handleCopyUri(prof)}
                            className="p-1 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700/70 text-slate-200 flex items-center justify-center transition shrink-0 cursor-pointer"
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
                            className="p-1 rounded-lg bg-cyan-500/15 hover:bg-cyan-500/25 border border-cyan-500/30 text-cyan-300 flex items-center justify-center transition shrink-0 cursor-pointer"
                            title="Изменить конфигурацию в приложении"
                          >
                            <Edit3 className="w-3.5 h-3.5" />
                          </button>

                          <button
                            onClick={() => handleOpenEditModal(prof, "text")}
                            className="p-1 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-indigo-300 flex items-center justify-center transition shrink-0 cursor-pointer"
                            title="Текст конфига (.conf / JSON)"
                          >
                            <FileText className="w-3.5 h-3.5" />
                          </button>

                          <button
                            onClick={() => handleDeleteProfile(prof.id)}
                            className="p-1 rounded-lg bg-rose-950/40 hover:bg-rose-900/60 border border-rose-500/30 text-rose-400 flex items-center justify-center transition shrink-0 cursor-pointer"
                            title="Удалить профиль"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>


          </div>
        )}

        {/* TAB 4: ROUTING & CORE */}
        {activeTab === "routing" && (
          <div className="flex-1 overflow-y-auto p-3.5 space-y-3 animate-tab-enter">
            <div className="flex items-center justify-between gap-2">
              <div className="min-w-0">
                <h2 className="text-sm font-bold text-white flex items-center gap-2">
                  <Globe className="w-4 h-4 text-cyan-400 shrink-0" />
                  <span>Раздельное туннелирование</span>
                </h2>
                <p className="text-[10px] text-slate-400 mt-0.5 truncate">
                  Правила обхода и списки приложений для каждого профиля (рекомендуется TUN)
                </p>
              </div>
              <button
                type="button"
                onClick={handleSaveRouting}
                className="px-3.5 py-1.5 rounded-xl bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-[11px] font-bold text-white shadow-md shadow-cyan-600/20 shrink-0 cursor-pointer transition"
              >
                Сохранить
              </button>
            </div>

            {/* Profile Selector for Routing (Compact) */}
            {effectiveDashboardProfiles.length > 0 && (
              <div className="p-2.5 rounded-xl bg-[#111827] border border-slate-800/90 soft-edge-block space-y-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5">
                    <Smartphone className="w-3.5 h-3.5 text-cyan-400" />
                    Профиль маршрутизации ({effectiveDashboardProfiles.length})
                  </span>
                </div>
                <div className="grid grid-cols-2 gap-1.5 max-h-[104px] overflow-y-auto pr-1 custom-scrollbar">
                  {effectiveDashboardProfiles.map((p) => {
                    const isTarget = p.id === (routingProfileId || selectedProfileId);
                    const isSplitActive = p.routing ? p.routing.split_tunneling : routing.split_tunneling;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => switchRoutingProfile(p.id)}
                        className={`px-2.5 py-1.5 rounded-lg text-[11px] font-semibold flex items-center justify-between gap-1.5 transition border cursor-pointer min-w-0 text-left ${
                          isTarget
                            ? "bg-cyan-500/20 border-cyan-400 text-white shadow-sm shadow-cyan-500/20"
                            : "bg-[#0b101d] border-slate-800 text-slate-400 hover:text-slate-200 hover:border-slate-700"
                        }`}
                      >
                        <div className="flex items-center gap-1.5 min-w-0 flex-1">
                          <span
                            style={
                              isSplitActive
                                ? {
                                    backgroundColor: "var(--color-accent)",
                                    boxShadow: "0 0 5px var(--color-accent-glow)",
                                  }
                                : undefined
                            }
                            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                              isSplitActive ? "" : isLightTheme ? "bg-slate-400" : "bg-slate-600"
                            }`}
                          />
                          <span className="truncate font-bold">{p.name}</span>
                        </div>
                        <span className="tunnel-meta-chip text-[8px] px-1 py-0.5 rounded uppercase font-mono shrink-0">
                          {p.protocol === "awg_quic"
                            ? "AWG+Q"
                            : p.protocol === "awg_vless"
                            ? "AWG+R"
                            : p.protocol === "amneziawg"
                            ? "AWG"
                            : p.protocol}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Master Split Tunneling Toggle Card (Compact) */}
            <div
              className={`p-3 rounded-xl border soft-edge-block transition-all ${
                routing.split_tunneling
                  ? "bg-[#111827] border-cyan-500/50"
                  : "bg-[#111827] border-slate-800/90"
              }`}
            >
              <div className="flex items-center justify-between gap-2.5">
                <div className="flex items-center gap-2.5 min-w-0">
                  <div
                    className={`p-2 rounded-lg border shrink-0 ${
                      routing.split_tunneling
                        ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-400"
                        : "bg-slate-800/60 border-slate-700/60 text-slate-400"
                    }`}
                  >
                    <Zap className="w-4 h-4" />
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <h3 className="text-xs font-bold text-white truncate">
                        Раздельное туннелирование: «{activeRoutingProfile?.name || "По умолчанию"}»
                      </h3>
                      <span
                        className={`text-[9px] px-2 py-0.5 rounded-full font-bold uppercase border shrink-0 ${
                          routing.split_tunneling
                            ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-300"
                            : "bg-slate-800 border-slate-700 text-slate-400"
                        }`}
                      >
                        {routing.split_tunneling ? "ВКЛ" : "ВЫКЛ"}
                      </span>
                    </div>
                    <p className="text-[10px] text-slate-400 mt-0.5 line-clamp-1">
                      {t.splitTunnelingMasterDesc}
                    </p>
                  </div>
                </div>

                <button
                  type="button"
                  onClick={handleToggleSplitForProfile}
                  className={`w-11 h-6 rounded-full transition-colors p-0.5 shrink-0 cursor-pointer border ${
                    routing.split_tunneling
                      ? "theme-toggle-on"
                      : isLightTheme
                      ? "bg-slate-300 border-slate-400/40"
                      : "bg-slate-700 border-slate-600/50"
                  }`}
                  title={
                    routing.split_tunneling
                      ? "Выключить раздельное туннелирование"
                      : "Включить раздельное туннелирование"
                  }
                >
                  <div
                    className={`w-5 h-5 rounded-full bg-white shadow-sm transform transition-transform ${
                      routing.split_tunneling ? "translate-x-5" : "translate-x-0"
                    }`}
                  />
                </button>
              </div>
            </div>

            {/* Split Tunneling Inactive Notice */}
            {!routing.split_tunneling && (
              <div
                className={`p-2.5 rounded-xl bg-amber-500/10 border border-amber-500/30 text-[11px] flex items-center gap-2 ${
                  isLightTheme ? "text-amber-800" : "text-amber-200"
                }`}
              >
                <AlertTriangle
                  className={`w-4 h-4 shrink-0 ${isLightTheme ? "text-amber-600" : "text-amber-400"}`}
                />
                <div>
                  <span className="font-bold">Внимание:</span> {t.splitTunnelingDisabledNotice}
                </div>
              </div>
            )}

            {/* Split Tunneling Rules Container */}
            <div
              className={`space-y-3 transition-all duration-300 ${
                !routing.split_tunneling
                  ? "opacity-35 pointer-events-none select-none filter grayscale-[35%]"
                  : ""
              }`}
            >
              {/* 1. ROUTING PRESETS (Compact Card) */}
              <div className="p-3 rounded-xl bg-[#111827] border border-slate-800/90 soft-edge-block space-y-2">
                <div className="flex items-center justify-between gap-2 flex-wrap">
                  <div className="flex items-center gap-2 min-w-0">
                    <div className="p-1.5 rounded-lg bg-cyan-500/15 border border-cyan-500/30 text-cyan-400 shrink-0">
                      <Bookmark className="w-3.5 h-3.5" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-[11px] font-bold uppercase tracking-wider text-white flex items-center gap-1.5">
                        <span>Предустановки маршрутизации</span>
                        {status.connected && (
                          <span
                            className={`text-[9px] px-1.5 py-0.5 rounded-full border flex items-center gap-0.5 font-semibold ${
                              isLightTheme
                                ? "bg-emerald-600/15 text-emerald-700 border-emerald-600/35"
                                : "bg-emerald-500/20 text-emerald-300 border-emerald-500/30"
                            }`}
                          >
                            <Zap className="w-2.5 h-2.5" /> На лету
                          </span>
                        )}
                      </div>
                      <p className="text-[10px] text-slate-400 truncate">
                        Быстрое переключение списков программ в один клик
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-1.5 shrink-0">
                    {status.connected && (
                      <button
                        type="button"
                        onClick={handleApplyOnTheFly}
                        disabled={isApplyingPreset}
                        className={`px-2 py-1 rounded-lg border text-[10px] font-semibold flex items-center gap-1 transition cursor-pointer ${
                          isLightTheme
                            ? "bg-emerald-600/15 hover:bg-emerald-600/25 border-emerald-600/40 text-emerald-700"
                            : "bg-emerald-500/15 hover:bg-emerald-500/25 border-emerald-500/40 text-emerald-300"
                        }`}
                        title="Применить текущие списки программ к работающему VPN"
                      >
                        <Zap className={`w-3 h-3 ${isApplyingPreset ? "animate-spin" : ""}`} />
                        <span>Применить</span>
                      </button>
                    )}
                    {activePresetId && !routingPresets.find((p) => p.id === activePresetId)?.is_builtin && (
                      <button
                        type="button"
                        onClick={handleUpdateCurrentPreset}
                        className="px-2 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-[10px] font-semibold text-slate-200 flex items-center gap-1 transition cursor-pointer"
                        title="Обновить текущий пресет"
                      >
                        <Save className="w-3 h-3 text-cyan-400" />
                        <span>Обновить</span>
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => {
                        invoke("open_popup_window", {
                          mode: "save_preset",
                          profileId: routingProfileId || selectedProfileId || null,
                          submode: routing.default_outbound || "direct",
                        });
                      }}
                      className="px-2.5 py-1 rounded-lg bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/40 text-[10px] font-semibold text-cyan-200 flex items-center gap-1 transition cursor-pointer"
                    >
                      <Plus className="w-3 h-3" />
                      <span>Пресет</span>
                    </button>
                  </div>
                </div>

                {/* Preset Chips (Compact) */}
                <div className="flex flex-wrap items-center gap-1.5">
                  {routingPresets.map((pr) => {
                    const isActive = activePresetId === pr.id;
                    const totalCount = (pr.proxy_processes || []).length + (pr.direct_processes || []).length;
                    return (
                      <div
                        key={pr.id}
                        onClick={() => handleSelectPreset(pr)}
                        className={`group relative flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-[11px] font-medium transition cursor-pointer select-none ${
                          isActive
                            ? "bg-cyan-500/20 border-cyan-400 text-white shadow-sm"
                            : "bg-[#0b101d] border-slate-800 text-slate-300 hover:border-slate-700 hover:text-white"
                        }`}
                      >
                        {pr.id.includes("games") ? (
                          <Gamepad2
                            className={`w-3 h-3 shrink-0 ${
                              isLightTheme ? "text-emerald-600" : "text-emerald-400"
                            }`}
                          />
                        ) : pr.id.includes("discord") ? (
                          <MessageSquare className="w-3 h-3 text-indigo-400 shrink-0" />
                        ) : pr.id.includes("browser") ? (
                          <Globe className="w-3 h-3 text-cyan-400 shrink-0" />
                        ) : pr.id.includes("media") ? (
                          <Sparkles
                            className={`w-3 h-3 shrink-0 ${
                              isLightTheme ? "text-amber-600" : "text-amber-400"
                            }`}
                          />
                        ) : (
                          <Zap className="w-3 h-3 text-cyan-400 shrink-0" />
                        )}
                        <span className="font-semibold truncate max-w-[115px]">{pr.name}</span>
                        <span className="text-[9px] font-mono text-cyan-400/90 font-bold">
                          ({totalCount})
                        </span>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeletePreset(pr.id);
                          }}
                          className="text-slate-500 hover:text-rose-400 p-0.5 opacity-60 group-hover:opacity-100 transition"
                          title="Удалить пресет"
                        >
                          <X className="w-2.5 h-2.5" />
                        </button>
                      </div>
                    );
                  })}
                </div>

                {/* Active Preset Description Banner */}
                <div className="pt-1 text-[10px] text-slate-400 flex items-center justify-between border-t border-slate-800/80">
                  <div className="flex items-center gap-1 truncate">
                    <span className="text-cyan-400 font-medium">Активен:</span>
                    <span className="text-slate-300 truncate">
                      {routingPresets.find((p) => p.id === activePresetId)?.name || "Пользовательский"}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={handleResetPresets}
                    className="text-[10px] text-slate-400 hover:text-cyan-300 transition shrink-0 ml-2 cursor-pointer"
                  >
                    Сбросить пресеты
                  </button>
                </div>
              </div>

              {/* ДВЕ СВЕРНУТЫЕ КОМПАКТНЫЕ ПЛИТКИ РЕЖИМОВ С КНОПКОЙ «НАСТРОИТЬ» */}
              <div className="grid grid-cols-1 gap-2.5">
                {/* ПЛИТКА 1: ТОЛЬКО ВЫБРАННОЕ ЧЕРЕЗ VPN (Белый список) */}
                <div
                  className={`routing-mode-card p-3 rounded-xl transition-all flex flex-col gap-2 ${
                    routing.default_outbound === "direct" ? "is-active" : ""
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div
                      onClick={() => {
                        if (routing.default_outbound !== "direct") {
                          const nextRouting = buildCurrentRoutingObject({
                            default_outbound: "direct",
                          });
                          void syncAndApplyRoutingImmediately(nextRouting);
                        }
                      }}
                      className="flex items-center gap-2.5 min-w-0 flex-1 cursor-pointer"
                    >
                      <div
                        className={`routing-mode-icon w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${
                          routing.default_outbound === "direct" ? "is-active" : ""
                        }`}
                      >
                        <Shield className="w-4 h-4" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <span
                            className={`text-xs font-bold ${
                              routing.default_outbound === "direct"
                                ? "text-white"
                                : "text-slate-200"
                            }`}
                          >
                            Только выбранное через VPN
                          </span>
                          <span className="routing-mode-pill text-[9px] px-1.5 py-0.5 rounded-full font-semibold font-mono">
                            Белый список
                          </span>
                        </div>
                        <p className="text-[10px] text-slate-400 truncate mt-0.5">
                          Через VPN идут только выбранные программы, домены и IP
                        </p>
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5 shrink-0">
                      <button
                        type="button"
                        onClick={() =>
                          invoke("open_popup_window", {
                            mode: "routing_rules",
                            profileId: routingProfileId || selectedProfileId || null,
                            submode: "proxy",
                          })
                        }
                        className="routing-mode-config-btn px-2.5 py-1 rounded-lg text-[11px] font-semibold flex items-center gap-1 transition cursor-pointer"
                      >
                        <Sliders className="w-3 h-3" />
                        <span>Настроить</span>
                      </button>

                      {routing.default_outbound === "direct" ? (
                        <span className="routing-mode-active-badge text-[10px] px-2.5 py-1 rounded-lg font-bold uppercase flex items-center gap-1">
                          <Check className="w-3 h-3 stroke-[3]" /> Активен
                        </span>
                      ) : (
                        <button
                          type="button"
                          onClick={() => {
                            const nextRouting = buildCurrentRoutingObject({
                              default_outbound: "direct",
                            });
                            void syncAndApplyRoutingImmediately(nextRouting);
                          }}
                          className="routing-mode-enable-btn text-[10px] px-2.5 py-1 rounded-lg transition font-semibold cursor-pointer"
                        >
                          Включить
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Compact counters summary */}
                  <div className="flex items-center gap-2 pt-1.5 border-t border-slate-800/80 text-[10px] text-slate-400 font-mono">
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      Программ: <b>{proxyProcessesList.length}</b>
                    </span>
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      Доменов:{" "}
                      <b>
                        {
                          proxyDomainsText
                            .split("\n")
                            .map((s) => s.trim())
                            .filter(Boolean).length
                        }
                      </b>
                    </span>
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      IP / CIDR:{" "}
                      <b>
                        {
                          proxyIpsText
                            .split("\n")
                            .map((s) => s.trim())
                            .filter(Boolean).length
                        }
                      </b>
                    </span>
                  </div>
                </div>

                {/* ПЛИТКА 2: ВЕСЬ ТРАФИК ЧЕРЕЗ VPN (Черный список / Исключения) */}
                <div
                  className={`routing-mode-card p-3 rounded-xl transition-all flex flex-col gap-2 ${
                    (routing.default_outbound || "proxy") === "proxy" ? "is-active" : ""
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div
                      onClick={() => {
                        if ((routing.default_outbound || "proxy") !== "proxy") {
                          const nextRouting = buildCurrentRoutingObject({
                            default_outbound: "proxy",
                          });
                          void syncAndApplyRoutingImmediately(nextRouting);
                        }
                      }}
                      className="flex items-center gap-2.5 min-w-0 flex-1 cursor-pointer"
                    >
                      <div
                        className={`routing-mode-icon w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${
                          (routing.default_outbound || "proxy") === "proxy" ? "is-active" : ""
                        }`}
                      >
                        <Zap className="w-4 h-4" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <span
                            className={`text-xs font-bold ${
                              (routing.default_outbound || "proxy") === "proxy"
                                ? "text-white"
                                : "text-slate-200"
                            }`}
                          >
                            Весь трафик через VPN
                          </span>
                          <span className="routing-mode-pill text-[9px] px-1.5 py-0.5 rounded-full font-semibold font-mono">
                            Черный список
                          </span>
                        </div>
                        <p className="text-[10px] text-slate-400 truncate mt-0.5">
                          Весь трафик через VPN, кроме выбранных исключений (мимо VPN)
                        </p>
                      </div>
                    </div>

                    <div className="flex items-center gap-1.5 shrink-0">
                      <button
                        type="button"
                        onClick={() =>
                          invoke("open_popup_window", {
                            mode: "routing_rules",
                            profileId: routingProfileId || selectedProfileId || null,
                            submode: "direct",
                          })
                        }
                        className="routing-mode-config-btn px-2.5 py-1 rounded-lg text-[11px] font-semibold flex items-center gap-1 transition cursor-pointer"
                      >
                        <Sliders className="w-3 h-3" />
                        <span>Настроить</span>
                      </button>

                      {(routing.default_outbound || "proxy") === "proxy" ? (
                        <span className="routing-mode-active-badge text-[10px] px-2.5 py-1 rounded-lg font-bold uppercase flex items-center gap-1">
                          <Check className="w-3 h-3 stroke-[3]" /> Активен
                        </span>
                      ) : (
                        <button
                          type="button"
                          onClick={() => {
                            const nextRouting = buildCurrentRoutingObject({
                              default_outbound: "proxy",
                            });
                            void syncAndApplyRoutingImmediately(nextRouting);
                          }}
                          className="routing-mode-enable-btn text-[10px] px-2.5 py-1 rounded-lg transition font-semibold cursor-pointer"
                        >
                          Включить
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Compact counters summary */}
                  <div className="flex items-center gap-2 pt-1.5 border-t border-slate-800/80 text-[10px] text-slate-400 font-mono">
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      Программ: <b>{directProcessesList.length}</b>
                    </span>
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      Доменов:{" "}
                      <b>
                        {
                          directDomainsText
                            .split("\n")
                            .map((s) => s.trim())
                            .filter(Boolean).length
                        }
                      </b>
                    </span>
                    <span className="routing-mode-counter px-2 py-0.5 rounded">
                      IP / CIDR:{" "}
                      <b>
                        {
                          directIpsText
                            .split("\n")
                            .map((s) => s.trim())
                            .filter(Boolean).length
                        }
                      </b>
                    </span>
                  </div>
                </div>
              </div>

            </div>


          </div>
        )}

        {/* TAB 5: SETTINGS & THEMES */}
        {activeTab === "settings" && (
          <div className="flex-1 overflow-y-auto p-3.5 space-y-4 animate-tab-enter">
            {/* Header */}
            <div>
              <h2 className="text-base font-bold text-white flex items-center gap-2">
                <Settings className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                {t.settingsTitle}
              </h2>
              <p className="text-[11px] text-slate-400 mt-0.5">{t.settingsSubtitle}</p>
            </div>

            {/* NETWORK OPERATING MODE SECTION */}
            <div
              className={`p-4 rounded-2xl border soft-edge-block transition-all duration-300 space-y-3 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Globe className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                  <div>
                    <h3 className="text-xs font-bold text-white">Режим работы сети</h3>
                    <p className="text-[10px] text-slate-400">
                      Выбор способа перехвата сетевого трафика в Windows
                    </p>
                  </div>
                </div>
                <span className="text-[9px] px-2 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-semibold font-mono">
                  {routing.mode === "tun" ? "TUN-режим" : "System Proxy"}
                </span>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <button
                  type="button"
                  onClick={() => handleQuickSwitchMode("system_proxy")}
                  className={`network-mode-card p-3 rounded-xl text-left transition-all cursor-pointer ${
                    routing.mode === "system_proxy" ? "is-active" : ""
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-bold">System Proxy</span>
                    <Globe className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                  </div>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Легкий режим без прав администратора (браузеры, Telegram, Discord).
                  </p>
                </button>

                <button
                  type="button"
                  onClick={() => handleQuickSwitchMode("tun")}
                  className={`network-mode-card p-3 rounded-xl text-left transition-all cursor-pointer ${
                    routing.mode === "tun" ? "is-active" : ""
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-bold">TUN-режим (Вирт. адаптер)</span>
                    <Zap className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                  </div>
                  <p className="text-[11px] text-slate-400 leading-relaxed">
                    Перехватывает весь трафик ПК, включая игры и любые приложения.
                  </p>
                </button>
              </div>
            </div>

            {/* SECURITY & KILL SWITCH SECTION (Compact) */}
            <div
              className={`p-3 rounded-2xl border soft-edge-block transition-all duration-300 space-y-2 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <ShieldAlert className={`w-3.5 h-3.5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-[11px] font-bold text-white">
                    Сетевая безопасность, Kill Switch и защита от утечек
                  </h3>
                  <p className="text-[9.5px] text-slate-400">
                    Защита от утечек IP, DNS и аварийная блокировка трафика
                  </p>
                </div>
              </div>

              <div className="grid grid-cols-1 gap-1.5 pt-0.5">
                {/* Kill Switch Toggle */}
                <div
                  onClick={handleToggleKillSwitch}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                        Kill Switch (Аварийная блокировка)
                      </span>
                      <span className="text-[8.5px] px-1 py-0.2 rounded bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-semibold">
                        PRO
                      </span>
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      Блокирует исходящий трафик в Windows при обрыве VPN
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      routing.kill_switch
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        routing.kill_switch ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* DNS Leak Protection Toggle */}
                <div
                  onClick={handleToggleDnsLeakProtect}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                        Защита от утечек DNS (DNS Leak Protection)
                      </span>
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      Шифрует DNS-запросы (53 порт) через Cloudflare DoH (1.1.1.1)
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      (routing.dns_leak_protect ?? true)
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        (routing.dns_leak_protect ?? true) ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* IPv6 Leak Shield Toggle */}
                <div
                  onClick={handleToggleBlockIpv6}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                        Блокировка протокола IPv6 (IPv6 Shield)
                      </span>
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      Предотвращает утечку реального IPv6 мимо VPN-туннеля
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      (routing.block_ipv6 ?? true)
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        (routing.block_ipv6 ?? true) ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* TLS ClientHello Splitting Card */}
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
                  className={`p-2 px-2.5 rounded-lg border cursor-pointer transition flex items-center justify-between gap-2.5 group ${
                    (routing.tls_fragment ?? true)
                      ? "bg-cyan-500/10 border-cyan-500/40"
                      : "bg-slate-900/60 border-slate-800/80 hover:border-slate-700"
                  }`}
                >
                  <div className="min-w-0 pr-1">
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <span className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                        Фрагментация TLS-рукопожатия (ClientHello)
                      </span>
                      <span
                        className={`text-[8.5px] font-bold px-1.5 py-0.2 rounded-full border ${
                          (routing.tls_fragment ?? true)
                            ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-300"
                            : "bg-slate-800 border-slate-700 text-slate-400"
                        }`}
                      >
                        {(routing.tls_fragment ?? true) ? "Активно" : "Выкл"}
                      </span>
                    </div>
                    <p className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      Разбивает ClientHello на фрагменты для VLESS Reality, TUIC и Hysteria 2
                    </p>
                  </div>

                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      (routing.tls_fragment ?? true)
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        (routing.tls_fragment ?? true) ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* SYSTEM AUTOSTART & TRAY BEHAVIOR SECTION (Compact) */}
            <div
              className={`p-3 rounded-2xl border soft-edge-block transition-all duration-300 space-y-2 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Monitor className={`w-3.5 h-3.5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-[11px] font-bold text-white">{t.systemSectionTitle}</h3>
                  <p className="text-[9.5px] text-slate-400">{t.systemSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-1 gap-1.5 pt-0.5">
                {/* Autostart Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ autostart: !autostart })}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                      {t.autostartTitle}
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      {t.autostartDesc}
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      autostart
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        autostart ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Start Minimized Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ startMinimized: !startMinimized })}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                      {t.startMinimizedTitle}
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      {t.startMinimizedDesc}
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      startMinimized
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        startMinimized ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Close to Tray Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ closeToTray: !closeToTray })}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                      {t.closeToTrayTitle}
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      {t.closeToTrayDesc}
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      closeToTray
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        closeToTray ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>

                {/* Auto-connect on startup Toggle */}
                <div
                  onClick={() => handleUpdateSystemSettings({ autoConnect: !autoConnect })}
                  className="p-2 px-2.5 rounded-lg bg-slate-900/60 border border-slate-800/80 hover:border-slate-700 cursor-pointer flex items-center justify-between gap-2.5 transition group"
                >
                  <div className="min-w-0 pr-1">
                    <div className="text-[11px] font-bold text-white group-hover:text-cyan-200 transition">
                      {t.autoConnectTitle}
                    </div>
                    <div className="text-[9.5px] text-slate-400 mt-0.5 leading-snug">
                      {t.autoConnectDesc}
                    </div>
                  </div>
                  <div
                    className={`w-9 h-5 rounded-full transition-all p-0.5 shrink-0 ${
                      autoConnect
                        ? "theme-toggle-on"
                        : isLightTheme
                        ? "bg-slate-300 border border-slate-400/40"
                        : "bg-slate-700 border border-white/10"
                    }`}
                  >
                    <div
                      className={`w-4 h-4 rounded-full bg-white shadow-sm transform transition-transform ${
                        autoConnect ? "translate-x-4" : "translate-x-0"
                      }`}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* INTERFACE LANGUAGE SECTION (Compact) */}
            <div
              className={`p-3 rounded-2xl border soft-edge-block transition-all duration-300 space-y-2 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Languages className={`w-3.5 h-3.5 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-[11px] font-bold text-white">{t.languageSectionTitle}</h3>
                  <p className="text-[9.5px] text-slate-400">{t.languageSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-1.5 pt-0.5">
                {/* Russian Card */}
                <div
                  onClick={() => handleUpdateSystemSettings({ language: "ru" })}
                  style={
                    language === "ru"
                      ? { boxShadow: "0 0 10px -2px var(--color-accent-glow)" }
                      : undefined
                  }
                  className={`p-2 rounded-lg cursor-pointer border transition-all flex items-center justify-between ${
                    language === "ru"
                      ? "border-cyan-400 bg-cyan-500/10 ring-1 ring-cyan-400/50"
                      : "border-slate-800 bg-slate-900/50 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <div className="w-6 h-6 rounded-md bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center font-black font-mono text-[10px] text-cyan-300 shrink-0">
                      RU
                    </div>
                    <div>
                      <div className="text-[11px] font-bold text-white">{t.langRu}</div>
                      <div className="text-[9px] text-slate-400">Русский</div>
                    </div>
                  </div>
                  <div
                    className={`w-3 h-3 rounded-full border flex items-center justify-center ${
                      language === "ru"
                        ? "border-cyan-400 bg-cyan-500"
                        : "border-slate-600 bg-transparent"
                    }`}
                  >
                    {language === "ru" && <div className="w-1 h-1 rounded-full bg-white" />}
                  </div>
                </div>

                {/* English Card */}
                <div
                  onClick={() => handleUpdateSystemSettings({ language: "en" })}
                  style={
                    language === "en"
                      ? { boxShadow: "0 0 10px -2px var(--color-accent-glow)" }
                      : undefined
                  }
                  className={`p-2 rounded-lg cursor-pointer border transition-all flex items-center justify-between ${
                    language === "en"
                      ? "border-cyan-400 bg-cyan-500/10 ring-1 ring-cyan-400/50"
                      : "border-slate-800 bg-slate-900/50 hover:border-slate-700"
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <div className="w-6 h-6 rounded-md bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center font-black font-mono text-[10px] text-cyan-300 shrink-0">
                      EN
                    </div>
                    <div>
                      <div className="text-[11px] font-bold text-white">{t.langEn}</div>
                      <div className="text-[9px] text-slate-400">English</div>
                    </div>
                  </div>
                  <div
                    className={`w-3 h-3 rounded-full border flex items-center justify-center ${
                      language === "en"
                        ? "border-cyan-400 bg-cyan-500"
                        : "border-slate-600 bg-transparent"
                    }`}
                  >
                    {language === "en" && <div className="w-1 h-1 rounded-full bg-white" />}
                  </div>
                </div>
              </div>
            </div>

            {/* THEMES & APPEARANCE SECTION (Compact) */}
            <div
              className={`p-3 rounded-2xl border soft-edge-block transition-all duration-300 space-y-2.5 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex flex-wrap items-center justify-between gap-1.5">
                <div className="flex items-center gap-2">
                  <Palette className={`w-3.5 h-3.5 ${currentTheme.styles.accentText}`} />
                  <div>
                    <h3 className="text-[11px] font-bold text-white">{t.themesSectionTitle}</h3>
                    <p className="text-[9.5px] text-slate-400">{t.themesSectionSubtitle}</p>
                  </div>
                </div>
                <span className="text-[9.5px] px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700 font-medium">
                  {t.activeTheme}: <b className="text-white">{currentTheme.name}</b>
                </span>
              </div>

              {/* Theme Grid Cards (Smaller) */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-1.5 pt-0.5">
                {THEMES.map((theme) => {
                  const isSelected = theme.id === currentThemeId;
                  return (
                    <div
                      key={theme.id}
                      onClick={() => handleSelectTheme(theme.id)}
                      style={
                        isSelected
                          ? { boxShadow: "0 0 12px -2px var(--color-accent-glow)" }
                          : undefined
                      }
                      className={`p-1.5 rounded-lg cursor-pointer border transition-all text-left relative group flex flex-col justify-between ${
                        isSelected
                          ? "border-cyan-400 bg-cyan-500/10 ring-1 ring-cyan-400/50"
                          : "border-slate-800 hover:border-slate-600 bg-slate-900/50 hover:bg-slate-800/30"
                      }`}
                    >
                      <div>
                        {/* Swatch Previews */}
                        <div className="flex items-center justify-between gap-1 mb-1">
                          <div className="flex items-center gap-0.5">
                            <div
                              className={`w-2.5 h-2.5 rounded-sm border ${
                                theme.isLight ? "border-slate-400/50" : "border-white/15"
                              }`}
                              style={{ backgroundColor: theme.preview.bg }}
                              title="Background"
                            />
                            <div
                              className={`w-2.5 h-2.5 rounded-sm border ${
                                theme.isLight ? "border-slate-400/50" : "border-white/15"
                              }`}
                              style={{ backgroundColor: theme.preview.card }}
                              title="Cards"
                            />
                            <div
                              className="w-2.5 h-2.5 rounded-sm border border-white/15"
                              style={{ backgroundColor: theme.preview.accent }}
                              title="Color Note Accent"
                            />
                            <div
                              className="w-2.5 h-2.5 rounded-sm border border-white/15"
                              style={{ backgroundColor: theme.preview.secondary }}
                              title="Secondary Accent"
                            />
                          </div>
                          {isSelected ? (
                            <span className="w-3.5 h-3.5 rounded-full bg-cyan-500/20 border border-cyan-500/40 flex items-center justify-center shrink-0">
                              <Check className="w-2 h-2 text-cyan-400" />
                            </span>
                          ) : (
                            <span className="text-[7.5px] px-1 py-0.2 rounded bg-slate-800/80 text-slate-400 font-medium shrink-0">
                              {theme.isLight ? "Light" : "Dark"}
                            </span>
                          )}
                        </div>

                        <div className="text-[10px] font-bold text-white group-hover:text-cyan-200 transition truncate mb-0.5">
                          {theme.name}
                        </div>

                        <p
                          className="text-[8.5px] text-slate-400 leading-tight line-clamp-1 mb-1.5"
                          title={theme.description}
                        >
                          {theme.description}
                        </p>
                      </div>

                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSelectTheme(theme.id);
                        }}
                        className={`w-full py-0.5 px-1.5 rounded text-[9px] font-semibold flex items-center justify-center gap-1 transition ${
                          isSelected
                            ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40"
                            : "bg-slate-800 text-slate-300 hover:bg-slate-700"
                        }`}
                      >
                        {isSelected ? (
                          <>
                            <Check className="w-2.5 h-2.5 text-cyan-400" />
                            <span>{t.themeSelected}</span>
                          </>
                        ) : (
                          <span>{t.applyTheme}</span>
                        )}
                      </button>
                    </div>
                  );
                })}
              </div>

              {/* Window Opacity Slider (Внешний вид и прозрачность окна) */}
              <div className="pt-2.5 border-t border-slate-800/80 space-y-1.5">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Sliders className={`w-3.5 h-3.5 ${currentTheme.styles.accentText}`} />
                    <div>
                      <h4 className="text-[11px] font-bold text-white">Прозрачность окна приложения</h4>
                    </div>
                  </div>
                  <span className="text-[11px] px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono font-bold">
                    {windowOpacity}%
                  </span>
                </div>

                <div className="flex items-center gap-2 pt-0.5">
                  <span className="text-[10px] text-slate-400 font-mono">70%</span>
                  <input
                    type="range"
                    min={70}
                    max={100}
                    step={1}
                    value={windowOpacity}
                    onChange={(e) => handleUpdateOpacity(parseInt(e.target.value, 10))}
                    className="flex-1 cursor-pointer h-1.5 rounded-lg appearance-none"
                  />
                  <span className="text-[10px] text-slate-400 font-mono">100%</span>
                  <button
                    onClick={() => handleUpdateOpacity(100)}
                    className="px-2 py-0.5 rounded-md bg-slate-800 hover:bg-slate-700 text-[10px] font-medium text-slate-300 transition"
                  >
                    100%
                  </button>
                </div>
              </div>
            </div>

            {/* SYSTEM & CORE SECTION */}
            <div
              className={`p-4 rounded-2xl border soft-edge-block transition-all duration-300 space-y-3 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Cpu className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                <div>
                  <h3 className="text-xs font-bold text-white">{t.coreSectionTitle}</h3>
                  <p className="text-[10px] text-slate-400">{t.coreSectionSubtitle}</p>
                </div>
              </div>

              <div className="grid grid-cols-1 gap-2.5 pt-1">
                <div className="p-3 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1">
                  <span className="text-[10px] text-slate-400">{t.coreVersionLabel}</span>
                  <div className="text-xs font-mono font-semibold text-white flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_#34d399]" />
                    {status.singbox_version || "sing-box (готов)"}
                  </div>
                </div>

                <div className="p-3 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1">
                  <span className="text-[10px] text-slate-400">{t.coreDriverLabel}</span>
                  <div className="text-xs font-mono font-semibold text-white flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_#34d399]" />
                    wintun.dll (High-performance Layer-3 TUN)
                  </div>
                </div>
              </div>

              <div className="flex flex-wrap gap-2 pt-1">
                <button
                  type="button"
                  onClick={handleDownloadCore}
                  disabled={downloadingCore}
                  className="px-3 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 border border-slate-700 flex items-center gap-1.5 transition"
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
                  className="px-3 py-1.5 rounded-xl bg-slate-800/60 hover:bg-slate-700/60 text-xs font-medium text-slate-400 hover:text-slate-200 transition"
                >
                  {t.clearLogsBtn}
                </button>
              </div>
            </div>

            {/* BACKUPS & FACTORY RESET SECTION */}
            <div
              className={`p-4 rounded-2xl border soft-edge-block transition-all duration-300 space-y-3 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Save className={`w-4 h-4 ${currentTheme.styles.accentText}`} />
                  <div>
                    <h3 className="text-xs font-bold text-white">
                      Резервные копии и сброс настроек
                    </h3>
                    <p className="text-[10px] text-slate-400">
                      Экспорт и восстановление профилей ({profiles.length}), правил и параметров или полный сброс
                    </p>
                  </div>
                </div>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <button
                  type="button"
                  onClick={handleExportBackup}
                  disabled={backupBusy !== "idle" || resetBusy}
                  className="py-2 px-3 rounded-xl bg-cyan-600/20 hover:bg-cyan-600/30 disabled:opacity-50 border border-cyan-500/35 text-xs font-semibold text-cyan-300 flex items-center justify-center gap-1.5 transition cursor-pointer"
                >
                  <Download className="w-3.5 h-3.5" />
                  {backupBusy === "export" ? "Сохранение..." : "Создать бэкап"}
                </button>

                <button
                  type="button"
                  onClick={handleImportBackup}
                  disabled={backupBusy !== "idle" || resetBusy}
                  className="py-2 px-3 rounded-xl bg-slate-800 hover:bg-slate-700 disabled:opacity-50 border border-slate-700 text-xs font-semibold text-slate-200 flex items-center justify-center gap-1.5 transition cursor-pointer"
                >
                  <Upload className="w-3.5 h-3.5" />
                  {backupBusy === "import" ? "Восстановление..." : "Восстановить из файла"}
                </button>
              </div>

              <div className="pt-2.5 border-t border-slate-800/70 space-y-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="min-w-0">
                    <div className="text-xs font-semibold text-rose-300 flex items-center gap-1.5">
                      <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                      <span>Сброс до заводских настроек</span>
                    </div>
                    <p className="text-[10px] text-slate-400 mt-0.5 leading-snug">
                      Удалит все туннели, правила, параметры и сбросит тему оформления (ядро sing-box сохранится).
                    </p>
                  </div>
                  {!confirmFactoryReset && (
                    <button
                      type="button"
                      onClick={() => setConfirmFactoryReset(true)}
                      disabled={resetBusy || backupBusy !== "idle"}
                      className="px-3 py-1.5 rounded-xl bg-rose-500/15 hover:bg-rose-500/25 border border-rose-500/40 text-xs font-semibold text-rose-300 flex items-center gap-1.5 transition cursor-pointer shrink-0"
                    >
                      <RotateCcw className="w-3.5 h-3.5" />
                      Сбросить
                    </button>
                  )}
                </div>

                {confirmFactoryReset && (
                  <div className="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/40 flex flex-wrap items-center justify-between gap-2">
                    <span className="text-[11px] font-medium text-rose-200">
                      Точно удалить все данные и сбросить настройки?
                    </span>
                    <div className="flex items-center gap-1.5 shrink-0">
                      <button
                        type="button"
                        onClick={handleFactoryReset}
                        disabled={resetBusy}
                        className="px-2.5 py-1 rounded-lg bg-rose-600 hover:bg-rose-500 disabled:opacity-50 text-[11px] font-bold text-white transition cursor-pointer"
                      >
                        {resetBusy ? "Сброс..." : "Подтвердить сброс"}
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmFactoryReset(false)}
                        disabled={resetBusy}
                        className="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-medium text-slate-300 transition cursor-pointer"
                      >
                        Отмена
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Bottom Footer Bar */}
            <div className="p-3 rounded-xl bg-slate-900/40 border border-slate-800/60 text-[10px] text-slate-400 text-center">
              <span className="font-semibold text-slate-300">NeuroVeil VPN v{APP_VERSION} (Build {APP_BUILD})</span>
            </div>

          </div>
        )}

        {/* TAB 6: ABOUT & DEVELOPER (MoDiE) */}
        {activeTab === "about" && (
          <div className="flex-1 overflow-y-auto p-3.5 space-y-4 animate-tab-enter">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-xl font-bold text-white flex items-center gap-2.5">
                  <Info className={`w-6 h-6 ${currentTheme.styles.accentText}`} />
                  О программе NeuroVeil VPN
                </h2>
                <p className="text-xs text-slate-400 mt-1">
                  Архитектура безопасности, специальные протоколы защиты соединения, функционал клиента и центр обновлений
                </p>
              </div>
              <span className="text-xs px-3 py-1 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono font-bold">
                Версия: {APP_VERSION} build {APP_BUILD}
              </span>
            </div>

            {/* Brand Hero Banner */}
            <div
              className={`p-6 rounded-2xl border soft-edge-block transition-all duration-300 relative overflow-hidden ${currentTheme.styles.cardBorder}`}
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
                      sing-box-lx 1.14+ Core
                    </span>
                    <span className="text-[10px] px-2.5 py-0.5 rounded-full bg-fuchsia-500/20 text-fuchsia-300 border border-fuchsia-500/30 font-mono">
                      Adaptive Handshake Shield
                    </span>
                  </div>
                  <p className="text-xs text-slate-300 mt-2 leading-relaxed">
                    NeuroVeil VPN — высокоскоростной защищенный клиент нового поколения для стабильного и безопасного доступа к сети. Объединяет 12 современных криптографических протоколов, специальные гибридные схемы защиты заголовков и рукопожатия WireGuard / Amnezia, интеллектуальный автоподбор портов, развертывание собственного VPS-сервера в 1 клик и раздельное туннелирование на лету.
                  </p>
                </div>
              </div>
            </div>

            {/* Key Client Capabilities & Functional Modules */}
            <div
              className={`p-5 rounded-2xl border soft-edge-block transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Zap className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <h3 className="text-sm font-bold text-white">Актуальный функционал и возможности клиента</h3>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3.5 pt-1">
                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-emerald-400 font-bold text-xs">
                    <RefreshCw className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                    Автоподбор порта (Auto Port Hunt)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Автоматический циклический перебор локальных UDP-портов (<code>ListenPort</code>) и пула <code>20000:65000</code> с контролем рукопожатия в реальном времени до полного установления канала связи.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-cyan-400 font-bold text-xs">
                    <Server className="w-3.5 h-3.5 text-cyan-400 shrink-0" />
                    Автодеплой на VPS по SSH в 1 клик
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Полностью автоматическая настройка чистого Linux-сервера: установка ядра <code>sing-box-lx</code>, включение Google BBR, генерация ключей X25519 / WireGuard, правил NAT / iptables и создание готовых профилей.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-indigo-400 font-bold text-xs">
                    <Bookmark className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
                    Маршрутизация «На лету» и Пресеты
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Мгновенное переключение списков программ (<code>.exe</code>), папок с играми, доменов и IP-подсетей (Белый или Черный список) прямо во время работы VPN без разрыва соединения.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-purple-400 font-bold text-xs">
                    <Smartphone className="w-3.5 h-3.5 text-purple-400 shrink-0" />
                    Менеджер устройств, QR и экспорт
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Выпуск персональных ключей для смартфонов, ноутбуков и друзей прямо с VPS, генерация QR-кодов и выгрузка готовых файлов <code>.conf</code> (Amnezia / WireGuard) и <code>.json</code>.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-amber-400 font-bold text-xs">
                    <ShieldCheck className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                    TLS ClientHello Фрагментация
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Фрагментация TCP-пакетов и TLS-записей рукопожатия (RFC 5246/8446) для защиты метаданных соединения на кабельных и мобильных сетях.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-rose-400 font-bold text-xs">
                    <ShieldAlert className="w-3.5 h-3.5 text-rose-400 shrink-0" />
                    Kill Switch, DNS DoH & IPv6 Shield
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Аварийная блокировка трафика при обрыве туннеля, перехват DNS через зашифрованный Cloudflare DoH (1.1.1.1), защита от утечек IPv6 и атомарное сохранение состояния.
                  </p>
                </div>
              </div>
            </div>

            {/* Architecture & Tech Specs Details */}
            <div
              className={`p-5 rounded-2xl border soft-edge-block transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
              style={{ backgroundColor: currentTheme.styles.cardBg }}
            >
              <div className="flex items-center gap-2">
                <Cpu className={`w-5 h-5 ${currentTheme.styles.accentText}`} />
                <h3 className="text-sm font-bold text-white">Технологический стек и поддерживаемые протоколы (12 режимов)</h3>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3.5 pt-1">
                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-sky-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-sky-400" />
                    AWG + QUIC Pre-Warm (Спецпротокол)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Отправляет 20-байтный пакет WebRTC STUN Binding Request (RFC 5389) и 1200-байтный QUIC v1 Initial (RFC 9000) с рабочего UDP-порта за 15 мс до старта AmneziaWG для предварительной инициализации UDP-сессии.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-fuchsia-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-fuchsia-400" />
                    Гибрид AWG + Reality (Спецпротокол)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Двухслойная инкапсуляция (<code>detour</code>): рукопожатие и датаграммы AmneziaWG / WireGuard упаковываются внутрь TLS 1.3 соединения VLESS Reality (TCP 443, <code>xtls-rprx-vision</code> + <code>xudp</code>).
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-emerald-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-emerald-400" />
                    AmneziaWG 3.1 & 2.0
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Шифрование заголовков ChaCha20 (HPK), настоящая 32-битная рандомизация <code>H1–H4</code>, неколлидирующие сдвиги <code>S1–S4</code>, мусорные пакеты <code>Jc</code>, случайный паддинг и совместимость с чистым AWG 2.0.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-cyan-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-cyan-400" />
                    VLESS Reality (Vision)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Нативная TLS 1.3 верификация с использованием профиля браузера Chrome uTLS и высокой производительностью без необходимости настраивать собственный домен.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-pink-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-pink-400" />
                    ShadowTLS v3 & CDN Shield (WS)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Протокол верификации рукопожатия L7 с защитой от внешнего сканирования (ShadowTLS v3) и маршрутизация через глобальную сеть Cloudflare Edge / Workers (VLESS WebSocket).
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-lime-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-lime-400" />
                    WebRTC Steganography (gRPC)
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Передача мультиплексированных потоков данных в формате мультимедийных пакетов реального времени поверх HTTP/2 gRPC с минимальной задержкой.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-purple-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-purple-400" />
                    Hysteria 2 & TUIC v5
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Ультраскоростные UDP QUIC-протоколы с контролем перегрузок BBR, обфускацией Salamander, Port Hopping (20000–65000 + порт 53) и 0-RTT рукопожатием.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-amber-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-amber-400" />
                    Shadowsocks 2022 & SSH Tunnel
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Криптографический стандарт AEAD <code>2022-blake3-aes-gcm</code> с мультиплексированием и прямой зашифрованный SSH-туннель (порт 22) с поддержкой каскадного проксирования.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 space-y-1.5">
                  <div className="flex items-center gap-2 text-teal-400 font-bold text-xs">
                    <span className="w-2 h-2 rounded-full bg-teal-400" />
                    Драйвер WinTun L3 & System Proxy
                  </div>
                  <p className="text-[11px] text-slate-300 leading-relaxed">
                    Два режима перехвата трафика: виртуальный сетевой адаптер уровня ядра <code>wintun.dll</code> (для всех игр и UDP-приложений) и легкий системный прокси Windows.
                  </p>
                </div>
              </div>
            </div>

            {/* Developer Card (MoDiE) */}
            <div
              className={`p-6 rounded-2xl border soft-edge-block transition-all duration-300 space-y-4 ${currentTheme.styles.cardBorder}`}
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
                      Разработка архитектуры клиента, сетевых протоколов и интерфейса NeuroVeil VPN
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

            {/* Updates Section */}
            <div
              className={`p-5 rounded-2xl border soft-edge-block transition-all duration-300 space-y-3.5 ${currentTheme.styles.cardBorder}`}
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
                      Автоматическая проверка и установка новых официальных релизов клиента
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
                      NeuroVeil VPN v{updateInfo?.current_version || APP_VERSION} (Build {APP_BUILD})
                    </span>
                  </div>
                  <div className="text-[11px] text-slate-400">
                    Канал обновлений: Официальный релизный канал (Stable)
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
                    {checkingUpdate ? "Проверка обновлений..." : "Проверить обновления"}
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
                          title="Скачать заново и переустановить актуальную сборку"
                        >
                          <Download className="w-3 h-3 text-cyan-400" />
                          {installingUpdate ? "Установка..." : "Переустановить сборку"}
                        </button>
                      )}
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

      </main>
    </div>
  );
}

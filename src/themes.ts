export interface ThemeVars {
  bgApp: string;
  bgSidebar: string;
  bgCard: string;
  bgCardAlt: string;
  bgInput: string;
  borderCard: string;
  colorAccent: string;
  colorAccentBorder: string;
  colorAccentBg: string;
  colorAccentGlow: string;
  btnGradFrom: string;
  btnGradTo: string;
  btnGradFromHover: string;
  btnGradToHover: string;
  navGradFrom: string;
  navGradTo: string;
}

export interface ThemeStyles {
  appBg: string;
  sidebarBg: string;
  topbarBg: string;
  cardBg: string;
  cardBorder: string;
  cardAltBg: string;
  accentText: string;
  accentGlow: string;
  buttonPrimary: string;
  activeNavGradient: string;
  brandGlow: string;
}

export interface Theme {
  id: string;
  name: string;
  description: string;
  badge: string;
  preview: {
    bg: string;
    card: string;
    accent: string;
    secondary: string;
  };
  vars: ThemeVars;
  styles: ThemeStyles;
}

function getThemeStyles(id: string, vars: ThemeVars): ThemeStyles {
  switch (id) {
    case "matrix_neural":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-emerald-400",
        accentGlow: "shadow-emerald-500/25",
        buttonPrimary: "bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white shadow-lg shadow-emerald-900/30",
        activeNavGradient: "bg-gradient-to-r from-emerald-500/20 to-teal-500/10 text-emerald-300 border border-emerald-500/30 shadow-sm",
        brandGlow: "shadow-emerald-500/20 border-emerald-500/40",
      };
    case "cyberpunk_neon":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-rose-400",
        accentGlow: "shadow-rose-500/25",
        buttonPrimary: "bg-gradient-to-r from-rose-600 to-pink-600 hover:from-rose-500 hover:to-pink-500 text-white shadow-lg shadow-rose-900/30",
        activeNavGradient: "bg-gradient-to-r from-rose-500/20 to-pink-500/10 text-rose-300 border border-rose-500/30 shadow-sm",
        brandGlow: "shadow-rose-500/20 border-rose-500/40",
      };
    case "midnight_ocean":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-sky-400",
        accentGlow: "shadow-sky-500/25",
        buttonPrimary: "bg-gradient-to-r from-blue-600 to-sky-600 hover:from-blue-500 hover:to-sky-500 text-white shadow-lg shadow-blue-900/30",
        activeNavGradient: "bg-gradient-to-r from-sky-500/20 to-blue-500/10 text-sky-300 border border-sky-500/30 shadow-sm",
        brandGlow: "shadow-sky-500/20 border-sky-500/40",
      };
    case "amoled_black":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-slate-200",
        accentGlow: "shadow-white/10",
        buttonPrimary: "bg-gradient-to-r from-zinc-700 to-zinc-800 hover:from-zinc-600 hover:to-zinc-700 text-white border border-zinc-600 shadow-md",
        activeNavGradient: "bg-gradient-to-r from-zinc-700/40 to-zinc-800/30 text-white border border-zinc-500/40 shadow-sm",
        brandGlow: "shadow-white/10 border-zinc-500/40",
      };
    case "crimson_red":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-red-400",
        accentGlow: "shadow-red-500/25",
        buttonPrimary: "bg-gradient-to-r from-red-600 to-rose-700 hover:from-red-500 hover:to-rose-600 text-white shadow-lg shadow-red-900/30",
        activeNavGradient: "bg-gradient-to-r from-red-500/20 to-orange-500/10 text-red-300 border border-red-500/30 shadow-sm",
        brandGlow: "shadow-red-500/20 border-red-500/40",
      };
    case "solar_amber":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-amber-400",
        accentGlow: "shadow-amber-500/25",
        buttonPrimary: "bg-gradient-to-r from-amber-600 to-orange-600 hover:from-amber-500 hover:to-orange-500 text-white shadow-lg shadow-amber-900/30",
        activeNavGradient: "bg-gradient-to-r from-amber-500/20 to-orange-500/10 text-amber-300 border border-amber-500/30 shadow-sm",
        brandGlow: "shadow-amber-500/20 border-amber-500/40",
      };
    case "synthwave_violet":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-purple-400",
        accentGlow: "shadow-purple-500/25",
        buttonPrimary: "bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-500 hover:to-indigo-500 text-white shadow-lg shadow-purple-900/30",
        activeNavGradient: "bg-gradient-to-r from-purple-500/20 to-indigo-500/10 text-purple-300 border border-purple-500/30 shadow-sm",
        brandGlow: "shadow-purple-500/20 border-purple-500/40",
      };
    case "nordic_frost":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-sky-600",
        accentGlow: "shadow-sky-500/20",
        buttonPrimary: "bg-gradient-to-r from-sky-600 to-blue-600 hover:from-sky-500 hover:to-blue-500 !text-white font-semibold shadow-md",
        activeNavGradient: "bg-gradient-to-r from-sky-500/20 to-blue-500/15 text-sky-800 font-semibold border border-sky-500/40 shadow-sm",
        brandGlow: "shadow-sky-500/20 border-sky-500/40",
      };
    case "cyber_quartz":
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-violet-600",
        accentGlow: "shadow-violet-500/20",
        buttonPrimary: "bg-gradient-to-r from-violet-600 to-indigo-600 hover:from-violet-500 hover:to-indigo-500 !text-white font-semibold shadow-md",
        activeNavGradient: "bg-gradient-to-r from-violet-500/20 to-indigo-500/15 text-violet-800 font-semibold border border-violet-500/40 shadow-sm",
        brandGlow: "shadow-violet-500/20 border-violet-500/40",
      };
    default:
      return {
        appBg: vars.bgApp,
        sidebarBg: vars.bgSidebar,
        topbarBg: vars.bgSidebar,
        cardBg: vars.bgCard,
        cardBorder: vars.borderCard,
        cardAltBg: vars.bgCardAlt,
        accentText: "text-cyan-400",
        accentGlow: "shadow-cyan-500/25",
        buttonPrimary: "bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-white shadow-lg",
        activeNavGradient: "bg-gradient-to-r from-cyan-500/20 to-indigo-500/10 text-cyan-300 border border-cyan-500/30 shadow-sm",
        brandGlow: "shadow-cyan-500/20 border-cyan-500/40",
      };
  }
}

function createTheme(
  id: string,
  name: string,
  description: string,
  badge: string,
  preview: { bg: string; card: string; accent: string; secondary: string },
  vars: ThemeVars
): Theme {
  return {
    id,
    name,
    description,
    badge,
    preview,
    vars,
    styles: getThemeStyles(id, vars),
  };
}

export const THEMES: Theme[] = [
  createTheme(
    "cyber_obsidian",
    "Cyber Obsidian",
    "Темный обсидиановый космос со светящимися неоново-голубыми синапсами",
    "По умолчанию",
    {
      bg: "#090d16",
      card: "#11192e",
      accent: "#06b6d4",
      secondary: "#6366f1",
    },
    {
      bgApp: "#090d16",
      bgSidebar: "#0d1322",
      bgCard: "#11192e",
      bgCardAlt: "#060911",
      bgInput: "#0b101d",
      borderCard: "rgba(30, 41, 59, 0.8)",
      colorAccent: "#06b6d4",
      colorAccentBorder: "rgba(6, 182, 212, 0.35)",
      colorAccentBg: "rgba(6, 182, 212, 0.15)",
      colorAccentGlow: "rgba(6, 182, 212, 0.3)",
      btnGradFrom: "#0891b2",
      btnGradTo: "#4f46e5",
      btnGradFromHover: "#06b6d4",
      btnGradToHover: "#6366f1",
      navGradFrom: "rgba(6, 182, 212, 0.2)",
      navGradTo: "rgba(99, 102, 241, 0.1)",
    }
  ),
  createTheme(
    "matrix_neural",
    "Emerald Matrix",
    "Хакерская цифровая матрица с изумрудно-мятными неоновыми трассами",
    "Хакерский",
    {
      bg: "#040a07",
      card: "#0c2116",
      accent: "#10b981",
      secondary: "#14b8a6",
    },
    {
      bgApp: "#040a07",
      bgSidebar: "#07150e",
      bgCard: "#0c2116",
      bgCardAlt: "#030c08",
      bgInput: "#06180f",
      borderCard: "rgba(16, 185, 129, 0.25)",
      colorAccent: "#10b981",
      colorAccentBorder: "rgba(16, 185, 129, 0.4)",
      colorAccentBg: "rgba(16, 185, 129, 0.15)",
      colorAccentGlow: "rgba(16, 185, 129, 0.3)",
      btnGradFrom: "#059669",
      btnGradTo: "#0d9488",
      btnGradFromHover: "#10b981",
      btnGradToHover: "#14b8a6",
      navGradFrom: "rgba(16, 185, 129, 0.25)",
      navGradTo: "rgba(20, 184, 166, 0.12)",
    }
  ),
  createTheme(
    "cyberpunk_neon",
    "Cyberpunk 2077",
    "Неоновый ночной мегаполис: горячий розовый и ультрафиолет",
    "Яркий Неон",
    {
      bg: "#0d0514",
      card: "#220c35",
      accent: "#f43f5e",
      secondary: "#ec4899",
    },
    {
      bgApp: "#0d0514",
      bgSidebar: "#160822",
      bgCard: "#220c35",
      bgCardAlt: "#0a030f",
      bgInput: "#180827",
      borderCard: "rgba(244, 63, 94, 0.28)",
      colorAccent: "#f43f5e",
      colorAccentBorder: "rgba(244, 63, 94, 0.45)",
      colorAccentBg: "rgba(244, 63, 94, 0.15)",
      colorAccentGlow: "rgba(244, 63, 94, 0.35)",
      btnGradFrom: "#e11d48",
      btnGradTo: "#db2777",
      btnGradFromHover: "#f43f5e",
      btnGradToHover: "#ec4899",
      navGradFrom: "rgba(244, 63, 94, 0.25)",
      navGradTo: "rgba(219, 39, 119, 0.12)",
    }
  ),
  createTheme(
    "midnight_ocean",
    "Midnight Ocean",
    "Королевский полуночный сапфир и океанический глубокий синий",
    "Спокойный",
    {
      bg: "#050c1c",
      card: "#0e2047",
      accent: "#38bdf8",
      secondary: "#3b82f6",
    },
    {
      bgApp: "#050c1c",
      bgSidebar: "#08142e",
      bgCard: "#0e2047",
      bgCardAlt: "#030712",
      bgInput: "#0a1736",
      borderCard: "rgba(59, 130, 246, 0.28)",
      colorAccent: "#38bdf8",
      colorAccentBorder: "rgba(56, 189, 248, 0.4)",
      colorAccentBg: "rgba(56, 189, 248, 0.15)",
      colorAccentGlow: "rgba(56, 189, 248, 0.3)",
      btnGradFrom: "#2563eb",
      btnGradTo: "#0284c7",
      btnGradFromHover: "#3b82f6",
      btnGradToHover: "#38bdf8",
      navGradFrom: "rgba(56, 189, 248, 0.25)",
      navGradTo: "rgba(37, 99, 235, 0.12)",
    }
  ),
  createTheme(
    "amoled_black",
    "Pure AMOLED Black",
    "Идеально черный фон без лишнего света для OLED-экранов",
    "OLED Minimal",
    {
      bg: "#000000",
      card: "#141418",
      accent: "#e2e8f0",
      secondary: "#71717a",
    },
    {
      bgApp: "#000000",
      bgSidebar: "#09090b",
      bgCard: "#141418",
      bgCardAlt: "#040405",
      bgInput: "#0e0e12",
      borderCard: "rgba(255, 255, 255, 0.12)",
      colorAccent: "#e2e8f0",
      colorAccentBorder: "rgba(255, 255, 255, 0.25)",
      colorAccentBg: "rgba(255, 255, 255, 0.1)",
      colorAccentGlow: "rgba(255, 255, 255, 0.2)",
      btnGradFrom: "#3f3f46",
      btnGradTo: "#27272a",
      btnGradFromHover: "#52525b",
      btnGradToHover: "#3f3f46",
      navGradFrom: "rgba(255, 255, 255, 0.14)",
      navGradTo: "rgba(255, 255, 255, 0.05)",
    }
  ),
  createTheme(
    "crimson_red",
    "Crimson Protocol",
    "Тактический багрово-рубиновый стиль с огненными акцентами",
    "Тактический",
    {
      bg: "#120507",
      card: "#280a0e",
      accent: "#ef4444",
      secondary: "#f97316",
    },
    {
      bgApp: "#120507",
      bgSidebar: "#1c070a",
      bgCard: "#280a0e",
      bgCardAlt: "#0a0204",
      bgInput: "#1f070a",
      borderCard: "rgba(239, 68, 68, 0.28)",
      colorAccent: "#ef4444",
      colorAccentBorder: "rgba(239, 68, 68, 0.45)",
      colorAccentBg: "rgba(239, 68, 68, 0.15)",
      colorAccentGlow: "rgba(239, 68, 68, 0.35)",
      btnGradFrom: "#dc2626",
      btnGradTo: "#ea580c",
      btnGradFromHover: "#ef4444",
      btnGradToHover: "#f97316",
      navGradFrom: "rgba(239, 68, 68, 0.25)",
      navGradTo: "rgba(234, 88, 12, 0.12)",
    }
  ),
  createTheme(
    "solar_amber",
    "Solar Amber",
    "Благородный темный графит с теплым золотистым и янтарным сиянием",
    "Теплый Янтарь",
    {
      bg: "#0c0a07",
      card: "#1a160e",
      accent: "#f59e0b",
      secondary: "#d97706",
    },
    {
      bgApp: "#0c0a07",
      bgSidebar: "#14110b",
      bgCard: "#1a160e",
      bgCardAlt: "#070604",
      bgInput: "#18140c",
      borderCard: "rgba(245, 158, 11, 0.28)",
      colorAccent: "#f59e0b",
      colorAccentBorder: "rgba(245, 158, 11, 0.45)",
      colorAccentBg: "rgba(245, 158, 11, 0.15)",
      colorAccentGlow: "rgba(245, 158, 11, 0.35)",
      btnGradFrom: "#d97706",
      btnGradTo: "#b45309",
      btnGradFromHover: "#f59e0b",
      btnGradToHover: "#d97706",
      navGradFrom: "rgba(245, 158, 11, 0.25)",
      navGradTo: "rgba(217, 119, 6, 0.12)",
    }
  ),
  createTheme(
    "synthwave_violet",
    "Synthwave Violet",
    "Ретро-футуристический глубокий индиго с неоновым фиолетовым свечением",
    "Неоновый Индиго",
    {
      bg: "#0b0816",
      card: "#19122e",
      accent: "#a855f7",
      secondary: "#8b5cf6",
    },
    {
      bgApp: "#0b0816",
      bgSidebar: "#110c22",
      bgCard: "#19122e",
      bgCardAlt: "#07050f",
      bgInput: "#16102a",
      borderCard: "rgba(168, 85, 247, 0.28)",
      colorAccent: "#a855f7",
      colorAccentBorder: "rgba(168, 85, 247, 0.45)",
      colorAccentBg: "rgba(168, 85, 247, 0.15)",
      colorAccentGlow: "rgba(168, 85, 247, 0.35)",
      btnGradFrom: "#9333ea",
      btnGradTo: "#6366f1",
      btnGradFromHover: "#a855f7",
      btnGradToHover: "#818cf8",
      navGradFrom: "rgba(168, 85, 247, 0.25)",
      navGradTo: "rgba(99, 102, 241, 0.12)",
    }
  ),
  createTheme(
    "nordic_frost",
    "Nordic Frost (Светлая)",
    "Кристально чистый скандинавский светлый стиль с лазурными акцентами",
    "Светлый Минимал",
    {
      bg: "#eef2f6",
      card: "#ffffff",
      accent: "#0284c7",
      secondary: "#2563eb",
    },
    {
      bgApp: "#eef2f6",
      bgSidebar: "#e2e8f0",
      bgCard: "#ffffff",
      bgCardAlt: "#f8fafc",
      bgInput: "#e2e8f0",
      borderCard: "rgba(203, 213, 225, 0.9)",
      colorAccent: "#0284c7",
      colorAccentBorder: "rgba(2, 132, 199, 0.4)",
      colorAccentBg: "rgba(2, 132, 199, 0.12)",
      colorAccentGlow: "rgba(2, 132, 199, 0.25)",
      btnGradFrom: "#0284c7",
      btnGradTo: "#2563eb",
      btnGradFromHover: "#0369a1",
      btnGradToHover: "#1d4ed8",
      navGradFrom: "rgba(2, 132, 199, 0.15)",
      navGradTo: "rgba(37, 99, 235, 0.08)",
    }
  ),
  createTheme(
    "cyber_quartz",
    "Cyber Quartz (Светлая)",
    "Футуристический жемчужный кварц с неоновыми фиолетовыми линиями",
    "Кварцевый Свет",
    {
      bg: "#f3f0f7",
      card: "#ffffff",
      accent: "#7c3aed",
      secondary: "#6366f1",
    },
    {
      bgApp: "#f3f0f7",
      bgSidebar: "#eae4f2",
      bgCard: "#ffffff",
      bgCardAlt: "#faf7fd",
      bgInput: "#eae4f2",
      borderCard: "rgba(216, 180, 254, 0.6)",
      colorAccent: "#7c3aed",
      colorAccentBorder: "rgba(124, 58, 237, 0.4)",
      colorAccentBg: "rgba(124, 58, 237, 0.12)",
      colorAccentGlow: "rgba(124, 58, 237, 0.25)",
      btnGradFrom: "#7c3aed",
      btnGradTo: "#4f46e5",
      btnGradFromHover: "#6d28d9",
      btnGradToHover: "#4338ca",
      navGradFrom: "rgba(124, 58, 237, 0.15)",
      navGradTo: "rgba(99, 102, 241, 0.08)",
    }
  ),
];

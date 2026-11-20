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
  isLight?: boolean;
  preview: {
    bg: string;
    card: string;
    accent: string;
    secondary: string;
  };
  vars: ThemeVars;
  styles: ThemeStyles;
}

function buildStyles(vars: ThemeVars, isLight?: boolean): ThemeStyles {
  return {
    appBg: vars.bgApp,
    sidebarBg: vars.bgSidebar,
    topbarBg: vars.bgSidebar,
    cardBg: vars.bgCard,
    cardBorder: vars.borderCard,
    cardAltBg: vars.bgCardAlt,
    accentText: "text-cyan-400",
    accentGlow: "shadow-cyan-500/25",
    buttonPrimary: isLight
      ? "bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 !text-white font-semibold shadow-md"
      : "bg-gradient-to-r from-cyan-600 to-indigo-600 hover:from-cyan-500 hover:to-indigo-500 text-white shadow-lg",
    activeNavGradient:
      "bg-gradient-to-r from-cyan-500/20 to-indigo-500/10 text-cyan-300 border border-cyan-500/30 shadow-sm",
    brandGlow: "shadow-cyan-500/20 border-cyan-500/40",
  };
}

function createTheme(
  id: string,
  name: string,
  description: string,
  badge: string,
  preview: { bg: string; card: string; accent: string; secondary: string },
  vars: ThemeVars,
  isLight = false
): Theme {
  return {
    id,
    name,
    description,
    badge,
    isLight,
    preview,
    vars,
    styles: buildStyles(vars, isLight),
  };
}

export const THEMES: Theme[] = [
  createTheme(
    "obsidian_cyan",
    "Obsidian Cyan",
    "Глубокий нейтральный карбон с ледяными голубыми акцентами и свечением",
    "По умолчанию",
    {
      bg: "#090a0f",
      card: "#141720",
      accent: "#22d3ee",
      secondary: "#3b82f6",
    },
    {
      bgApp: "#090a0f",
      bgSidebar: "#0e1017",
      bgCard: "#141720",
      bgCardAlt: "#0b0d14",
      bgInput: "#10131c",
      borderCard: "rgba(148, 163, 184, 0.10)",
      colorAccent: "#22d3ee",
      colorAccentBorder: "rgba(34, 211, 238, 0.34)",
      colorAccentBg: "rgba(6, 182, 212, 0.14)",
      colorAccentGlow: "rgba(34, 211, 238, 0.32)",
      btnGradFrom: "#0891b2",
      btnGradTo: "#2563eb",
      btnGradFromHover: "#06b6d4",
      btnGradToHover: "#3b82f6",
      navGradFrom: "rgba(6, 182, 212, 0.18)",
      navGradTo: "rgba(37, 99, 235, 0.08)",
    }
  ),
  createTheme(
    "amoled_black",
    "Pure AMOLED Black",
    "Идеально черный OLED-фон с платиновыми гранями и мягким белым свечением",
    "OLED Minimal",
    {
      bg: "#000000",
      card: "#141418",
      accent: "#f4f4f5",
      secondary: "#71717a",
    },
    {
      bgApp: "#000000",
      bgSidebar: "#09090b",
      bgCard: "#141418",
      bgCardAlt: "#070709",
      bgInput: "#111115",
      borderCard: "rgba(255, 255, 255, 0.08)",
      colorAccent: "#f4f4f5",
      colorAccentBorder: "rgba(255, 255, 255, 0.28)",
      colorAccentBg: "rgba(255, 255, 255, 0.11)",
      colorAccentGlow: "rgba(255, 255, 255, 0.24)",
      btnGradFrom: "#52525b",
      btnGradTo: "#27272a",
      btnGradFromHover: "#71717a",
      btnGradToHover: "#3f3f46",
      navGradFrom: "rgba(255, 255, 255, 0.16)",
      navGradTo: "rgba(255, 255, 255, 0.05)",
    }
  ),
  createTheme(
    "carbon_emerald",
    "Carbon Emerald",
    "Матовый тёмный антрацит с чистыми изумрудно-мятными нотами",
    "Мятный Неон",
    {
      bg: "#090b0c",
      card: "#131718",
      accent: "#34d399",
      secondary: "#14b8a6",
    },
    {
      bgApp: "#090b0c",
      bgSidebar: "#0d1011",
      bgCard: "#131718",
      bgCardAlt: "#0b0e0f",
      bgInput: "#101415",
      borderCard: "rgba(148, 163, 184, 0.10)",
      colorAccent: "#34d399",
      colorAccentBorder: "rgba(52, 211, 153, 0.34)",
      colorAccentBg: "rgba(16, 185, 129, 0.14)",
      colorAccentGlow: "rgba(52, 211, 153, 0.30)",
      btnGradFrom: "#059669",
      btnGradTo: "#0d9488",
      btnGradFromHover: "#10b981",
      btnGradToHover: "#14b8a6",
      navGradFrom: "rgba(16, 185, 129, 0.18)",
      navGradTo: "rgba(20, 184, 166, 0.08)",
    }
  ),
  createTheme(
    "graphite_violet",
    "Graphite Amethyst",
    "Строгий графит с благородными аметистовыми акцентами и подсветкой",
    "Аметист",
    {
      bg: "#0a0a0e",
      card: "#15151e",
      accent: "#c084fc",
      secondary: "#818cf8",
    },
    {
      bgApp: "#0a0a0e",
      bgSidebar: "#0f0f15",
      bgCard: "#15151e",
      bgCardAlt: "#0c0c12",
      bgInput: "#12121a",
      borderCard: "rgba(161, 161, 170, 0.10)",
      colorAccent: "#c084fc",
      colorAccentBorder: "rgba(192, 132, 252, 0.34)",
      colorAccentBg: "rgba(168, 85, 247, 0.14)",
      colorAccentGlow: "rgba(192, 132, 252, 0.32)",
      btnGradFrom: "#9333ea",
      btnGradTo: "#4f46e5",
      btnGradFromHover: "#a855f7",
      btnGradToHover: "#6366f1",
      navGradFrom: "rgba(168, 85, 247, 0.18)",
      navGradTo: "rgba(99, 102, 241, 0.08)",
    }
  ),
  createTheme(
    "onyx_crimson",
    "Onyx Crimson",
    "Чёрный матовый оникс с яркими рубиново-коралловыми нотами",
    "Рубин",
    {
      bg: "#0a090a",
      card: "#161316",
      accent: "#fb7185",
      secondary: "#f43f5e",
    },
    {
      bgApp: "#0a090a",
      bgSidebar: "#100e10",
      bgCard: "#161316",
      bgCardAlt: "#0d0b0d",
      bgInput: "#131013",
      borderCard: "rgba(161, 161, 170, 0.10)",
      colorAccent: "#fb7185",
      colorAccentBorder: "rgba(251, 113, 133, 0.34)",
      colorAccentBg: "rgba(244, 63, 94, 0.14)",
      colorAccentGlow: "rgba(244, 63, 94, 0.32)",
      btnGradFrom: "#e11d48",
      btnGradTo: "#db2777",
      btnGradFromHover: "#f43f5e",
      btnGradToHover: "#ec4899",
      navGradFrom: "rgba(244, 63, 94, 0.18)",
      navGradTo: "rgba(219, 39, 119, 0.08)",
    }
  ),
  createTheme(
    "basalt_amber",
    "Basalt Gold",
    "Сдержанный тёмный базальт с тёплым янтарно-золотым свечением",
    "Золото",
    {
      bg: "#0b0a09",
      card: "#171512",
      accent: "#fbbf24",
      secondary: "#f97316",
    },
    {
      bgApp: "#0b0a09",
      bgSidebar: "#110f0d",
      bgCard: "#171512",
      bgCardAlt: "#0e0c0a",
      bgInput: "#14120f",
      borderCard: "rgba(161, 161, 170, 0.10)",
      colorAccent: "#fbbf24",
      colorAccentBorder: "rgba(251, 191, 36, 0.34)",
      colorAccentBg: "rgba(245, 158, 11, 0.14)",
      colorAccentGlow: "rgba(245, 158, 11, 0.30)",
      btnGradFrom: "#d97706",
      btnGradTo: "#ea580c",
      btnGradFromHover: "#f59e0b",
      btnGradToHover: "#f97316",
      navGradFrom: "rgba(245, 158, 11, 0.18)",
      navGradTo: "rgba(234, 88, 12, 0.08)",
    }
  ),
  createTheme(
    "alabaster_azure",
    "Alabaster Azure",
    "Чистый светлый алебастр с выразительными лазурно-синими акцентами",
    "Светлая",
    {
      bg: "#f4f6f9",
      card: "#ffffff",
      accent: "#0284c7",
      secondary: "#2563eb",
    },
    {
      bgApp: "#f4f6f9",
      bgSidebar: "#eaeef4",
      bgCard: "#ffffff",
      bgCardAlt: "#f8fafc",
      bgInput: "#eef2f7",
      borderCard: "rgba(148, 163, 184, 0.28)",
      colorAccent: "#0284c7",
      colorAccentBorder: "rgba(2, 132, 199, 0.38)",
      colorAccentBg: "rgba(2, 132, 199, 0.11)",
      colorAccentGlow: "rgba(2, 132, 199, 0.28)",
      btnGradFrom: "#0284c7",
      btnGradTo: "#2563eb",
      btnGradFromHover: "#0369a1",
      btnGradToHover: "#1d4ed8",
      navGradFrom: "rgba(2, 132, 199, 0.14)",
      navGradTo: "rgba(37, 99, 235, 0.06)",
    },
    true
  ),
  createTheme(
    "pearl_violet",
    "Pearl Lavender",
    "Светлый жемчужный фон с мягкими лавандово-фиолетовыми нотами",
    "Светлая",
    {
      bg: "#f5f4f8",
      card: "#ffffff",
      accent: "#7c3aed",
      secondary: "#4f46e5",
    },
    {
      bgApp: "#f5f4f8",
      bgSidebar: "#eceaf2",
      bgCard: "#ffffff",
      bgCardAlt: "#faf9fc",
      bgInput: "#f0edf6",
      borderCard: "rgba(148, 163, 184, 0.26)",
      colorAccent: "#7c3aed",
      colorAccentBorder: "rgba(124, 58, 237, 0.36)",
      colorAccentBg: "rgba(124, 58, 237, 0.11)",
      colorAccentGlow: "rgba(124, 58, 237, 0.26)",
      btnGradFrom: "#7c3aed",
      btnGradTo: "#4f46e5",
      btnGradFromHover: "#6d28d9",
      btnGradToHover: "#4338ca",
      navGradFrom: "rgba(124, 58, 237, 0.14)",
      navGradTo: "rgba(79, 70, 229, 0.06)",
    },
    true
  ),
  createTheme(
    "porcelain_emerald",
    "Porcelain Mint",
    "Лёгкий фарфоровый свет с освежающими нефритово-изумрудными акцентами",
    "Светлая",
    {
      bg: "#f3f7f5",
      card: "#ffffff",
      accent: "#059669",
      secondary: "#0d9488",
    },
    {
      bgApp: "#f3f7f5",
      bgSidebar: "#e8efec",
      bgCard: "#ffffff",
      bgCardAlt: "#f7fbf9",
      bgInput: "#ebf2ef",
      borderCard: "rgba(148, 163, 184, 0.28)",
      colorAccent: "#059669",
      colorAccentBorder: "rgba(5, 150, 105, 0.36)",
      colorAccentBg: "rgba(5, 150, 105, 0.11)",
      colorAccentGlow: "rgba(5, 150, 105, 0.26)",
      btnGradFrom: "#059669",
      btnGradTo: "#0d9488",
      btnGradFromHover: "#047857",
      btnGradToHover: "#0f766e",
      navGradFrom: "rgba(5, 150, 105, 0.14)",
      navGradTo: "rgba(13, 148, 136, 0.06)",
    },
    true
  ),
];

const LEGACY_THEME_MAP: Record<string, string> = {
  cyber_obsidian: "obsidian_cyan",
  cyber_midnight: "obsidian_cyan",
  midnight_ocean: "obsidian_cyan",
  matrix_neural: "carbon_emerald",
  synthwave_violet: "graphite_violet",
  cyberpunk_neon: "onyx_crimson",
  crimson_red: "onyx_crimson",
  solar_amber: "basalt_amber",
  nordic_frost: "alabaster_azure",
  cyber_quartz: "pearl_violet",
  monochrome_bw: "amoled_black",
  titanium_grey: "amoled_black",
};

export function resolveThemeId(themeId?: string | null): string {
  if (!themeId) return THEMES[0].id;
  if (THEMES.some((t) => t.id === themeId)) return themeId;
  return LEGACY_THEME_MAP[themeId] || THEMES[0].id;
}

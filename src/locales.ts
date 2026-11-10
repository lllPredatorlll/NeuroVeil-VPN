export type Language = "ru" | "en";

export interface Translations {
  // Navigation
  navDashboard: string;
  navVps: string;
  navProfiles: string;
  navRouting: string;
  navSettings: string;
  navAbout: string;
  badgeThemes: string;
  coreInstalled: string;
  coreRequired: string;

  // Window Controls
  minimize: string;
  maximize: string;
  close: string;

  // Settings Sections
  settingsTitle: string;
  settingsSubtitle: string;
  themesSectionTitle: string;
  themesSectionSubtitle: string;
  activeTheme: string;
  applyTheme: string;
  themeSelected: string;

  // System Settings Section
  systemSectionTitle: string;
  systemSectionSubtitle: string;
  autostartTitle: string;
  autostartDesc: string;
  startMinimizedTitle: string;
  startMinimizedDesc: string;
  closeToTrayTitle: string;
  closeToTrayDesc: string;
  autoConnectTitle: string;
  autoConnectDesc: string;

  // Language Section
  languageSectionTitle: string;
  languageSectionSubtitle: string;
  langRu: string;
  langEn: string;

  // About Section
  aboutTitle: string;
  aboutSubtitle: string;
  aboutAppDesc: string;
  specCore: string;
  specDriver: string;
  specProtocols: string;
  specSecurity: string;
  specPlatform: string;
  specArchitecture: string;

  // Core Section
  coreSectionTitle: string;
  coreSectionSubtitle: string;
  coreVersionLabel: string;
  coreDriverLabel: string;
  reinstallCoreBtn: string;
  clearLogsBtn: string;
  exportLogsBtn: string;
  copyLogsSuccess: string;
  splitTunnelingMasterTitle: string;
  splitTunnelingMasterDesc: string;
  splitTunnelingEnabledBadge: string;
  splitTunnelingDisabledBadge: string;
  splitTunnelingDisabledNotice: string;
  selectProfileForRouting: string;
  allTrafficViaVpn: string;
  manageRoutingLink: string;
}

export const LOCALES: Record<Language, Translations> = {
  ru: {
    navDashboard: "Подключение",
    navVps: "VPS Автодеплой",
    navProfiles: "Устройства и Ключи",
    navRouting: "Маршруты и Ядро",
    navSettings: "Настройки",
    navAbout: "О программе",
    badgeThemes: "Темы",
    coreInstalled: "Готово",
    coreRequired: "Не скачано",

    minimize: "Свернуть",
    maximize: "Развернуть",
    close: "Закрыть (в трей)",

    settingsTitle: "Настройки приложения и внешний вид",
    settingsSubtitle: "Настройте автозапуск, системный трей, темы оформления и язык интерфейса NeuroVeil VPN.",
    themesSectionTitle: "Темы оформления и цветовая гамма",
    themesSectionSubtitle: "Мгновенно меняет цветовую палитру, акценты, карточки и стиль окон клиента",
    activeTheme: "Активна",
    applyTheme: "Применить тему",
    themeSelected: "Выбрана",

    systemSectionTitle: "Автозапуск и поведение в системе",
    systemSectionSubtitle: "Параметры запуска вместе с Windows, системный трей и автоматическое подключение",
    autostartTitle: "Автозапуск при старте Windows",
    autostartDesc: "Запускать NeuroVeil VPN автоматически при входе в систему через планировщик задач",
    startMinimizedTitle: "Запускать свернутым в трей",
    startMinimizedDesc: "При старте системы не открывать главное окно, а работать в фоновом режиме в трее",
    closeToTrayTitle: "Сворачивать в трей при закрытии (✕)",
    closeToTrayDesc: "При нажатии на крестик окно скрывается в трей и VPN продолжает работать",
    autoConnectTitle: "Автоподключение к VPN при запуске",
    autoConnectDesc: "Автоматически поднимать VPN-соединение к активному профилю сразу после старта",

    languageSectionTitle: "Язык интерфейса (Language)",
    languageSectionSubtitle: "Выберите предпочитаемый язык для меню, кнопок и уведомлений приложения",
    langRu: "Русский (Russian)",
    langEn: "English (Английский)",

    aboutTitle: "О программе NeuroVeil VPN",
    aboutSubtitle: "Технологический стек, архитектура безопасности и системная информация",
    aboutAppDesc: "NeuroVeil VPN — высокоскоростной защищенный клиент нового поколения на базе ядра sing-box с поддержкой современных протоколов AmneziaWG 3.1, VLESS Reality, Hysteria 2 и TUIC v5 с адаптивной защитой от блокировок и ТСПУ.",
    specCore: "Ядро маршрутизации: sing-box v1.14.4 (Go Runtime)",
    specDriver: "Виртуальный адаптер: WinTun Layer-3 High-Performance Driver (wintun.dll)",
    specProtocols: "Протоколы: Hysteria 2 (Salamander Obfs) & TUIC v5 (BBR Congestion)",
    specSecurity: "Безопасность: ECDSA prime256v1 TLS, Zero-Logs, Root SSH Inbound Auth",
    specPlatform: "Архитектура клиента: Tauri v2 (Rust Backend + React 18 + Vite)",
    specArchitecture: "Окружение: Windows x86_64 (Режим администратора)",

    coreSectionTitle: "Системные службы и компоненты ядра",
    coreSectionSubtitle: "Управление исполняемым файлом sing-box и драйвером сетевого адаптера WinTun",
    coreVersionLabel: "Версия ядра sing-box:",
    coreDriverLabel: "Драйвер виртуального адаптера:",
    reinstallCoreBtn: "Переустановить / Обновить sing-box",
    clearLogsBtn: "Очистить журнал",
    exportLogsBtn: "Копировать журнал в файл",
    copyLogsSuccess: "Журнал ядра успешно сохранен в файл!",
    splitTunnelingMasterTitle: "Раздельное туннелирование для профиля",
    splitTunnelingMasterDesc: "При выключении: весь трафик ПК направляется через VPN. При включении: активируются списки программ, сайтов и прямое подключение к RU-сервисам.",
    splitTunnelingEnabledBadge: "Раздельное туннелирование включено",
    splitTunnelingDisabledBadge: "Выключено (Весь трафик идет через VPN)",
    splitTunnelingDisabledNotice: "Раздельное туннелирование отключено для этого профиля. Все соединения и программы направляются через VPN без исключений. Включите тумблер выше, чтобы активировать правила раздельного туннелирования.",
    selectProfileForRouting: "Профиль для настройки маршрутов:",
    allTrafficViaVpn: "Весь трафик ПК через VPN без разделения",
    manageRoutingLink: "Настроить раздельное туннелирование →",
  },
  en: {
    navDashboard: "Connection",
    navVps: "VPS Auto-deploy",
    navProfiles: "Devices & Keys",
    navRouting: "Routing & Core",
    navSettings: "Settings",
    navAbout: "About NeuroVeil",
    badgeThemes: "Themes",
    coreInstalled: "Ready",
    coreRequired: "Not Installed",

    minimize: "Minimize",
    maximize: "Maximize",
    close: "Close (to Tray)",

    settingsTitle: "Application Settings & Appearance",
    settingsSubtitle: "Configure autostart, system tray, visual themes and interface language of NeuroVeil VPN.",
    themesSectionTitle: "Visual Themes & Color Palette",
    themesSectionSubtitle: "Instantly switches color palette, accent highlights, cards and window style",
    activeTheme: "Active",
    applyTheme: "Apply Theme",
    themeSelected: "Selected",

    systemSectionTitle: "Autostart & System Behavior",
    systemSectionSubtitle: "Windows startup parameters, system tray minimization and auto-connection",
    autostartTitle: "Start with Windows",
    autostartDesc: "Launch NeuroVeil VPN automatically on user logon via Windows Task Scheduler",
    startMinimizedTitle: "Start Minimized to System Tray",
    startMinimizedDesc: "Run in the background in system tray without opening the main window on boot",
    closeToTrayTitle: "Minimize to Tray on Close (✕)",
    closeToTrayDesc: "Clicking the close button hides window into system tray while VPN stays active",
    autoConnectTitle: "Auto-connect to VPN on Launch",
    autoConnectDesc: "Automatically establish VPN tunnel to the active profile immediately after launch",

    languageSectionTitle: "Interface Language",
    languageSectionSubtitle: "Select your preferred language for menus, buttons and notifications",
    langRu: "Русский (Russian)",
    langEn: "English (US)",

    aboutTitle: "About NeuroVeil VPN",
    aboutSubtitle: "Technology stack, security architecture and system specifications",
    aboutAppDesc: "NeuroVeil VPN is a high-performance, next-generation secure proxy client built on top of the sing-box core, powered by AmneziaWG 3.1, VLESS Reality, Hysteria 2 and TUIC v5 with anti-censorship obfuscation.",
    specCore: "Routing Engine: sing-box v1.14.4 (Go Runtime)",
    specDriver: "Virtual Adapter: WinTun Layer-3 High-Performance Driver (wintun.dll)",
    specProtocols: "Protocols: Hysteria 2 (Salamander Obfs) & TUIC v5 (BBR Congestion)",
    specSecurity: "Security: ECDSA prime256v1 TLS, Zero-Logs, Root SSH Inbound Auth",
    specPlatform: "Client Framework: Tauri v2 (Rust Backend + React 18 + Vite)",
    specArchitecture: "Environment: Windows x86_64 (Elevated Administrator Mode)",

    coreSectionTitle: "System Services & Core Components",
    coreSectionSubtitle: "Manage sing-box executable binary and WinTun virtual network adapter",
    coreVersionLabel: "sing-box Core Version:",
    coreDriverLabel: "Virtual Adapter Driver:",
    reinstallCoreBtn: "Reinstall / Update sing-box",
    clearLogsBtn: "Clear Log History",
    exportLogsBtn: "Export Logs to File",
    copyLogsSuccess: "Core log successfully exported to file!",
    splitTunnelingMasterTitle: "Split Tunneling for Profile",
    splitTunnelingMasterDesc: "When disabled: all PC traffic is strictly routed through VPN. When enabled: custom application rules, domain rules and bypasses are active.",
    splitTunnelingEnabledBadge: "Split Tunneling Active",
    splitTunnelingDisabledBadge: "Disabled (All Traffic via VPN)",
    splitTunnelingDisabledNotice: "Split tunneling is disabled for this profile. All network traffic goes directly through VPN without exceptions. Enable the toggle above to activate split routing rules.",
    selectProfileForRouting: "Profile to configure routes:",
    allTrafficViaVpn: "All PC traffic routed via VPN without splitting",
    manageRoutingLink: "Configure split tunneling →",
  },
};

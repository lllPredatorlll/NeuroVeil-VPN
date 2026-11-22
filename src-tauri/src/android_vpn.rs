#![cfg(target_os = "android")]

use crate::models::RunningProcessInfo;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    AppHandle, Manager, Runtime,
};

pub struct AndroidVpnHandle<R: Runtime>(pub PluginHandle<R>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartVpnPayload {
    config_json: String,
    profile_name: String,
    profile_id: String,
    protocol: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileVpnStatus {
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub connecting: bool,
    #[serde(default)]
    pub socks_port: u16,
    #[serde(default)]
    pub up_bytes: u64,
    #[serde(default)]
    pub down_bytes: u64,
    #[serde(default)]
    pub up_speed: u64,
    #[serde(default)]
    pub down_speed: u64,
    #[serde(default)]
    pub logs: Vec<String>,
    #[serde(default)]
    pub libbox_version: String,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartVpnResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub connecting: bool,
    #[serde(default)]
    pub socks_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatteryOptResult {
    #[serde(default)]
    pub ignored: bool,
    #[serde(default)]
    pub requested: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickTileResult {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledAppsResp {
    #[serde(default)]
    apps: Vec<InstalledAppItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledAppItem {
    name: String,
    package_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PickFilePayload {
    mime_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFileResult {
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub content_base64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveFilePayload {
    filename: String,
    mime_type: String,
    content_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveFileResp {
    #[serde(default)]
    cancelled: bool,
    #[serde(default)]
    saved_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct OpenUrlPayload {
    url: String,
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("nv-vpn-bridge")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.neuroveil.vpn2", "VpnBridgePlugin")?;
            app.manage(AndroidVpnHandle(handle));
            Ok(())
        })
        .build()
}

pub fn start_vpn<R: Runtime>(
    app: &AppHandle<R>,
    config_json: String,
    profile_name: String,
    profile_id: String,
    protocol: String,
) -> Result<StartVpnResult, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    state
        .0
        .run_mobile_plugin::<StartVpnResult>(
            "startVpn",
            StartVpnPayload {
                config_json,
                profile_name,
                profile_id,
                protocol,
            },
        )
        .map_err(|e| format!("Android VPN error: {}", e))
}

pub fn is_battery_optimization_ignored<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let res: BatteryOptResult = state
        .0
        .run_mobile_plugin("isBatteryOptimizationIgnored", serde_json::json!({}))
        .map_err(|e| format!("Battery optimization check error: {}", e))?;
    Ok(res.ignored)
}

pub fn request_battery_optimization_exemption<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<BatteryOptResult, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    state
        .0
        .run_mobile_plugin::<BatteryOptResult>(
            "requestBatteryOptimizationExemption",
            serde_json::json!({}),
        )
        .map_err(|e| format!("Ошибка открытия настроек оптимизации батареи: {}", e))
}

pub fn stop_vpn<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let _: serde_json::Value = state
        .0
        .run_mobile_plugin("stopVpn", serde_json::json!({}))
        .map_err(|e| format!("Android VPN stop error: {}", e))?;
    Ok(())
}

pub fn get_vpn_status<R: Runtime>(app: &AppHandle<R>) -> Result<MobileVpnStatus, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    state
        .0
        .run_mobile_plugin::<MobileVpnStatus>("getVpnStatus", serde_json::json!({}))
        .map_err(|e| format!("Android status error: {}", e))
}

pub fn request_add_quick_tile<R: Runtime>(app: &AppHandle<R>) -> Result<QuickTileResult, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    state
        .0
        .run_mobile_plugin::<QuickTileResult>("requestAddQuickTile", serde_json::json!({}))
        .map_err(|e| format!("Ошибка добавления значка быстрых настроек: {}", e))
}

pub fn sync_tile_profile<R: Runtime>(
    app: &AppHandle<R>,
    config_json: String,
    profile_name: String,
    profile_id: String,
    protocol: String,
) -> Result<(), String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let _: serde_json::Value = state
        .0
        .run_mobile_plugin(
            "syncTileProfile",
            StartVpnPayload {
                config_json,
                profile_name,
                profile_id,
                protocol,
            },
        )
        .map_err(|e| format!("Android tile sync error: {}", e))?;
    Ok(())
}

pub fn clear_logs<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let _: serde_json::Value = state
        .0
        .run_mobile_plugin("clearLogs", serde_json::json!({}))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_installed_apps<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<RunningProcessInfo>, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let resp: InstalledAppsResp = state
        .0
        .run_mobile_plugin("getInstalledApps", serde_json::json!({}))
        .map_err(|e| format!("Ошибка получения списка приложений: {}", e))?;
    Ok(resp
        .apps
        .into_iter()
        .map(|a| RunningProcessInfo {
            name: a.name,
            path: a.package_name,
        })
        .collect())
}

pub fn pick_file_bytes<R: Runtime>(
    app: &AppHandle<R>,
    mime_types: Vec<String>,
) -> Result<Option<(String, Vec<u8>)>, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let res: PickedFileResult = state
        .0
        .run_mobile_plugin("pickFile", PickFilePayload { mime_types })
        .map_err(|e| format!("Ошибка выбора файла: {}", e))?;
    if res.cancelled || res.content_base64.is_empty() {
        return Ok(None);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(res.content_base64.trim())
        .map_err(|e| format!("Ошибка декодирования файла: {}", e))?;
    Ok(Some((res.name, bytes)))
}

pub fn save_file_bytes<R: Runtime>(
    app: &AppHandle<R>,
    filename: String,
    mime_type: String,
    bytes: &[u8],
) -> Result<Option<String>, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let res: SaveFileResp = state
        .0
        .run_mobile_plugin(
            "saveFile",
            SaveFilePayload {
                filename,
                mime_type,
                content_base64: b64,
            },
        )
        .map_err(|e| format!("Ошибка сохранения файла: {}", e))?;
    if res.cancelled || res.saved_path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(res.saved_path))
    }
}

pub fn open_url<R: Runtime>(app: &AppHandle<R>, url: String) -> Result<(), String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    let _: serde_json::Value = state
        .0
        .run_mobile_plugin("openUrl", OpenUrlPayload { url })
        .map_err(|e| format!("Не удалось открыть ссылку: {}", e))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallApkPayload {
    apk_path: String,
}

pub fn install_apk<R: Runtime>(app: &AppHandle<R>, apk_path: String) -> Result<QuickTileResult, String> {
    let state = app.state::<AndroidVpnHandle<R>>();
    state
        .0
        .run_mobile_plugin::<QuickTileResult>("installApk", InstallApkPayload { apk_path })
        .map_err(|e| format!("Не удалось запустить установщик APK: {}", e))
}

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_dialog::DialogExt;
use tempfile::NamedTempFile;
use toml_edit::{value, DocumentMut};
use uuid::Uuid;

const APP_DIR: &str = "codex-key-manager";
const PROFILES_FILE: &str = "profiles.json";
const DATA_VERSION: u32 = 2;
const APPLICATION_ID: &str = "codex";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    api_key: String,
    base_url: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationConfig {
    id: String,
    directory: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredData {
    version: u32,
    application: ApplicationConfig,
    profiles: Vec<Profile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProfile {
    id: String,
    name: String,
    directory: String,
    api_key: String,
    base_url: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ImportFile {
    Current(StoredData),
    Legacy(Vec<LegacyProfile>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    directory: String,
    profiles: Vec<Profile>,
    active_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportPreview {
    directory: Option<String>,
    profile_count: usize,
}

fn profiles_path() -> Result<PathBuf, String> {
    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "找不到 Windows APPDATA 目录".to_string())?;
    Ok(app_data.join(APP_DIR).join(PROFILES_FILE))
}

fn default_directory() -> String {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".codex")
        .to_string_lossy()
        .into_owned()
}

fn default_data() -> StoredData {
    StoredData {
        version: DATA_VERSION,
        application: ApplicationConfig {
            id: APPLICATION_ID.to_string(),
            directory: default_directory(),
        },
        profiles: Vec::new(),
    }
}

fn comparable_directory(directory: &str) -> String {
    directory
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn data_from_import(file: ImportFile) -> Result<StoredData, String> {
    match file {
        ImportFile::Current(data) => {
            if data.version != DATA_VERSION {
                return Err(format!("不支持的数据版本：{}", data.version));
            }
            if data.application.id != APPLICATION_ID {
                return Err("导入文件不是 Codex 配置".to_string());
            }
            Ok(data)
        }
        ImportFile::Legacy(profiles) => {
            let mut directories = profiles
                .iter()
                .map(|profile| profile.directory.trim())
                .filter(|directory| !directory.is_empty());
            let directory = directories
                .next()
                .map(str::to_string)
                .unwrap_or_else(default_directory);
            let comparable = comparable_directory(&directory);
            if directories.any(|candidate| comparable_directory(candidate) != comparable) {
                return Err("旧版导入文件包含多个 Codex 目录，无法合并为一个应用设置".to_string());
            }
            Ok(StoredData {
                version: DATA_VERSION,
                application: ApplicationConfig {
                    id: APPLICATION_ID.to_string(),
                    directory,
                },
                profiles: profiles
                    .into_iter()
                    .map(|profile| Profile {
                        id: profile.id,
                        name: profile.name,
                        api_key: profile.api_key,
                        base_url: profile.base_url,
                    })
                    .collect(),
            })
        }
    }
}

fn parse_data(content: &str) -> Result<StoredData, String> {
    let file: ImportFile =
        serde_json::from_str(content).map_err(|error| format!("配置文件格式错误：{error}"))?;
    data_from_import(file)
}

fn load_data_file() -> Result<StoredData, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(default_data());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取配置列表失败：{error}"))?;
    parse_data(&content).map_err(|error| format!("解析配置列表失败：{error}"))
}

fn save_data_file(data: &StoredData) -> Result<(), String> {
    let path = profiles_path()?;
    let content =
        serde_json::to_string_pretty(data).map_err(|error| format!("序列化配置失败：{error}"))?;
    write_file_atomic(&path, &format!("{content}\n"))
}

fn write_file_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "文件目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建文件目录失败：{error}"))?;
    let mut temp =
        NamedTempFile::new_in(parent).map_err(|error| format!("创建临时文件失败：{error}"))?;
    temp.write_all(content.as_bytes())
        .map_err(|error| format!("写入临时文件失败：{error}"))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("同步临时文件失败：{error}"))?;
    temp.persist(path)
        .map_err(|error| format!("替换文件失败：{}", error.error))?;
    Ok(())
}

fn validate_directory(directory: &str) -> Result<(), String> {
    let directory = Path::new(directory.trim());
    if !directory.is_dir() {
        return Err("配置目录不存在".to_string());
    }
    if !directory.join("auth.json").is_file() {
        return Err("配置目录中缺少 auth.json".to_string());
    }
    if !directory.join("config.toml").is_file() {
        return Err("配置目录中缺少 config.toml".to_string());
    }
    Ok(())
}

fn validate_profile(profile: &Profile) -> Result<(), String> {
    if profile.name.trim().is_empty() {
        return Err("请输入配置名称".to_string());
    }
    if profile.api_key.trim().is_empty() {
        return Err("请输入 OPENAI_API_KEY".to_string());
    }
    if !(profile.base_url.starts_with("http://") || profile.base_url.starts_with("https://")) {
        return Err("base_url 必须以 http:// 或 https:// 开头".to_string());
    }
    Ok(())
}

fn normalize_profile(mut profile: Profile) -> Result<Profile, String> {
    profile.id = profile.id.trim().to_string();
    profile.name = profile.name.trim().to_string();
    profile.api_key = profile.api_key.trim().to_string();
    profile.base_url = profile.base_url.trim().trim_end_matches('/').to_string();
    if profile.id.is_empty() {
        profile.id = Uuid::new_v4().to_string();
    }
    validate_profile(&profile)?;
    Ok(profile)
}

fn merge_profiles(mut existing: Vec<Profile>, imported: Vec<Profile>) -> Vec<Profile> {
    for profile in imported {
        if let Some(saved) = existing.iter_mut().find(|saved| saved.id == profile.id) {
            *saved = profile;
        } else {
            existing.push(profile);
        }
    }
    existing
}

fn updated_auth(content: &str, api_key: &str) -> Result<String, String> {
    let mut auth: JsonValue =
        serde_json::from_str(content).map_err(|error| format!("auth.json 格式错误：{error}"))?;
    let object = auth
        .as_object_mut()
        .ok_or_else(|| "auth.json 顶层必须是 JSON 对象".to_string())?;
    object.insert(
        "OPENAI_API_KEY".to_string(),
        JsonValue::String(api_key.to_string()),
    );
    serde_json::to_string_pretty(&auth)
        .map(|value| format!("{value}\n"))
        .map_err(|error| format!("生成 auth.json 失败：{error}"))
}

fn updated_config(content: &str, base_url: &str) -> Result<String, String> {
    let mut config = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("config.toml 格式错误：{error}"))?;
    let open_ai = config
        .get_mut("model_providers")
        .and_then(|item| item.as_table_like_mut())
        .and_then(|providers| providers.get_mut("OpenAI"))
        .and_then(|item| item.as_table_like_mut())
        .ok_or_else(|| "config.toml 中缺少 [model_providers.OpenAI]".to_string())?;
    open_ai.insert("base_url", value(base_url));
    Ok(config.to_string())
}

fn current_values(directory: &str) -> Result<(String, String), String> {
    let directory = Path::new(directory);
    let auth_content = fs::read_to_string(directory.join("auth.json"))
        .map_err(|error| format!("读取 auth.json 失败：{error}"))?;
    let auth: JsonValue = serde_json::from_str(&auth_content)
        .map_err(|error| format!("auth.json 格式错误：{error}"))?;
    let api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "auth.json 中缺少 OPENAI_API_KEY".to_string())?;

    let config_content = fs::read_to_string(directory.join("config.toml"))
        .map_err(|error| format!("读取 config.toml 失败：{error}"))?;
    let config = config_content
        .parse::<DocumentMut>()
        .map_err(|error| format!("config.toml 格式错误：{error}"))?;
    let base_url = config
        .get("model_providers")
        .and_then(|item| item.as_table_like())
        .and_then(|providers| providers.get("OpenAI"))
        .and_then(|item| item.as_table_like())
        .and_then(|open_ai| open_ai.get("base_url"))
        .and_then(|item| item.as_str())
        .ok_or_else(|| "config.toml 中缺少 [model_providers.OpenAI].base_url".to_string())?;
    Ok((api_key.to_string(), base_url.to_string()))
}

fn apply_values(directory: &Path, api_key: &str, base_url: &str) -> Result<(), String> {
    let auth_path = directory.join("auth.json");
    let config_path = directory.join("config.toml");
    let old_auth =
        fs::read_to_string(&auth_path).map_err(|error| format!("读取 auth.json 失败：{error}"))?;
    let old_config = fs::read_to_string(&config_path)
        .map_err(|error| format!("读取 config.toml 失败：{error}"))?;
    let new_auth = updated_auth(&old_auth, api_key)?;
    let new_config = updated_config(&old_config, base_url)?;

    fs::write(&auth_path, new_auth).map_err(|error| format!("写入 auth.json 失败：{error}"))?;
    if let Err(error) = fs::write(&config_path, new_config) {
        return match fs::write(&auth_path, old_auth) {
            Ok(()) => Err(format!("写入 config.toml 失败，auth.json 已恢复：{error}")),
            Err(restore_error) => Err(format!(
                "写入 config.toml 失败，且 auth.json 恢复失败：{error}；{restore_error}"
            )),
        };
    }
    Ok(())
}

fn state_from(data: StoredData) -> AppState {
    let active_id =
        current_values(&data.application.directory)
            .ok()
            .and_then(|(api_key, base_url)| {
                data.profiles
                    .iter()
                    .find(|profile| profile.api_key == api_key && profile.base_url == base_url)
                    .map(|profile| profile.id.clone())
            });
    AppState {
        directory: data.application.directory,
        profiles: data.profiles,
        active_id,
    }
}

#[tauri::command]
fn get_state() -> Result<AppState, String> {
    load_data_file().map(state_from)
}

#[tauri::command]
fn save_application(directory: String) -> Result<AppState, String> {
    let directory = directory.trim().to_string();
    validate_directory(&directory)?;
    let mut data = load_data_file()?;
    data.application.directory = directory;
    save_data_file(&data)?;
    Ok(state_from(data))
}

#[tauri::command]
fn save_profile(mut profile: Profile) -> Result<AppState, String> {
    let mut data = load_data_file()?;
    validate_directory(&data.application.directory)?;
    if profile.id.is_empty() {
        profile = normalize_profile(profile)?;
        data.profiles.push(profile);
    } else if let Some(saved) = data
        .profiles
        .iter_mut()
        .find(|saved| saved.id == profile.id)
    {
        profile = normalize_profile(profile)?;
        *saved = profile;
    } else {
        return Err("要编辑的配置不存在".to_string());
    }
    save_data_file(&data)?;
    Ok(state_from(data))
}

#[tauri::command]
fn delete_profile(id: String) -> Result<AppState, String> {
    let mut data = load_data_file()?;
    let old_len = data.profiles.len();
    data.profiles.retain(|profile| profile.id != id);
    if data.profiles.len() == old_len {
        return Err("要删除的配置不存在".to_string());
    }
    save_data_file(&data)?;
    Ok(state_from(data))
}

#[tauri::command]
fn apply_profile(id: String) -> Result<AppState, String> {
    let data = load_data_file()?;
    validate_directory(&data.application.directory)?;
    let profile = data
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| "要切换的配置不存在".to_string())?;
    validate_profile(profile)?;

    apply_values(
        Path::new(&data.application.directory),
        &profile.api_key,
        &profile.base_url,
    )?;
    Ok(state_from(data))
}

#[tauri::command]
fn preview_import(path: String) -> Result<ImportPreview, String> {
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取导入文件失败：{error}"))?;
    let imported = parse_data(&content).map_err(|error| format!("导入文件格式错误：{error}"))?;
    if imported.profiles.is_empty() {
        return Err("导入文件中没有配置".to_string());
    }
    Ok(ImportPreview {
        directory: Some(imported.application.directory),
        profile_count: imported.profiles.len(),
    })
}

#[tauri::command]
fn import_profiles(path: String, import_directory: bool) -> Result<AppState, String> {
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取导入文件失败：{error}"))?;
    let imported = parse_data(&content).map_err(|error| format!("导入文件格式错误：{error}"))?;
    if imported.profiles.is_empty() {
        return Err("导入文件中没有配置".to_string());
    }
    let profiles = imported
        .profiles
        .into_iter()
        .map(normalize_profile)
        .collect::<Result<Vec<_>, _>>()?;
    let mut data = load_data_file()?;
    if import_directory {
        let directory = imported.application.directory.trim().to_string();
        validate_directory(&directory)?;
        data.application.directory = directory;
    }
    data.profiles = merge_profiles(data.profiles, profiles);
    save_data_file(&data)?;
    Ok(state_from(data))
}

#[tauri::command]
async fn export_profiles(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name("codex-key-manager-backup.json")
        .add_filter("JSON 配置", &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = file
        .into_path()
        .map_err(|error| format!("导出路径无效：{error}"))?;
    let data = load_data_file()?;
    let content = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("生成导出文件失败：{error}"))?;
    write_file_atomic(&path, &format!("{content}\n"))?;
    Ok(true)
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let open = MenuItem::with_id(app, "open", "打开", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let icon = app.default_window_icon().cloned().ok_or("应用图标缺失")?;

            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(icon.clone())?;
            }

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Codex Key Manager")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_application,
            save_profile,
            delete_profile,
            apply_profile,
            preview_import,
            import_profiles,
            export_profiles,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        apply_values, current_values, merge_profiles, parse_data, updated_auth, updated_config,
        Profile, DATA_VERSION,
    };
    use std::fs;

    #[test]
    fn updates_only_requested_values() {
        let auth = updated_auth(r#"{"OPENAI_API_KEY":"old","other":1}"#, "new").unwrap();
        assert!(auth.contains(r#""OPENAI_API_KEY": "new""#));
        assert!(auth.contains(r#""other": 1"#));

        let config = updated_config(
            "title = \"keep\"\n\n[model_providers.OpenAI]\nname = \"OpenAI\"\nbase_url = \"old\"\n",
            "https://example.com",
        )
        .unwrap();
        assert!(config.contains("title = \"keep\""));
        assert!(config.contains("name = \"OpenAI\""));
        assert!(config.contains("base_url = \"https://example.com\""));
    }

    #[test]
    fn applies_values_to_codex_files() {
        let directory =
            std::env::temp_dir().join(format!("codex-key-manager-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("auth.json"),
            "{\n  \"OPENAI_API_KEY\": \"old\",\n  \"keep\": true\n}\n",
        )
        .unwrap();
        fs::write(
            directory.join("config.toml"),
            "keep = true\n\n[model_providers.OpenAI]\nbase_url = \"old\"\nwire_api = \"responses\"\n",
        )
        .unwrap();

        apply_values(&directory, "new-key", "https://example.com").unwrap();
        assert_eq!(
            current_values(directory.to_str().unwrap()).unwrap(),
            ("new-key".to_string(), "https://example.com".to_string())
        );
        assert!(fs::read_to_string(directory.join("auth.json"))
            .unwrap()
            .contains("\"keep\": true"));
        assert!(fs::read_to_string(directory.join("config.toml"))
            .unwrap()
            .contains("wire_api = \"responses\""));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn merges_imported_profiles_by_id() {
        let profile = |id: &str, name: &str| Profile {
            id: id.to_string(),
            name: name.to_string(),
            api_key: "key".to_string(),
            base_url: "https://example.com".to_string(),
        };
        let merged = merge_profiles(
            vec![profile("same", "old"), profile("kept", "kept")],
            vec![profile("same", "new"), profile("added", "added")],
        );

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].name, "new");
        assert_eq!(merged[1].name, "kept");
        assert_eq!(merged[2].name, "added");
    }

    #[test]
    fn migrates_legacy_profile_array() {
        let data = parse_data(
            r#"[{
                "id":"one",
                "name":"First",
                "directory":"C:\\Users\\Admin\\.codex",
                "apiKey":"key",
                "baseUrl":"https://example.com"
            }]"#,
        )
        .unwrap();

        assert_eq!(data.version, DATA_VERSION);
        assert_eq!(data.application.directory, r"C:\Users\Admin\.codex");
        assert_eq!(data.profiles.len(), 1);
        assert_eq!(data.profiles[0].name, "First");
    }

    #[test]
    fn rejects_legacy_profiles_with_different_directories() {
        let result = parse_data(
            r#"[
              {"id":"one","name":"One","directory":"C:\\one","apiKey":"a","baseUrl":"https://one.example"},
              {"id":"two","name":"Two","directory":"C:\\two","apiKey":"b","baseUrl":"https://two.example"}
            ]"#,
        );
        let error = match result {
            Ok(_) => panic!("不同目录应拒绝迁移"),
            Err(error) => error,
        };

        assert!(error.contains("多个 Codex 目录"));
    }

    #[test]
    fn accepts_equivalent_legacy_windows_directories() {
        let data = parse_data(
            r#"[
              {"id":"one","name":"One","directory":"C:\\Users\\Admin\\.codex","apiKey":"a","baseUrl":"https://one.example"},
              {"id":"two","name":"Two","directory":"C:/Users/Admin/.codex/","apiKey":"b","baseUrl":"https://two.example"}
            ]"#,
        )
        .unwrap();

        assert_eq!(data.application.directory, r"C:\Users\Admin\.codex");
        assert_eq!(data.profiles.len(), 2);
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml_edit::{value, DocumentMut};
use uuid::Uuid;

const APP_DIR: &str = "codex-key-manager";
const PROFILES_FILE: &str = "profiles.json";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    directory: String,
    api_key: String,
    base_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppState {
    profiles: Vec<Profile>,
    active_id: Option<String>,
}

fn profiles_path() -> Result<PathBuf, String> {
    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "找不到 Windows APPDATA 目录".to_string())?;
    Ok(app_data.join(APP_DIR).join(PROFILES_FILE))
}

fn load_profiles_file() -> Result<Vec<Profile>, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取配置列表失败：{error}"))?;
    serde_json::from_str(&content).map_err(|error| format!("解析配置列表失败：{error}"))
}

fn save_profiles_file(profiles: &[Profile]) -> Result<(), String> {
    let path = profiles_path()?;
    let parent = path.parent().ok_or_else(|| "配置目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("序列化配置失败：{error}"))?;
    fs::write(path, format!("{content}\n")).map_err(|error| format!("保存配置失败：{error}"))
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
    let directory = Path::new(profile.directory.trim());
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

fn state_from(profiles: Vec<Profile>) -> AppState {
    let active_id = profiles.iter().find_map(|profile| {
        current_values(&profile.directory)
            .ok()
            .filter(|(api_key, base_url)| {
                api_key == &profile.api_key && base_url == &profile.base_url
            })
            .map(|_| profile.id.clone())
    });
    AppState {
        profiles,
        active_id,
    }
}

#[tauri::command]
fn get_state() -> Result<AppState, String> {
    load_profiles_file().map(state_from)
}

#[tauri::command]
fn save_profile(mut profile: Profile) -> Result<AppState, String> {
    profile.name = profile.name.trim().to_string();
    profile.directory = profile.directory.trim().to_string();
    profile.api_key = profile.api_key.trim().to_string();
    profile.base_url = profile.base_url.trim().trim_end_matches('/').to_string();
    validate_profile(&profile)?;

    let mut profiles = load_profiles_file()?;
    if profile.id.is_empty() {
        profile.id = Uuid::new_v4().to_string();
        profiles.push(profile);
    } else if let Some(saved) = profiles.iter_mut().find(|saved| saved.id == profile.id) {
        *saved = profile;
    } else {
        return Err("要编辑的配置不存在".to_string());
    }
    save_profiles_file(&profiles)?;
    Ok(state_from(profiles))
}

#[tauri::command]
fn delete_profile(id: String) -> Result<AppState, String> {
    let mut profiles = load_profiles_file()?;
    let old_len = profiles.len();
    profiles.retain(|profile| profile.id != id);
    if profiles.len() == old_len {
        return Err("要删除的配置不存在".to_string());
    }
    save_profiles_file(&profiles)?;
    Ok(state_from(profiles))
}

#[tauri::command]
fn apply_profile(id: String) -> Result<AppState, String> {
    let profiles = load_profiles_file()?;
    let profile = profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| "要切换的配置不存在".to_string())?;
    validate_profile(profile)?;

    apply_values(
        Path::new(&profile.directory),
        &profile.api_key,
        &profile.base_url,
    )?;
    Ok(state_from(profiles))
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.minimize();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_profile,
            delete_profile,
            apply_profile,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{apply_values, current_values, updated_auth, updated_config};
    use std::{fs, path::PathBuf};

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

        fs::remove_dir_all(PathBuf::from(directory)).unwrap();
    }
}

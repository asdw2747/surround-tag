use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::tags::TagPolicy;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_TRACK_NAME: &str = "5.1 Surround (E-AC-3 640 kbps)";
pub const DEFAULT_TEMPLATE: &str = "{track:02} - {title} (5.1).mka";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Skip,
    Suffix,
    Overwrite,
}

impl CollisionPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skip => "Skip",
            Self::Suffix => "Auto-suffix (2)",
            Self::Overwrite => "Overwrite",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub mkvpropedit_path: Option<PathBuf>,
    pub mkvmerge_path: Option<PathBuf>,
    pub mkvextract_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
    pub track_name: String,
    pub language: String,
    pub set_default: bool,
    pub tag_policy: TagPolicy,
    pub rename_enabled: bool,
    pub rename_template: String,
    pub collision_policy: CollisionPolicy,
    pub keep_backup: bool,
    pub kodi_folder_art: bool,
    pub scan_recursive: bool,
    pub last_source_dir: Option<PathBuf>,
    pub last_target_dir: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            mkvpropedit_path: None,
            mkvmerge_path: None,
            mkvextract_path: None,
            ffprobe_path: None,
            track_name: DEFAULT_TRACK_NAME.to_owned(),
            language: "eng".to_owned(),
            set_default: true,
            tag_policy: TagPolicy::Merge,
            rename_enabled: false,
            rename_template: DEFAULT_TEMPLATE.to_owned(),
            collision_policy: CollisionPolicy::Skip,
            keep_backup: false,
            kodi_folder_art: true,
            scan_recursive: true,
            last_source_dir: None,
            last_target_dir: None,
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("surround-tag")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("surround-tag")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn undo_path() -> PathBuf {
    config_dir().join("undo_last_batch.json")
}

pub fn jobs_dir() -> PathBuf {
    cache_dir().join("jobs")
}

pub fn load() -> (AppConfig, Option<String>) {
    let path = config_path();
    let mut warning = None;
    let mut config = match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<AppConfig>(&text) {
            Ok(config) => config,
            Err(error) => {
                warning = Some(format!(
                    "Could not read {}: {error}. Defaults loaded.",
                    path.display()
                ));
                AppConfig::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
        Err(error) => {
            warning = Some(format!("Could not read {}: {error}", path.display()));
            AppConfig::default()
        }
    };

    if config.schema_version == 0 {
        config.schema_version = CONFIG_SCHEMA_VERSION;
    }
    if config.track_name.trim().is_empty() {
        config.track_name = DEFAULT_TRACK_NAME.to_owned();
    }
    if config.language.trim().is_empty() {
        config.language = "eng".to_owned();
    }
    if config.rename_template.trim().is_empty() {
        config.rename_template = DEFAULT_TEMPLATE.to_owned();
    }

    (config, warning)
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    let json = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    atomic_write(&path, &json)
        .map_err(|error| format!("Could not save {}: {error}", path.display()))
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    ));
    fs::write(&temp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

pub fn unique_job_dir() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let dir = jobs_dir().join(format!("job-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kodi_folder_art_defaults_to_enabled_for_old_configs() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(config.kodi_folder_art);
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
    }

    #[test]
    fn kodi_folder_art_can_be_disabled() {
        let config: AppConfig = serde_json::from_str(r#"{"kodi_folder_art": false}"#).unwrap();
        assert!(!config.kodi_folder_art);
    }
}

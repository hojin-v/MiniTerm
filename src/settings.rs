use crate::shell::ShellProfile;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn data_directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MINITERM_DATA_DIR") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("MINITERM_DATA_DIR은 절대 경로여야 합니다.".into());
        }
        return Ok(path);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|path| PathBuf::from(path).join("MiniTerm"))
        .ok_or("LOCALAPPDATA가 없습니다.".into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema_version: u32,
    pub profile: ShellProfile,
    pub theme: String,
    pub font_family: String,
    pub font_size: u16,
    pub scrollback: u32,
    pub alternate_scroll: bool,
    pub notifications: bool,
    pub notification_sound: bool,
    pub shortcut: String,
    pub autostart: bool,
    pub window: crate::geometry::WindowPreferences,
    pub prompt: crate::prompt::PromptPreferences,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            profile: ShellProfile::default(),
            theme: "dark".into(),
            font_family: String::new(),
            font_size: 14,
            scrollback: 10_000,
            alternate_scroll: false,
            notifications: true,
            notification_sound: false,
            shortcut: "Ctrl+Alt+Space".into(),
            autostart: false,
            window: Default::default(),
            prompt: Default::default(),
        }
    }
}

impl Settings {
    /// Apply only the fields edited this frame. An unfinished text field must not
    /// prevent an unrelated control from updating the last accepted preferences.
    pub(crate) fn with_edits(&self, before: &Self, edited: &Self) -> Self {
        let mut next = self.clone();
        macro_rules! copy_edits {
            ($($field:ident),+ $(,)?) => {
                $(if before.$field != edited.$field {
                    next.$field = edited.$field.clone();
                })+
            };
        }
        copy_edits!(
            profile,
            theme,
            font_family,
            font_size,
            scrollback,
            alternate_scroll,
            notifications,
            notification_sound,
            shortcut,
            autostart,
            window,
            prompt
        );
        next
    }

    pub fn validate(&self) -> Result<(), String> {
        crate::native::parse_shortcut(&self.shortcut)?;
        self.window.validate()?;
        self.prompt.validate()?;
        if self.schema_version != 1 {
            return Err("지원하지 않는 설정 버전입니다.".into());
        }
        if !["dark", "light"].contains(&self.theme.as_str()) {
            return Err("지원하지 않는 테마입니다.".into());
        }
        if !(10..=24).contains(&self.font_size) || !(1000..=50_000).contains(&self.scrollback) {
            return Err("글꼴 크기 또는 스크롤백 범위를 확인하세요.".into());
        }
        if self.font_family.len() > 1024 {
            return Err("글꼴 파일 경로를 확인하세요.".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let settings: Self = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("설정을 읽을 수 없습니다: {e}"))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("json.tmp");
        fs::write(
            &temp,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_text_does_not_block_an_unrelated_live_preference() {
        let accepted = Settings::default();
        let mut draft = accepted.clone();
        draft.shortcut = "Ctrl+Alt+".into();
        assert!(accepted.with_edits(&accepted, &draft).validate().is_err());
        let before = draft.clone();
        draft.notifications = false;
        let next = accepted.with_edits(&before, &draft);
        assert!(!next.notifications);
        assert_eq!(next.shortcut, accepted.shortcut);
        let path = std::env::temp_dir().join(format!(
            "miniterm-live-settings-{}.json",
            uuid::Uuid::new_v4()
        ));
        next.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), next);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn a_rejected_live_edit_keeps_the_last_saved_preferences() {
        let path = std::env::temp_dir().join(format!(
            "miniterm-live-settings-{}.json",
            uuid::Uuid::new_v4()
        ));
        let accepted = Settings {
            theme: "light".into(),
            window: crate::geometry::WindowPreferences {
                transparency: 25,
                ..Default::default()
            },
            ..Settings::default()
        };
        accepted.save(&path).unwrap();
        let mut invalid = accepted.clone();
        invalid.shortcut = "Ctrl+Alt+".into();
        assert!(invalid.save(&path).is_err());
        assert_eq!(Settings::load(&path).unwrap(), accepted);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn earlier_corner_settings_migrate_without_resetting_other_preferences() {
        let path =
            std::env::temp_dir().join(format!("miniterm-settings-{}.json", uuid::Uuid::new_v4()));
        fs::write(&path, r#"{"theme":"light","window":{"cornerRadius":24,"transparency":35},"prompt":{"showHost":false,"userLabel":"dev"}}"#).unwrap();
        let settings = Settings::load(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(settings.window.transparency, 35);
        assert_eq!(settings.theme, "light");
        assert!(!settings.prompt.show_host);
        assert_eq!(settings.prompt.user_label, "dev");
    }
    #[test]
    fn existing_settings_acquire_window_defaults_without_losing_preferences() {
        let settings: Settings =
            serde_json::from_str(r#"{"schemaVersion":1,"theme":"light","fontSize":14}"#).unwrap();
        assert_eq!(settings.window, Default::default());
        assert_eq!(settings.prompt, Default::default());
        assert_eq!(settings.theme, "light");
        assert_eq!(settings.font_size, 14);
        assert!(settings.validate().is_ok());
    }
    #[test]
    fn rejects_unknown_versions_and_unbounded_scrollback() {
        let mut settings = Settings {
            schema_version: 2,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
        settings.schema_version = 1;
        settings.scrollback = u32::MAX;
        assert!(settings.validate().is_err());
    }
}

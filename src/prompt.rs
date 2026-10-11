use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PathStyle {
    #[default]
    Full,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PromptPreferences {
    pub enabled: bool,
    pub show_user: bool,
    pub show_host: bool,
    pub show_path: bool,
    pub user_label: String,
    pub host_label: String,
    pub path_label: String,
    pub path_style: PathStyle,
    pub symbol: String,
}

impl Default for PromptPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            show_user: true,
            show_host: true,
            show_path: true,
            user_label: String::new(),
            host_label: String::new(),
            path_label: String::new(),
            path_style: PathStyle::Full,
            symbol: "›".into(),
        }
    }
}

impl PromptPreferences {
    pub fn validate(&self) -> Result<(), String> {
        for (text, limit) in [
            (&self.user_label, 64),
            (&self.host_label, 64),
            (&self.path_label, 128),
            (&self.symbol, 8),
        ] {
            if text.chars().count() > limit
                || text
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
            {
                return Err("프롬프트 별칭과 기호는 줄바꿈 없이 입력하세요. 별칭은 64자, 경로는 128자, 기호는 8자까지입니다.".into());
            }
        }
        Ok(())
    }

    pub fn preview(&self) -> String {
        self.compose(
            if self.user_label.is_empty() {
                "you"
            } else {
                &self.user_label
            },
            if self.host_label.is_empty() {
                "host"
            } else {
                &self.host_label
            },
            if !self.path_label.is_empty() {
                &self.path_label
            } else if self.path_style == PathStyle::Folder {
                "miniterm"
            } else {
                "~/projects/miniterm"
            },
            &self.symbol,
        )
    }

    fn compose(&self, user: &str, host: &str, path: &str, symbol: &str) -> String {
        let mut pieces = Vec::new();
        match (self.show_user, self.show_host) {
            (true, true) => pieces.push(format!("{user}@{host}")),
            (true, false) => pieces.push(user.into()),
            (false, true) => pieces.push(host.into()),
            _ => {}
        }
        if self.show_path {
            pieces.push(path.into());
        }
        if !symbol.is_empty() {
            pieces.push(symbol.into());
        }
        let mut result = pieces.join(" ");
        result.push(' ');
        result
    }

    fn state(&self) -> Result<String, String> {
        self.validate()?;
        // Bash expands these variables after decoding PS1 escapes. Their values are never
        // evaluated as shell code or reinterpreted as prompt escapes, even for $(), ` or \n.
        let user = if self.user_label.is_empty() {
            r"\u"
        } else {
            "${__miniterm_prompt_user}"
        };
        let host = if self.host_label.is_empty() {
            r"\h"
        } else {
            "${__miniterm_prompt_host}"
        };
        let path = if !self.path_label.is_empty() {
            "${__miniterm_prompt_path}"
        } else if self.path_style == PathStyle::Folder {
            r"\W"
        } else {
            r"\w"
        };
        let prompt = self.compose(
            &format!(r"\[\e[32m\]{user}\[\e[0m\]"),
            &format!(r"\[\e[32m\]{host}\[\e[0m\]"),
            &format!(r"\[\e[36m\]{path}\[\e[0m\]"),
            if self.symbol.is_empty() {
                ""
            } else {
                r"\[\e[35m\]${__miniterm_prompt_symbol}\[\e[0m\]"
            },
        );
        Ok(format!(
            "{}\n{prompt}\n{}\n{}\n{}\n{}\n",
            u8::from(self.enabled),
            self.user_label,
            self.host_label,
            self.path_label,
            self.symbol
        ))
    }
}

/// A session owns its bootstrap and atomically replaced prompt data. Bash reads six small
/// lines only when displaying a prompt; the application never writes commands into the PTY.
pub struct PromptFiles {
    directory: PathBuf,
}
impl PromptFiles {
    pub fn create(root: &Path, preferences: &PromptPreferences) -> Result<Self, String> {
        preferences.validate()?;
        let directory = root.join(format!("prompt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let files = Self { directory };
        fs::write(
            files.directory.join("bootstrap.bash"),
            include_str!("prompt.bash"),
        )
        .map_err(|e| e.to_string())?;
        files.update(preferences)?;
        Ok(files)
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn install_windows_bridge(&self) -> Result<(), String> {
        let Some(script) = crate::shell::codex_windows_bridge() else {
            return Ok(());
        };
        fs::write(self.directory.join("windows-bridge.bash"), script).map_err(|e| e.to_string())
    }
    pub fn update(&self, preferences: &PromptPreferences) -> Result<(), String> {
        let temporary = self.directory.join("state.tmp");
        fs::write(&temporary, preferences.state()?).map_err(|e| e.to_string())?;
        fs::rename(&temporary, self.directory.join("state")).map_err(|e| e.to_string())
    }
}
impl Drop for PromptFiles {
    fn drop(&mut self) {
        // Remove only the three files this session owns; never recursively delete a path.
        for name in [
            "bootstrap.bash",
            "state",
            "state.tmp",
            "windows-bridge.bash",
        ] {
            let _ = fs::remove_file(self.directory.join(name));
        }
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_hides_separators_and_rejects_terminal_control_injection() {
        let mut preferences = PromptPreferences {
            show_user: false,
            show_path: false,
            ..Default::default()
        };
        assert_eq!(preferences.preview(), "host › ");
        preferences.show_host = false;
        assert_eq!(preferences.preview(), "› ");
        preferences.symbol.clear();
        assert_eq!(preferences.preview(), " ");
        for text in ["a\nb", "\x1b[2J", "a\r", "\u{2028}"] {
            preferences.user_label = text.into();
            assert!(preferences.validate().is_err());
        }
    }
}

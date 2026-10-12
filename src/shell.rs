use portable_pty::CommandBuilder;
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    RegKey,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShellProfile {
    GitBash {
        #[serde(default)]
        bash_path: Option<PathBuf>,
        #[serde(default)]
        directory: Option<PathBuf>,
    },
    Wsl {
        distribution: String,
        #[serde(default)]
        user: Option<String>,
        #[serde(default = "linux_home")]
        directory: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslDistribution {
    pub name: String,
    pub version: u32,
    pub default: bool,
}

/// Reading the registered distributions never starts WSL or a virtual machine.
pub fn wsl_distributions() -> Vec<WslDistribution> {
    let Ok(root) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Lxss")
    else {
        return Vec::new();
    };
    let default: String = root.get_value("DefaultDistribution").unwrap_or_default();
    let mut distributions = Vec::new();
    for id in root.enum_keys().filter_map(Result::ok) {
        if let Ok(key) = root.open_subkey(&id) {
            if let Ok(name) = key.get_value::<String, _>("DistributionName") {
                distributions.push(WslDistribution {
                    name,
                    version: key.get_value("Version").unwrap_or(1),
                    default: id.eq_ignore_ascii_case(&default),
                });
            }
        }
    }
    distributions.sort_by_key(|distribution| (!distribution.default, distribution.name.clone()));
    distributions
}

pub fn wsl_executable() -> PathBuf {
    PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
        .join("System32/wsl.exe")
}

fn bash_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn codex_function(launch: &str) -> String {
    format!(
        "if [[ $(builtin type -t codex) != function && $(builtin type -t codex) != alias ]]; then\n  codex() {{\n    local __miniterm_notify __miniterm_arg __miniterm_inline=1\n    local -a __miniterm_codex_args=()\n    for __miniterm_arg in \"$@\"; do\n      if [[ $__miniterm_arg == --no-alt-screen ]]; then __miniterm_inline=0; fi\n    done\n    if [[ ${{MINITERM_CODEX_INLINE:-1}} != 0 && $__miniterm_inline == 1 ]]; then\n      __miniterm_codex_args+=(--no-alt-screen)\n    fi\n    if [[ -n ${{MINITERM_NOTIFY_EXE:-}} ]] && __miniterm_notify=$(command \"$MINITERM_NOTIFY_EXE\" codex-notify-override \"$@\" 2>/dev/null) && [[ $__miniterm_notify == \"notify=[\"* ]]; then\n      __miniterm_codex_args+=(-c \"$__miniterm_notify\")\n    fi\n    command {launch} \"${{__miniterm_codex_args[@]}}\" \"$@\"\n  }}\nfi\n"
    )
}

/// Use the companion Node bridge of the first Windows Codex launcher, retaining
/// argv, native TTY handles and the wrapper's own CLI discovery. Never rewrite it.
/// Prefer the inline TUI in this HUD: fullscreen confirmation dialogs otherwise
/// cover the middle of the transcript. MINITERM_CODEX_INLINE=0 opts out per launch.
pub fn codex_windows_bridge() -> Option<String> {
    let path = env::var_os("PATH")?;
    let directories = windows_path_entries(&path);
    for dir in &directories {
        if dir.join("codex.exe").is_file() {
            return Some(codex_function("codex"));
        }
        let launcher = dir.join("codex.cmd");
        if !launcher.is_file() {
            continue;
        }
        let header = std::fs::read_to_string(&launcher).ok()?;
        if !header.contains("AI_BATTERY_MANAGED_CODEX_WRAPPER=1") {
            return None;
        }
        let bridge = dir.join("codex.cmd.ai-battery.cjs");
        if !bridge.is_file() {
            return None;
        }
        let node = directories
            .iter()
            .map(|p| p.join("node.exe"))
            .find(|p| p.is_file())?;
        return Some(codex_function(&format!(
            "{} {}",
            bash_quote(&node.to_string_lossy().replace('\\', "/")),
            bash_quote(&bridge.to_string_lossy().replace('\\', "/"))
        )));
    }
    None
}

fn windows_path_entries(path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let text = path.to_string_lossy();
    if !path_needs_repair(path) {
        return env::split_paths(path).collect();
    }
    // A stray quote must not hide every subsequent Windows PATH directory.
    text.split(';')
        .map(|p| PathBuf::from(p.trim().trim_matches('"')))
        .collect()
}

fn path_needs_repair(path: &std::ffi::OsStr) -> bool {
    env::split_paths(path).any(|p| p.to_string_lossy().contains(';') && !p.is_dir())
        || !path
            .to_string_lossy()
            .matches('"')
            .count()
            .is_multiple_of(2)
}

fn linux_home() -> String {
    "~".into()
}

impl Default for ShellProfile {
    fn default() -> Self {
        Self::GitBash {
            bash_path: None,
            directory: None,
        }
    }
}

pub fn detect_git_bash() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(key) = RegKey::predef(hive).open_subkey("SOFTWARE\\GitForWindows") {
            if let Ok(root) = key.get_value::<String, _>("InstallPath") {
                candidates.push(PathBuf::from(root).join("bin/bash.exe"));
            }
        }
    }
    for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(var) {
            candidates.push(PathBuf::from(root).join("Git/bin/bash.exe"));
        }
    }
    if let Some(root) = env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(root).join("Programs/Git/bin/bash.exe"));
    }
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            if dir.join("git.exe").is_file() {
                if let Some(root) = dir.parent() {
                    candidates.push(root.join("bin/bash.exe"));
                }
            }
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

impl ShellProfile {
    pub fn command(&self, session_id: &str) -> Result<CommandBuilder, String> {
        self.command_with_prompt(session_id, None)
    }

    pub fn command_with_prompt(
        &self,
        session_id: &str,
        prompt_directory: Option<&std::path::Path>,
    ) -> Result<CommandBuilder, String> {
        let mut command = match self {
            Self::GitBash {
                bash_path,
                directory,
            } => {
                let bash = match bash_path {
                    Some(path) => Some(path.clone()),
                    None => crate::runtime::bundled_bash()?.or_else(detect_git_bash),
                }.ok_or_else(||
                    "Git for Windows Bash를 찾지 못했습니다. Git for Windows를 설치하거나 설정에서 bash.exe 경로를 지정하세요.".to_string())?;
                if !bash.is_file()
                    || !bash
                        .file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case("bash.exe"))
                {
                    return Err(format!("Bash 실행 파일을 확인하세요: {}", bash.display()));
                }
                let dir = directory
                    .clone()
                    .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
                    .ok_or("Windows 시작 폴더를 지정하세요.")?;
                if !dir.is_dir() {
                    return Err(format!("시작 폴더가 없습니다: {}", dir.display()));
                }
                let mut command = CommandBuilder::new(&bash);
                command.args(if prompt_directory.is_some() {
                    vec!["--login", "--noprofile", "-i"]
                } else {
                    vec!["--login", "-i"]
                });
                command.cwd(&dir);
                command.env("CHERE_INVOKING", "1");
                command.env("MSYSTEM", "MINGW64");
                command.env("SHELL", "/bin/bash");
                command
            }
            Self::Wsl {
                distribution,
                user,
                directory,
            } => {
                if distribution.trim().is_empty() {
                    return Err("WSL 배포판 이름을 지정하세요.".into());
                }
                if (directory != "~" && !directory.starts_with('/'))
                    || directory.chars().any(char::is_control)
                {
                    return Err("WSL 시작 폴더는 ~ 또는 Linux 절대 경로여야 합니다.".into());
                }
                let distro = wsl_distributions()
                    .into_iter()
                    .find(|distro| distro.name.eq_ignore_ascii_case(distribution))
                    .ok_or(
                        "등록된 WSL 배포판을 선택하세요. WSL 프로필에는 WSL2 배포판이 필요합니다.",
                    )?;
                if distro.version != 2 {
                    return Err("이 프로필은 WSL2 배포판을 지원합니다.".into());
                }
                if let Some(user) = user {
                    if user.is_empty()
                        || user.len() > 128
                        || user.starts_with('-')
                        || user.chars().any(|c| c.is_whitespace() || c.is_control())
                    {
                        return Err("WSL 사용자 이름을 확인하세요. 비우면 배포판의 기본 사용자를 사용합니다.".into());
                    }
                }
                let wsl = wsl_executable();
                if !wsl.is_file() {
                    return Err("WSL을 찾지 못했습니다. WSL 프로필을 사용하려면 WSL2와 배포판을 준비하세요.".into());
                }
                let mut command = CommandBuilder::new(wsl);
                command.args(["--distribution", &distro.name]);
                if let Some(user) = user {
                    command.args(["--user", user]);
                }
                command.args(["--cd", directory, "--exec", "/bin/bash", "--login"]);
                if prompt_directory.is_some() {
                    command.arg("--noprofile");
                }
                command.arg("-i");
                let existing = env::var("WSLENV").unwrap_or_default();
                let mut vars: Vec<&str> = existing.split(':').filter(|s| !s.is_empty()).collect();
                for required in [
                    "TERM",
                    "COLORTERM",
                    "TERM_PROGRAM",
                    "TERM_PROGRAM_VERSION",
                    "MINITERM_SESSION_ID",
                    "MINITERM_INSTANCE_ID",
                    "MINITERM_DIAGNOSTICS",
                    "MINITERM_DATA_DIR/p",
                    "MINITERM_NOTIFY_EXE/p",
                    "MINITERM_PROMPT_DIR/p",
                ] {
                    if !vars
                        .iter()
                        .any(|s| s.split('/').next() == required.split('/').next())
                    {
                        vars.push(required);
                    }
                }
                command.env("WSLENV", vars.join(":"));
                command
            }
        };
        command.env("MINITERM_SESSION_ID", session_id);
        if let Some(directory) = prompt_directory {
            command.env(
                "MINITERM_PROMPT_DIR",
                directory.to_string_lossy().replace('\\', "/"),
            );
            command.env(
                "PROMPT_COMMAND",
                "builtin source \"$MINITERM_PROMPT_DIR/bootstrap.bash\"",
            );
            // PROMPT_COMMAND must also reach the Linux Bash. It is a fixed script, never user input.
            if matches!(self, Self::Wsl { .. }) {
                let existing = command
                    .get_env("WSLENV")
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                command.env("WSLENV", format!("{existing}:PROMPT_COMMAND"));
            }
        }
        if let Ok(exe) = env::current_exe() {
            command.env("MINITERM_NOTIFY_EXE", exe);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "MiniTerm");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        // GUI launches from automation often inherit NO_COLOR=1. This interactive terminal
        // advertises color; users can still explicitly set NO_COLOR inside their shell.
        command.env_remove("NO_COLOR");
        if let Some(path) = env::var_os("PATH") {
            if path_needs_repair(&path) {
                if let Ok(path) = env::join_paths(windows_path_entries(&path)) {
                    command.env("PATH", path);
                }
            }
        }
        Ok(command)
    }

    pub fn label(&self) -> String {
        match self {
            Self::GitBash { .. } => "Bash".into(),
            Self::Wsl { distribution, .. } => format!("WSL · {distribution}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_explicit_bash_never_falls_back() {
        let profile = ShellProfile::GitBash {
            bash_path: Some("C:\\missing\\bash.exe".into()),
            directory: None,
        };
        assert!(profile
            .command("test")
            .unwrap_err()
            .contains("Bash 실행 파일"));
    }
    #[test]
    fn interactive_shell_advertises_color_without_inheriting_automation_no_color() {
        let command = ShellProfile::default().command("environment-test").unwrap();
        assert!(command.get_env("NO_COLOR").is_none());
        assert_eq!(command.get_env("TERM").unwrap(), "xterm-256color");
        assert_eq!(command.get_env("COLORTERM").unwrap(), "truecolor");
        assert_eq!(command.get_env("TERM_PROGRAM").unwrap(), "MiniTerm");
    }
    #[test]
    fn malformed_quotes_cannot_hide_later_windows_commands() {
        let paths = windows_path_entries(std::ffi::OsStr::new(
            r#"C:\Tools";C:\Other";C:\Users\example\.local\bin"#,
        ));
        assert_eq!(
            paths,
            vec![
                PathBuf::from(r"C:\Tools"),
                PathBuf::from(r"C:\Other"),
                PathBuf::from(r"C:\Users\example\.local\bin")
            ]
        );
    }

    #[test]
    fn wsl_requires_a_distribution_and_linux_directory() {
        let empty = ShellProfile::Wsl {
            distribution: "".into(),
            user: None,
            directory: "~".into(),
        };
        assert!(empty.command("test").is_err());
        let windows = ShellProfile::Wsl {
            distribution: "Ubuntu".into(),
            user: None,
            directory: "C:\\Projects".into(),
        };
        assert!(windows.command("test").is_err());
    }

    #[test]
    fn older_wsl_profiles_keep_the_distribution_default_user() {
        let profile: ShellProfile =
            serde_json::from_str(r#"{"kind":"wsl","distribution":"Ubuntu"}"#).unwrap();
        assert!(
            matches!(profile, ShellProfile::Wsl { user: None, directory, .. } if directory == "~")
        );
    }
}

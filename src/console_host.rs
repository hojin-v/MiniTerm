use std::sync::OnceLock;
use windows_sys::Win32::System::LibraryLoader::{
    SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
};

/// Keep an unrelated PATH/CWD DLL from silently replacing the console provider.
/// App-local providers and Windows system libraries remain available. This
/// changes DLL lookup in MiniTerm, without changing the shell's command PATH.
pub fn configure_dll_search() -> Result<(), String> {
    static CONFIGURED: OnceLock<Result<(), String>> = OnceLock::new();
    CONFIGURED
        .get_or_init(|| {
            if unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) } == 0 {
                Err(format!(
                    "콘솔 DLL 검색 경로를 설정하지 못했습니다: {}",
                    std::io::Error::last_os_error()
                ))
            } else {
                Ok(())
            }
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::windows::ffi::OsStrExt, path::PathBuf, process::Command};
    use windows_sys::Win32::{Foundation::FreeLibrary, System::LibraryLoader::LoadLibraryW};

    fn wide(path: &std::ffi::OsStr) -> Vec<u16> {
        path.encode_wide().chain(Some(0)).collect()
    }

    #[test]
    fn excludes_ambient_dlls_in_a_fresh_process() {
        let root = std::env::temp_dir().join(format!(
            "miniterm-dll-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let ambient = root.join("ambient");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir(&ambient).unwrap();
        let executable = app.join("console-search-test.exe");
        let dll = ambient.join("conpty.dll");
        fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        // Use an existing harmless Windows DLL, giving it the contested name.
        // No compiler, installed AI Battery, or network is required by this test.
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
        fs::copy(system.join("version.dll"), &dll).unwrap();
        let helper = format!(
            "{}::dll_search_in_child",
            module_path!().split_once("::").unwrap().1
        );
        let output = Command::new(&executable)
            .args(["--exact", &helper, "--ignored", "--nocapture"])
            .env("MINITERM_DLL_SEARCH_FIXTURE", &dll)
            .env("PATH", &ambient)
            .current_dir(&app)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh-process DLL search failed in {}: {} {}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_file(executable).unwrap();
        fs::remove_file(dll).unwrap();
        fs::remove_dir(app).unwrap();
        fs::remove_dir(ambient).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    #[ignore = "isolated child of excludes_ambient_dlls_in_a_fresh_process"]
    fn dll_search_in_child() {
        let fixture = std::env::var_os("MINITERM_DLL_SEARCH_FIXTURE").unwrap();
        let name = wide(std::ffi::OsStr::new("conpty.dll"));
        // Establish that the injected PATH can load a DLL before the policy.
        let ambient = unsafe { LoadLibraryW(name.as_ptr()) };
        assert!(!ambient.is_null(), "fixture DLL was not loadable");
        assert_ne!(unsafe { FreeLibrary(ambient) }, 0);
        configure_dll_search().unwrap();
        configure_dll_search().unwrap();
        let unexpected = unsafe { LoadLibraryW(name.as_ptr()) };
        assert!(unexpected.is_null(), "PATH DLL bypassed the search policy");
        // Explicit DLL paths stay usable for components which select a provider.
        let explicit = unsafe { LoadLibraryW(wide(&fixture).as_ptr()) };
        assert!(!explicit.is_null(), "explicit DLL path stopped working");
        assert_ne!(unsafe { FreeLibrary(explicit) }, 0);
    }
}

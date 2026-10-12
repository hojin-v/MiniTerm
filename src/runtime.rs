use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
    BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE,
    BCRYPT_SHA256_ALGORITHM,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    schema_version: u32,
    architecture: String,
    bash_sha256: String,
    msys_sha256: String,
}

struct FileHash {
    algorithm: BCRYPT_ALG_HANDLE,
    hash: BCRYPT_HASH_HANDLE,
}

impl Drop for FileHash {
    fn drop(&mut self) {
        unsafe {
            if !self.hash.is_null() {
                BCryptDestroyHash(self.hash);
            }
            if !self.algorithm.is_null() {
                BCryptCloseAlgorithmProvider(self.algorithm, 0);
            }
        }
    }
}

fn hash_status(status: i32) -> Result<(), String> {
    if status < 0 {
        Err(format!("Bash 파일 검증 오류: {status:#x}"))
    } else {
        Ok(())
    }
}

/// Stream files through Windows CNG rather than allocating the entire DLL.
fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut state = FileHash {
        algorithm: null_mut(),
        hash: null_mut(),
    };
    unsafe {
        hash_status(BCryptOpenAlgorithmProvider(
            &mut state.algorithm,
            BCRYPT_SHA256_ALGORITHM,
            null(),
            0,
        ))?;
        // Supported Windows versions allocate and release the hash object.
        hash_status(BCryptCreateHash(
            state.algorithm,
            &mut state.hash,
            null_mut(),
            0,
            null(),
            0,
            0,
        ))?;
    }
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        unsafe {
            hash_status(BCryptHashData(state.hash, buffer.as_ptr(), count as u32, 0))?;
        }
    }
    let mut digest = [0_u8; 32];
    unsafe {
        hash_status(BCryptFinishHash(
            state.hash,
            digest.as_mut_ptr(),
            digest.len() as u32,
            0,
        ))?;
    }
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// An absent bundle permits the installed Git profile; a damaged bundle is an
/// error. Never disguise an incomplete update by silently using another Bash.
pub fn bundled_bash_at(application_directory: &Path) -> Result<Option<PathBuf>, String> {
    let root = application_directory.join("runtime/git");
    if !root.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    File::open(root.join("miniterm-runtime.json"))
        .map_err(|e| format!("MiniTerm Bash 구성 파일을 확인하세요: {e}"))?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 {
        return Err("MiniTerm Bash 구성 파일이 너무 큽니다.".into());
    }
    let manifest: RuntimeManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("MiniTerm Bash 구성 파일을 확인하세요: {e}"))?;
    if manifest.schema_version != 1 || manifest.architecture != "x64" {
        return Err("지원하지 않는 MiniTerm Bash 구성입니다.".into());
    }
    let bash = root.join("usr/bin/bash.exe");
    for (path, expected) in [
        (&bash, manifest.bash_sha256),
        (&root.join("usr/bin/msys-2.0.dll"), manifest.msys_sha256),
    ] {
        if expected.len() != 64
            || !expected.bytes().all(|c| c.is_ascii_hexdigit())
            || !file_sha256(path)?.eq_ignore_ascii_case(&expected)
        {
            return Err(format!(
                "MiniTerm Bash 파일이 설치 정보와 일치하지 않습니다: {}",
                path.display()
            ));
        }
    }
    for relative in ["etc/profile", "cmd/git.exe"] {
        if !root.join(relative).is_file() {
            return Err(format!("MiniTerm Bash 파일이 없습니다: {relative}"));
        }
    }
    Ok(Some(bash))
}

pub fn bundled_bash() -> Result<Option<PathBuf>, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    bundled_bash_at(executable.parent().ok_or("앱 실행 경로를 확인하세요.")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("miniterm-runtime-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn valid_bundle(&self) -> PathBuf {
            let root = self.0.join("runtime/git");
            for directory in ["usr/bin", "etc", "cmd"] {
                fs::create_dir_all(root.join(directory)).unwrap();
            }
            for file in [
                "usr/bin/bash.exe",
                "usr/bin/msys-2.0.dll",
                "etc/profile",
                "cmd/git.exe",
            ] {
                fs::write(root.join(file), b"abc").unwrap();
            }
            // Standard SHA256 test vector, independent of the CNG implementation.
            let sha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
            fs::write(
                root.join("miniterm-runtime.json"),
                serde_json::to_vec(&serde_json::json!({
                    "schemaVersion": 1, "architecture": "x64", "bashSha256": sha, "msysSha256": sha
                }))
                .unwrap(),
            )
            .unwrap();
            root
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn absent_bundle_preserves_installed_git_support() {
        let fixture = Fixture::new();
        assert!(bundled_bash_at(&fixture.0).unwrap().is_none());
    }

    #[test]
    fn verified_bundle_resolves_and_modified_dll_is_rejected() {
        let fixture = Fixture::new();
        let root = fixture.valid_bundle();
        assert_eq!(
            bundled_bash_at(&fixture.0).unwrap(),
            Some(root.join("usr/bin/bash.exe"))
        );
        fs::write(root.join("usr/bin/msys-2.0.dll"), b"corrupted").unwrap();
        assert!(bundled_bash_at(&fixture.0)
            .unwrap_err()
            .contains("일치하지"));
    }

    #[test]
    fn partial_bundle_is_an_error_instead_of_a_fallback() {
        let fixture = Fixture::new();
        let root = fixture.valid_bundle();
        fs::remove_file(root.join("cmd/git.exe")).unwrap();
        assert!(bundled_bash_at(&fixture.0)
            .unwrap_err()
            .contains("파일이 없습니다"));
        fs::remove_file(root.join("miniterm-runtime.json")).unwrap();
        assert!(bundled_bash_at(&fixture.0).is_err());
    }

    #[test]
    #[ignore = "Requires a prepared runtime next to this test executable; verifies the real default profile"]
    fn prepared_bundle_is_selected_by_default_profile() {
        let bash = bundled_bash()
            .unwrap()
            .expect("Prepared bundle is required");
        let command = crate::shell::ShellProfile::default()
            .command("bundled-runtime-selection")
            .unwrap();
        assert_eq!(command.get_argv()[0], bash.as_os_str());
        let installed = crate::shell::detect_git_bash().expect("Installed Git control is required");
        let explicit = crate::shell::ShellProfile::GitBash {
            bash_path: Some(installed.clone()),
            directory: None,
        }
        .command("explicit-runtime-selection")
        .unwrap();
        assert_eq!(explicit.get_argv()[0], installed.as_os_str());
    }
}

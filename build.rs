use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=assets/miniterm.ico");
    if !env::var("TARGET").unwrap().ends_with("windows-msvc") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/miniterm.ico");
    let resource = output.join("miniterm.rc");
    let compiled = output.join("miniterm.res");
    fs::write(
        &resource,
        format!("1 ICON \"{}\"\n", icon.to_string_lossy().replace('\\', "/")),
    )
    .unwrap();
    let mut compilers = vec![PathBuf::from("rc.exe")];
    if let Some(root) = env::var_os("ProgramFiles(x86)") {
        let bin = PathBuf::from(root).join("Windows Kits/10/bin");
        let mut versions: Vec<_> = fs::read_dir(bin)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("x64/rc.exe"))
            .filter(|path| path.is_file())
            .collect();
        versions.sort();
        compilers.extend(versions.into_iter().rev());
    }
    for compiler in compilers {
        if Command::new(compiler)
            .arg("/nologo")
            .arg("/c65001")
            .arg("/fo")
            .arg(&compiled)
            .arg(&resource)
            .status()
            .is_ok_and(|status| status.success())
        {
            println!("cargo:rustc-link-arg-bin=miniterm={}", compiled.display());
            return;
        }
    }
    panic!("Windows SDK rc.exe is required to embed the MiniTerm application icon.");
}

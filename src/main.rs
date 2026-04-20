use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use winreg::enums::*;
use winreg::RegKey;

use rfd::{MessageButtons, MessageDialog, MessageLevel};

const INSTALL_DIR: &str = "C:\\Program Files\\backuptracker";
const JAR_NAME: &str = "backuptracker.jar";
const BAT_NAME: &str = "backuptracker.bat";

const RELEASE_API: &str =
    "https://api.github.com/repos/mtchs-code-intern/backup-tracker/releases/latest";

fn main() {
    if !is_elevated() {
        relaunch_as_admin();
        return;
    }

    if !is_java_installed() {
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title("Java Required")
            .set_description("Java is not installed or not in PATH.\n\nOpening download page...")
            .set_buttons(MessageButtons::Ok)
            .show();

        let _ = Command::new("cmd")
            .args(["/C", "start https://www.java.com/en/download/"])
            .spawn();

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

fn is_java_installed() -> bool {
    Command::new("java")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
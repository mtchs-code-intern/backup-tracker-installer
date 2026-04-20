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
    // 🔴 Elevation
    if !is_elevated() {
        relaunch_as_admin();
        return;
    }

    // 🔴 Java check
    if !is_java_installed() {
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title("Java Required")
            .set_description(
                "Java is not installed or not in PATH.\n\nThe download page will open now.",
            )
            .set_buttons(MessageButtons::Ok)
            .show();

        let _ = Command::new("cmd")
            .args(["/C", "start https://www.java.com/en/download/"])
            .spawn();

        return;
    }

    // 🔴 Run install
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
    let path = Path::new(INSTALL_DIR);

    if !path.exists() {
        fs::create_dir_all(path)?;
    }

    Ok(())
}

//
// 🔴 Elevation
//
fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn relaunch_as_admin() {
    let exe = std::env::current_exe().expect("Failed to get current exe");

    let status = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs -Wait",
                exe.display()
            ),
        ])
        .status();

    if status.is_err() {
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title("Elevation Failed")
            .set_description("Failed to request administrator privileges.")
            .set_buttons(MessageButtons::Ok)
            .show();
    }
}

//
// 🔴 GitHub structs
//
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
// 🔴 Download JAR
//
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
    fs::write(&dest, &bytes)?;

    Ok(())
}

fn to_io_error<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

//
// 🔴 BAT launcher
//
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
// 🔴 PATH update
//
fn add_to_path() -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;

    let current_path: String = env.get_value("PATH").unwrap_or_default();

    if !current_path.contains(INSTALL_DIR) {
        let new_path = if current_path.is_empty() {
            INSTALL_DIR.to_string()
        } else {
            format!("{};{}", current_path, INSTALL_DIR)
        };

        env.set_value("PATH", &new_path)?;

        MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title("PATH Updated")
            .set_description("Restart your terminal to use BackupTracker.")
            .set_buttons(MessageButtons::Ok)
            .show();
    }

    Ok(())
}
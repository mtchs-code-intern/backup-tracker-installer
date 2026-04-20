use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use winreg::enums::*;
use winreg::RegKey;

const INSTALL_DIR: &str = "C:\\Program Files\\backuptracker";
const JAR_NAME: &str = "backuptracker.jar";
const BAT_NAME: &str = "backuptracker.bat";

fn main() {
    println!("Starting BackupTracker installer...");

    if !is_java_installed() {
        eprintln!("Java is not installed or not in PATH.");
        eprintln!("Please install Java and re-run this installer.");
        return;
    }

    if let Err(e) = run_install() {
        eprintln!("Installation failed: {}", e);
        return;
    }

    println!("Installation completed successfully!");
}

fn run_install() -> io::Result<()> {
    create_install_dir()?;
    copy_jar()?;
    create_bat()?;
    add_to_path()?;
    Ok(())
}

fn is_java_installed() -> bool {
    Command::new("java")
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn create_install_dir() -> io::Result<()> {
    let path = Path::new(INSTALL_DIR);

    if !path.exists() {
        println!("Creating install directory...");
        fs::create_dir_all(path)?;
    } else {
        println!("Install directory already exists.");
    }

    Ok(())
}

fn copy_jar() -> io::Result<()> {
    println!("Copying JAR file...");

    // Get current executable directory
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "Failed to get executable directory")
    })?;

    let source = exe_dir.join(JAR_NAME);
    let dest = Path::new(INSTALL_DIR).join(JAR_NAME);

    if !source.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} not found next to installer", JAR_NAME),
        ));
    }

    fs::copy(source, dest)?;

    Ok(())
}

fn create_bat() -> io::Result<()> {
    println!("Creating launcher...");

    let bat_path = Path::new(INSTALL_DIR).join(BAT_NAME);
    let mut file = File::create(bat_path)?;

    let contents = r#"@echo off
java -jar "%~dp0backuptracker.jar"
"#;

    file.write_all(contents.as_bytes())?;

    Ok(())
}

fn add_to_path() -> io::Result<()> {
    println!("Adding install directory to PATH...");

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
        println!("PATH updated. You may need to restart your terminal.");
    } else {
        println!("PATH already contains install directory.");
    }

    Ok(())
}
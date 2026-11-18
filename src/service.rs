use std::fs;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{anyhow, Result};

pub const UDEV_RULE_CONTENT: &str = r#"# Lenovo Legion / LOQ 4-zone RGB keyboard lighting control
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="048d", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="048d", MODE="0666", TAG+="uaccess"
"#;

pub fn setup_udev_rules() -> Result<()> {
    let udev_path = "/etc/udev/rules.d/99-lenovo-rgb.rules";

    // Check if we are already root or if pkexec/sudo is needed
    let is_root = unsafe { libc::geteuid() == 0 };

    if is_root {
        fs::write(udev_path, UDEV_RULE_CONTENT)?;
        let _ = Command::new("udevadm").args(["control", "--reload-rules"]).status();
        let _ = Command::new("udevadm")
            .args(["trigger", "--subsystem-match=hidraw", "--action=change"])
            .status();
        println!("Successfully installed udev rule at {}", udev_path);
    } else {
        // Try pkexec
        println!("Requesting elevation via pkexec to install udev rule...");
        let script = format!(
            "cat << 'EOF' > {}\n{}\nEOF\nudevadm control --reload-rules\nudevadm trigger --subsystem-match=hidraw --action=change",
            udev_path, UDEV_RULE_CONTENT
        );

        let status = Command::new("pkexec")
            .args(["bash", "-c", &script])
            .status()
            .or_else(|_| Command::new("sudo").args(["bash", "-c", &script]).status())?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to install udev rules. Please run manually:\n\
                 sudo tee /etc/udev/rules.d/99-lenovo-rgb.rules << 'EOF'\n\
                 {}\nEOF\n\
                 sudo udevadm control --reload-rules && sudo udevadm trigger",
                UDEV_RULE_CONTENT
            ));
        }
        println!("Successfully installed udev rule at {}", udev_path);
    }

    Ok(())
}

pub fn get_systemd_user_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("systemd").join("user")
}

pub fn install_systemd_service(profile: &str) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.to_string_lossy();

    let service_content = format!(
        r#"[Unit]
Description=Lenovo RGB Keyboard Profile Restorer ({profile})
After=default.target

[Service]
Type=oneshot
ExecStart={exe_path} profile apply "{profile}"
RemainAfterExit=yes

[Install]
WantedBy=default.target
"#
    );

    let dir = get_systemd_user_dir();
    fs::create_dir_all(&dir)?;

    let service_file = dir.join("lenovo-rgb.service");
    fs::write(&service_file, service_content)?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let enable_status = Command::new("systemctl")
        .args(["--user", "enable", "--now", "lenovo-rgb.service"])
        .status()?;

    if enable_status.success() {
        println!("Systemd user service installed and enabled at {:?}", service_file);
        println!("Profile '{}' will automatically apply on login!", profile);
    } else {
        println!("Service created at {:?}, run: systemctl --user enable --now lenovo-rgb.service", service_file);
    }

    Ok(())
}

pub fn uninstall_systemd_service() -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "lenovo-rgb.service"])
        .status();

    let service_file = get_systemd_user_dir().join("lenovo-rgb.service");
    if service_file.exists() {
        fs::remove_file(&service_file)?;
        println!("Removed systemd service at {:?}", service_file);
    }

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    Ok(())
}

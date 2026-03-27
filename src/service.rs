use std::path::PathBuf;

pub fn install() {
    #[cfg(target_os = "macos")]
    install_launchd();

    #[cfg(target_os = "linux")]
    install_systemd();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        eprintln!("Service installation is not supported on this platform.");
        eprintln!("Run `scanprojects` in a terminal or use your OS task scheduler.");
    }
}

#[cfg(target_os = "macos")]
fn install_launchd() {
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("scanprojects"));
    let plist_name = "com.scanprojects.daemon";
    let plist_path = dirs::home_dir()
        .unwrap()
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", plist_name));

    let log_path = crate::config::data_dir().join("scanprojects.log");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>"#,
        plist_name,
        binary.display(),
        log_path.display(),
        log_path.display(),
    );

    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    std::fs::write(&plist_path, &plist).expect("Failed to write plist");

    println!("Installed: {}", plist_path.display());
    println!();
    println!("To start now:");
    println!("  launchctl load {}", plist_path.display());
    println!();
    println!("To stop:");
    println!("  launchctl unload {}", plist_path.display());
    println!();
    println!("To uninstall:");
    println!("  launchctl unload {}", plist_path.display());
    println!("  rm {}", plist_path.display());
}

#[cfg(target_os = "linux")]
fn install_systemd() {
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("scanprojects"));
    let unit_name = "scanprojects";
    let unit_path = dirs::home_dir()
        .unwrap()
        .join(".config/systemd/user")
        .join(format!("{}.service", unit_name));

    let unit = format!(
        r#"[Unit]
Description=scanprojects — local port manager
After=network.target

[Service]
ExecStart={}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        binary.display(),
    );

    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    std::fs::write(&unit_path, &unit).expect("Failed to write systemd unit");

    println!("Installed: {}", unit_path.display());
    println!();
    println!("To start now:");
    println!("  systemctl --user enable --now {}", unit_name);
    println!();
    println!("To stop:");
    println!("  systemctl --user stop {}", unit_name);
    println!();
    println!("To uninstall:");
    println!("  systemctl --user disable --now {}", unit_name);
    println!("  rm {}", unit_path.display());
}

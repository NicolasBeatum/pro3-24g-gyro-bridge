use crate::protocol::VID_8BITDO;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub const VIGEM_VERSION: &str = "1.22.0";
pub const VIGEM_RELEASE_URL: &str = "https://github.com/nefarius/ViGEmBus/releases/tag/v1.22.0";
const VIGEM_DOWNLOAD_URL: &str =
    "https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe";
const VIGEM_EXPECTED_BYTES: u64 = 6_278_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallState {
    Idle,
    Downloading,
    Verifying,
    RunningInstaller,
    Done,
    Error(String),
}

/// Fast local check (<2 ms) for whether the signed ViGEmBus kernel driver is installed.
pub fn is_vigem_installed() -> bool {
    vigem_rust::Client::connect().is_ok()
}

/// Fast local check for whether any 8BitDo 2.4 GHz dongle PID (310B / 600A / 3109) is plugged in.
pub fn is_dongle_present() -> bool {
    hidapi::HidApi::new()
        .map(|api| api.device_list().any(|d| d.vendor_id() == VID_8BITDO))
        .unwrap_or(false)
}

/// Opens a URL in the user's default Windows browser without flashing a console window.
pub fn open_url(url: &str) {
    let _ = Command::new("cmd")
        .args(["/C", "start", "", url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// Downloads the official signed Nefarius ViGEmBus v1.22.0 installer into %TEMP%,
/// verifies both exact file size and its Windows Authenticode signature, then runs it
/// so Windows UAC displays Nefarius's verified publisher certificate.
pub fn spawn_vigem_installer(state: Arc<Mutex<InstallState>>) {
    let set = |s: &Arc<Mutex<InstallState>>, v: InstallState| {
        if let Ok(mut g) = s.lock() {
            *g = v;
        }
    };

    if let Ok(g) = state.lock() {
        if matches!(
            *g,
            InstallState::Downloading
                | InstallState::Verifying
                | InstallState::RunningInstaller
        ) {
            return;
        }
    }

    set(&state, InstallState::Downloading);
    std::thread::Builder::new()
        .name("vigem-setup".into())
        .spawn(move || {
            let dest: PathBuf = std::env::temp_dir().join("ViGEmBus_1.22.0_x64_x86_arm64.exe");
            let dest_str = dest.to_string_lossy().to_string();

            // 1) Download via built-in Windows curl.exe
            let dl = Command::new("curl.exe")
                .args([
                    "-fL",
                    "--silent",
                    "--show-error",
                    "--connect-timeout",
                    "15",
                    "--max-time",
                    "120",
                    "-o",
                    &dest_str,
                    VIGEM_DOWNLOAD_URL,
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            match dl {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    set(
                        &state,
                        InstallState::Error(if msg.is_empty() {
                            "Download failed".into()
                        } else {
                            msg
                        }),
                    );
                    return;
                }
                Err(e) => {
                    set(&state, InstallState::Error(format!("curl.exe error: {e}")));
                    return;
                }
            }

            // 2) Verify exact official size + Windows Authenticode digital signature
            set(&state, InstallState::Verifying);
            let meta_ok = std::fs::metadata(&dest)
                .map(|m| m.len() == VIGEM_EXPECTED_BYTES)
                .unwrap_or(false);
            if !meta_ok {
                let _ = std::fs::remove_file(&dest);
                set(
                    &state,
                    InstallState::Error("Unexpected installer size — please download manually.".into()),
                );
                return;
            }

            let sig = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "(Get-AuthenticodeSignature -LiteralPath $env:PRO3_SETUP_FILE).Status",
                ])
                .env("PRO3_SETUP_FILE", &dest_str)
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            let sig_valid = sig
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("Valid"))
                .unwrap_or(false);

            if !sig_valid {
                let _ = std::fs::remove_file(&dest);
                set(
                    &state,
                    InstallState::Error(
                        "Authenticode signature verification failed — please download manually.".into(),
                    ),
                );
                return;
            }

            // 3) Launch official signed installer (triggers native UAC prompt) and wait
            set(&state, InstallState::RunningInstaller);
            let run = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    "Start-Process -FilePath $env:PRO3_SETUP_FILE -Wait",
                ])
                .env("PRO3_SETUP_FILE", &dest_str)
                .creation_flags(CREATE_NO_WINDOW)
                .status();

            let _ = std::fs::remove_file(&dest);
            match run {
                Ok(st) if st.success() => set(&state, InstallState::Done),
                Ok(_) => set(
                    &state,
                    InstallState::Error("Installer cancelled or did not finish.".into()),
                ),
                Err(e) => set(&state, InstallState::Error(format!("Launch failed: {e}"))),
            }
        })
        .ok();
}

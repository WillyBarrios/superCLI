use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use eframe::egui;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API_URL: &str = "https://api.github.com/repos/WillyBarrios/superCLI/releases/latest";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches(['v', 'V']);
        let base = trimmed.split('-').next().unwrap_or(trimmed);
        let mut parts = base.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Returns true if `latest` is at least 2 minor versions (or >= 1 major version) ahead
    /// of `self`. Patch-only updates (e.g. 0.1.0 -> 0.1.5) NEVER count toward deprecation.
    pub fn is_deprecated_compared_to(self, latest: SemVer) -> bool {
        if latest.major > self.major {
            return true;
        }
        if latest.major == self.major && latest.minor >= self.minor + 2 {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version_tag: String,
    pub latest: SemVer,
    pub download_url: String,
    pub is_mandatory: bool,
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Idle,
    Available(UpdateInfo),
    Downloading(UpdateInfo),
    ReadyToRestart(String),
    Error(UpdateInfo, String),
}

/// Lightweight background OTA updater with zero steady-state RAM overhead.
/// Spawns a short-lived thread at startup to query GitHub Releases and exits immediately.
pub struct OtaUpdater {
    pub state: Arc<Mutex<UpdateStatus>>,
}

impl OtaUpdater {
    pub fn new(ctx: egui::Context) -> Self {
        let state = Arc::new(Mutex::new(UpdateStatus::Idle));
        let state_clone = Arc::clone(&state);

        thread::spawn(move || {
            if let Some(info) = check_latest_release() {
                if let Ok(mut guard) = state_clone.lock() {
                    *guard = UpdateStatus::Available(info);
                }
                ctx.request_repaint();
            }
        });

        Self { state }
    }

    pub fn snapshot(&self) -> UpdateStatus {
        self.state
            .lock()
            .map(|s| s.clone())
            .unwrap_or(UpdateStatus::Idle)
    }

    /// Downloads the installer/package for the current OS and launches a clean upgrade.
    pub fn start_install(&self, info: UpdateInfo, ctx: egui::Context) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = UpdateStatus::Downloading(info.clone());
        }
        ctx.request_repaint();

        let state_clone = Arc::clone(&self.state);
        thread::spawn(move || {
            match download_and_launch_installer(&info) {
                Ok(msg) => {
                    if let Ok(mut guard) = state_clone.lock() {
                        *guard = UpdateStatus::ReadyToRestart(msg);
                    }
                    ctx.request_repaint();
                    #[cfg(windows)]
                    {
                        // Give the installer a moment to launch, then exit so it can replace supercli.exe cleanly.
                        thread::sleep(Duration::from_millis(700));
                        std::process::exit(0);
                    }
                }
                Err(err) => {
                    if let Ok(mut guard) = state_clone.lock() {
                        *guard = UpdateStatus::Error(info, err);
                    }
                    ctx.request_repaint();
                }
            }
        });
    }
}

fn curl_command() -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("curl.exe");
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new("curl")
    }
}

fn check_latest_release() -> Option<UpdateInfo> {
    let current = SemVer::parse(CURRENT_VERSION)?;
    let output = curl_command()
        .args([
            "-fsSL",
            "--max-time",
            "8",
            "-H",
            "User-Agent: superCLI-OTA",
            "-H",
            "Accept: application/vnd.github+json",
            RELEASES_API_URL,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8(output.stdout).ok()?;
    let tag = extract_json_string(&body, "tag_name")?;
    let latest = SemVer::parse(&tag)?;

    if latest <= current {
        return None;
    }

    let download_url = find_platform_asset_url(&body)?;
    let is_mandatory = current.is_deprecated_compared_to(latest);

    Some(UpdateInfo {
        version_tag: tag,
        latest,
        download_url,
        is_mandatory,
    })
}

fn download_and_launch_installer(info: &UpdateInfo) -> Result<String, String> {
    #[cfg(windows)]
    {
        let temp_dir = std::env::temp_dir();
        // Clean up any previous temporary superCLI installers before saving the new one.
        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("supercli-update-") && name.ends_with(".exe") {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }

        let installer_path: PathBuf =
            temp_dir.join(format!("supercli-update-{}-setup.exe", info.version_tag));
        let status = curl_command()
            .args([
                "-fsSL",
                "--max-time",
                "120",
                "-H",
                "User-Agent: superCLI-OTA",
                "-o",
                &installer_path.to_string_lossy(),
                &info.download_url,
            ])
            .stdin(Stdio::null())
            .status()
            .map_err(|e| format!("Error al ejecutar curl.exe: {e}"))?;

        if !status.success() {
            return Err("No se pudo descargar el instalador desde GitHub Releases.".to_string());
        }

        Command::new(&installer_path)
            .spawn()
            .map_err(|e| format!("No se pudo abrir el instalador: {e}"))?;

        Ok(format!(
            "Instalador {} iniciado. Cerrando versión anterior...",
            info.version_tag
        ))
    }

    #[cfg(target_os = "linux")]
    {
        let temp_dir = std::env::temp_dir();
        let archive_path =
            temp_dir.join(format!("supercli-update-{}-linux.tar.gz", info.version_tag));
        let extract_dir = temp_dir.join(format!("supercli-extract-{}", info.version_tag));
        let _ = fs::remove_dir_all(&extract_dir);
        fs::create_dir_all(&extract_dir)
            .map_err(|e| format!("No se pudo preparar directorio temporal: {e}"))?;

        let status = curl_command()
            .args([
                "-fsSL",
                "--max-time",
                "120",
                "-H",
                "User-Agent: superCLI-OTA",
                "-o",
                &archive_path.to_string_lossy(),
                &info.download_url,
            ])
            .stdin(Stdio::null())
            .status()
            .map_err(|e| format!("Error al ejecutar curl: {e}"))?;

        if !status.success() {
            return Err("No se pudo descargar el paquete Linux desde GitHub Releases.".to_string());
        }

        let tar_status = Command::new("tar")
            .args([
                "-xzf",
                &archive_path.to_string_lossy(),
                "-C",
                &extract_dir.to_string_lossy(),
            ])
            .status()
            .map_err(|e| format!("Error ejecutando tar: {e}"))?;
        if !tar_status.success() {
            return Err("Falló la extracción del paquete de actualización.".to_string());
        }

        let install_sh = extract_dir.join("install.sh");
        if install_sh.exists() {
            let st = Command::new("bash")
                .arg(&install_sh)
                .status()
                .map_err(|e| format!("Error ejecutando install.sh: {e}"))?;
            if !st.success() {
                return Err("install.sh devolvió error.".to_string());
            }
        }
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&extract_dir);

        Ok(format!(
            "superCLI {} instalado. Reinicia la app para usar la nueva versión.",
            info.version_tag
        ))
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("Actualización automática no soportada en este sistema operativo.".to_string())
    }
}

/// Minimal JSON string value extractor (avoids pulling serde_json just for 2 fields).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let pos = json.find(&pattern)?;
    let rest = &json[pos + pattern.len()..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let content = &after_colon[1..];
    let end = content.find('"')?;
    Some(content[..end].to_string())
}

fn find_platform_asset_url(json: &str) -> Option<String> {
    let mut urls = Vec::new();
    let needle = "\"browser_download_url\"";
    let mut slice = json;
    while let Some(idx) = slice.find(needle) {
        let rest = &slice[idx + needle.len()..];
        if let Some(colon) = rest.find(':') {
            let after = rest[colon + 1..].trim_start();
            if let Some(stripped) = after.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    urls.push(stripped[..end].to_string());
                    slice = &stripped[end + 1..];
                    continue;
                }
            }
        }
        break;
    }

    #[cfg(windows)]
    {
        // Prefer the Inno Setup .exe installer so old installations are replaced cleanly.
        urls.iter()
            .find(|u| u.ends_with("windows-x86_64-setup.exe") || u.ends_with(".exe"))
            .cloned()
    }

    #[cfg(target_os = "linux")]
    {
        urls.iter()
            .find(|u| u.ends_with("linux-x86_64.tar.gz") || u.ends_with(".tar.gz"))
            .cloned()
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        urls.into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parsing_works() {
        assert_eq!(
            SemVer::parse("v0.2.1"),
            Some(SemVer {
                major: 0,
                minor: 2,
                patch: 1
            })
        );
        assert_eq!(
            SemVer::parse("1.0.0-rc1"),
            Some(SemVer {
                major: 1,
                minor: 0,
                patch: 0
            })
        );
    }

    #[test]
    fn deprecation_ignores_patches_and_blocks_at_two_minors_behind() {
        let v0_1_0 = SemVer::parse("0.1.0").unwrap();
        let v0_1_2 = SemVer::parse("0.1.2").unwrap();
        let v0_1_9 = SemVer::parse("0.1.9").unwrap();
        let v0_2_0 = SemVer::parse("0.2.0").unwrap();
        let v0_3_0 = SemVer::parse("0.3.0").unwrap();
        let v1_0_0 = SemVer::parse("1.0.0").unwrap();

        // Patch bumps NEVER trigger mandatory block
        assert!(!v0_1_0.is_deprecated_compared_to(v0_1_2));
        assert!(!v0_1_0.is_deprecated_compared_to(v0_1_9));

        // 1 minor version behind -> non-intrusive notice only
        assert!(!v0_1_0.is_deprecated_compared_to(v0_2_0));

        // 2 minor versions behind (0.1.x -> 0.3.0) -> mandatory block
        assert!(v0_1_0.is_deprecated_compared_to(v0_3_0));
        assert!(v0_1_9.is_deprecated_compared_to(v0_3_0));

        // Major version bump -> mandatory block
        assert!(v0_1_0.is_deprecated_compared_to(v1_0_0));
    }
}

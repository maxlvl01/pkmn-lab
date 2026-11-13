//! Self-update from GitHub releases. Uses the system `curl` (built into Windows 10/11, macOS, Linux)
//! so the tool needs no HTTP/TLS dependency.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

const REPO: &str = "maxlvl01/pkmn-lab";
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// Release asset for this platform, e.g. "pkmn-lab-windows-x86_64.exe".
fn asset_name() -> String {
    format!("pkmn-lab-{}-{}{}", std::env::consts::OS, std::env::consts::ARCH, std::env::consts::EXE_SUFFIX)
}

pub fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    // Windows: the full System32 path, because a bare "curl" is looked up in the exe's own folder first.
    #[cfg(windows)]
    let mut cmd = Command::new(std::path::Path::new(&std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join(r"System32\curl.exe"));
    #[cfg(not(windows))]
    let mut cmd = Command::new("curl");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing up from the GUI
    }
    let out = cmd
        .args(["-fsSL", "--max-time", "20", "-H", "User-Agent: pkmn-lab (personal fan project)"])
        .args(args)
        .output()
        .map_err(|e| format!("curl not found: {e}"))?;
    if !out.status.success() {
        return Err(format!("download failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

/// "0.10.2" -> [0, 10, 2], so versions compare numerically, not as text.
fn version(v: &str) -> Vec<u32> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Latest release: (version tag, download url of our asset if the release has one).
fn latest() -> Result<(String, Option<String>), String> {
    let body = curl(&[&format!("https://api.github.com/repos/{REPO}/releases/latest")])?;
    let json: serde_json::Value = serde_json::from_slice(&body).map_err(|e| format!("bad release info: {e}"))?;
    let tag = json["tag_name"].as_str().ok_or("release has no tag")?.to_string();
    let url = json["assets"].as_array().into_iter().flatten()
        .find(|a| a["name"] == asset_name())
        .and_then(|a| a["browser_download_url"].as_str())
        .map(String::from);
    Ok((tag, url))
}

/// Download the new exe next to the running one and swap them. The running exe can be renamed
/// (Windows locks it against deletion, not renaming), so: current -> .old, new -> current.
fn install(url: &str, exe: &Path) -> Result<(), String> {
    let new = exe.with_extension("new");
    let old = exe.with_extension("old");
    let bytes = curl(&[url])?;
    // Guard against an HTML error page or a truncated file replacing a working program.
    let looks_ok = bytes.len() > 1_000_000 && (!cfg!(windows) || bytes.starts_with(b"MZ"));
    if !looks_ok {
        return Err("downloaded file does not look like a pkmn-lab program; kept the current version".into());
    }
    std::fs::write(&new, &bytes).map_err(|e| format!("{}: {e}", new.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("could not move the current program aside: {e}"))?;
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe); // put the working version back
        return Err(format!("could not install the update: {e}"));
    }
    Ok(())
}

fn stamp_file(exe: &Path) -> PathBuf {
    exe.with_extension("update-check")
}

pub enum Outcome {
    NotDue,
    UpToDate,
    Installed(String),
}

/// Check GitHub (at most once a day unless `force`) and install a newer release next to the running exe.
pub fn check(force: bool) -> Result<Outcome, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(exe.with_extension("old")); // leftover from the last update
    let stamp = stamp_file(&exe);
    let due = std::fs::metadata(&stamp)
        .and_then(|m| m.modified())
        .map(|t| SystemTime::now().duration_since(t).unwrap_or_default() > CHECK_EVERY)
        .unwrap_or(true);
    if !force && !due { return Ok(Outcome::NotDue); }
    let _ = std::fs::write(&stamp, b"");
    let (tag, url) = latest()?;
    if version(&tag) <= version(env!("CARGO_PKG_VERSION")) { return Ok(Outcome::UpToDate); }
    let url = url.ok_or(format!("release {tag} has no download for {}", asset_name()))?;
    install(&url, &exe)?;
    Ok(Outcome::Installed(tag))
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions_compare_numerically() {
        use super::version;
        assert!(version("v0.10.0") > version("0.9.9"));
        assert!(version("0.1.0") <= version("v0.1.0"));
    }

    #[test]
    fn curl_is_found() {
        assert!(super::curl(&["--version"]).is_ok_and(|o| o.starts_with(b"curl")));
    }
}

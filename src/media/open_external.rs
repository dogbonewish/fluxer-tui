use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn video_extension_from_label(label: &str) -> &'static str {
    let ext = Path::new(label)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mp4") => "mp4",
        Some("webm") => "webm",
        Some("mov") => "mov",
        Some("mkv") => "mkv",
        Some("avi") => "avi",
        Some("m4v") => "m4v",
        Some("ogv") => "ogv",
        Some("gif") => "gif",
        _ => "mp4",
    }
}

/// Write bytes to a unique file under the system temp dir (for opening with an external app).
pub fn write_temp_video_bytes(label: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let ext = video_extension_from_label(label);
    let uniq = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("fluxer-tui-video-{uniq}.{ext}");
    let path = std::env::temp_dir().join(name);
    fs::write(&path, bytes)?;
    Ok(path)
}

fn command_ok(name: &str, st: std::process::ExitStatus) -> io::Result<()> {
    if st.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{name} exited with {st}")))
    }
}

/// Open a file with the desktop default application (video player, etc.).
pub fn open_file_path(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let st = Command::new("open").arg(path).status()?;
        return command_ok("open", st);
    }
    #[cfg(target_os = "windows")]
    {
        let s = path.as_os_str().to_string_lossy().into_owned();
        let st = Command::new("cmd").args(["/C", "start", "", &s]).status()?;
        return command_ok("cmd /C start", st);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let st = Command::new("xdg-open").arg(path).status()?;
        return command_ok("xdg-open", st);
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(unix, not(target_os = "macos"))
    )))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no open command for this OS",
        ))
    }
}

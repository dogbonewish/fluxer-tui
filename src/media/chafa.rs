use anyhow::{Context, Result, bail};
use std::io::Write;
use std::process::Command;

pub async fn chafa_from_bytes(bytes: &[u8], cols: u16, rows: u16) -> Result<Vec<String>> {
    let cols = cols.max(12);
    let rows = rows.max(6);

    let mut tmp = tempfile::Builder::new()
        .prefix("fluxer-tui-img-")
        .suffix(".bin")
        .tempfile()
        .context("temp file")?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    let path = tmp.path().to_path_buf();

    let size = format!("{cols}x{rows}");
    let out = tokio::task::spawn_blocking(move || -> Result<std::process::Output> {
        let _keep = tmp;
        Command::new("chafa")
            .arg("--format=symbols")
            .arg("--polite=off")
            .arg("--work=9")
            .arg("--size")
            .arg(&size)
            .arg(&path)
            .output()
            .context("failed to run chafa (install from https://hpjansson.org/chafa/)")
    })
    .await
    .context("chafa task")??;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim();
        if msg.is_empty() {
            bail!("chafa exited with {}", out.status);
        }
        bail!("chafa: {msg}");
    }

    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().map(|l| l.to_string()).collect())
}

// The way
// Your kisses tasted
// Skeeved me the Hell out
// Like shitting naked
// Why would
// I wanna stay friends?
// Rather get raped by
// Clowns again

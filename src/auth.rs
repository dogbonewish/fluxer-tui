use crate::api::client::FluxerHttpClient;
use crate::api::types::UserPrivateResponse;
use crate::config::AppConfig;
use anyhow::{Context, Result, bail};
use std::io::{self, Write};
use std::process::Command;
use tokio::time::{Duration, sleep};

pub struct AuthContext {
    pub token: String,
    pub me: UserPrivateResponse,
}

pub async fn ensure_auth(
    base_client: &FluxerHttpClient,
    config: &mut AppConfig,
    cli_token: Option<String>,
    webapp_url: &str,
) -> Result<AuthContext> {
    if let Some(token) = cli_token.or_else(|| config.token.clone()) {
        let client = base_client.with_token(token.clone());
        if let Ok(me) = client.current_user().await {
            config.token = Some(token.clone());
            return Ok(AuthContext { token, me });
        }
        eprintln!("Stored token was rejected, starting browser login.");
    }

    let handoff = base_client
        .handoff_initiate()
        .await
        .context("failed to initiate browser handoff")?;

    let code = &handoff.code;
    let formatted = if code.len() == 8 {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.clone()
    };

    copy_to_clipboard(&formatted);

    eprintln!();
    eprintln!("  Your login code: {formatted}");
    eprintln!("  (copied to clipboard)");
    eprintln!();
    eprintln!("  Opening your browser to complete login...");
    eprintln!("  If the browser doesn't open, go to:");
    eprintln!("  {webapp_url}/login?desktop_handoff=1");
    eprintln!();

    let login_url = format!("{webapp_url}/login?desktop_handoff=1");
    let _ = open_url(&login_url);

    eprint!("  Waiting for browser login");
    io::stderr().flush().ok();

    let max_attempts = 150; // 5 minutes at 2s intervals
    for _ in 0..max_attempts {
        sleep(Duration::from_secs(2)).await;
        eprint!(".");
        io::stderr().flush().ok();

        match base_client.handoff_status(code).await {
            Ok(status) if status.status == "completed" => {
                eprintln!(" done!");
                let token = status
                    .token
                    .ok_or_else(|| anyhow::anyhow!("handoff completed without a token"))?;

                let me = base_client
                    .with_token(token.clone())
                    .current_user()
                    .await
                    .context("handoff succeeded but user verification failed")?;

                config.token = Some(token.clone());
                return Ok(AuthContext { token, me });
            }
            Ok(status) if status.status == "expired" => {
                eprintln!();
                bail!("login code expired, please try again");
            }
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    eprintln!();
    bail!("timed out waiting for browser login")
}

fn copy_to_clipboard(text: &str) {
    // try wayland first, then X11
    let attempts: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (cmd, args) in attempts {
        if let Ok(mut child) = Command::new(cmd)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "start";

    Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to open browser")?;
    Ok(())
}

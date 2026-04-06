use crate::config::ProviderConfig;
use super::ApiEvent;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

const PROFILE_FILE: &str = if cfg!(windows) { "nul" } else { "/dev/null" };

pub async fn stream(
    cfg: &ProviderConfig,
    query: &str,
    system_prompt: &str,
    tx: mpsc::UnboundedSender<ApiEvent>,
) {
    let model_id = &cfg.model;
    let repo = &cfg.huggingface_repo;

    if repo.is_empty() || model_id.is_empty() {
        let _ = tx.send(ApiEvent::Error(
            "litert_lm requires both 'huggingface_repo' and 'model' in config".into(),
        ));
        return;
    }

    // Use `litert-lm list` to check CLI availability and whether the model is already imported.
    let list = Command::new("litert-lm")
        .arg("list")
        .env("LLVM_PROFILE_FILE", PROFILE_FILE)
        .output()
        .await;

    let needs_import = match list {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            !stdout.lines().any(|line| line.contains(model_id.as_str()))
        }
        Ok(_) => true,
        Err(_) => {
            let _ = tx.send(ApiEvent::Error(
                "litert-lm CLI not found. Install: https://ai.google.dev/edge/litert-lm/cli"
                    .into(),
            ));
            return;
        }
    };

    if needs_import {
        let import = Command::new("litert-lm")
            .args(["import", "--from-huggingface-repo", repo, model_id])
            .env("LLVM_PROFILE_FILE", PROFILE_FILE)
            .output()
            .await;

        match import {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let msg = String::from_utf8_lossy(&o.stderr);
                let _ = tx.send(ApiEvent::Error(format!("litert-lm import failed: {msg}")));
                return;
            }
            Err(e) => {
                let _ = tx.send(ApiEvent::Error(format!("litert-lm import error: {e}")));
                return;
            }
        }
    }

    let prompt = format!("{system_prompt}\n\n{query}");

    let child = Command::new("litert-lm")
        .arg("run")
        .arg(model_id)
        .arg(format!("--prompt={prompt}"))
        .env("LLVM_PROFILE_FILE", PROFILE_FILE)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(ApiEvent::Error(format!("Failed to run litert-lm: {e}")));
            return;
        }
    };

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        let mut first = true;
        while let Ok(Some(line)) = lines.next_line().await {
            if first {
                first = false;
            } else if tx.send(ApiEvent::Delta("\n".into())).is_err() {
                return;
            }
            if tx.send(ApiEvent::Delta(line)).is_err() {
                return;
            }
        }
    }

    match child.wait().await {
        Ok(s) if s.success() => {
            let _ = tx.send(ApiEvent::Done);
        }
        Ok(_) => {
            let _ = tx.send(ApiEvent::Error("litert-lm exited with an error".into()));
        }
        Err(e) => {
            let _ = tx.send(ApiEvent::Error(format!("litert-lm error: {e}")));
        }
    }
}

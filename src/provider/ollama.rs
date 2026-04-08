use super::ApiEvent;
use crate::config::ProviderConfig;
use tokio::sync::mpsc;

pub async fn stream(
    cfg: &ProviderConfig,
    query: &str,
    system_prompt: &str,
    tx: mpsc::UnboundedSender<ApiEvent>,
) {
    let client = reqwest::Client::new();
    let base_url = if cfg.api_url.is_empty() {
        "http://localhost:11434"
    } else {
        cfg.api_url.trim_end_matches('/')
    };
    let url = format!("{base_url}/api/chat");

    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": query}
        ],
        "stream": true,
    });

    let request = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body);

    let Some(resp) = super::send_request(request, &tx).await else {
        return;
    };

    // Ollama streams newline-delimited JSON (not SSE).
    // Each line: {"message":{"role":"assistant","content":"token"},"done":false}
    let mut resp = resp;
    let mut buf = String::new();

    loop {
        match resp.chunk().await {
            Ok(Some(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim().to_string();
                    buf = buf[pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }
                    let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
                        continue;
                    };

                    if let Some(text) = json["message"]["content"].as_str() {
                        if !text.is_empty() && tx.send(ApiEvent::Delta(text.to_string())).is_err() {
                            return;
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                let _ = tx.send(ApiEvent::Error(e.to_string()));
                return;
            }
        }
    }

    let _ = tx.send(ApiEvent::Done);
}

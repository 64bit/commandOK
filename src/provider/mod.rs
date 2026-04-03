pub mod claude;
pub mod gemini;
pub mod ollama;
pub mod openai;

use crate::config::ProviderConfig;
use tokio::sync::mpsc;

pub enum ApiEvent {
    Delta(String),
    Done,
    Error(String),
}

#[derive(Clone)]
pub enum Provider {
    Anthropic(ProviderConfig),
    OpenAi(ProviderConfig),
    Google(ProviderConfig),
    Ollama(ProviderConfig),
}

impl Provider {
    pub fn from_name(name: &str, cfg: &ProviderConfig) -> Self {
        match name {
            "anthropic" => Provider::Anthropic(cfg.clone()),
            "openai" => Provider::OpenAi(cfg.clone()),
            "google" => Provider::Google(cfg.clone()),
            "ollama" => Provider::Ollama(cfg.clone()),
            _ => unreachable!("validated in config"),
        }
    }

    pub async fn stream(
        &self,
        query: &str,
        system_prompt: &str,
        tx: mpsc::UnboundedSender<ApiEvent>,
    ) {
        match self {
            Provider::Anthropic(cfg) => claude::stream(cfg, query, system_prompt, tx).await,
            Provider::OpenAi(cfg) => openai::stream(cfg, query, system_prompt, tx).await,
            Provider::Google(cfg) => gemini::stream(cfg, query, system_prompt, tx).await,
            Provider::Ollama(cfg) => ollama::stream(cfg, query, system_prompt, tx).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Send a request; on failure, push an Error event and return None.
pub async fn send_request(
    request: reqwest::RequestBuilder,
    tx: &mpsc::UnboundedSender<ApiEvent>,
) -> Option<reqwest::Response> {
    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(ApiEvent::Error(e.to_string()));
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let _ = tx.send(ApiEvent::Error(format!("{status}: {text}")));
        return None;
    }

    Some(resp)
}

/// Parse an SSE byte stream. For every `data: <json>` line, call `extract`
/// to pull out the text delta. Sends `Delta` events for each piece of text
/// and a final `Done` when the stream closes.
pub async fn parse_sse_stream(
    mut resp: reqwest::Response,
    tx: &mpsc::UnboundedSender<ApiEvent>,
    extract: impl Fn(&serde_json::Value) -> Option<String>,
) {
    let mut buf = String::new();

    loop {
        match resp.chunk().await {
            Ok(Some(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim_end_matches('\r').to_string();
                    buf = buf[pos + 1..].to_string();

                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    // OpenAI sends "data: [DONE]" — skip non-JSON payloads.
                    let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                        continue;
                    };

                    if let Some(text) = extract(&json) {
                        if tx.send(ApiEvent::Delta(text)).is_err() {
                            return; // receiver dropped
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

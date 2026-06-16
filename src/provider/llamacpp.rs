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
        "http://localhost:8080"
    } else {
        cfg.api_url.trim_end_matches('/')
    };
    let url = format!("{base_url}/v1/chat/completions");

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

    super::parse_sse_stream(resp, &tx, |json| {
        json["choices"][0]["delta"]["content"]
            .as_str()
            .map(String::from)
    })
    .await;
}

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
    let body = serde_json::json!({
        "model": cfg.model,
        "max_tokens": 256,
        "stream": true,
        "system": system_prompt,
        "messages": [{"role": "user", "content": query}]
    });

    let request = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body);

    let Some(resp) = super::send_request(request, &tx).await else {
        return;
    };

    super::parse_sse_stream(resp, &tx, |json| {
        if json["type"] == "content_block_delta" {
            json["delta"]["text"].as_str().map(String::from)
        } else {
            None
        }
    })
    .await;
}

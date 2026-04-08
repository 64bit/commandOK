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
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
        cfg.model, cfg.api_key
    );
    let body = serde_json::json!({
        "contents": [{"role": "user", "parts": [{"text": query}]}],
        "systemInstruction": {"parts": [{"text": system_prompt}]},
    });

    let request = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body);

    let Some(resp) = super::send_request(request, &tx).await else {
        return;
    };

    super::parse_sse_stream(resp, &tx, |json| {
        json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(String::from)
    })
    .await;
}

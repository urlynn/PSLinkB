//! Twitch 公开 GQL

use crate::log;

/// Client-ID
const TWITCH_GQL_CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
const TWITCH_GQL_URL: &str = "https://gql.twitch.tv/gql";

/// 从 stream_key(live_<id>_<token>...)解析 broadcaster_id,失败返回 None
pub fn parse_broadcaster_id(stream_key: &str) -> Option<String> {
    let rest = stream_key.strip_prefix("live_")?;
    let id = rest.split('_').next()?;
    if id.bytes().all(|b| b.is_ascii_digit()) && id.bytes().next().is_some() {
        Some(id.to_string())
    } else {
        None
    }
}

/// 查 Twitch 直播标题
pub async fn fetch_live_title(broadcaster_id: &str) -> Option<String> {
    let query = "query($id:ID!){ user(id:$id){ lastBroadcast { title } } }";
    let body = serde_json::json!({
        "query": query,
        "variables": { "id": broadcaster_id },
    })
    .to_string();

    let client = reqwest::Client::new();
    let resp = match client
        .post(TWITCH_GQL_URL)
        .header("Client-ID", TWITCH_GQL_CLIENT_ID)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log!(warn, "Twitch: GQL 请求失败 - {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            log!(warn, "Twitch: GQL 响应解析失败 - {}", e);
            return None;
        }
    };

    match json["data"]["user"]["lastBroadcast"]["title"].as_str() {
        Some(t) if !t.is_empty() => Some(t.to_string()),
        _ => None,
    }
}

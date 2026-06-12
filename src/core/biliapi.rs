//! 错误统一走 AppError::BiliAPI(operation, code, message)。

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Deserialize;
use crate::core::error::AppError;
use crate::core::error::bili_common_error;

// ————————————————————————————————————————————————————————————
// 常量
// ————————————————————————————————————————————————————————————

const APP_KEY: &str = "aae92bc66f3edfab";
const APP_SEC: &str = "af125a0d5279fd576c1b4418a3e8276d";

// ————————————————————————————————————————————————————————————
// 公共数据结构
// ————————————————————————————————————————————————————————————

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub uname: String,
    pub uid: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamInfo {
    Offline,
    FakeLive,
    Live,
}

pub enum StartLiveResult {
    Success { rtmp_url: String, stream_key: String },
    Failed { code: i32, message: String, face_auth_url: Option<String> },
}

// ————————————————————————————————————————————————————————————
// 内部类型 - 反序列化
// ————————————————————————————————————————————————————————————

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BiliApiResp<T> {
    code: i32,
    message: String,
    msg: Option<String>,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    curr_version: String,
    build: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StartLiveData {
    change: i32,
    status: String,
    rtmp: RtmpInfo,
    #[serde(default)]
    qr: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RtmpInfo {
    addr: String,
    code: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StopLiveData {
    change: i32,
    status: String,
}

// ————————————————————————————————————————————————————————————
// 签名 or 工具
// ————————————————————————————————————————————————————————————

/// 简单 URL 编码：非 ASCII 字母数字 + 保留字符 -> %XX
fn url_encode(s: &str) -> String {
    s.bytes().map(|b| {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            (b as char).to_string()
        } else {
            format!("%{:02X}", b)
        }
    }).collect()
}

fn get_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

///取错误消息：msg -> message -> "" fallback
fn bili_api_msg(json: &serde_json::Value) -> &str {
    json["msg"].as_str()
        .or_else(|| json["message"].as_str())
        .unwrap_or("")
}

fn calculate_sign(params: &BTreeMap<String, String>) -> String {
    let query_string: String = params.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    let sign_string = format!("{}{}", query_string, APP_SEC);
    let sign = format!("{:x}", md5::compute(sign_string.as_bytes()));
    sign
}

fn build_signed_params(mut params: BTreeMap<String, String>) -> String {
    params.insert("appkey".to_string(), APP_KEY.to_string());
    let sign = calculate_sign(&params);
    params.insert("sign".to_string(), sign);
    params.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

// ————————————————————————————————————————————————————————————
// 获取用户信息（验证 Cookie）
// ————————————————————————————————————————————————————————————

const OP_USER_INFO: &str = "GetUserInfo";

pub async fn get_user_info(cookie_str: &str) -> Result<Option<UserInfo>, AppError> {
    let client = reqwest::Client::builder().cookie_store(true).build()?;
    let json: serde_json::Value = client
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header("Cookie", cookie_str)
        .send().await?.json().await?;

    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        // -101: 未登录 -> Ok(None) 触发扫码；其他: 上报错误
        if code == -101 {
            return Ok(None);
        }
        let msg = bili_common_error(code).unwrap_or(bili_api_msg(&json));
        return Err(AppError::bili_api(OP_USER_INFO, code, msg));
    }

    let uname = json["data"]["uname"].as_str().unwrap_or("?").to_string();
    let uid   = json["data"]["mid"].as_i64().unwrap_or(0);
    if uid == 0 {
        return Err(AppError::bili_api(OP_USER_INFO, 0, "响应结构异常: mid=0"));
    }
    Ok(Some(UserInfo { uname, uid }))
}

// ————————————————————————————————————————————————————————————
// 获取直播间 ID (通过 UID)
// ————————————————————————————————————————————————————————————

const OP_GET_ROOM_ID: &str = "GetRoomId";

pub async fn get_room_id(uid: i64) -> Result<u64, AppError> {
    let json: serde_json::Value = reqwest::get(&format!(
        "https://api.live.bilibili.com/room/v1/Room/get_status_info_by_uids?uids[]={}", uid
    )).await?.json().await?;

    let code = json["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        let msg = bili_common_error(code).unwrap_or(bili_api_msg(&json));
        return Err(AppError::bili_api(OP_GET_ROOM_ID, code, msg));
    }

    let uid_key = uid.to_string();
    if let Some(info) = json["data"].get(&uid_key) {
        if let Some(room_id) = info["room_id"].as_u64() {
            if room_id > 0 { return Ok(room_id); }
        }
    }
    Err(AppError::bili_api(OP_GET_ROOM_ID, 0, "该账号未开通直播间"))
}

// ————————————————————————————————————————————————————————————
// 检测推流状态 (getRoomPlayInfo)
// ————————————————————————————————————————————————————————————

const OP_GET_STREAM_INFO: &str = "GetStreamInfo";

pub async fn get_stream_info(room_id: u64) -> Result<StreamInfo, AppError> {
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo\
         ?room_id={}&protocol=0,1&format=0,1,2&codec=0,1&qn=10000&platform=web&ptype=8",
        room_id
    );
    let json: serde_json::Value = reqwest::get(&url).await?.json().await?;

    let code = json["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        let msg = bili_common_error(code).unwrap_or(bili_api_msg(&json));
        return Err(AppError::bili_api(OP_GET_STREAM_INFO, code, msg));
    }
    // serde_json::Value::Null 索引任意路径仍返回 Null，被 is_null() 捕获
    let pi = &json["data"]["playurl_info"];
    if pi.is_null() { return Ok(StreamInfo::Offline); }
    if let Some(streams) = pi["playurl"]["stream"].as_array() {
        let format_count: usize = streams.iter()
            .filter_map(|s| s["format"].as_array())
            .map(|f| f.len())
            .sum();
        if format_count >= 3 { return Ok(StreamInfo::Live); }
    }
    Ok(StreamInfo::FakeLive)
}

/// FLV 探测 Fallback (playUrl)
pub async fn flv_probe(room_id: u64) -> bool {
    let Ok(resp) = reqwest::get(&format!(
        "https://api.live.bilibili.com/room/v1/Room/playUrl?cid={}&qn=10000&platform=web", room_id
    )).await else { return false; };
    let Ok(json) = resp.json::<serde_json::Value>().await else { return false; };
    if json["code"].as_i64() != Some(0) { return false; }

    let Some(url) = json["data"]["durl"][0]["url"].as_str() else { return false; };

    let Ok(resp) = reqwest::Client::new()
        .head(url)
        .header("Referer", "https://live.bilibili.com/")
        .header("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(3))
        .send().await else { return false; };

    resp.status().as_u16() == 200
}

// ————————————————————————————————————————————————————————————
// 更新直播间信息
// ————————————————————————————————————————————————————————————

const OP_UPDATE_ROOM: &str = "UpdateRoom";

pub async fn update_room(
    cookie_str: &str, csrf: &str, room_id: u64,
    title: Option<&str>, area_id: Option<&str>,
) -> Result<(), AppError> {
    let client = reqwest::Client::new();
    let mut body = format!("room_id={}&csrf={}&csrf_token={}", room_id, csrf, csrf);
    if let Some(t) = title { body.push_str(&format!("&title={}", url_encode(t))); }
    if let Some(a) = area_id { body.push_str(&format!("&area_id={}", url_encode(a))); }

    let json: serde_json::Value = client
        .post("https://api.live.bilibili.com/room/v1/Room/update")
        .header("Cookie", cookie_str)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body).send().await?.json().await?;

    let code = json["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        let msg = bili_common_error(code).unwrap_or(bili_api_msg(&json));
        return Err(AppError::bili_api(OP_UPDATE_ROOM, code, msg));
    }
    Ok(())
}

// ————————————————————————————————————————————————————————————
// 获取直播姬版本
// ————————————————————————————————————————————————————————————

const OP_GET_LIVE_VERSION: &str = "GetLiveVersion";

pub async fn get_live_version(client: &reqwest::Client) -> Result<(String, i64), AppError> {
    let ts = get_timestamp().to_string();
    let mut params = BTreeMap::new();
    params.insert("system_version".to_string(), "2".to_string());
    params.insert("ts".to_string(), ts);
    let signed = build_signed_params(params);
    let url = format!("https://api.live.bilibili.com/xlive/app-blink/v1/liveVersionInfo/getHomePageLiveVersion?{}", signed);
    let body = client.get(&url).send().await?.text().await?;
    let resp: BiliApiResp<VersionData> = serde_json::from_str(&body)?;
    if resp.code != 0 {
        return Err(AppError::bili_api(OP_GET_LIVE_VERSION, resp.code as i64, resp.message));
    }
    let data = resp.data.ok_or_else(||
        AppError::bili_api(OP_GET_LIVE_VERSION, 0, "响应结构异常: data=null"))?;
    Ok((data.curr_version, data.build))
}

// ————————————————————————————————————————————————————————————
// 开始直播
// ————————————————————————————————————————————————————————————

const OP_START_LIVE: &str = "StartLive";

pub enum StartLiveMode { Full, Simple }

pub async fn start_live(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, uid: Option<&str>,
    room_id: u64, area_v2: &str, title: Option<&str>,
    mode: StartLiveMode,
) -> Result<StartLiveResult, AppError> {

    let mut params = BTreeMap::new();
    params.insert("room_id".to_string(), room_id.to_string());
    params.insert("area_v2".to_string(), area_v2.to_string());
    params.insert("platform".to_string(), "pc_link".to_string());
    params.insert("backup_stream".to_string(), "0".to_string());
    params.insert("csrf".to_string(), csrf.to_string());
    params.insert("csrf_token".to_string(), csrf.to_string());
    if let Some(t) = title { if !t.is_empty() { params.insert("title".to_string(), t.to_string()); } }

    let body_and_mode = match mode {
        StartLiveMode::Full => {
            let (version, build) = get_live_version(client).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            params.insert("version".to_string(), version);
            params.insert("build".to_string(), build.to_string());
            params.insert("ts".to_string(), get_timestamp().to_string());
            (build_signed_params(params), "Full")
        }
        StartLiveMode::Simple => {
            (params.into_iter().map(|(k,v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"), "Simple")
        }
    };
    let (body, mode) = body_and_mode;
    let response = client
        .post("https://api.live.bilibili.com/room/v1/Room/startLive")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://link.bilibili.com")
        .header("Referer", "https://link.bilibili.com/p/center/index")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36")
        .header("Cookie", cookie_str)
        .body(body).send().await?;

    let status = response.status();
    let body = response.text().await?;
    let api_response: BiliApiResp<StartLiveData> = serde_json::from_str(&body)?;

    match api_response.code {
        0 => {
            eprintln!("[Bili:Live] {} - {} -> {} - ✓ 开播成功", OP_START_LIVE, mode, status);
            let data = api_response.data.ok_or_else(||
                AppError::bili_api(OP_START_LIVE, 0, "响应结构异常: data=null"))?;
            Ok(StartLiveResult::Success { rtmp_url: data.rtmp.addr, stream_key: data.rtmp.code })
        }
        60024 => {
            let qr_url = api_response.data.as_ref().and_then(|d| d.qr.clone());
            Ok(StartLiveResult::Failed { code: 60024, message: api_response.message, face_auth_url: qr_url })
        }
        60043 => {
            let face_auth_url = uid.map(|u| format!(
                "https://www.bilibili.com/blackboard/live/face-auth-middle.html?source_event=400&mid={}", u));
            Ok(StartLiveResult::Failed { code: 60043, message: api_response.message, face_auth_url })
        }
        code => Ok(StartLiveResult::Failed { code, message: api_response.message, face_auth_url: None }),
    }
}

// ————————————————————————————————————————————————————————————
// 7. 关闭直播
// ————————————————————————————————————————————————————————————

const OP_STOP_LIVE: &str = "StopLive";
#[allow(dead_code)]
const OP_PLAY_URL: &str = "PlayUrl";

pub async fn stop_live(
    client: &reqwest::Client, csrf: &str, room_id: u64,
) -> Result<(), AppError> {
    let mut params = BTreeMap::new();
    params.insert("csrf".to_string(), csrf.to_string());
    params.insert("csrf_token".to_string(), csrf.to_string());
    params.insert("platform".to_string(), "pc_link".to_string());
    params.insert("room_id".to_string(), room_id.to_string());
    let signed = build_signed_params(params);

    let response = client
        .post("https://api.live.bilibili.com/room/v1/Room/stopLive")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json, text/plain, */*")
        .header("Origin", "https://link.bilibili.com")
        .header("Referer", "https://link.bilibili.com/p/center/index")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36")
        .body(signed).send().await?;

    let status = response.status();
    let body = response.text().await?;
    eprintln!("[Bili:Live] StopLive -> {} - ✓ 关播成功", status);
    let api_response: BiliApiResp<StopLiveData> = serde_json::from_str(&body)?;
    match api_response.code {
        0 => Ok(()),
        _ => Err(AppError::bili_api(OP_STOP_LIVE, api_response.code as i64, api_response.message)),
    }
}

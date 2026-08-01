//! 错误统一走 AppError::BiliAPI(operation, code, message)。

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Deserialize;
use reqwest::cookie::Jar;
use std::sync::Arc;
use crate::log;
use crate::core::error::AppError;
use crate::core::error::bili_common_error;

// ————————————————————————————————————————————————————————————
// 常量
// ————————————————————————————————————————————————————————————

// LiveHime
const LIVEHIME_KEY: &str = "aae92bc66f3edfab";
const LIVEHIME_SEC: &str = "af125a0d5279fd576c1b4418a3e8276d";

// Electron
const ELECTRON_KEY: &str = "aa1e74ee4874176e";
const ELECTRON_SEC: &str = "54e6a9a31b911cd5fc0daa66ebf94bc4";
const ELECTRON_BUILD: &str = "1001017006";

// UA
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 Edg/129.0.0.0";

/// 开播客户端 + 关播配对
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveClient {
    Electron,
    Livehime,
}

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
struct BiliApiResp<T> {
    code: i32,
    message: String,
    data: Option<T>,
    msg: Option<String>,
}

impl<T> BiliApiResp<T> {
    /// msg -> message fallback
    fn err_msg(&self) -> &str {
        self.msg.as_deref().unwrap_or(&self.message)
    }
}

#[derive(Debug, Deserialize)]
struct VersionData {
    curr_version: String,
    build: i64,
}

#[derive(Debug, Deserialize)]
struct StartLiveData {
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
struct StopLiveData {}

// ————————————————————————————————————————————————————————————
// 签名 or 工具
// ————————————————————————————————————————————————————————————

/// URL 编码
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

fn sign_query(params: &BTreeMap<String, String>, appsec: &str) -> String {
    let query: String = params.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    format!("{:x}", md5::compute(format!("{}{}", query, appsec).as_bytes()))
}

/// 签名
fn build_signed(mut params: BTreeMap<String, String>, appkey: &str, appsec: &str) -> String {
    params.insert("appkey".to_string(), appkey.to_string());
    let sign = sign_query(&params, appsec);
    params.insert("sign".to_string(), sign);
    params.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

/// 统一 header
fn common_headers(rb: reqwest::RequestBuilder, cookie_str: &str) -> reqwest::RequestBuilder {
    rb.header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Encoding", "identity")
        .header("Origin", "https://link.bilibili.com")
        .header("Referer", "https://link.bilibili.com/p/center/index")
        .header("User-Agent", BROWSER_UA)
        .header("Cookie", cookie_str.to_string())
}

// ————————————————————————————————————————————————————————————
// 获取用户信息 (验证 Cookie)
// ————————————————————————————————————————————————————————————

const OP_USER_INFO: &str = "GetUserInfo";

pub async fn get_user_info(cookie_str: &str) -> Result<Option<UserInfo>, AppError> {
    let client = reqwest::Client::builder().cookie_store(true).build()?;
    let resp: BiliApiResp<serde_json::Value> = client
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header("Cookie", cookie_str)
        .send().await?.json().await?;

    if resp.code != 0 {
        if resp.code == -101 { return Ok(None); }
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api(OP_USER_INFO, resp.code as i64, msg));
    }

    let uname = resp.data.as_ref().and_then(|d| d["uname"].as_str()).unwrap_or("?").to_string();
    let uid   = resp.data.as_ref().and_then(|d| d["mid"].as_i64()).unwrap_or(0);
    if uid == 0 {
        return Err(AppError::bili_api(OP_USER_INFO, resp.code as i64, resp.err_msg()));
    }
    Ok(Some(UserInfo { uname, uid }))
}

// ————————————————————————————————————————————————————————————
// 获取直播间 ID (通过 UID)
// ————————————————————————————————————————————————————————————

const OP_GET_ROOM_ID: &str = "GetRoomId";

pub async fn get_room_id(uid: i64) -> Result<u64, AppError> {
    let resp: BiliApiResp<serde_json::Value> = reqwest::get(&format!(
        "https://api.live.bilibili.com/room/v1/Room/get_status_info_by_uids?uids[]={}", uid
    )).await?.json().await?;

    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api(OP_GET_ROOM_ID, resp.code as i64, msg));
    }

    let uid_key = uid.to_string();
    if let Some(info) = resp.data.as_ref().and_then(|d| d.get(&uid_key))
        && let Some(room_id) = info["room_id"].as_u64()
        && room_id > 0
    {
        return Ok(room_id);
    }
    Err(AppError::bili_api(OP_GET_ROOM_ID, resp.code as i64, resp.err_msg()))
}

// ————————————————————————————————————————————————————————————
// 直播分区列表
// ————————————————————————————————————————————————————————————

const OP_GET_AREA_LIST: &str = "GetAreaList";

#[derive(Debug, Deserialize)]
pub struct AreaParent {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub list: Vec<AreaChild>,
}

#[derive(Debug, Deserialize)]
pub struct AreaChild {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: String,
    #[serde(default)]
    pub parent_name: String,
}

pub async fn get_area_list() -> Result<Vec<AreaParent>, AppError> {
    let resp: BiliApiResp<Vec<AreaParent>> = reqwest::get(
        "https://api.live.bilibili.com/room/v1/Area/getList"
    ).await?.json().await?;

    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api(OP_GET_AREA_LIST, resp.code as i64, msg));
    }
    resp.data.ok_or_else(|| AppError::bili_api(OP_GET_AREA_LIST, 0, ""))
}

/// 查找子分区 id
pub fn resolve_area_id(list: &[AreaParent], target: &str) -> Option<String> {
    for parent in list {
        for child in &parent.list {
            if child.name == target {
                return Some(child.id.clone());
            }
        }
    }
    None
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
    let resp: BiliApiResp<serde_json::Value> = reqwest::get(&url).await?.json().await?;

    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api(OP_GET_STREAM_INFO, resp.code as i64, msg));
    }
    let Some(ref data) = resp.data else { return Ok(StreamInfo::Offline) };
    let pi = &data["playurl_info"];
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
) -> Result<Option<String>, AppError> {
    let client = reqwest::Client::new();
    let mut body = format!("room_id={}&csrf={}&csrf_token={}", room_id, csrf, csrf);
    if let Some(t) = title { body.push_str(&format!("&title={}", url_encode(t))); }
    if let Some(a) = area_id { body.push_str(&format!("&area_id={}", url_encode(a))); }

    let resp: BiliApiResp<serde_json::Value> = common_headers(
        client.post("https://api.live.bilibili.com/room/v1/Room/update"), cookie_str
    ).body(body).send().await?.json().await?;

    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api(OP_UPDATE_ROOM, resp.code as i64, msg));
    }

    // audit_title 存在 = 标题变动
    match resp.data.as_ref().and_then(|d| d["audit_info"]["audit_title"].as_str()) {
        Some(t) if !t.is_empty() => Ok(Some(t.to_string())),
        _ => Ok(None),
    }
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
    let signed = build_signed(params, LIVEHIME_KEY, LIVEHIME_SEC);
    let url = format!("https://api.live.bilibili.com/xlive/app-blink/v1/liveVersionInfo/getHomePageLiveVersion?{}", signed);
    let body = client.get(&url).send().await?.text().await?;
    let resp: BiliApiResp<VersionData> = serde_json::from_str(&body)?;
    if resp.code != 0 {
        return Err(AppError::bili_api(OP_GET_LIVE_VERSION, resp.code as i64, resp.message));
    }
    let data = resp.data.ok_or_else(||
        AppError::bili_api(OP_GET_LIVE_VERSION, 0, ""))?;
    Ok((data.curr_version, data.build))
}

// ————————————————————————————————————————————————————————————
// 开始直播
// ————————————————————————————————————————————————————————————

const OP_START_LIVE: &str = "StartLive";

/// 解析 startLive 响应
fn parse_start_response(body: &str, uid: Option<&str>) -> Result<StartLiveResult, AppError> {
    let api_response: BiliApiResp<StartLiveData> = serde_json::from_str(body)?;
    match api_response.code {
        0 => {
            let data = api_response.data.ok_or_else(||
                AppError::bili_api(OP_START_LIVE, 0, ""))?;
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

/// Electron 开播
pub async fn start_live_electron(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, uid: Option<&str>,
    room_id: u64, area_v2: &str, title: Option<&str>,
) -> Result<StartLiveResult, AppError> {
    let mut params = BTreeMap::new();
    params.insert("room_id".to_string(), room_id.to_string());
    params.insert("platform".to_string(), "web_electron_link".to_string());
    params.insert("area_v2".to_string(), area_v2.to_string());
    params.insert("csrf".to_string(), csrf.to_string());
    params.insert("ts".to_string(), get_timestamp().to_string());
    params.insert("build".to_string(), ELECTRON_BUILD.to_string());
    if let Some(t) = title && !t.is_empty() { params.insert("title".to_string(), t.to_string()); }
    let body = build_signed(params, ELECTRON_KEY, ELECTRON_SEC);

    let response = common_headers(
        client.post("https://api.live.bilibili.com/room/v1/Room/startLive"),
        cookie_str,
    ).body(body).send().await?;
    let status = response.status();
    let body = response.text().await?;
    let result = parse_start_response(&body, uid)?;
    if matches!(result, StartLiveResult::Success { .. }) {
        log!(ok, "[Bili:Live] {} - Electron -> {} - ✓ 开播成功", OP_START_LIVE, status);
    }
    Ok(result)
}

/// 直播姬开播
pub async fn start_live_livehime(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, uid: Option<&str>,
    room_id: u64, area_v2: &str, title: Option<&str>,
) -> Result<StartLiveResult, AppError> {
    let (version, build) = get_live_version(client).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut params = BTreeMap::new();
    params.insert("build".to_string(), build.to_string());
    params.insert("platform".to_string(), "pc_link".to_string());
    params.insert("ts".to_string(), get_timestamp().to_string());
    params.insert("version".to_string(), version);
    params.insert("area_v2".to_string(), area_v2.to_string());
    params.insert("csrf".to_string(), csrf.to_string());
    params.insert("csrf_token".to_string(), csrf.to_string());
    params.insert("room_id".to_string(), room_id.to_string());
    if let Some(t) = title && !t.is_empty() { params.insert("title".to_string(), t.to_string()); }
    let body = build_signed(params, LIVEHIME_KEY, LIVEHIME_SEC);

    let response = common_headers(
        client.post("https://api.live.bilibili.com/room/v1/Room/startLive"),
        cookie_str,
    ).body(body).send().await?;
    let status = response.status();
    let body = response.text().await?;
    let result = parse_start_response(&body, uid)?;
    if matches!(result, StartLiveResult::Success { .. }) {
        log!(ok, "[Bili:Live] {} - Livehime -> {} - ✓ 开播成功", OP_START_LIVE, status);
    }
    Ok(result)
}

// ————————————————————————————————————————————————————————————
// 人脸验证状态查询
// ————————————————————————————————————————————————————————————

pub enum FaceStatus {
    Verified,
    NotYet,
    Abnormal(String),
}

pub async fn check_face_status(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, room_id: u64,
) -> FaceStatus {
    let body = format!("room_id={}&face_auth_code=60024&csrf={}&csrf_token={}&visit_id=", room_id, csrf, csrf);
    let resp = common_headers(
        client.post("https://api.live.bilibili.com/xlive/app-blink/v1/preLive/IsUserIdentifiedByFaceAuth"), cookie_str
    ).body(body).send().await;

    let text = match resp {
        Ok(r) => match r.text().await {
            Ok(t) => t,
            Err(e) => return FaceStatus::Abnormal(e.to_string()),
        },
        Err(e) => return FaceStatus::Abnormal(e.to_string()),
    };

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return FaceStatus::Abnormal(text),
    };

    if json.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return FaceStatus::Abnormal(text);
    }
    let data = &json["data"];
    match data.get("is_identified") {
        Some(v) if v.is_boolean() => {
            if v.as_bool() == Some(true) { FaceStatus::Verified } else { FaceStatus::NotYet }
        }
        None if data.is_object() => FaceStatus::NotYet,
        _ => FaceStatus::Abnormal(text),
    }
}

// ————————————————————————————————————————————————————————————
// 7. 关闭直播
// ————————————————————————————————————————————————————————————

const OP_STOP_LIVE: &str = "StopLive";

async fn parse_stop_response(response: reqwest::Response) -> Result<(), AppError> {
    let status = response.status();
    let body = response.text().await?;
    let api_response: BiliApiResp<StopLiveData> = serde_json::from_str(&body)?;
    match api_response.code {
        0 => { log!(ok, "[Bili:Live] StopLive -> {} - ✓ 关播成功", status); Ok(()) }
        _ => Err(AppError::bili_api(OP_STOP_LIVE, api_response.code as i64, api_response.message)),
    }
}

/// Electron 关播
pub async fn stop_live_electron(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, room_id: u64,
) -> Result<(), AppError> {
    let body = format!(
        "room_id={}&platform=web_electron_link&csrf={}&csrf_token={}&visit_id=",
        room_id, csrf, csrf);
    let response = common_headers(
        client.post("https://api.live.bilibili.com/room/v1/Room/stopLive"),
        cookie_str,
    ).body(body).send().await?;
    parse_stop_response(response).await
}

/// 直播姬关播
pub async fn stop_live_livehime(
    client: &reqwest::Client, cookie_str: &str, csrf: &str, room_id: u64,
) -> Result<(), AppError> {
    let mut params = BTreeMap::new();
    params.insert("csrf".to_string(), csrf.to_string());
    params.insert("csrf_token".to_string(), csrf.to_string());
    params.insert("platform".to_string(), "pc_link".to_string());
    params.insert("room_id".to_string(), room_id.to_string());
    let body = build_signed(params, LIVEHIME_KEY, LIVEHIME_SEC);
    let response = common_headers(
        client.post("https://api.live.bilibili.com/room/v1/Room/stopLive"),
        cookie_str,
    ).body(body).send().await?;
    parse_stop_response(response).await
}

// ————————————————————————————————————————————————————————————
// QR 扫码登录
// ————————————————————————————————————————————————————————————

#[derive(Debug, Deserialize)]
pub struct QrCodeData {
    pub url: String,
    #[serde(alias = "oauth_key")]
    pub qrcode_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ScanStatusData {
    pub url: String,
    pub code: i64,
}

pub async fn generate_qr(client: &reqwest::Client) -> Result<(String, String), AppError> {
    let resp: BiliApiResp<QrCodeData> = client
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
        .send().await?.json().await?;
    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api("QrLogin", resp.code as i64, msg));
    }
    let data = resp.data.ok_or_else(|| AppError::bili_api("QrLogin", 0, ""))?;
    Ok((data.url, data.qrcode_key))
}

pub async fn poll_qr_status(client: &reqwest::Client, key: &str) -> Result<ScanStatusData, AppError> {
    let url = format!(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}", key);
    let resp: BiliApiResp<ScanStatusData> = client.get(&url).send().await?.json().await?;
    if resp.code != 0 {
        let msg = bili_common_error(resp.code as i64).unwrap_or(resp.err_msg());
        return Err(AppError::bili_api("QrLogin", resp.code as i64, msg));
    }
    resp.data.ok_or_else(|| AppError::bili_api("QrLogin", 0, ""))
}

pub(crate) fn qr_login_client() -> Result<(reqwest::Client, Arc<Jar>), AppError> {
    let jar = Arc::new(Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(Arc::clone(&jar))
        .build()?;
    Ok((client, jar))
}

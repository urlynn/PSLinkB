//! 状态机：事件 -> 状态 -> 副作用

use crate::config::Config;
use crate::config::rtmp_url;
use crate::core::effect::Effect;
use crate::core::event::Event;
use crate::core::error::start_live_error;
use crate::core::blive::LiveMode;
use crate::core::biliapi::LiveClient;
use crate::core::state::GlobalState;
use crate::log;
use tokio::sync::watch;

/// 系统状态
#[derive(Debug, Clone)]
enum State {
    /// 空闲
    Idle,
    /// PS5 推流中 — Manual 模式等待手动开播 | Auto 模式开播失败后暂存
    Ps5Streaming,
    /// PS5 已推流 - 开播 API 调用中
    LivePreparing {
        app: String,
        stream_key: String,
    },
    /// PS5 已推流 - 等人脸验证 (60024/60043), watcher 在轮询
    WaitingFaceAuth {
        app: String,
        stream_key: String,
    },
    /// 直播中 — FFmpeg 推流 + 弹幕运行中
    Live {
        app: String,
        ps5_key: String,
        bili_url: String,
        bili_key: String,
        retried: bool,
        client: LiveClient,
    },
}

/// FFmpeg 命令
#[derive(Debug)]
pub enum FfmpegCmd {
    Start {
        ps5_app: String,
        ps5_stream_key: String,
        bilibili_rtmp_url: String,
        bilibili_stream_key: String,
    },
    Stop,
}

/// B站 API 命令
#[derive(Debug)]
pub enum BilibiliCmd {
    StartLive {
        room_id: u64,
        area_v2: String,
    },
    StopLive {
        room_id: u64,
        client: LiveClient,
    },
    /// 启动人脸验证 watcher (1s 探测)
    StartFaceWatch {
        room_id: u64,
    },
    /// 停止人脸验证 watcher
    StopFaceWatch,
    /// Twitch 标题同步 B站直播间
    SyncTwitchTitle {
        room_id: u64,
        broadcaster_id: String,
    },
    /// 更新 B站直播间信息
    UpdateRoom {
        room_id: u64,
        title: Option<String>,
        area: Option<String>,
    },
}

/// 弹幕命令
#[derive(Debug)]
pub enum DanmakuCmd {
    Start { room_id: u64 },
    Stop,
}

/// 系统状态机
pub struct System {
    state: State,
    pub(crate) config: Config,
    local_ip: String,
    state_rx: watch::Receiver<GlobalState>,
    notify_queue: Vec<String>,
}

impl System {
    pub fn new(config: Config, state_rx: watch::Receiver<GlobalState>) -> Self {
        Self {
            state: State::Idle,
            config,
            local_ip: crate::utils::ip::local_ip(),
            state_rx,
            notify_queue: Vec::new(),
        }
    }

    /// 处理一个事件，返回需要执行的副作用列表 - 系统业务逻辑
    pub fn handle(&mut self, event: Event) -> Vec<Effect> {
        let mut effects = self.transition(event);
        if !self.state_rx.borrow().channel_name.is_empty() {
            for msg in self.notify_queue.drain(..) {
                effects.push(Effect::NotifyPs5(msg));
            }
        }
        effects
    }

    // ── 内部：状态转换逻辑 ──

    fn transition(&mut self, event: Event) -> Vec<Effect> {
        let current_state = self.state.clone();
        match (&current_state, event) {
            // ————————————————————————————————————————————
            // PS5 开始推流
            // ————————————————————————————————————————————
            (State::Idle, Event::RtmpPublish { app, stream_key }) => {
                let app_c = app.clone();
                let key_c = stream_key.clone();
                let url = rtmp_url(&self.local_ip, &app, &stream_key);
                crate::luci::set("live", &format!(r#"{{"status":"{}"}}"#, url));
                let mut effects = vec![Effect::Log(format!(
                    "Stream Url - {}",
                    rtmp_url(&self.local_ip, &app, &stream_key)
                ))];

                if self.config.live.live_mode == LiveMode::Auto {
                    self.state = State::LivePreparing {
                        app: app_c.clone(),
                        stream_key: key_c.clone(),
                    };
                    effects.push(Effect::BilibiliStartLive {
                        room_id: self.config.live.room_id,
                        area_v2: self.config.live.area_v2.clone(),
                    });
                } else {
                    self.state = State::Ps5Streaming;
                    if self.config.live.live_mode == LiveMode::Manual {
                        effects.push(Effect::StartDanmaku { room_id: self.config.live.room_id });
                    }
                }

                effects
            }

            // ————————————————————————————————————————————
            // PS5 停止推流 — 回到 Idle
            // ————————————————————————————————————————————
            (
                State::Ps5Streaming
                | State::LivePreparing { .. }
                | State::WaitingFaceAuth { .. }
                | State::Live { .. },
                Event::RtmpUnpublish,
            ) => {
                let was_live = matches!(
                    self.state,
                    State::LivePreparing { .. } | State::Live { .. }
                );
                let mut effects = self.cleanup(was_live);
                effects.push(Effect::Log("PS5 stream ended - System cleaning up".into()));
                effects
            }

            // ————————————————————————————————————————————
            // 开播成功 -> 进入直播，启动 FFmpeg + 弹幕
            // ————————————————————————————————————————————
            (
                State::LivePreparing { app, stream_key }
                | State::WaitingFaceAuth { app, stream_key },
                Event::BilibiliLiveStarted {
                    rtmp_url,
                    stream_key: bilibili_key,
                    client,
                },
            ) => {
                self.state = State::Live {
                    app: app.clone(),
                    ps5_key: stream_key.clone(),
                    bili_url: rtmp_url.clone(),
                    bili_key: bilibili_key.clone(),
                    retried: false,
                    client,
                };
                crate::luci::set("qr", r#"{"url":"","status":"done"}"#);

                let mut effects = vec![
                    Effect::StopFaceWatch,
                    Effect::StartFfmpeg {
                        ps5_app: app.clone(),
                        ps5_stream_key: stream_key.clone(),
                        bilibili_rtmp_url: rtmp_url,
                        bilibili_stream_key: bilibili_key,
                    },
                    Effect::StartDanmaku {
                        room_id: self.config.live.room_id,
                    },
                ];

                // 标题同步
                if !self.config.live.title.is_empty() {
                    effects.push(Effect::UpdateRoom {
                        room_id: self.config.live.room_id,
                        title: Some(self.config.live.title.clone()),
                    });
                } else if let Some(bid) = crate::core::twitch::parse_broadcaster_id(stream_key)
                {
                    effects.push(Effect::SyncTwitchTitle {
                        room_id: self.config.live.room_id,
                        broadcaster_id: bid,
                    });
                }

                effects
            }

            // ————————————————————————————————————————————
            // B站需要人脸验证(60024/60043) -> 进 WaitingFaceAuth, 起人脸 watcher
            // ————————————————————————————————————————————
            (State::LivePreparing { app, stream_key }, Event::BilibiliAuthRequired { face_auth_url }) => {
                self.state = State::WaitingFaceAuth { app: app.clone(), stream_key: stream_key.clone() };
                self.notify_queue.push("需要人脸验证".into());
                let mut effects = vec![
                    Effect::StartFaceWatch { room_id: self.config.live.room_id },
                    Effect::Log("需人脸验证，正在等待...".into()),
                ];
                if let Some(url) = face_auth_url {
                    effects.push(Effect::Log(format!("验证链接: {}", url)));
                }
                effects
            }
            (State::WaitingFaceAuth { .. }, Event::BilibiliAuthRequired { .. }) => {
                // 重试仍需验证 -> 继续等
                vec![]
            }

            // ————————————————————————————————————————————
            // B站开播失败 -> 退回 Ps5Streaming，通知 PS5
            // -101: cookie 失效 -> 通知 PS5 后重启
            // ————————————————————————————————————————————
            (
                State::LivePreparing { .. } | State::WaitingFaceAuth { .. },
                Event::BilibiliLiveStartFailed { code, message },
            ) => {
                let mut effects = vec![
                    Effect::StopFaceWatch,
                    Effect::Log(format!("StartLive failed ({}): {}", code, message)),
                ];

                if code == -101 {
                    self.state = State::Idle;
                    effects.push(Effect::NotifyPs5("登录已过期，请重新扫码".into()));
                    effects.push(Effect::Restart);
                } else {
                    self.state = State::Ps5Streaming;
                    self.notify_queue.push(format!("开播失败({}): {}", code, start_live_error(code, &message)));
                }
                effects
            }
            // ————————————————————————————————————————————
            // 人脸 watcher: 该再试一发开播
            // ————————————————————————————————————————————
            (State::WaitingFaceAuth { .. }, Event::TryStartLive) => {
                vec![Effect::BilibiliStartLive {
                    room_id: self.config.live.room_id,
                    area_v2: self.config.live.area_v2.clone(),
                }]
            }
            // ————————————————————————————————————————————
            // 人脸 watcher: 3min 超时 -> 回 Idle 
            // 踢不了 PS5, 单方面结束, Todo: 思考有没有我方结束方法
            // ————————————————————————————————————————————
            (State::WaitingFaceAuth { .. }, Event::FaceTimeout) => {
                let mut effects = self.cleanup(false);
                self.notify_queue.push("人脸验证超时, 请重新开播".into());
                effects.push(Effect::Log("人脸验证超时 - 放弃".into()));
                effects
            }

            // ————————————————————————————————————————————
            // 弹幕连接成功 -> 进入 PS5 通知队列
            // ————————————————————————————————————————————
            (_, Event::DanmakuReady) => {
                self.notify_queue.push("弹幕已连接".into());
                vec![]
            }

            // ————————————————————————————————————————————
            // PS5 IRC 就绪 —> 触发通知队列 drain
            // ————————————————————————————————————————————
            (_, Event::Ps5IrcReady { .. }) => {
                vec![]
            }

            // ————————————————————————————————————————————
            // B站关播结果 — 仅日志
            // ————————————————————————————————————————————
            (_, Event::BilibiliLiveStopped) => {
                vec![]
            }
            (_, Event::BilibiliLiveStopFailed { code, message }) => {
                vec![Effect::Log(format!("StopLive failed ({}): {}", code, message))]
            }

            // ————————————————————————————————————————————
            // 直播流状态确认/超时
            // ————————————————————————————————————————————
            (_, Event::BilibiliStreamConfirmed { .. }) => {
                crate::luci::clear("error");
                self.notify_queue.push("开播成功".into());
                vec![]
            }
            (_, Event::TitleUpdated(title)) => {
                vec![Effect::NotifyPs5(format!("直播间标题已更新为：{}", title))]
            }
            (State::Live { app, ps5_key, bili_url, bili_key, retried: false, client }, Event::BilibiliStreamTimeout { .. }) => {
                log!(warn, "Bili:Live: Live stream unconfirmed - FFmpeg restream");
                self.state = State::Live {
                    app: app.clone(), ps5_key: ps5_key.clone(),
                    bili_url: bili_url.clone(), bili_key: bili_key.clone(),
                    retried: true,
                    client: *client,
                };
                self.notify_queue.push("推流重试".into());
                vec![
                    Effect::StopFfmpeg,
                    Effect::StartFfmpeg {
                        ps5_app: app.clone(),
                        ps5_stream_key: ps5_key.clone(),
                        bilibili_rtmp_url: bili_url.clone(),
                        bilibili_stream_key: bili_key.clone(),
                    },
                ]
            }
            // timeout when already retried -> fall through to cleanup
            (State::Live { .. }, Event::BilibiliStreamTimeout { .. }) => {
                let mut effects = self.cleanup(true);
                effects.push(Effect::Log("Stream unconfirmed after retry".into()));
                effects
            }

            // ————————————————————————————————————————————
            // FFmpeg 错误 — system 层统一重试一次
            // ————————————————————————————————————————————
            (State::Live { app, ps5_key, bili_url, bili_key, retried: false, client }, Event::FfmpegError(status)) => {
                self.state = State::Live {
                    app: app.clone(), ps5_key: ps5_key.clone(),
                    bili_url: bili_url.clone(), bili_key: bili_key.clone(),
                    retried: true,
                    client: *client,
                };
                self.notify_queue.push("推流重试".into());
                vec![
                    Effect::StopFfmpeg,
                    Effect::StartFfmpeg {
                        ps5_app: app.clone(),
                        ps5_stream_key: ps5_key.clone(),
                        bilibili_rtmp_url: bili_url.clone(),
                        bilibili_stream_key: bili_key.clone(),
                    },
                    Effect::Log(format!("FFmpeg: {}", status.message())),
                ]
            }
            // already retried -> give up
            (State::Live { .. }, Event::FfmpegError(status)) => {
                let msg = status.message();
                let mut effects = self.cleanup(true);
                effects.push(Effect::Log(format!("FFmpeg: {}", msg)));
                self.notify_queue.push(format!("推流失败: {}", msg));
                effects
            }
            // non-Live states (e.g. LivePreparing) -> cleanup directly
            (_, Event::FfmpegError(status)) => {
                let msg = status.message();
                let was_live = matches!(self.state, State::LivePreparing { .. } | State::Live { .. });
                let mut effects = self.cleanup(was_live);
                effects.push(Effect::Log(format!("FFmpeg: {}", msg)));
                self.notify_queue.push(format!("推流失败: {}", msg));
                effects
            }

            // ————————————————————————————————————————————
            // 系统关闭 — 清理所有服务 + 关播
            // ————————————————————————————————————————————
            (_, Event::Shutdown) => {
                let was_live = matches!(
                    self.state,
                    State::LivePreparing { .. } | State::Live { .. }
                );
                self.cleanup(was_live)
            }
            // 无效转换 — 静默忽略
            _ => Vec::new(),
        }
    }

    /// 关播清理：state->Idle, luci, StopFfmpeg, StopDanmaku, BilibiliStopLive
    fn cleanup(&mut self, was_live: bool) -> Vec<Effect> {
        // 关播配对
        let client = match &self.state {
            State::Live { client, .. } => *client,
            _ => LiveClient::Electron,
        };
        self.state = State::Idle;
        self.notify_queue.clear();
        crate::luci::set("live", r#"{"status":""}"#);
        let mut effects = vec![
            Effect::StopFfmpeg,
            Effect::StopDanmaku,
            Effect::StopFaceWatch,
        ];
        if was_live {
            effects.push(Effect::BilibiliStopLive {
                room_id: self.config.live.room_id,
                client,
            });
        }
        effects
    }
}

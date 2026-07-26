//! RTMP Actor

use rtmp_rs::{RtmpHandler, RtmpServer, ServerConfig, AuthResult};
use rtmp_rs::session::{SessionContext, StreamContext};
use rtmp_rs::protocol::message::{ConnectParams, PlayParams, PublishParams};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::core::error::AppError;
use tokio::sync::{mpsc, Mutex};

/// 流事件类型
#[derive(Debug, Clone)]
pub enum StreamEventType {
    Started,
    Stopped,
}

/// 流事件
#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub app: String,
    pub stream_key: String,
    pub event_type: StreamEventType,
}

/// RTMP Actor
pub struct RtmpActor {
    port: u16,
    event_tx: mpsc::Sender<StreamEvent>,
    current_stream: Arc<Mutex<Option<String>>>,
}

impl RtmpActor {
    pub fn new(port: u16, event_tx: mpsc::Sender<StreamEvent>) -> Self {
        Self {
            port,
            event_tx,
            current_stream: Arc::new(Mutex::new(None)),
        }
    }
    
    pub async fn run(self) -> Result<(), AppError> {
        let addr: SocketAddr = format!("0.0.0.0:{}", self.port).parse()?;
        let handler = RtmpHandlerImpl {
            event_tx: Arc::new(self.event_tx),
            current_stream: self.current_stream.clone(),
        };
        
        let server = RtmpServer::new(
            ServerConfig::default()
                .bind(addr)
                .max_connections(100)
                .chunk_size(4096)
                .idle_timeout(std::time::Duration::from_secs(5)),
            handler,
        );
        
        server.run().await?;
        
        Ok(())
    }
}

/// RTMP 事件处理器
struct RtmpHandlerImpl {
    event_tx: Arc<mpsc::Sender<StreamEvent>>,
    current_stream: Arc<Mutex<Option<String>>>,
}

impl RtmpHandlerImpl {
    /// 统一的流事件处理逻辑
    async fn handle_stream_event(
        &self,
        app: String,
        stream_key: String,
        event_type: StreamEventType,
    ) -> AuthResult {
        // 更新当前流状态
        let mut current = self.current_stream.lock().await;

        match event_type {
            StreamEventType::Started => {
                *current = Some(stream_key.clone());
            },
            StreamEventType::Stopped => {
                *current = None;
            },
        }

        let event = StreamEvent {
            app,
            stream_key,
            event_type,
        };

        if self.event_tx.send(event).await.is_err() {
            eprintln!("[RTMP] Failed to send stream event");
        }

        AuthResult::Accept
    }
}

impl RtmpHandler for RtmpHandlerImpl {
    fn on_connect(
        &self,
        ctx: &SessionContext,
        params: &ConnectParams,
    ) -> impl std::future::Future<Output = AuthResult> + Send {
        let flash_ver = params.flash_ver.as_deref().unwrap_or("?");
        eprintln!("[RTMP] Connect - {} - {}", flash_ver, ctx.peer_addr);
        async move { AuthResult::Accept }
    }

    fn on_publish(
        &self,
        ctx: &SessionContext,
        params: &PublishParams,
    ) -> impl std::future::Future<Output = AuthResult> + Send {
        let app = ctx.app.clone();
        let stream_key = params.stream_key.clone();
        let flash_ver = ctx.connect_params.as_ref()
            .and_then(|c| c.flash_ver.as_deref()).unwrap_or("?");
        let handler = self.clone_for_future();

        eprintln!("[RTMP] Publish - {} - {}", flash_ver, ctx.peer_addr);

        async move {
            handler.handle_stream_event(app, stream_key, StreamEventType::Started).await
        }
    }

    fn on_play(
        &self,
        _ctx: &SessionContext,
        _params: &PlayParams,
    ) -> impl std::future::Future<Output = AuthResult> + Send {
        // 观众连接 - 静默允许播放
        async { AuthResult::Accept }
    }

    fn on_unpublish(&self, ctx: &StreamContext) -> impl std::future::Future<Output = ()> + Send {
        let handler = self.clone_for_future();
        let flash_ver = ctx.session.connect_params.as_ref()
            .and_then(|c| c.flash_ver.as_deref()).unwrap_or("?");
        eprintln!("[RTMP] Unpublish - {} - {}", flash_ver, ctx.session.peer_addr);

        async move {
            *handler.current_stream.lock().await = None;

            let event = StreamEvent {
                app: String::new(),
                stream_key: String::new(),
                event_type: StreamEventType::Stopped,
            };

            if handler.event_tx.send(event).await.is_err() {
                eprintln!("[RTMP] Failed to send stop event");
            }
        }
    }

    fn on_disconnect(&self, ctx: &SessionContext) -> impl std::future::Future<Output = ()> + Send {
        let fv = ctx.connect_params.as_ref()
            .and_then(|c| c.flash_ver.as_deref()).unwrap_or("?");
        eprintln!("[RTMP] Disconnect - {} - {}", fv, ctx.peer_addr);

        // PS5
        let is_ps5 = fv.starts_with("PlayStation5");
        let flash_ver = fv.to_string();
        let addr = ctx.peer_addr.to_string();
        let handler = self.clone_for_future();

        async move {
            if !is_ps5 { return; }
            if handler.current_stream.lock().await.take().is_none() { return; }
            eprintln!("[WARN] {} - {} - Abnormal disconnect -> Cleanup", flash_ver, addr);
            let event = StreamEvent {
                app: String::new(),
                stream_key: String::new(),
                event_type: StreamEventType::Stopped,
            };
            let _ = handler.event_tx.send(event).await;
        }
    }
}

impl RtmpHandlerImpl {
    /// 为异步闭包克隆必要的引用
    fn clone_for_future(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
            current_stream: self.current_stream.clone(),
        }
    }
}

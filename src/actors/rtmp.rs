/// RTMP Actor

use rtmp_rs::{RtmpHandler, RtmpServer, ServerConfig, AuthResult};
use rtmp_rs::session::{SessionContext, StreamContext};
use rtmp_rs::protocol::message::{ConnectParams, PlayParams, PublishParams};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;

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
            active_streams: Arc::new(Mutex::new(HashMap::new())),
        };
        
        eprintln!("[RTMP] Listening on {}", addr);
        
        let server = RtmpServer::new(
            ServerConfig::default()
                .bind(addr)
                .max_connections(100)
                .chunk_size(4096),
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
    active_streams: Arc<Mutex<HashMap<String, String>>>, // stream_key -> app
}

impl RtmpHandlerImpl {
    /// 统一的流事件处理逻辑
    async fn handle_stream_event(
        &self,
        app: String,
        stream_key: String,
        event_type: StreamEventType,
    ) -> AuthResult {
        // 更新当前流状态和活跃流映射
        let mut current = self.current_stream.lock().await;
        let mut active = self.active_streams.lock().await;

        match event_type {
            StreamEventType::Started => {
                *current = Some(stream_key.clone());
                active.insert(stream_key.clone(), app.clone());
            },
            StreamEventType::Stopped => {
                *current = None;
                active.remove(&stream_key);
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
        let app = params.app.clone();
        eprintln!("[RTMP] Connect addr={} app={}", ctx.peer_addr, app);
        async move { AuthResult::Accept }
    }

    fn on_publish(
        &self,
        ctx: &SessionContext,
        params: &PublishParams,
    ) -> impl std::future::Future<Output = AuthResult> + Send {
        let app = ctx.app.clone();
        let stream_key = params.stream_key.clone();
        let handler = self.clone_for_future();
        
        eprintln!("[RTMP] Publish addr={} app={} stream={}", ctx.peer_addr, app, stream_key);
        
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
        let stream_key = ctx.stream_key.clone();
        
        eprintln!("[RTMP] Unpublish stream={}", stream_key);

        async move {
            let mut active = handler.active_streams.lock().await;
            active.remove(&ctx.stream_key);

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
}

impl RtmpHandlerImpl {
    /// 为异步闭包克隆必要的引用
    fn clone_for_future(&self) -> Self {
        Self {
            event_tx: self.event_tx.clone(),
            current_stream: self.current_stream.clone(),
            active_streams: self.active_streams.clone(),
        }
    }
}

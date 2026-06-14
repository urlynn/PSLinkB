//! 弹幕消息通道：mpsc / broadcast 统一管道

use async_trait::async_trait;
use blivemsg::types::Message;

// 编译时检查：必须启用且仅启用一个 channel
#[cfg(all(feature = "channel-mpsc", feature = "channel-broadcast"))]
compile_error!("channel-mpsc and channel-broadcast are mutually exclusive. Enable only one.");
#[cfg(not(any(feature = "channel-mpsc", feature = "channel-broadcast")))]
compile_error!("Must enable one of: channel-mpsc, channel-broadcast");

/// 弹幕通道消息 — 弹幕和通知共用一条管道
#[derive(Debug, Clone)]
pub enum DanmuMessage {
    Danmaku(Message),
    Notify(String),
}

#[derive(Debug)]
pub struct SendError;

/// 统一的弹幕发送接口
#[async_trait]
pub trait DanmuSender: Send + Sync {
    async fn send_danmu(&self, msg: DanmuMessage) -> Result<(), SendError>;
}

/// 统一的弹幕接收接口
#[async_trait]
pub trait DanmuReceiver: Send + Sync {
    async fn recv_danmu(&mut self) -> Option<DanmuMessage>;
}

// ————————————————————————————————————————————————————————————
// MPSC 模式
// ————————————————————————————————————————————————————————————

#[cfg(feature = "channel-mpsc")]
mod mpsc_impl {
    use super::*;
    use tokio::sync::mpsc;

    pub type DanmuTx = mpsc::Sender<DanmuMessage>;
    pub type DanmuRx = mpsc::Receiver<DanmuMessage>;

    pub fn create_danmu_channel(capacity: usize) -> (DanmuTx, DanmuRx) {
        mpsc::channel(capacity)
    }

    #[async_trait]
    impl DanmuSender for mpsc::Sender<DanmuMessage> {
        async fn send_danmu(&self, msg: DanmuMessage) -> Result<(), SendError> {
            self.send(msg).await.map_err(|_| SendError)
        }
    }

    #[async_trait]
    impl DanmuReceiver for mpsc::Receiver<DanmuMessage> {
        async fn recv_danmu(&mut self) -> Option<DanmuMessage> {
            self.recv().await
        }
    }
}

#[cfg(feature = "channel-mpsc")]
pub use mpsc_impl::*;

// ————————————————————————————————————————————————————————————
// Broadcast 模式
// ————————————————————————————————————————————————————————————

#[cfg(feature = "channel-broadcast")]
mod broadcast_impl {
    use super::*;
    use tokio::sync::broadcast;

    pub type DanmuTx = broadcast::Sender<DanmuMessage>;
    pub type DanmuRx = broadcast::Receiver<DanmuMessage>;

    pub fn create_danmu_channel(capacity: usize) -> (DanmuTx, DanmuRx) {
        broadcast::channel(capacity)
    }

    #[async_trait]
    impl DanmuSender for broadcast::Sender<DanmuMessage> {
        async fn send_danmu(&self, msg: DanmuMessage) -> Result<(), SendError> {
            self.send(msg).map(|_| ()).map_err(|_| SendError)
        }
    }

    #[async_trait]
    impl DanmuReceiver for broadcast::Receiver<DanmuMessage> {
        async fn recv_danmu(&mut self) -> Option<DanmuMessage> {
            match self.recv().await {
                Ok(msg) => Some(msg),
                Err(_) => None,
            }
        }
    }
}

#[cfg(feature = "channel-broadcast")]
pub use broadcast_impl::*;

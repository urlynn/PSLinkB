/// 统一重连策略框架

use std::time::Duration;

/// 重连策略 Trait
pub trait RestartPolicy {
    /// 是否应该重连
    fn should_restart(&self, attempt: u32) -> bool;

    /// 获取下一次重试的延迟时间
    fn next_delay(&self, attempt: u32) -> Duration;
}

/// 指数退避策略
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始延迟时间
    pub initial_delay: Duration,
    /// 最大延迟时间
    pub max_delay: Duration,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RestartPolicy for ExponentialBackoff {
    fn should_restart(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    fn next_delay(&self, attempt: u32) -> Duration {
        let exponential_delay = self.initial_delay * 2u32.pow(attempt.min(10));
        std::cmp::min(exponential_delay, self.max_delay)
    }
}

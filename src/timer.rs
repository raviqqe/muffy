use tokio::time::Instant;

pub trait Timer: Send + Sync {
    fn now(&self) -> Instant;
}

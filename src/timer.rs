mod clock;
mod stub;

pub use clock::*;
pub use stub::*;

use tokio::time::Instant;

pub trait Timer: Send + Sync {
    fn now(&self) -> Instant;
}

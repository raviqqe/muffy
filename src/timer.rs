mod clock;
#[cfg(test)]
mod stub;

pub use clock::*;
#[cfg(test)]
pub use stub::*;

use tokio::time::Instant;

pub trait Timer: Send + Sync {
    fn now(&self) -> Instant;
}

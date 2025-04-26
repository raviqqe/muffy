use crate::{cache::Cache, response::Response};
use alloc::sync::Arc;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

pub struct Context {
    stdout: Mutex<Stdout>,
    origin: String,
    request_semaphore: Semaphore,
    cache: Cache<Result<Response, Arc<reqwest::Error>>>,
}

impl Context {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            stdout: stdout().into(),
            request_semaphore: Semaphore::new(8),
            cache: Cache::new(),
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }

    pub const fn request_semaphore(&self) -> &Semaphore {
        &self.request_semaphore
    }

    pub const fn cache(&self) -> &Cache<Result<Response, Arc<reqwest::Error>>> {
        &self.cache
    }
}

use crate::{cache::Cache, response::Response};
use alloc::sync::Arc;
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

pub struct Context {
    stdout: Mutex<Stdout>,
    origin: String,
    request_semaphore: Semaphore,
    request_cache: Cache<Result<Response, Arc<reqwest::Error>>>,
    checks: HashSet<String>,
}

impl Context {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            stdout: stdout().into(),
            request_semaphore: Semaphore::new(512),
            request_cache: Cache::new(),
            checks: HashSet::with_capacity(1 << 10),
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

    pub const fn request_cache(&self) -> &Cache<Result<Response, Arc<reqwest::Error>>> {
        &self.request_cache
    }

    pub const fn checks(&self) -> &HashSet<String> {
        &self.checks
    }
}

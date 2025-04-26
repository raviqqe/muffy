use crate::cache::Cache;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

pub struct Context {
    stdout: Mutex<Stdout>,
    origin: String,
    request_semaphore: Semaphore,
    cache: Cache<String>,
}

impl Context {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            stdout: stdout().into(),
            request_semaphore: Semaphore::new(8),
            cache: Default::default(),
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

    pub const fn cache(&self) -> &Cache<String> {
        &self.cache
    }
}

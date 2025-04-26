use crate::{
    cache::Cache, enriched_http_client::EnrichedHttpClient, http_client::HttpClient,
    response::Response,
};
use alloc::sync::Arc;
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

pub struct Context<T: HttpClient> {
    http_client: EnrichedHttpClient<T>,
    stdout: Mutex<Stdout>,
    origin: String,
    file_semaphore: Semaphore,
    request_cache: Cache<Result<Response, Arc<reqwest::Error>>>,
    checks: HashSet<String>,
}

impl<T: HttpClient> Context<T> {
    pub fn new(http_client: EnrichedHttpClient<T>, origin: String, file_limit: usize) -> Self {
        Self {
            http_client,
            origin,
            stdout: stdout().into(),
            file_semaphore: Semaphore::new(file_limit),
            request_cache: Cache::new(),
            checks: HashSet::with_capacity(1 << 10),
        }
    }

    pub fn http_client(&self) -> &EnrichedHttpClient<T> {
        &self.http_client
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }

    pub const fn file_semaphore(&self) -> &Semaphore {
        &self.file_semaphore
    }

    pub const fn request_cache(&self) -> &Cache<Result<Response, Arc<reqwest::Error>>> {
        &self.request_cache
    }

    pub const fn checks(&self) -> &HashSet<String> {
        &self.checks
    }
}

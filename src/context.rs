use crate::full_http_client::FullHttpClient;
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

pub struct Context {
    http_client: FullHttpClient,
    stdout: Mutex<Stdout>,
    origin: String,
    file_semaphore: Semaphore,
    checks: HashSet<String>,
}

impl Context {
    pub fn new(http_client: FullHttpClient, origin: String, file_limit: usize) -> Self {
        Self {
            http_client,
            origin,
            stdout: stdout().into(),
            file_semaphore: Semaphore::new(file_limit),
            checks: HashSet::with_capacity(1 << 10),
        }
    }

    pub fn http_client(&self) -> &FullHttpClient {
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

    pub const fn checks(&self) -> &HashSet<String> {
        &self.checks
    }
}

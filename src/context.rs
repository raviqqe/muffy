use crate::{
    cache::{Cache, MemoryCache},
    error::Error,
    full_http_client::FullHttpClient,
    metrics::Metrics,
};
use robotxt::Robots;
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, mpsc::Sender},
};

pub struct Context {
    http_client: FullHttpClient,
    stdout: Mutex<Stdout>,
    origin: String,
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<Metrics, Error>> + Send>>,
    robots: Box<dyn Cache<Robots>>,
}

impl Context {
    pub fn new(
        http_client: FullHttpClient,
        job_sender: Sender<Box<dyn Future<Output = Result<Metrics, Error>> + Send>>,
        robots: Box<dyn Cache<Robots>>,
        origin: String,
    ) -> Self {
        Self {
            http_client,
            origin,
            stdout: stdout().into(),
            documents: HashSet::with_capacity(1 << 10),
            job_sender,
            robots,
        }
    }

    pub const fn http_client(&self) -> &FullHttpClient {
        &self.http_client
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }

    pub const fn documents(&self) -> &HashSet<String> {
        &self.documents
    }

    pub const fn job_sender(
        &self,
    ) -> &Sender<Box<dyn Future<Output = Result<Metrics, Error>> + Send>> {
        &self.job_sender
    }

    pub fn robots(&self) -> &dyn Cache<Robots> {
        self.robots.as_ref()
    }
}

use crate::{error::Error, http_client::CachedHttpClient, metrics::Metrics};
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, mpsc::Sender},
};

pub struct Context {
    http_client: CachedHttpClient,
    stdout: Mutex<Stdout>,
    origin: String,
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<Metrics, Error>> + Send>>,
}

impl Context {
    pub fn new(
        http_client: CachedHttpClient,
        job_sender: Sender<Box<dyn Future<Output = Result<Metrics, Error>> + Send>>,
        origin: String,
    ) -> Self {
        Self {
            http_client,
            origin,
            stdout: stdout().into(),
            documents: HashSet::with_capacity(1 << 10),
            job_sender,
        }
    }

    pub const fn http_client(&self) -> &CachedHttpClient {
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
}

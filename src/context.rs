use crate::{document::Document, error::Error, http_client::CachedHttpClient};
use scc::HashSet;
use tokio::sync::mpsc::Sender;

pub struct Context {
    http_client: CachedHttpClient,
    origin: String,
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<Document, Error>> + Send>>,
}

impl Context {
    pub fn new(
        http_client: CachedHttpClient,
        job_sender: Sender<Box<dyn Future<Output = Result<Document, Error>> + Send>>,
        origin: String,
    ) -> Self {
        Self {
            http_client,
            origin,
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

    pub const fn documents(&self) -> &HashSet<String> {
        &self.documents
    }

    pub const fn job_sender(
        &self,
    ) -> &Sender<Box<dyn Future<Output = Result<Document, Error>> + Send>> {
        &self.job_sender
    }
}

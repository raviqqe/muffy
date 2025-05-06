use crate::{Config, document_output::DocumentOutput, error::Error};
use scc::HashSet;
use tokio::sync::mpsc::Sender;

pub struct Context {
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
    config: Config,
}

impl Context {
    pub fn new(
        job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
        config: Config,
    ) -> Self {
        Self {
            documents: HashSet::with_capacity(1 << 10),
            job_sender,
            config,
        }
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub const fn documents(&self) -> &HashSet<String> {
        &self.documents
    }

    pub const fn job_sender(
        &self,
    ) -> &Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>> {
        &self.job_sender
    }
}

use crate::{document_output::DocumentOutput, error::Error};
use scc::HashSet;
use tokio::sync::mpsc::Sender;

pub struct Context {
    origin: String,
    documents: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
}

impl Context {
    pub fn new(
        job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
        origin: String,
    ) -> Self {
        Self {
            origin,
            documents: HashSet::with_capacity(1 << 10),
            job_sender,
        }
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
    ) -> &Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>> {
        &self.job_sender
    }
}

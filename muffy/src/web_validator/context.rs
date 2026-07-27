use crate::{Config, document_output::DocumentOutput, error::Error};
use std::{collections::HashSet, sync::Mutex};
use tokio::sync::mpsc::Sender;

pub struct Context {
    documents: Mutex<HashSet<String>>,
    job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
    config: Config,
}

impl Context {
    pub fn new(
        job_sender: Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>>,
        config: Config,
    ) -> Self {
        Self {
            documents: Mutex::new(HashSet::with_capacity(1 << 10)),
            job_sender,
            config,
        }
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub fn insert_document(&self, url: String) -> bool {
        self.documents.lock().unwrap().insert(url)
    }

    pub const fn job_sender(
        &self,
    ) -> &Sender<Box<dyn Future<Output = Result<DocumentOutput, Error>> + Send>> {
        &self.job_sender
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::channel;

    #[test]
    fn insert_document() {
        let context = Context::new(
            channel(1).0,
            Config::new(vec![], Default::default(), Default::default()),
        );

        assert!(context.insert_document("https://foo.com/".into()));
        assert!(!context.insert_document("https://foo.com/".into()));
    }

    #[test]
    fn insert_different_documents() {
        let context = Context::new(
            channel(1).0,
            Config::new(vec![], Default::default(), Default::default()),
        );

        assert!(context.insert_document("https://foo.com/".into()));
        assert!(context.insert_document("https://foo.com/bar".into()));
    }
}

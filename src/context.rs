use crate::error::Error;
use crate::full_http_client::FullHttpClient;
use scc::HashSet;
use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, mpsc::Sender},
};

pub struct Context {
    http_client: FullHttpClient,
    stdout: Mutex<Stdout>,
    origin: String,
    checks: HashSet<String>,
    job_sender: Sender<Box<dyn Future<Output = Result<(), Error>> + Send>>,
}

impl Context {
    pub fn new(
        http_client: FullHttpClient,
        job_sender: Sender<Box<dyn Future<Output = Result<(), Error>> + Send>>,
        origin: String,
    ) -> Self {
        Self {
            http_client,
            origin,
            stdout: stdout().into(),
            checks: HashSet::with_capacity(1 << 10),
            job_sender,
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

    pub const fn checks(&self) -> &HashSet<String> {
        &self.checks
    }

    pub const fn job_sender(&self) -> &Sender<Box<dyn Future<Output = Result<(), Error>> + Send>> {
        &self.job_sender
    }
}

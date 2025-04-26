use tokio::{
    io::{Stdout, stdout},
    sync::{Mutex, Semaphore},
};

#[derive(Debug)]
pub struct Context {
    stdout: Mutex<Stdout>,
    origin: String,
    request_semaphore: Semaphore,
}

impl Context {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            stdout: stdout().into(),
            request_semaphore: Semaphore::new(8),
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }

    pub fn request_semaphore(&self) -> &Semaphore {
        &self.request_semaphore
    }
}

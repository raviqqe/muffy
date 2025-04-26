use tokio::{
    io::{Stdout, stdout},
    sync::Mutex,
};

#[derive(Debug)]
pub struct Context {
    stdout: Mutex<Stdout>,
    origin: String,
}

impl Context {
    pub fn new(origin: String) -> Self {
        Self {
            origin,
            stdout: stdout().into(),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }
}

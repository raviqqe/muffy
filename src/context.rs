use tokio::{
    io::{Stdout, stdout},
    sync::Mutex,
};

#[derive(Debug)]
pub struct Context {
    stdout: Mutex<Stdout>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            stdout: stdout().into(),
        }
    }

    pub const fn stdout(&self) -> &Mutex<Stdout> {
        &self.stdout
    }
}

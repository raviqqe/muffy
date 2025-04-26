use tokio::io::{Stdout, stdout};

#[derive(Debug)]
pub struct Context {
    stdout: Stdout,
}

impl Context {
    pub fn new() -> Self {
        Self { stdout: stdout() }
    }
}

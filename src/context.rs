use tokio::io::Stdout;

#[derive(Debug, Default)]
pub struct Context {}

impl Context {
    pub const fn new() -> Self {
        Self { stdout: Stdout }
    }
}

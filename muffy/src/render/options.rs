/// A rendering format.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, clap::ValueEnum)]
pub enum RenderFormat {
    /// Human-readable text.
    #[default]
    Text,
    /// JSON.
    Json,
}

/// Rendering options.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct RenderOptions {
    format: RenderFormat,
    verbose: bool,
}

impl RenderOptions {
    /// Creates rendering options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the rendering format.
    pub const fn format(&self) -> RenderFormat {
        self.format
    }

    /// Returns whether verbose output is enabled.
    pub const fn verbose(&self) -> bool {
        self.verbose
    }

    /// Sets a rendering format.
    pub const fn set_format(mut self, format: RenderFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets whether verbose output is enabled.
    pub const fn set_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

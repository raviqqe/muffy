/// A rendering format.
#[derive(Clone, Copy, Debug)]
pub enum RenderFormat {
    // JSON.
    Json,
    // Human-readable text.
    Text,
}

/// Rendering options.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    format: RenderFormat,
    verbose: bool,
}

impl RenderOptions {
    /// Creates a new `RenderOptions` instance.
    pub fn new(format: RenderFormat, verbose: bool) -> Self {
        Self { format, verbose }
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

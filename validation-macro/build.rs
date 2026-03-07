//! A build script.

use std::error::Error;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    Command::new("ln")
        .arg("-s")
        .arg(
            Path::new("..")
                .join("..")
                .join("vendor")
                .join("validator")
                .join("schema"),
        )
        .arg("src")
        .output()?;

    Ok(())
}

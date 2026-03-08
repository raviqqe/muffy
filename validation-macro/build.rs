//! A build script.

use core::error::Error;
use std::{fs::exists, path::Path, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    if !exists(Path::new("src").join("schema")) {
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
    }

    Ok(())
}

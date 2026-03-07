//! A build script.

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    Command::new("ln")
        .arg("-s")
        .arg(Path::new("..").join())
        .output()?;

    Ok(())
}

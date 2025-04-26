use crate::context::Context;
use crate::error::Error;
use colored::Colorize;
use tokio::io::{AsyncWriteExt, Stdout};
use url::Url;

pub async fn render(
    context: &Context,
    url: &Url,
    results: &[Result<(), Error>],
) -> Result<(), Error> {
    let mut stdout = context.stdout().lock().await;

    render_line(&mut stdout, &url.to_string().yellow()).await?;

    for result in results {
        match result {
            Ok(()) => render_line(&mut stdout, &"  OK".green()).await?,
            Err(error) => render_line(&mut stdout, &format!("  {} {error}", "ERROR".red())).await?,
        }
    }

    Ok(())
}

async fn render_line(stdout: &mut Stdout, string: &str) -> Result<(), Error> {
    stdout.write_all(format!("{}\n", string).as_bytes()).await?;

    Ok(())
}

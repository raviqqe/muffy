mod context;

use tokio::spawn;

#[tokio::main]
async fn main() {
    validate_page().await?;
}

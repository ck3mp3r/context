use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
    context::cli::run().await
}

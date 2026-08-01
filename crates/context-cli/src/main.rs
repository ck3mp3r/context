use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
    context_cli::init();
    context_cli::cli::run().await
}

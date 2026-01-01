use miette::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls with ring crypto provider
    context::init();
    context::cli::run().await
}

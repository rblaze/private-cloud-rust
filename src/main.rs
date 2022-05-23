use anyhow::Result;
use private_cloud::aws::{create_aws_config, AWS};
use private_cloud::provider::CloudProvider;
use tracing_subscriber::filter::EnvFilter;

async fn run() -> Result<()> {
    let config = create_aws_config()?;
    let provider = AWS::load_from_config(config).await?;
    private_cloud::cloud::run(&provider).await
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    // TODO split create/connect/run modes
    if let Err(e) = run().await {
        eprintln!("Fatal error: {:?}", e);
    }
}

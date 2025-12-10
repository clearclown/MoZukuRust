use anyhow::Result;
use tower_lsp::{LspService, Server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use mozuku_rs::server::MozukuServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting MoZuku Language Server...");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(MozukuServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

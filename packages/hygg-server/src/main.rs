//! Binary entry point. All bootstrapping lives in [`hygg_server::runtime`] so
//! the same pieces can be reused by downstream embedders.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  hygg_server::runtime::run().await
}

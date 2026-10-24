#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    mdpdf_app::server::run().await
}

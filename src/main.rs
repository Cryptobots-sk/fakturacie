#[tokio::main]
async fn main() {
    if let Err(error) = fakturacie::web::run().await {
        eprintln!("Fakturácie: {}", error.message);
        std::process::exit(1);
    }
}

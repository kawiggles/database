#[tokio::main]
async fn main() {
    database::run().await;
}

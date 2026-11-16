use clap::Parser;
#[derive(Parser)]
#[command(about = "Native foundation readiness server; search is not implemented")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: std::net::SocketAddr,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.listen.ip().is_loopback() {
        return Err("foundation listener must be loopback".into());
    }
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    println!(
        "{}",
        serde_json::json!({"address": listener.local_addr()?.to_string(), "mode": "foundation"})
    );
    axum_serve(listener).await?;
    Ok(())
}
async fn axum_serve(listener: tokio::net::TcpListener) -> Result<(), std::io::Error> {
    ssx_http::serve(listener, ssx_infra::foundation_runtime()).await
}

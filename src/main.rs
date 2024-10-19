use std::net::{IpAddr, SocketAddr};

use axum::Router;
use tokio::net::TcpListener;

mod middleware;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env))).await.unwrap();
    let router: Router<()> = Router::new()
        .layer(axum::middleware::from_fn(middleware::convert_errors_to_json));
    axum::serve(tcp_connector, router).await.unwrap()
}

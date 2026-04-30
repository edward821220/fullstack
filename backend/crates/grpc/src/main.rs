use config::AppConfig;
use std::net::SocketAddr;
use tonic::transport::Server;

mod proto {
    pub mod greetings {
        pub mod v1 {
            tonic::include_proto!("greetings.v1");
        }
    }
}

use proto::greetings::v1::{
    HealthCheckRequest, HealthCheckResponse, SayHelloRequest, SayHelloResponse,
    greetings_service_server::{GreetingsService, GreetingsServiceServer},
};

pub struct GreetingsImpl;

#[tonic::async_trait]
impl GreetingsService for GreetingsImpl {
    async fn say_hello(
        &self,
        request: tonic::Request<SayHelloRequest>,
    ) -> Result<tonic::Response<SayHelloResponse>, tonic::Status> {
        let name = request.into_inner().name;
        Ok(tonic::Response::new(SayHelloResponse {
            message: format!("Hello, {}!", name),
        }))
    }

    async fn health_check(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(HealthCheckResponse {
            status: "SERVING".to_owned(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::load()?;

    migration::run(&config.database).await?;

    let addr: SocketAddr = format!("{}:{}", config.grpc.host, config.grpc.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<GreetingsServiceServer<GreetingsImpl>>()
        .await;

    tracing::info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(health_service)
        .add_service(GreetingsServiceServer::new(GreetingsImpl))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async {
                tokio::signal::ctrl_c().await.ok();
                tracing::info!("gRPC server graceful shutdown");
            },
        )
        .await?;

    Ok(())
}

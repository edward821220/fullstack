use crate::auth::{GrpcAuthInterceptor, GrpcAuthState};
use crate::health::HealthGrpcService;
use crate::proto::health::v1::health_service_server::HealthServiceServer;
use config::AppConfig;
use repo::AnyUserRepo;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use svc::audit::PiiMode;

pub mod auth;
pub mod health;
pub mod proto;

pub async fn serve(
    config: AppConfig,
    _repo: AnyUserRepo,
    health_checker: Arc<dyn svc::HealthChecker>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    infra::ensure_jwt_crypto_provider();

    let audit_exporter =
        infra::create_audit_exporter(&config.audit, &config.observability.service_name);
    let _audit_service = svc::AuditService::new(audit_exporter, PiiMode::Redact);

    // Build gRPC auth interceptor with real JWT validation when enabled.
    let auth_state = if config.grpc.auth_enabled {
        let state = Arc::new(GrpcAuthState::new(config.auth.clone()));
        state
            .prime_cache()
            .await
            .map_err(|e| format!("Failed to prime gRPC JWKS cache: {e}"))?;

        // Spawn background JWKS refresh.
        let refresh_state = Arc::clone(&state);
        let cache_ttl = Duration::from_secs(config.auth.jwks_cache_duration_secs);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(cache_ttl).await;
                if let Err(e) = refresh_state.refresh_jwks().await {
                    tracing::warn!("gRPC JWKS background refresh failed: {e}");
                }
            }
        });

        Some(state)
    } else {
        None
    };

    let auth_interceptor = GrpcAuthInterceptor { state: auth_state };

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let health_svc = HealthServiceServer::with_interceptor(
        HealthGrpcService::new(health_checker),
        auth_interceptor.clone(),
    );

    let mut server_builder = tonic::transport::Server::builder();

    if config.grpc.tls.enabled {
        let cert = tokio::fs::read(&config.grpc.tls.cert_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read gRPC TLS cert from '{}': {e}",
                    config.grpc.tls.cert_path
                )
            })?;
        let key = tokio::fs::read(&config.grpc.tls.key_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read gRPC TLS key from '{}': {e}",
                    config.grpc.tls.key_path
                )
            })?;
        let identity = tonic::transport::Identity::from_pem(cert, key);

        let tls_config = if let Some(ref ca_path) = config.grpc.tls.ca_cert_path {
            let ca = tokio::fs::read(ca_path)
                .await
                .map_err(|e| format!("Failed to read gRPC TLS CA from '{ca_path}': {e}"))?;
            let ca_cert = tonic::transport::Certificate::from_pem(ca);
            tonic::transport::ServerTlsConfig::new()
                .identity(identity)
                .client_ca_root(ca_cert)
        } else {
            tonic::transport::ServerTlsConfig::new().identity(identity)
        };

        server_builder = server_builder.tls_config(tls_config)?;
    }

    server_builder
        .add_service(health_svc)
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

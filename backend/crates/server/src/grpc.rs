use std::{net::SocketAddr, sync::Arc};

use config::AppConfig;
use repo::AnyUserRepo;
use svc::{ProvisioningPolicy, UserService, UserServiceTrait};
use tonic::{Request, Response, Status};

use crate::audit::{NoopExporter, OtelLogsExporter, SyslogExporter};
use crate::middleware::OidcValidator;
use crate::state::AppState;

pub mod proto {
    pub mod health {
        pub mod v1 {
            tonic::include_proto!("health.v1");
        }
    }
}

use proto::health::v1::{
    HealthCheckRequest, HealthCheckResponse,
    health_service_server::{HealthService, HealthServiceServer},
};

#[derive(Clone)]
pub struct HealthGrpcService {
    state: Arc<AppState>,
}

impl HealthGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl HealthService for HealthGrpcService {
    #[tracing::instrument(skip(self, _request))]
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.state.svc.health_check().await.map_err(|e| {
            tracing::error!("gRPC health check failed: {e}");
            Status::internal("Health check failed")
        })?;

        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_owned(),
        }))
    }
}

pub async fn serve(
    config: AppConfig,
    repo: AnyUserRepo,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = Arc::new(UserService::new(repo));
    let oidc_validator = Arc::new(OidcValidator::new(config.auth.clone()));
    let provisioning =
        ProvisioningPolicy::new(config.auth.allowed_email_domains.clone(), "user".to_owned());

    let audit_exporter = match config.audit.exporter.as_str() {
        "syslog" => {
            let cfg = config
                .audit
                .syslog
                .as_ref()
                .expect("syslog config validated");
            Arc::new(SyslogExporter::new(
                cfg.host.clone(),
                cfg.port,
                cfg.protocol.clone(),
                &cfg.facility,
                config.observability.service_name.clone(),
                std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_owned()),
            )) as Arc<dyn svc::AuditExporter>
        }
        "otel-logs" => {
            let cfg = config
                .audit
                .otel_logs
                .as_ref()
                .expect("otel_logs config validated");
            Arc::new(OtelLogsExporter::new(
                cfg.endpoint.clone(),
                cfg.timeout_seconds,
                config.observability.service_name.clone(),
            )) as Arc<dyn svc::AuditExporter>
        }
        _ => Arc::new(NoopExporter) as Arc<dyn svc::AuditExporter>,
    };
    let audit_service = svc::AuditService::new(audit_exporter);

    let app_state = Arc::new(AppState {
        svc,
        oidc: oidc_validator,
        provisioning,
        audit: audit_service,
    });

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<HealthServiceServer<HealthGrpcService>>()
        .await;

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(HealthServiceServer::new(HealthGrpcService::new(app_state)))
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

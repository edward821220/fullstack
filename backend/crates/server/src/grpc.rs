use std::{net::SocketAddr, sync::Arc};

use config::AppConfig;
use dto::UserResponse;
use repo::AnyUserRepo;
use svc::{ProvisioningPolicy, UserService, UserServiceTrait};
use tonic::{Request, Response, Status, metadata::MetadataMap};
use uuid::Uuid;

use crate::middleware::OidcValidator;
use crate::middleware::authz::{Role, authorize_role};
use crate::middleware::oidc::{AuthFailure, AuthUser};
use crate::state::AppState;

pub mod proto {
    pub mod users {
        pub mod v1 {
            tonic::include_proto!("users.v1");
        }
    }
}

use proto::users::v1::{
    GetUserRequest, GetUserResponse, HealthCheckRequest, HealthCheckResponse, ListUsersRequest,
    ListUsersResponse, User,
    users_service_server::{UsersService, UsersServiceServer},
};

#[derive(Clone)]
pub struct UsersGrpcService {
    state: Arc<AppState>,
}

impl UsersGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl UsersService for UsersGrpcService {
    #[tracing::instrument(skip(self, request))]
    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let auth_user = authenticate_request(&self.state, request.metadata()).await?;
        require_role(auth_user.as_ref(), &Role::Manager)?;

        let user_id = Uuid::parse_str(&request.get_ref().id)
            .map_err(|_| Status::invalid_argument("id must be a valid UUID"))?;

        let user = self
            .state
            .svc
            .get_user(user_id)
            .await
            .map_err(service_error_to_status)?;

        Ok(Response::new(GetUserResponse {
            user: Some(to_proto_user(&user)),
        }))
    }

    #[tracing::instrument(skip(self, request))]
    async fn list_users(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let auth_user = authenticate_request(&self.state, request.metadata()).await?;
        require_role(auth_user.as_ref(), &Role::Manager)?;

        let page = match request.get_ref().page {
            0 => 1,
            page => page,
        };
        let per_page = match request.get_ref().per_page {
            0 => 20,
            per_page => per_page,
        };

        if !(1..=100).contains(&per_page) {
            return Err(Status::invalid_argument(
                "per_page must be between 1 and 100",
            ));
        }

        let (users, total) = self
            .state
            .svc
            .list_users(page, per_page)
            .await
            .map_err(service_error_to_status)?;

        Ok(Response::new(ListUsersResponse {
            data: users.iter().map(to_proto_user).collect(),
            total,
            page,
            per_page,
        }))
    }

    #[tracing::instrument(skip(self, _request))]
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.state
            .svc
            .health_check()
            .await
            .map_err(service_error_to_status)?;

        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
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

    let app_state = Arc::new(AppState {
        svc,
        oidc: oidc_validator,
        provisioning,
    });

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<UsersServiceServer<UsersGrpcService>>()
        .await;

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tonic::transport::Server::builder()
        .add_service(health_service)
        .add_service(UsersServiceServer::new(UsersGrpcService::new(app_state)))
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

async fn authenticate_request(
    state: &Arc<AppState>,
    metadata: &MetadataMap,
) -> Result<Option<AuthUser>, Status> {
    if !state.oidc.auth_enabled() {
        return Ok(None);
    }

    let token = metadata
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| Status::unauthenticated("Missing or invalid Bearer token"))?;

    state
        .oidc
        .authenticate_token(token, state.svc.as_ref(), &state.provisioning)
        .await
        .map(Some)
        .map_err(auth_failure_to_status)
}

fn require_role(user: Option<&AuthUser>, minimum_role: &Role) -> Result<(), Status> {
    if let Some(user) = user {
        authorize_role(&user.role, minimum_role).map_err(|e| match e {
            crate::middleware::authz::AuthzError::Forbidden(detail) => {
                Status::permission_denied(detail)
            }
        })?;
    }

    Ok(())
}

fn auth_failure_to_status(error: AuthFailure) -> Status {
    match error {
        AuthFailure::Unauthorized(detail) => Status::unauthenticated(detail),
        AuthFailure::Forbidden(detail) => Status::permission_denied(detail),
        AuthFailure::Internal(detail) => Status::internal(detail),
    }
}

fn service_error_to_status(error: svc::Error) -> Status {
    match error {
        svc::Error::NotFound { id } => Status::not_found(format!("User with id {id} not found")),
        svc::Error::InvalidInput { message } => Status::invalid_argument(message),
        svc::Error::NotInWhitelist { email } => {
            Status::permission_denied(format!("User with email {email} not in whitelist"))
        }
        svc::Error::Repository { source } => {
            tracing::error!("gRPC repository error: {source}");
            Status::internal("Internal server error")
        }
    }
}

fn to_proto_user(user: &model::user::User) -> User {
    let response = UserResponse::from(user);

    User {
        id: response.id.to_string(),
        email: response.email,
        display_name: response.display_name,
        role: response.role,
        email_verified: response.email_verified,
        created_at: response.created_at,
        updated_at: response.updated_at,
    }
}

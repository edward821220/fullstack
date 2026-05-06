use crate::middleware::oidc::OidcValidator;
use repo::AnyUserRepo;
use std::sync::Arc;
use svc::{AuditService, HealthChecker, ProvisioningPolicy, UserService};

pub struct AppState {
    pub svc: Arc<UserService<AnyUserRepo>>,
    pub health: Arc<dyn HealthChecker>,
    pub oidc: Arc<OidcValidator>,
    pub provisioning: ProvisioningPolicy,
    pub audit: AuditService,
}

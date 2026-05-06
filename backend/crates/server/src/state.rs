use std::sync::Arc;

use repo::AnyUserRepo;
use svc::{AuditService, HealthChecker, ProvisioningPolicy, UserService};

use crate::middleware::oidc::OidcValidator;

pub struct AppState {
    pub svc: Arc<UserService<AnyUserRepo>>,
    pub health: Arc<dyn HealthChecker>,
    pub oidc: Arc<OidcValidator>,
    pub provisioning: ProvisioningPolicy,
    pub audit: AuditService,
}

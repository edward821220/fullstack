use std::sync::Arc;

use svc::{ProvisioningPolicy, UserService};

use crate::middleware::oidc::OidcValidator;

pub struct AppState {
    pub svc: Arc<UserService>,
    pub oidc: Arc<OidcValidator>,
    pub provisioning: ProvisioningPolicy,
}

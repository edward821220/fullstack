use std::sync::Arc;

use repo::AnyUserRepo;
use svc::{ProvisioningPolicy, UserService};

use crate::middleware::oidc::OidcValidator;

pub struct AppState {
    pub svc: Arc<UserService<AnyUserRepo>>,
    pub oidc: Arc<OidcValidator>,
    pub provisioning: ProvisioningPolicy,
}

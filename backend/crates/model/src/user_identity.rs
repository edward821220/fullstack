use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub issuer: String,
    pub external_sub: String,
    pub created_at: time::OffsetDateTime,
}

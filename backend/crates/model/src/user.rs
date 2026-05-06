use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::role::Role;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub email_verified: bool,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
    pub version: i64,
}

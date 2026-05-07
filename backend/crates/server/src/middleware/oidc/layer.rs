use super::{AuthDisabledMarker, AuthFailure};
use crate::state::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use svc::AuditEvent;

pub async fn oidc_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthFailure> {
    if !state.oidc.auth_enabled() {
        req.extensions_mut().insert(AuthDisabledMarker);
        return Ok(next.run(req).await);
    }
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            state.audit.record(AuditEvent::AuthFailure {
                reason: "Missing or invalid Bearer token".to_owned(),
            });
            AuthFailure::Unauthorized("Missing or invalid Bearer token".to_owned())
        })?;
    let auth_user = state
        .oidc
        .authenticate_token(token, state.svc.as_ref(), &state.provisioning)
        .await
        .map_err(|e| {
            state.audit.record(AuditEvent::AuthFailure {
                reason: format!("{e:?}"),
            });
            e
        })?;
    state.audit.record(AuditEvent::AuthSuccess {
        user_id: auth_user.user_id,
        email: auth_user.email.clone(),
        role: auth_user.role.to_string(),
        sub: auth_user.sub.clone(),
    });
    req.extensions_mut().insert(auth_user);
    Ok(next.run(req).await)
}

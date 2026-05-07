use async_trait::async_trait;
use openidconnect::core::CoreProviderMetadata;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<serde_json::Value>,
}

/// Port for OIDC HTTP operations. Production uses reqwest; tests inject a fake.
#[async_trait]
pub trait OidcHttpClient: Send + Sync {
    async fn fetch_jwks(&self, uri: &str) -> Result<JwksResponse, String>;
    async fn fetch_metadata(&self, url: &str) -> Result<CoreProviderMetadata, String>;
}

pub struct ReqwestOidcClient {
    pub client: reqwest::Client,
}

#[async_trait]
impl OidcHttpClient for ReqwestOidcClient {
    async fn fetch_jwks(&self, uri: &str) -> Result<JwksResponse, String> {
        self.client
            .get(uri)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch JWKS: {e}"))?
            .json::<JwksResponse>()
            .await
            .map_err(|e| format!("Failed to parse JWKS: {e}"))
    }

    async fn fetch_metadata(&self, url: &str) -> Result<CoreProviderMetadata, String> {
        self.client
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch OIDC discovery: {e}"))?
            .json::<CoreProviderMetadata>()
            .await
            .map_err(|e| format!("Failed to parse OIDC discovery: {e}"))
    }
}

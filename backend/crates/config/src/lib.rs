use serde::Deserialize;
use snafu::Snafu;

pub mod loader;
pub mod validation;

pub use loader::ConfigDir;

fn default_metrics_enabled() -> bool {
    true
}

fn default_otlp_timeout_seconds() -> u64 {
    5
}

fn default_grpc_enabled() -> bool {
    false
}

fn default_allow_all_domains() -> bool {
    false
}

fn default_rate_limit_enabled() -> bool {
    false
}

fn default_rate_limit_requests_per_second() -> u32 {
    10
}

fn default_rate_limit_burst_size() -> u32 {
    20
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Configuration error: {source}"))]
    Load { source: Box<figment::Error> },

    #[snafu(display("Invalid config: {message}"))]
    Invalid { message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub observability: ObservabilityConfig,
    pub grpc: GrpcConfig,
    pub audit: AuditConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl AppConfig {
    /// Returns true when the deployment environment is explicitly `local`.
    /// All non-local environments (sandbox, staging, production) are treated
    /// as "remote" for security checks that should not apply to a developer laptop.
    pub fn is_local(&self) -> bool {
        self.server.environment.is_local()
    }

    pub fn is_production(&self) -> bool {
        self.server.environment.is_production()
    }

    /// Resolve the database password, reading from file if `password_file` is set.
    pub fn resolve_db_password(&self) -> Result<String> {
        match &self.database.password_file {
            Some(path) if !path.is_empty() => std::fs::read_to_string(path)
                .map(|s| s.trim().to_owned())
                .map_err(|e| Error::Invalid {
                    message: format!("Failed to read database password file '{path}': {e}"),
                }),
            _ => Ok(self.database.password.clone()),
        }
    }
}

fn default_docs_enabled() -> bool {
    true
}

fn default_max_request_body_size() -> usize {
    1_048_576 // 1 MiB
}

fn default_environment() -> Environment {
    Environment::Production
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Local,
    Sandbox,
    Staging,
    #[default]
    Production,
}

impl Environment {
    pub fn is_local(&self) -> bool {
        matches!(self, Environment::Local)
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }
}

fn default_metrics_interval_seconds() -> u64 {
    15
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_environment")]
    pub environment: Environment,
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
    pub shutdown_timeout_seconds: u64,
    pub tls: TlsConfig,
    pub cors_origins: Vec<String>,
    #[serde(default = "default_docs_enabled")]
    pub docs_enabled: bool,
    #[serde(default = "default_max_request_body_size")]
    pub max_request_body_size: usize,
    #[serde(default = "default_metrics_interval_seconds")]
    pub metrics_interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseDriver {
    #[default]
    Mssql,
    Postgres,
}

impl DatabaseDriver {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseDriver::Mssql => "mssql",
            DatabaseDriver::Postgres => "postgres",
        }
    }
}

fn default_run_migrations_on_startup() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub driver: DatabaseDriver,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub password_file: Option<String>,
    pub max_connections: u32,
    pub connect_retry_attempts: u32,
    pub connect_retry_delay_ms: u64,
    #[serde(default)]
    pub encrypt: bool,
    #[serde(default)]
    pub trust_cert: bool,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default = "default_run_migrations_on_startup")]
    pub run_migrations_on_startup: bool,
}

impl DatabaseConfig {
    pub fn driver(&self) -> &DatabaseDriver {
        &self.driver
    }

    /// Resolve the effective database password, preferring `password_file`.
    pub fn resolve_password(&self) -> String {
        match &self.password_file {
            Some(path) if !path.is_empty() => {
                std::fs::read_to_string(path)
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "Failed to read database password file '{path}': {e}; falling back to password field"
                        );
                        self.password.clone()
                    })
            }
            _ => self.password.clone(),
        }
    }

    pub fn extract_mssql_database_name(&self) -> std::result::Result<String, String> {
        if self.database.is_empty() {
            Err("database name is empty".to_owned())
        } else {
            Ok(self.database.clone())
        }
    }

    /// Build a Postgres connection string from discrete fields.
    pub fn to_postgres_url(&self) -> String {
        let password = self.resolve_password();
        let mut url = format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, password, self.host, self.port, self.database
        );
        let mut params = Vec::new();
        if self.encrypt {
            if self.trust_cert {
                // trust_cert bypasses both CA and hostname verification;
                // only permitted in local development (validated separately).
                params.push("sslmode=require".to_owned());
            } else {
                // verify-full validates both the CA chain and the server hostname.
                params.push("sslmode=verify-full".to_owned());
                if let Some(ref ca_path) = self.ca_cert_path
                    && !ca_path.is_empty()
                {
                    params.push(format!("sslrootcert={}", ca_path));
                }
            }
        }
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
        url
    }
}

fn default_allowed_algorithms() -> Vec<String> {
    vec![
        "RS256".to_owned(),
        "RS384".to_owned(),
        "RS512".to_owned(),
        "ES256".to_owned(),
        "ES384".to_owned(),
        "ES512".to_owned(),
    ]
}

fn default_clock_skew_seconds() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub issuer_url: String,
    pub audience: Vec<String>,
    pub jwks_cache_duration_secs: u64,
    pub allowed_email_domains: Vec<String>,
    #[serde(default = "default_allow_all_domains")]
    pub allow_all_domains: bool,
    pub role_claim_source: RoleClaimSource,
    pub discovery_mode: DiscoveryMode,
    #[serde(default)]
    pub manual_endpoints: Option<ManualOidcEndpoints>,
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,
    #[serde(default = "default_allowed_algorithms")]
    pub allowed_algorithms: Vec<String>,
    #[serde(default)]
    pub require_email_verified: bool,
    #[serde(default = "default_clock_skew_seconds")]
    pub clock_skew_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RoleClaimSource {
    Roles,
    Groups,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryMode {
    Discovery,
    Manual,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualOidcEndpoints {
    pub jwks_uri: String,
    pub issuer: String,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    pub log_format: LogFormat,
    pub log_level: String,
    pub service_name: String,
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    #[serde(default)]
    pub metrics_auth_token: Option<String>,
    #[serde(default)]
    pub otlp: OtlpConfig,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OtlpProtocol {
    Grpc,
    #[default]
    Http,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub protocol: OtlpProtocol,
    #[serde(default = "default_otlp_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            protocol: OtlpProtocol::default(),
            timeout_seconds: default_otlp_timeout_seconds(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
    #[serde(default = "default_rate_limit_requests_per_second")]
    pub requests_per_second: u32,
    #[serde(default = "default_rate_limit_burst_size")]
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: default_rate_limit_enabled(),
            requests_per_second: default_rate_limit_requests_per_second(),
            burst_size: default_rate_limit_burst_size(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GrpcTlsConfig {
    #[serde(default)]
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrpcConfig {
    #[serde(default = "default_grpc_enabled")]
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub auth_enabled: bool,
    #[serde(default)]
    pub tls: GrpcTlsConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PiiMode {
    Full,
    Redact,
}

impl Default for PiiMode {
    fn default() -> Self {
        Self::Redact
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    pub exporter: String,
    #[serde(default)]
    pub syslog: Option<SyslogConfig>,
    #[serde(default)]
    pub otel_logs: Option<OtelLogsConfig>,
    #[serde(default)]
    pub pii_mode: PiiMode,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyslogConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_syslog_protocol")]
    pub protocol: String,
    #[serde(default = "default_syslog_facility")]
    pub facility: String,
    #[serde(default)]
    pub tls_enabled: bool,
}

fn default_syslog_protocol() -> String {
    "udp".to_owned()
}

fn default_syslog_facility() -> String {
    "local0".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtelLogsConfig {
    pub endpoint: String,
    #[serde(default = "default_otlp_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> AppConfig {
        AppConfig {
            server: ServerConfig {
                environment: Environment::Local,
                host: "127.0.0.1".to_owned(),
                port: 3001,
                timeout_seconds: 30,
                shutdown_timeout_seconds: 30,
                tls: TlsConfig {
                    enabled: false,
                    cert_path: String::new(),
                    key_path: String::new(),
                },
                cors_origins: vec!["http://localhost:3000".to_owned()],
                docs_enabled: true,
                max_request_body_size: 1_048_576,
                metrics_interval_seconds: 15,
            },
            database: DatabaseConfig {
                driver: DatabaseDriver::Mssql,
                host: "localhost".to_owned(),
                port: 1433,
                database: "fullstack_template".to_owned(),
                username: "sa".to_owned(),
                password: "MyS3cur3P@ssphrase!".to_owned(),
                password_file: None,
                max_connections: 10,
                connect_retry_attempts: 3,
                connect_retry_delay_ms: 100,
                encrypt: false,
                trust_cert: false,
                ca_cert_path: None,
                run_migrations_on_startup: true,
            },
            auth: AuthConfig {
                enabled: false,
                issuer_url: "http://localhost:8080/dex".to_owned(),
                audience: vec!["fullstack-template".to_owned()],
                jwks_cache_duration_secs: 3600,
                allowed_email_domains: vec![],
                allow_all_domains: false,
                role_claim_source: RoleClaimSource::Groups,
                discovery_mode: DiscoveryMode::Discovery,
                manual_endpoints: None,
                danger_accept_invalid_certs: false,
                allowed_algorithms: default_allowed_algorithms(),
                require_email_verified: false,
                clock_skew_seconds: 60,
            },
            observability: ObservabilityConfig {
                log_format: LogFormat::Pretty,
                log_level: "info".to_owned(),
                service_name: "fullstack-template".to_owned(),
                metrics_enabled: true,
                metrics_auth_token: None,
                otlp: OtlpConfig::default(),
            },
            grpc: GrpcConfig {
                enabled: false,
                host: "127.0.0.1".to_owned(),
                port: 50051,
                auth_enabled: false,
                tls: GrpcTlsConfig {
                    enabled: false,
                    cert_path: String::new(),
                    key_path: String::new(),
                    ca_cert_path: None,
                },
            },
            audit: AuditConfig {
                exporter: "none".to_owned(),
                syslog: None,
                otel_logs: None,
                pii_mode: PiiMode::Full,
            },
            rate_limit: RateLimitConfig::default(),
        }
    }

    #[test]
    fn validate_should_accept_default_observability_configuration() {
        let config = make_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_reject_empty_service_name() {
        let mut config = make_config();
        config.observability.service_name = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_require_otlp_endpoint_when_enabled() {
        let mut config = make_config();
        config.observability.otlp.enabled = true;
        config.observability.otlp.endpoint = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_reject_zero_otlp_timeout() {
        let mut config = make_config();
        config.observability.otlp.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_require_tls_cert_when_tls_enabled() {
        let mut config = make_config();
        config.server.tls.enabled = true;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_accept_tls_when_configured() {
        let mut config = make_config();
        config.server.tls.enabled = true;
        config.server.tls.cert_path = "/tmp/cert.pem".to_owned();
        config.server.tls.key_path = "/tmp/key.pem".to_owned();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_require_auth_config_when_enabled() {
        let mut config = make_config();
        config.auth.enabled = true;
        config.auth.issuer_url = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_reject_http_issuer_outside_localhost() {
        let mut config = make_config();
        config.auth.enabled = true;
        config.auth.issuer_url = "http://idp.bank.com".to_owned();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_allow_http_issuer_for_localhost() {
        let mut config = make_config();
        config.auth.enabled = true;
        config.auth.issuer_url = "http://localhost:8080/dex".to_owned();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_reject_trust_cert_in_production() {
        let mut config = make_config();
        config.database.trust_cert = true;
        config.server.environment = Environment::Production;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_allow_trust_cert_for_localhost() {
        let mut config = make_config();
        config.database.trust_cert = true;
        config.auth.enabled = true;
        config.auth.issuer_url = "http://localhost:8080/dex".to_owned();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_reject_danger_accept_invalid_certs_for_non_local() {
        let mut config = make_config();
        config.auth.enabled = true;
        config.auth.issuer_url = "https://idp.bank.com".to_owned();
        config.auth.danger_accept_invalid_certs = true;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_allow_danger_accept_invalid_certs_for_localhost() {
        let mut config = make_config();
        config.auth.enabled = true;
        config.auth.issuer_url = "http://localhost:8080/dex".to_owned();
        config.auth.danger_accept_invalid_certs = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_require_metrics_auth_token_in_production() {
        let mut config = make_config();
        config.observability.metrics_enabled = true;
        config.server.environment = Environment::Production;
        assert!(config.validate().is_err());

        config.observability.metrics_auth_token = Some("secret".to_owned());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_require_explicit_jit_scope_in_production() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.auth.enabled = true;
        config.server.environment = Environment::Production;
        config.auth.allowed_email_domains = vec![];
        assert!(config.validate().is_err());

        config.auth.allowed_email_domains = vec!["bank.com".to_owned()];
        assert!(config.validate().is_ok());

        config.auth.allowed_email_domains = vec![];
        config.auth.allow_all_domains = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_reject_unreliable_audit_in_production() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.audit.exporter = "syslog".to_owned();
        config.audit.syslog = Some(SyslogConfig {
            host: "logs.bank.com".to_owned(),
            port: 514,
            protocol: "udp".to_owned(),
            facility: "local0".to_owned(),
            tls_enabled: false,
        });
        assert!(config.validate().is_err());

        config.audit.syslog = Some(SyslogConfig {
            host: "logs.bank.com".to_owned(),
            port: 514,
            protocol: "tcp+tls".to_owned(),
            facility: "local0".to_owned(),
            tls_enabled: true,
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_require_https_for_otlp_audit_in_production() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.audit.exporter = "otel-logs".to_owned();
        config.audit.otel_logs = Some(OtelLogsConfig {
            endpoint: "http://collector.bank.com:4318/v1/logs".to_owned(),
            timeout_seconds: 5,
        });
        assert!(config.validate().is_err());

        config.audit.otel_logs = Some(OtelLogsConfig {
            endpoint: "https://collector.bank.com:4318/v1/logs".to_owned(),
            timeout_seconds: 5,
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_require_grpc_tls_in_production() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.grpc.enabled = true;
        assert!(config.validate().is_err());

        config.grpc.tls.enabled = true;
        config.grpc.tls.cert_path = "/tmp/grpc.crt".to_owned();
        config.grpc.tls.key_path = "/tmp/grpc.key".to_owned();
        config.grpc.auth_enabled = true;
        config.grpc.tls.ca_cert_path = Some("/tmp/grpc-ca.crt".to_owned());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_should_reject_grpc_prod_with_tls_but_no_auth() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.grpc.enabled = true;
        config.grpc.tls.enabled = true;
        config.grpc.tls.cert_path = "/tmp/grpc.crt".to_owned();
        config.grpc.tls.key_path = "/tmp/grpc.key".to_owned();
        config.grpc.tls.ca_cert_path = Some("/tmp/grpc-ca.crt".to_owned());
        // auth_enabled is false
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_reject_grpc_prod_with_tls_but_no_client_ca() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.grpc.enabled = true;
        config.grpc.tls.enabled = true;
        config.grpc.tls.cert_path = "/tmp/grpc.crt".to_owned();
        config.grpc.tls.key_path = "/tmp/grpc.key".to_owned();
        config.grpc.auth_enabled = true;
        // ca_cert_path is None
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_should_accept_grpc_prod_with_tls_mtls_ca_and_auth() {
        let mut config = make_config();
        config.observability.metrics_enabled = false;
        config.server.environment = Environment::Production;
        config.grpc.enabled = true;
        config.grpc.tls.enabled = true;
        config.grpc.tls.cert_path = "/tmp/grpc.crt".to_owned();
        config.grpc.tls.key_path = "/tmp/grpc.key".to_owned();
        config.grpc.auth_enabled = true;
        config.grpc.tls.ca_cert_path = Some("/tmp/grpc-ca.crt".to_owned());
        assert!(config.validate().is_ok());
    }
}

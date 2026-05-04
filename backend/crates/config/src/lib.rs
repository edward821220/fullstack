use figment::{
    Figment,
    providers::{Env, Format, Yaml},
};
use serde::Deserialize;
use snafu::Snafu;
use std::net::SocketAddr;

fn default_metrics_enabled() -> bool {
    true
}

fn default_otlp_timeout_seconds() -> u64 {
    5
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
    pub shutdown_timeout_seconds: u64,
    pub cors_origins: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub driver: DatabaseDriver,
    pub database_url: String,
    pub max_connections: u32,
    pub connect_retry_attempts: u32,
    pub connect_retry_delay_ms: u64,
    #[serde(default)]
    pub encrypt: bool,
}

impl DatabaseConfig {
    pub fn driver(&self) -> &DatabaseDriver {
        &self.driver
    }

    pub fn to_tiberius_config(&self) -> std::result::Result<tiberius::Config, url::ParseError> {
        let url = url::Url::parse(&self.database_url)?;

        let host = url.host_str().ok_or(url::ParseError::EmptyHost)?;

        let port: u16 = url.port().unwrap_or(1433);

        let database = url
            .path_segments()
            .and_then(|mut segs| segs.next())
            .filter(|s| !s.is_empty())
            .ok_or(url::ParseError::EmptyHost)?;

        let user = url.username().to_owned();
        let password = url.password().ok_or(url::ParseError::EmptyHost)?.to_owned();

        let mut config = tiberius::Config::new();
        config.host(host);
        config.port(port);
        config.database(database);
        config.authentication(tiberius::AuthMethod::sql_server(&user, &password));

        if self.encrypt {
            config.encryption(tiberius::EncryptionLevel::Required);
            config.trust_cert();
        } else {
            config.encryption(tiberius::EncryptionLevel::NotSupported);
        }

        Ok(config)
    }

    pub fn extract_mssql_database_name(&self) -> std::result::Result<String, url::ParseError> {
        let url = url::Url::parse(&self.database_url)?;
        url.path_segments()
            .and_then(|mut segs| segs.next())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned())
            .ok_or(url::ParseError::EmptyHost)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub issuer_url: String,
    pub audience: Vec<String>,
    pub jwks_cache_duration_secs: u64,
    pub allowed_email_domains: Vec<String>,
    pub role_claim_source: RoleClaimSource,
    pub discovery_mode: DiscoveryMode,
    #[serde(default)]
    pub manual_endpoints: Option<ManualOidcEndpoints>,
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,
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
pub struct GrpcConfig {
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let local_path = std::path::Path::new("config/local.yaml");
        let mut figment = Figment::new().merge(Yaml::file("config/default.yaml"));

        if local_path.exists() {
            figment = figment.merge(Yaml::file(local_path));
        }

        figment
            .merge(Env::prefixed("APP_").split("__"))
            .extract()
            .map_err(|e| Error::Load {
                source: Box::new(e),
            })
    }

    pub fn validate(&self) -> Result<()> {
        self.rest_addr()?;
        self.grpc_addr()?;

        if self.auth.enabled {
            if self.auth.issuer_url.is_empty() {
                return Err(Error::Invalid {
                    message: "auth.issuer_url must be set when auth is enabled".to_owned(),
                });
            }
            if self.auth.audience.is_empty() {
                return Err(Error::Invalid {
                    message: "auth.audience must contain at least one audience value when auth is enabled".to_owned(),
                });
            }
        }

        if self.database.database_url.is_empty() {
            return Err(Error::Invalid {
                message: "database.database_url must be set".to_owned(),
            });
        }

        if self.observability.service_name.trim().is_empty() {
            return Err(Error::Invalid {
                message: "observability.service_name must be set".to_owned(),
            });
        }

        if self.observability.otlp.enabled && self.observability.otlp.endpoint.trim().is_empty() {
            return Err(Error::Invalid {
                message: "observability.otlp.endpoint must be set when OTLP export is enabled"
                    .to_owned(),
            });
        }

        if self.observability.otlp.timeout_seconds == 0 {
            return Err(Error::Invalid {
                message: "observability.otlp.timeout_seconds must be greater than 0".to_owned(),
            });
        }

        Ok(())
    }

    pub fn rest_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .map_err(|e| Error::Invalid {
                message: format!(
                    "Invalid REST server address {}:{}: {}",
                    self.server.host, self.server.port, e
                ),
            })
    }

    pub fn grpc_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.grpc.host, self.grpc.port)
            .parse()
            .map_err(|e| Error::Invalid {
                message: format!(
                    "Invalid gRPC server address {}:{}: {}",
                    self.grpc.host, self.grpc.port, e
                ),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> AppConfig {
        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_owned(),
                port: 3001,
                timeout_seconds: 30,
                shutdown_timeout_seconds: 30,
                cors_origins: vec!["http://localhost:3000".to_owned()],
            },
            database: DatabaseConfig {
                driver: DatabaseDriver::Mssql,
                database_url: "mssql://sa:Password!123@localhost:1433/fullstack_template"
                    .to_owned(),
                max_connections: 10,
                connect_retry_attempts: 3,
                connect_retry_delay_ms: 100,
                encrypt: false,
            },
            auth: AuthConfig {
                enabled: false,
                issuer_url: "http://localhost:8080/dex".to_owned(),
                audience: vec!["fullstack-template".to_owned()],
                jwks_cache_duration_secs: 3600,
                allowed_email_domains: vec![],
                role_claim_source: RoleClaimSource::Groups,
                discovery_mode: DiscoveryMode::Discovery,
                manual_endpoints: None,
                danger_accept_invalid_certs: false,
            },
            observability: ObservabilityConfig {
                log_format: LogFormat::Pretty,
                log_level: "info".to_owned(),
                service_name: "fullstack-template".to_owned(),
                metrics_enabled: true,
                otlp: OtlpConfig::default(),
            },
            grpc: GrpcConfig {
                host: "127.0.0.1".to_owned(),
                port: 50051,
            },
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
}

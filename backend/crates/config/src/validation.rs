use crate::{AppConfig, Error, Result};
use std::net::SocketAddr;

impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        self.rest_addr()?;
        if self.grpc.enabled {
            self.grpc_addr()?;
        }

        // TLS validation
        if self.server.tls.enabled {
            if self.server.tls.cert_path.is_empty() {
                return Err(Error::Invalid {
                    message: "server.tls.cert_path must be set when TLS is enabled. \
                              For local development, create config/local.yaml with tls.enabled: false"
                        .to_owned(),
                });
            }
            if self.server.tls.key_path.is_empty() {
                return Err(Error::Invalid {
                    message: "server.tls.key_path must be set when TLS is enabled. \
                              For local development, create config/local.yaml with tls.enabled: false"
                        .to_owned(),
                });
            }
        }

        // Auth validation
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
            if self.auth.issuer_url.starts_with("http://") {
                let is_localhost = self.auth.issuer_url.contains("localhost")
                    || self.auth.issuer_url.contains("127.0.0.1");
                if !is_localhost {
                    return Err(Error::Invalid {
                        message: "auth.issuer_url must use https:// in production (non-localhost)"
                            .to_owned(),
                    });
                }
            }
            if self.auth.allowed_algorithms.is_empty() {
                return Err(Error::Invalid {
                    message: "auth.allowed_algorithms must not be empty".to_owned(),
                });
            }
            if self.auth.danger_accept_invalid_certs {
                let is_local = self.auth.issuer_url.contains("localhost")
                    || self.auth.issuer_url.contains("127.0.0.1");
                if !is_local {
                    return Err(Error::Invalid {
                        message: "auth.danger_accept_invalid_certs is only permitted for localhost IdP. \
                                  For bank/private-CA IdPs, mount the CA bundle and use SSL_CERT_FILE instead."
                            .to_owned(),
                    });
                }
            }
        }

        // Database validation
        if self.database.host.is_empty() {
            return Err(Error::Invalid {
                message: "database.host must be set".to_owned(),
            });
        }
        if self.database.database.is_empty() {
            return Err(Error::Invalid {
                message: "database.database must be set".to_owned(),
            });
        }
        if self.database.username.is_empty() {
            return Err(Error::Invalid {
                message: "database.username must be set".to_owned(),
            });
        }

        let db_password = self.resolve_db_password()?;
        if db_password.is_empty() {
            return Err(Error::Invalid {
                message: "database.password must be set (or password_file must point to a non-empty file)".to_owned(),
            });
        }

        // Reject known weak/dev passwords outside local development
        if !self.is_local() {
            let lower = db_password.to_lowercase();
            let weak_passwords: &[&str] = &[
                "password",
                "123456",
                "admin",
                "sa",
                "root",
                "letmein",
                "welcome",
                "monkey",
                "1234567890",
                "password123",
                "qwerty",
                "abc123",
                "changeme",
                "default",
            ];
            if weak_passwords.iter().any(|w| lower.contains(w)) || db_password.len() < 12 {
                return Err(Error::Invalid {
                    message: "database.password is too weak for non-local environments. \
                              Use a strong password or set password_file to a secret mount."
                        .to_owned(),
                });
            }
        }

        // Reject blanket trust_cert outside local development
        if self.database.trust_cert && !self.is_local() {
            return Err(Error::Invalid {
                message: "database.trust_cert is not allowed in non-local environments. \
                          For bank/private-CA databases, set database.ca_cert_path to the enterprise CA bundle."
                    .to_owned(),
            });
        }

        // Validate CA cert path if provided
        if let Some(ref ca_path) = self.database.ca_cert_path
            && !ca_path.is_empty()
            && !std::path::Path::new(ca_path).exists()
        {
            return Err(Error::Invalid {
                message: format!("database.ca_cert_path '{ca_path}' does not exist"),
            });
        }

        if self.database.password_file.is_some() && !self.database.password.is_empty() {
            // Warn if both are set, but don't fail — password_file takes precedence at runtime
            tracing::warn!(
                "Both database.password and database.password_file are set; password_file takes precedence"
            );
        }

        // Observability validation
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

        // Metrics fail-closed: require auth token when enabled in production.
        // Sandbox and staging may run metrics without a token for debugging.
        if self.observability.metrics_enabled && self.is_production() {
            let token_set = self
                .observability
                .metrics_auth_token
                .as_ref()
                .is_some_and(|t| !t.is_empty());
            if !token_set {
                return Err(Error::Invalid {
                    message: "observability.metrics_auth_token must be set when metrics are enabled in production. \
                              For local development, metrics are also disabled by default."
                        .to_owned(),
                });
            }
        }

        // JIT provisioning scope: require explicit allowlist or escape hatch in production
        if self.auth.enabled
            && !self.is_local()
            && self.auth.allowed_email_domains.is_empty()
            && !self.auth.allow_all_domains
        {
            return Err(Error::Invalid {
                message: "auth.allowed_email_domains is empty and auth.allow_all_domains is false. \
                              For bank rollout, explicitly list allowed domains or set auth.allow_all_domains: true."
                    .to_owned(),
            });
        }

        // Audit validation
        match self.audit.exporter.as_str() {
            "none" => {}
            "syslog" => {
                let cfg = self.audit.syslog.as_ref().ok_or_else(|| Error::Invalid {
                    message: "audit.syslog must be configured when exporter is 'syslog'".to_owned(),
                })?;
                if cfg.host.is_empty() {
                    return Err(Error::Invalid {
                        message: "audit.syslog.host must be set".to_owned(),
                    });
                }
                if cfg.protocol != "udp" && cfg.protocol != "tcp" && cfg.protocol != "tcp+tls" {
                    return Err(Error::Invalid {
                        message: "audit.syslog.protocol must be 'udp', 'tcp', or 'tcp+tls'"
                            .to_owned(),
                    });
                }
                // Enforce reliable transport outside local development
                if !self.is_local() {
                    if cfg.protocol == "udp" {
                        return Err(Error::Invalid {
                            message: "audit.syslog.protocol 'udp' is not allowed in non-local environments. \
                                      Use 'tcp+tls' for reliable, encrypted audit transport."
                                .to_owned(),
                        });
                    }
                    if cfg.protocol == "tcp" && !cfg.tls_enabled {
                        return Err(Error::Invalid {
                            message: "audit.syslog cleartext TCP is not allowed in non-local environments. \
                                      Use 'tcp+tls' or enable tls_enabled."
                                .to_owned(),
                        });
                    }
                }
            }
            "otel-logs" => {
                let cfg = self
                    .audit
                    .otel_logs
                    .as_ref()
                    .ok_or_else(|| Error::Invalid {
                        message: "audit.otel_logs must be configured when exporter is 'otel-logs'"
                            .to_owned(),
                    })?;
                if cfg.endpoint.is_empty() {
                    return Err(Error::Invalid {
                        message: "audit.otel_logs.endpoint must be set".to_owned(),
                    });
                }
                if cfg.timeout_seconds == 0 {
                    return Err(Error::Invalid {
                        message: "audit.otel_logs.timeout_seconds must be greater than 0"
                            .to_owned(),
                    });
                }
                // Enforce TLS for OTLP outside local development
                if !self.is_local() && !cfg.endpoint.starts_with("https://") {
                    return Err(Error::Invalid {
                        message:
                            "audit.otel_logs.endpoint must use https:// in non-local environments."
                                .to_owned(),
                    });
                }
            }
            other => {
                return Err(Error::Invalid {
                    message: format!(
                        "audit.exporter must be 'none', 'syslog', or 'otel-logs', got: {other}"
                    ),
                });
            }
        }

        // gRPC production safety
        if self.grpc.enabled && !self.is_local() {
            if self.grpc.tls.enabled {
                if self.grpc.tls.cert_path.is_empty() {
                    return Err(Error::Invalid {
                        message: "grpc.tls.cert_path must be set when gRPC TLS is enabled"
                            .to_owned(),
                    });
                }
                if self.grpc.tls.key_path.is_empty() {
                    return Err(Error::Invalid {
                        message: "grpc.tls.key_path must be set when gRPC TLS is enabled"
                            .to_owned(),
                    });
                }
                if !self.grpc.auth_enabled {
                    return Err(Error::Invalid {
                        message: "grpc.auth_enabled must be true in non-local environments. \
                                  Service-to-service gRPC requires JWT validation."
                            .to_owned(),
                    });
                }
                let has_ca = self
                    .grpc
                    .tls
                    .ca_cert_path
                    .as_ref()
                    .is_some_and(|s| !s.is_empty());
                if !has_ca {
                    return Err(Error::Invalid {
                        message: "grpc.tls.ca_cert_path must be set in non-local environments. \
                                  Service-to-service gRPC requires mTLS client CA verification."
                            .to_owned(),
                    });
                }
            } else {
                return Err(Error::Invalid {
                    message: "grpc.tls.enabled must be true in non-local environments. \
                              For local development, disable grpc or keep it unencrypted."
                        .to_owned(),
                });
            }
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

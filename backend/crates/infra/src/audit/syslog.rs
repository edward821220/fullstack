use std::sync::Arc;
use svc::audit::{AuditError, AuditEvent, AuditExporter};
/// Syslog exporter (RFC 5424).
/// Supports UDP (default), TCP, and TCP+TLS.
pub struct SyslogExporter {
    host: String,
    port: u16,
    protocol: String,
    facility: u8,
    app_name: String,
    hostname: String,
    tls_connector: Option<tokio_rustls::TlsConnector>,
}
impl SyslogExporter {
    pub fn new(
        host: String,
        port: u16,
        protocol: String,
        facility_name: &str,
        app_name: String,
        hostname: String,
        tls_enabled: bool,
    ) -> Self {
        let facility = match facility_name.to_lowercase().as_str() {
            "kern" => 0,
            "user" => 1,
            "mail" => 2,
            "daemon" => 3,
            "auth" => 4,
            "syslog" => 5,
            "lpr" => 6,
            "news" => 7,
            "uucp" => 8,
            "cron" => 9,
            "authpriv" => 10,
            "ftp" => 11,
            "local0" => 16,
            "local1" => 17,
            "local2" => 18,
            "local3" => 19,
            "local4" => 20,
            "local5" => 21,
            "local6" => 22,
            "local7" => 23,
            _ => 16, // default local0
        };
        let tls_connector = if tls_enabled {
            let mut roots = tokio_rustls::rustls::RootCertStore::empty();
            let cert_result = rustls_native_certs::load_native_certs();
            for cert in cert_result.certs {
                roots.add(cert).ok();
            }
            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            Some(tokio_rustls::TlsConnector::from(Arc::new(config)))
        } else {
            None
        };
        Self {
            host,
            port,
            protocol: protocol.to_lowercase(),
            facility,
            app_name,
            hostname,
            tls_connector,
        }
    }
    fn to_rfc5424(&self, event: &AuditEvent) -> String {
        let severity = match event.level() {
            tracing::Level::INFO => 6,
            tracing::Level::WARN => 4,
            tracing::Level::ERROR => 3,
            _ => 6,
        };
        let pri = self.facility * 8 + severity;
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        let msg = format!("{event}");
        format!(
            "<{pri}>1 {timestamp} {hostname} {app_name} - - - {msg}",
            pri = pri,
            timestamp = timestamp,
            hostname = self.hostname,
            app_name = self.app_name,
        )
    }
}
#[async_trait::async_trait]
impl AuditExporter for SyslogExporter {
    async fn export(&self, event: AuditEvent) -> Result<(), AuditError> {
        let msg = self.to_rfc5424(&event);
        let addr = format!("{}:{}", self.host, self.port);
        if self.protocol == "tcp" || self.tls_connector.is_some() {
            let stream =
                tokio::net::TcpStream::connect(&addr)
                    .await
                    .map_err(|e| AuditError::Export {
                        message: format!("Syslog TCP connect failed: {e}"),
                    })?;
            if let Some(connector) = &self.tls_connector {
                let server_name = self
                    .host
                    .clone()
                    .try_into()
                    .map_err(|e| AuditError::Export {
                        message: format!("Invalid syslog TLS server name: {e}"),
                    })?;
                let mut tls_stream = connector.connect(server_name, stream).await.map_err(|e| {
                    AuditError::Export {
                        message: format!("Syslog TLS handshake failed: {e}"),
                    }
                })?;
                use tokio::io::AsyncWriteExt;
                if let Err(e) = tls_stream.write_all(msg.as_bytes()).await {
                    return Err(AuditError::Export {
                        message: format!("Syslog TLS write failed: {e}"),
                    });
                }
                if let Err(e) = tls_stream.write_all(b"\n").await {
                    return Err(AuditError::Export {
                        message: format!("Syslog TLS write failed: {e}"),
                    });
                }
            } else {
                let mut stream = stream;
                use tokio::io::AsyncWriteExt;
                if let Err(e) = stream.write_all(msg.as_bytes()).await {
                    return Err(AuditError::Export {
                        message: format!("Syslog TCP write failed: {e}"),
                    });
                }
                if let Err(e) = stream.write_all(b"\n").await {
                    return Err(AuditError::Export {
                        message: format!("Syslog TCP write failed: {e}"),
                    });
                }
            }
        } else {
            // UDP
            match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                Ok(socket) => {
                    if let Err(e) = socket.send_to(msg.as_bytes(), &addr).await {
                        return Err(AuditError::Export {
                            message: format!("Syslog UDP send failed: {e}"),
                        });
                    }
                }
                Err(e) => {
                    return Err(AuditError::Export {
                        message: format!("Syslog UDP bind failed: {e}"),
                    });
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;

    fn sample_event() -> AuditEvent {
        AuditEvent::AuthSuccess {
            user_id: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            email: "alice@example.com".to_owned(),
            role: "admin".to_owned(),
            sub: "auth0|123".to_owned(),
        }
    }

    #[tokio::test]
    async fn udp_sends_rfc5424_message() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();

        let exporter = SyslogExporter::new(
            addr.ip().to_string(),
            addr.port(),
            "udp".to_owned(),
            "local0",
            "test-app".to_owned(),
            "test-host".to_owned(),
            false,
        );

        let event = sample_event();
        let export_handle = tokio::spawn(async move { exporter.export(event).await });

        let mut buf = vec![0u8; 2048];
        let (len, _) = tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();

        export_handle.await.unwrap().unwrap();

        let msg = String::from_utf8(buf[..len].to_vec()).unwrap();
        // PRI = facility(16) * 8 + severity(6) = 134
        assert!(
            msg.starts_with("<134>1 "),
            "unexpected PRI or version: {msg}"
        );
        assert!(msg.contains("test-host"), "missing hostname: {msg}");
        assert!(msg.contains("test-app"), "missing app_name: {msg}");
        assert!(msg.contains("auth_success"), "missing event display: {msg}");
        assert!(msg.contains("alice@example.com"), "missing email: {msg}");
    }

    #[tokio::test]
    async fn tcp_sends_rfc5424_message_with_newline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let accept_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            let n = stream.read(&mut buf).await.unwrap();
            String::from_utf8(buf[..n].to_vec()).unwrap()
        });

        let exporter = SyslogExporter::new(
            addr.ip().to_string(),
            addr.port(),
            "tcp".to_owned(),
            "local0",
            "test-app".to_owned(),
            "test-host".to_owned(),
            false,
        );

        exporter.export(sample_event()).await.unwrap();

        let msg = tokio::time::timeout(Duration::from_secs(5), accept_handle)
            .await
            .unwrap()
            .unwrap();

        assert!(
            msg.starts_with("<134>1 "),
            "unexpected PRI or version: {msg}"
        );
        assert!(
            msg.ends_with('\n'),
            "TCP message should end with newline: {msg}"
        );
        assert!(msg.contains("test-host"), "missing hostname: {msg}");
        assert!(msg.contains("auth_success"), "missing event display: {msg}");
    }

    #[tokio::test]
    async fn facility_affects_pri() {
        // auth (facility 4) + INFO (severity 6) = PRI 38
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();

        let exporter = SyslogExporter::new(
            addr.ip().to_string(),
            addr.port(),
            "udp".to_owned(),
            "auth",
            "app".to_owned(),
            "host".to_owned(),
            false,
        );

        let export_handle = tokio::spawn(async move {
            exporter.export(sample_event()).await.unwrap();
        });

        let mut buf = vec![0u8; 2048];
        let (len, _) = tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();

        export_handle.await.unwrap();
        let msg = String::from_utf8(buf[..len].to_vec()).unwrap();
        assert!(msg.starts_with("<38>1 "), "auth facility(4)*8+6=38: {msg}");
    }

    #[tokio::test]
    async fn warn_event_affects_severity() {
        // local0 (facility 16) + WARN (severity 4) = PRI 132
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();

        let exporter = SyslogExporter::new(
            addr.ip().to_string(),
            addr.port(),
            "udp".to_owned(),
            "local0",
            "app".to_owned(),
            "host".to_owned(),
            false,
        );

        let event = AuditEvent::AuthFailure {
            reason: "expired".to_owned(),
        };

        let export_handle = tokio::spawn(async move { exporter.export(event).await.unwrap() });

        let mut buf = vec![0u8; 2048];
        let (len, _) = tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();

        export_handle.await.unwrap();
        let msg = String::from_utf8(buf[..len].to_vec()).unwrap();
        assert!(
            msg.starts_with("<132>1 "),
            "WARN severity(4): 16*8+4=132: {msg}"
        );
    }
}

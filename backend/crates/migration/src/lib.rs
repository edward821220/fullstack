mod postgres_migrations {
    refinery::embed_migrations!("./migrations/postgres");
}

mod mssql_migrations {
    refinery::embed_migrations!("./migrations/mssql");
}

/// Custom certificate verifier that accepts any certificate.
/// Only used for local development with `database.trust_cert: true`.
#[derive(Debug)]
struct AcceptAnyCert;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _server_name: &tokio_rustls::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> Result<tokio_rustls::rustls::client::danger::ServerCertVerified, tokio_rustls::rustls::Error>
    {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        vec![
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA512,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            tokio_rustls::rustls::SignatureScheme::ED25519,
        ]
    }
}

fn make_postgres_tls_connector(
    config: &config::DatabaseConfig,
) -> Result<tokio_postgres_rustls::MakeRustlsConnect, Box<dyn std::error::Error>> {
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();

    if config.trust_cert {
        let client_config = tokio_rustls::rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
            .with_no_client_auth();
        return Ok(tokio_postgres_rustls::MakeRustlsConnect::new(client_config));
    }

    if let Some(ref ca_path) = config.ca_cert_path
        && !ca_path.is_empty()
    {
        let cert_file = std::fs::read(ca_path)
            .map_err(|e| format!("Failed to read database CA cert file '{ca_path}': {e}"))?;
        let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(&cert_file[..]))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to parse database CA cert: {e}"))?;
        for cert in certs {
            roots.add(cert)?;
        }
    } else {
        let cert_result = rustls_native_certs::load_native_certs();
        for cert in cert_result.certs {
            roots.add(cert).ok();
        }
    }

    let client_config = tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(tokio_postgres_rustls::MakeRustlsConnect::new(client_config))
}

use std::sync::Arc;

pub async fn run(config: &config::DatabaseConfig) -> Result<(), Box<dyn std::error::Error>> {
    use config::DatabaseDriver;

    match config.driver() {
        DatabaseDriver::Postgres => {
            tracing::info!("Running PostgreSQL migrations via refinery");

            let url = config.to_postgres_url();

            let mut client = if config.encrypt {
                let tls = make_postgres_tls_connector(config)?;
                let (client, connection) = tokio_postgres::connect(&url, tls).await?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("PostgreSQL migration connection error: {e}");
                    }
                });
                client
            } else {
                let (client, connection) =
                    tokio_postgres::connect(&url, tokio_postgres::NoTls).await?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("PostgreSQL migration connection error: {e}");
                    }
                });
                client
            };

            let report = postgres_migrations::migrations::runner()
                .run_async(&mut client)
                .await?;

            for migration in report.applied_migrations() {
                tracing::info!(
                    "PostgreSQL migration applied: {} {}",
                    migration.version(),
                    migration.name()
                );
            }

            tracing::info!("PostgreSQL migrations complete");
        }
        DatabaseDriver::Mssql => {
            tracing::info!("Running MSSQL migrations via refinery");
            let db_name = config.extract_mssql_database_name()?;

            let tiberius_config = config.to_tiberius_config()?;

            // Connect to master first, create database if it doesn't exist
            {
                let mut master_config = tiberius_config.clone();
                master_config.database("master");
                let addr = master_config.get_addr().to_owned();
                let tcp = tokio::net::TcpStream::connect(addr).await?;
                tcp.set_nodelay(true)?;
                let mut master_client = tiberius::Client::connect(
                    master_config,
                    tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tcp),
                )
                .await?;

                let create_db = format!(
                    "IF NOT EXISTS (SELECT name FROM sys.databases WHERE name = '{db_name}') CREATE DATABASE [{db_name}]"
                );
                master_client.execute(&create_db, &[]).await?;
                tracing::info!("MSSQL database '{}' ensured", db_name);
            }

            let addr = tiberius_config.get_addr().to_owned();
            let tcp = tokio::net::TcpStream::connect(addr).await?;
            tcp.set_nodelay(true)?;

            let mut client = tiberius::Client::connect(
                tiberius_config,
                tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tcp),
            )
            .await?;

            let report = mssql_migrations::migrations::runner()
                .run_async(&mut client)
                .await?;

            for migration in report.applied_migrations() {
                tracing::info!(
                    "MSSQL migration applied: {} {}",
                    migration.version(),
                    migration.name()
                );
            }

            tracing::info!("MSSQL migrations complete");
        }
    }

    Ok(())
}

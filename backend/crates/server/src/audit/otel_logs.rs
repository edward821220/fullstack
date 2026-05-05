use std::time::Duration;

use svc::audit::{AuditError, AuditEvent, AuditExporter};

use super::proxy::AuditEventProxy;

/// OTLP Logs exporter.
/// Sends structured JSON over HTTP to an OTEL Collector.
pub struct OtelLogsExporter {
    client: reqwest::Client,
    endpoint: String,
    service_name: String,
}

impl OtelLogsExporter {
    pub fn new(endpoint: String, timeout_seconds: u64, service_name: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .unwrap_or_default();
        Self {
            client,
            endpoint,
            service_name,
        }
    }

    fn to_otlp_body(&self, event: &AuditEvent) -> serde_json::Value {
        let proxy = AuditEventProxy::from(event);
        let timestamp_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let severity_number = match event.level() {
            tracing::Level::INFO => 9,
            tracing::Level::WARN => 13,
            tracing::Level::ERROR => 17,
            _ => 9,
        };

        let mut attributes = vec![serde_json::json!(
            {"key": "audit.event_type", "value": {"stringValue": proxy.event_type}}
        )];
        if let Some(v) = &proxy.user_id {
            attributes.push(serde_json::json!(
                {"key": "audit.user_id", "value": {"stringValue": v}}
            ));
        }
        if let Some(v) = &proxy.email {
            attributes.push(serde_json::json!(
                {"key": "audit.email", "value": {"stringValue": v}}
            ));
        }
        if let Some(v) = &proxy.role {
            attributes.push(serde_json::json!(
                {"key": "audit.role", "value": {"stringValue": v}}
            ));
        }
        if let Some(v) = &proxy.actor_id {
            attributes.push(serde_json::json!(
                {"key": "audit.actor_id", "value": {"stringValue": v}}
            ));
        }
        if let Some(v) = &proxy.target_id {
            attributes.push(serde_json::json!(
                {"key": "audit.target_id", "value": {"stringValue": v}}
            ));
        }
        if let Some(v) = &proxy.reason {
            attributes.push(serde_json::json!(
                {"key": "audit.reason", "value": {"stringValue": v}}
            ));
        }

        serde_json::json!({
            "resourceLogs": [
                {
                    "resource": {
                        "attributes": [
                            {"key": "service.name", "value": {"stringValue": self.service_name}}
                        ]
                    },
                    "scopeLogs": [
                        {
                            "scope": {"name": "audit"},
                            "logRecords": [
                                {
                                    "timeUnixNano": timestamp_nanos.to_string(),
                                    "severityNumber": severity_number,
                                    "body": {"stringValue": format!("{event}")},
                                    "attributes": attributes
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    }
}

#[async_trait::async_trait]
impl AuditExporter for OtelLogsExporter {
    async fn export(&self, event: AuditEvent) -> Result<(), AuditError> {
        let body = self.to_otlp_body(&event);
        match self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(())
                } else {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    Err(AuditError::Export {
                        message: format!("OTLP logs export failed: HTTP {status}, body: {text}"),
                    })
                }
            }
            Err(e) => Err(AuditError::Export {
                message: format!("OTLP logs request failed: {e}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    fn sample_event() -> AuditEvent {
        AuditEvent::AuthSuccess {
            user_id: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            email: "alice@example.com".to_owned(),
            role: "admin".to_owned(),
            sub: "auth0|123".to_owned(),
        }
    }

    #[test]
    fn otlp_body_structure() {
        let exporter = OtelLogsExporter::new(
            "http://localhost:4318/v1/logs".to_owned(),
            5,
            "test-service".to_owned(),
        );
        let body = exporter.to_otlp_body(&sample_event());

        let resource_logs = body["resourceLogs"].as_array().unwrap();
        assert_eq!(resource_logs.len(), 1);

        let resource = &resource_logs[0]["resource"];
        let svc_attr = resource["attributes"].as_array().unwrap();
        assert_eq!(svc_attr[0]["key"], "service.name");
        assert_eq!(svc_attr[0]["value"]["stringValue"], "test-service");

        let log_records = &resource_logs[0]["scopeLogs"][0]["logRecords"];
        let record = &log_records.as_array().unwrap()[0];
        assert_eq!(record["severityNumber"], 9);
        assert!(
            record["timeUnixNano"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                > 0
        );
        assert!(
            record["body"]["stringValue"]
                .as_str()
                .unwrap()
                .contains("auth_success")
        );

        let attrs = record["attributes"].as_array().unwrap();
        let keys: Vec<_> = attrs.iter().map(|a| a["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&"audit.event_type"));
        assert!(keys.contains(&"audit.user_id"));
        assert!(keys.contains(&"audit.email"));
        assert!(keys.contains(&"audit.role"));
    }

    #[test]
    fn warn_event_sets_severity_13() {
        let exporter = OtelLogsExporter::new(
            "http://localhost:4318/v1/logs".to_owned(),
            5,
            "svc".to_owned(),
        );
        let event = AuditEvent::AuthFailure {
            reason: "expired".to_owned(),
        };
        let body = exporter.to_otlp_body(&event);
        let record = &body["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert_eq!(record["severityNumber"], 13);
        assert!(
            record["body"]["stringValue"]
                .as_str()
                .unwrap()
                .contains("auth_failure")
        );
    }

    #[tokio::test]
    async fn export_posts_json_to_endpoint() {
        let received = Arc::new(Mutex::new(None));

        let app = {
            let rx = Arc::clone(&received);
            axum::Router::new().route(
                "/v1/logs",
                axum::routing::post(
                    move |body: axum::extract::Json<serde_json::Value>| async move {
                        *rx.lock().await = Some(body.0);
                        axum::http::StatusCode::OK
                    },
                ),
            )
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give the server a moment to start listening.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let exporter = OtelLogsExporter::new(
            format!("http://{}/v1/logs", addr),
            5,
            "test-service".to_owned(),
        );

        exporter.export(sample_event()).await.unwrap();

        let body = received
            .lock()
            .await
            .clone()
            .expect("mock server should have received a request");
        assert!(body.get("resourceLogs").is_some());

        let record = &body["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
        assert_eq!(record["severityNumber"], 9);
        let attrs = record["attributes"].as_array().unwrap();
        let keys: Vec<_> = attrs.iter().map(|a| a["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&"audit.event_type"));
    }

    #[tokio::test]
    async fn export_returns_error_on_http_failure() {
        let app = axum::Router::new().route(
            "/v1/logs",
            axum::routing::post(|| async { axum::http::StatusCode::INTERNAL_SERVER_ERROR }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let exporter =
            OtelLogsExporter::new(format!("http://{}/v1/logs", addr), 5, "svc".to_owned());

        let result = exporter.export(sample_event()).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("HTTP 500"));
    }
}

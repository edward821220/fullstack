use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

use super::event::{AuditError, AuditEvent};

/// Metrics for audit delivery health.
#[derive(Debug, Default)]
pub struct AuditMetrics {
    pub events_dropped: AtomicU64,
    pub exports_failed: AtomicU64,
}

#[async_trait::async_trait]
pub trait AuditExporter: Send + Sync {
    async fn export(&self, event: AuditEvent) -> Result<(), AuditError>;
}

/// Async audit dispatch channel with bounded backpressure and retry.
///
/// Owns the background task that drains events from an mpsc channel,
/// retries failed exports with exponential backoff, and records metrics.
pub struct AuditDispatcher {
    sender: mpsc::Sender<AuditEvent>,
    metrics: Arc<AuditMetrics>,
}

impl AuditDispatcher {
    /// Create a new dispatcher and spawn the background export task.
    pub fn new(exporter: Arc<dyn AuditExporter>) -> Self {
        // Bounded channel provides backpressure; excess events are dropped.
        const CHANNEL_CAPACITY: usize = 10_000;
        let (sender, mut receiver) = mpsc::channel::<AuditEvent>(CHANNEL_CAPACITY);
        let metrics = Arc::new(AuditMetrics::default());
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let mut retries = 0;
                let max_retries = 3;
                let mut delay_ms = 100;

                loop {
                    match exporter.export(event.clone()).await {
                        Ok(()) => break,
                        Err(e) => {
                            retries += 1;
                            if retries > max_retries {
                                tracing::error!(
                                    "Audit export failed after {max_retries} retries: {e}"
                                );
                                metrics_clone.exports_failed.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            tracing::warn!(
                                "Audit export failed (retry {retries}/{max_retries}): {e}"
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            delay_ms *= 2;
                        }
                    }
                }
            }
        });

        Self { sender, metrics }
    }

    /// Attempt to send an event into the channel.
    /// Drops the event (and increments metrics) when the channel is full or closed.
    pub fn send(&self, event: AuditEvent) {
        match self.sender.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.metrics.events_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!("Audit channel full, event dropped (total_dropped={dropped})");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!("Audit channel closed, event dropped");
            }
        }
    }

    pub fn metrics(&self) -> &AuditMetrics {
        &self.metrics
    }
}

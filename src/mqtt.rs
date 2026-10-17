use crate::models::ScanEvent;
use crate::repo;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use sqlx::PgPool;
use std::time::Duration;

/// Subscribes to scanner events and applies them to the database.
///
/// Scanner events are fire-and-forget: a scan for an unknown SKU is logged and
/// skipped rather than killing the loop, because one bad barcode must not stop
/// the warehouse feed.
pub async fn run(pool: PgPool, host: String, port: u16, topic: String) {
    let mut opts = MqttOptions::new("warehouse-tracker", host, port);
    opts.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(opts, 32);
    if let Err(e) = client.subscribe(&topic, QoS::AtLeastOnce).await {
        tracing::error!(error = %e, "mqtt subscribe failed");
        return;
    }
    tracing::info!(%topic, "listening for scanner events");

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let payload = String::from_utf8_lossy(&p.payload).to_string();
                match serde_json::from_str::<ScanEvent>(&payload) {
                    Ok(scan) => {
                        match repo::move_item_by_sku(&pool, &scan.sku, &scan.location, "scanner")
                            .await
                        {
                            Ok(item) => {
                                tracing::info!(sku = %item.sku, location = %item.location, "scan applied")
                            }
                            Err(e) => tracing::warn!(sku = %scan.sku, error = %e, "scan ignored"),
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, %payload, "malformed scan event"),
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "mqtt connection lost, retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

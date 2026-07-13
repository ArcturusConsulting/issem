//! High-speed telemetry ingestion actor pushing spatial vehicle matrices directly to Redis.

use std::error::Error;
use log::{info, warn, error};

pub async fn start_telemetry_uplink(
    config: crate::ZenohDriverConfig,
    zenoh_session: &zenoh::Session,
    redis_client: redis::Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("📤 [Southbound Telemetry] Initializing multi-tenant telemetry ingestion subsystem...");

    let mut redis_conn = redis_client.get_multiplexed_tokio_connection().await?;
    let telemetry_selector = "*/amcl_pose";
    let subscriber = zenoh_session.declare_subscriber(telemetry_selector).await?;

    info!("📥 [Southbound Telemetry] Zenoh subscriber bound to routing expression: '{}'", telemetry_selector);

    tokio::spawn(async move {
        while let Ok(sample) = subscriber.recv_async().await {
            let key_expr = sample.key_expr().as_str();
            
            let serial_number = match key_expr.split('/').next() {
                Some(serial) if !serial.is_empty() => serial.to_string(),
                _ => {
                    warn!("⚠️ [Telemetry] Received unparseable or anonymous routing path token: {}", key_expr);
                    continue;
                }
            };

            if !config.target_amr_serials.contains(&serial_number) {
                continue;
            }

            let payload = sample.payload().to_bytes();

            let amcl_msg: crate::ros_msg::PoseWithCovarianceStamped = match cdr::deserialize(&payload) {
                Ok(msg) => msg,
                Err(err) => {
                    error!("❌ [Telemetry] CDR payload decoding fault for asset [{}]: {}", serial_number, err);
                    continue;
                }
            };

            let x = amcl_msg.pose.pose.position.x;
            let y = amcl_msg.pose.pose.position.y;

            let q = &amcl_msg.pose.pose.orientation;
            let siny_cosp: f64 = 2.0 * (q.w * q.z + q.x * q.y);
            let cosy_cosp: f64 = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
            let theta: f64 = siny_cosp.atan2(cosy_cosp);

            let current_timestamp = match std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => duration.as_millis() as u64,
                    Err(_) => 0,
                };

            let redis_hash_key = format!("amr:{}:pose", serial_number);

            let update_result: redis::RedisResult<()> = redis::pipe()
                .hset(&redis_hash_key, "x", x)
                .hset(&redis_hash_key, "y", y)
                .hset(&redis_hash_key, "theta", theta)
                .hset(&redis_hash_key, "last_updated", current_timestamp)
                .query_async(&mut redis_conn)
                .await;

            if let Err(err) = update_result {
                error!("❌ [Telemetry] Failed to persist state coordinates for asset [{}] into Redis: {}", serial_number, err);
            }
        }
    });

    Ok(())
}
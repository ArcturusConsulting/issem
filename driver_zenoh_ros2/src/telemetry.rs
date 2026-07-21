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

            // Whitelist Security Filter: Only allow configured serial numbers
            if !config.target_amr_serials.contains(&serial_number) {
                continue;
            }

            // Get current timestamp for telemetry logging and state freshness
            let current_timestamp = match std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => duration.as_millis() as u64,
                    Err(_) => 0,
                };

            // Update active AMR session heartbeat in Redis for overall system visibility
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg("active_amrs")
                .arg(current_timestamp)
                .arg(&serial_number)
                .query_async(&mut redis_conn)
                .await;

            // ==============================================================================
            // 🏎️ TELEMETRY SERIALIZATION & STATE PERSISTENCE
            // ==============================================================================
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
                continue;
            }

            // ==============================================================================
            // 🎯 GEOMETRIC ARRIVAL HANDSHAKE
            // ==============================================================================
            // Retrieve active target coordinates from Redis to determine if we are at the goal node.
            let goal_key = format!("amr:{}:active_goal", serial_number);
            if let Ok(Some(raw_goal)) = redis::cmd("GET")
                .arg(&goal_key)
                .query_async::<_, Option<String>>(&mut redis_conn)
                .await 
            {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_goal) {
                    if let (Some(target_x), Some(target_y)) = (json["x"].as_f64(), json["y"].as_f64()) {
                        let delta_x = x - target_x;
                        let delta_y = y - target_y;
                        let distance_to_goal = (delta_x * delta_x + delta_y * delta_y).sqrt();

                        // If the robot is within 15 cm of the target coordinates, promote target_node_id to last_node_id
                        if distance_to_goal < 0.15 {
                            let target_node_key = format!("amr:{}:target_node_id", serial_number);
                            let last_node_key = format!("amr:{}:last_node_id", serial_number);

                            if let Ok(Some(reached_node)) = redis::cmd("GET")
                                .arg(&target_node_key)
                                .query_async::<_, Option<String>>(&mut redis_conn)
                                .await
                            {
                                let _: Result<(), _> = redis::cmd("SET")
                                    .arg(&last_node_key)
                                    .arg(&reached_node)
                                    .query_async(&mut redis_conn)
                                    .await;
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(())
}
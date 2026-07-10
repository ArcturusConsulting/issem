//! Externalized state database operations using the asynchronous Redis client framework.

use std::error::Error;
use log::{info, error};
use redis::AsyncCommands;

/// Sets up the default infrastructure bits inside Redis for all configured AMRs.
pub async fn initialize_fleet_registry(
    serials: &[String],
    redis_client: &redis::Client,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut conn = redis_client.get_multiplexed_tokio_connection().await?;
    
    for serial in serials {
        let lifecycle_key = format!("amr:{}:lifecycle", serial);
        
        // Add default state settings atomically if they don't exist yet
        let _: redis::RedisResult<()> = redis::pipe()
            .hset_nx(&lifecycle_key, "paused", "false")
            .hset_nx(&lifecycle_key, "operating_mode", "AUTOMATIC")
            .sadd("amr:fleet:active_serials", serial)
            .query_async(&mut conn)
            .await;
            
        info!("🗄️ [Redis] State boundaries validated for asset: {}", serial);
    }
    Ok(())
}

/// Temporarily caches the high-level corporate target coordinates inside the database tier.
pub async fn cache_active_goal(
    serial: &str,
    x: f64,
    y: f64,
    theta: f64,
    conn: &mut redis::aio::MultiplexedConnection,
) {
    let goal_key = format!("amr:{}:active_goal", serial);
    let payload = serde_json::json!({ "x": x, "y": y, "theta": theta }).to_string();
    
    if let Err(err) = conn.set::<_, _, ()>(&goal_key, payload).await {
        error!("❌ [State Cache] Failed to update goal cache for [{}]: {}", serial, err);
    }
}

/// Pulls the original pre-paused trajectory targets back out of the database tier.
pub async fn retrieve_cached_goal(
    serial: &str,
    conn: &mut redis::aio::MultiplexedConnection,
) -> Option<(f64, f64, f64)> {
    let goal_key = format!("amr:{}:active_goal", serial);
    
    if let Ok(Some(raw_string)) = conn.get::<_, Option<String>>(&goal_key).await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_string) {
            let x = json["x"].as_f64()?;
            let y = json["y"].as_f64()?;
            let theta = json["theta"].as_f64()?;
            return Some((x, y, theta));
        }
    }
    None
}

/// Updates the localized pause state bit inside the asset's database topology.
pub async fn set_pause_state(
    serial: &str,
    is_paused: bool,
    conn: &mut redis::aio::MultiplexedConnection,
) {
    let lifecycle_key = format!("amr:{}:lifecycle", serial);
    let val = if is_paused { "true" } else { "false" };
    
    let _: redis::RedisResult<()> = conn.hset(&lifecycle_key, "paused", val).await;
}
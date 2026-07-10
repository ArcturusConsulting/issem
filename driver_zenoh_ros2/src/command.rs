//! Downlink Command Engine translating internal primitives into native DDS wire format.

use std::error::Error;
use log::{info, warn, error};
use redis::AsyncCommands;
use tokio::sync::mpsc::Receiver;

use crate::{ZenohDriverConfig, SouthboundCommand};
use crate::ros_msg::primitives::{PoseStamped, RosHeader, RosTime, Pose, Point3D, Quaternion};

pub async fn start_command_downlink(
    _config: ZenohDriverConfig,
    zenoh_session: zenoh::Session,
    redis_client: redis::Client,
    mut command_receiver: Receiver<SouthboundCommand>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("📥 [Southbound Command] Initializing command downlink processing module...");

    let mut redis_conn = redis_client.get_multiplexed_tokio_connection().await?;

    tokio::spawn(async move {
        while let Some(command) = command_receiver.recv().await {
            match command {
                SouthboundCommand::NavigateToPose { serial_number, x, y, theta } => {
                    info!("🎯 [Command] Translating navigation target for asset [{}]: ({:.2}, {:.2}, {:.2} rad)", serial_number, x, y, theta);

                    let q_z = (theta / 2.0).sin();
                    let q_w = (theta / 2.0).cos();

                    let nav_goal = PoseStamped {
                        header: RosHeader {
                            stamp: RosTime { sec: 0, nanosec: 0 },
                            frame_id: "map".to_string(),
                        },
                        pose: Pose {
                            position: Point3D { x, y, z: 0.0 },
                            orientation: Quaternion { x: 0.0, y: 0.0, z: q_z, w: q_w },
                        },
                    };

                    let serialized_goal = match nav_goal.to_cdr_bytes() {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("❌ [Command] Failed to serialize PoseStamped for asset [{}]: {}", serial_number, err);
                            continue;
                        }
                    };

                    let zenoh_topic = format!("{}/goal_pose", serial_number);
                    let publisher = match zenoh_session.declare_publisher(&zenoh_topic).await {
                        Ok(pub_handle) => pub_handle,
                        Err(err) => {
                            error!("❌ [Command] Failed to declare Zenoh publisher for topic [{}]: {}", zenoh_topic, err);
                            continue;
                        }
                    };

                    if let Err(err) = publisher.put(serialized_goal).await {
                        error!("❌ [Command] Zenoh buffer write drop on topic [{}]: {:?}", zenoh_topic, err);
                    } else {
                        info!("📤 [Command] High-priority trajectory injected cleanly onto '{}'", zenoh_topic);
                    }
                }

                SouthboundCommand::PreemptAndHalt { serial_number } => {
                    info!("🛑 [Command] Executing zero-distance target preemption for asset [{}]...", serial_number);

                    let redis_hash_key = format!("amr:{}:pose", serial_number);
                    
                    let redis_fields: Result<std::collections::HashMap<String, String>, _> = redis_conn
                        .hgetall(&redis_hash_key)
                        .await;

                    let (current_x, current_y, current_theta) = match redis_fields {
                        Ok(map) if map.contains_key("x") && map.contains_key("y") && map.contains_key("theta") => {
                            let x: f64 = map["x"].parse().unwrap_or(0.0);
                            let y: f64 = map["y"].parse().unwrap_or(0.0);
                            let theta: f64 = map["theta"].parse().unwrap_or(0.0);
                            (x, y, theta)
                        }
                        _ => {
                            warn!("⚠️ [Command] Redis lookup missed for key '{}'. Preempting to origin defaults.", redis_hash_key);
                            (0.0, 0.0, 0.0)
                        }
                    };

                    let q_z = (current_theta / 2.0).sin();
                    let q_w = (current_theta / 2.0).cos();

                    let halt_goal = PoseStamped {
                        header: RosHeader {
                            stamp: RosTime { sec: 0, nanosec: 0 },
                            frame_id: "map".to_string(),
                        },
                        pose: Pose {
                            position: Point3D { x: current_x, y: current_y, z: 0.0 },
                            orientation: Quaternion { x: 0.0, y: 0.0, z: q_z, w: q_w },
                        },
                    };

                    let serialized_halt = match halt_goal.to_cdr_bytes() {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("❌ [Command] Failed to serialize PreemptAndHalt layout for asset [{}]: {}", serial_number, err);
                            continue;
                        }
                    };

                    let zenoh_topic = format!("{}/goal_pose", serial_number);
                    let publisher = match zenoh_session.declare_publisher(&zenoh_topic).await {
                        Ok(pub_handle) => pub_handle,
                        Err(err) => {
                            error!("❌ [Command] Failed to declare Zenoh publisher for topic [{}]: {}", zenoh_topic, err);
                            continue;
                        }
                    };

                    if let Err(err) = publisher.put(serialized_halt).await {
                        error!("❌ [Command] Zenoh buffer write drop during preemption on topic [{}]: {:?}", zenoh_topic, err);
                    } else {
                        info!("📤 [Command] Zero-distance goal successfully forced onto topic '{}'. Active path preempted.", zenoh_topic);
                    }
                }
            }
        }
    });

    Ok(())
}
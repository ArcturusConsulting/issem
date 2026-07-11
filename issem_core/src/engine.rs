//! Processing router maps executing coordination transitions and throttled status publications.

use std::error::Error;
use std::time::Duration;
use log::{info, warn, error};
use rumqttc::AsyncClient;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio::time::interval;
use redis::AsyncCommands;

use adapter_vda5050::NorthboundEvent;
use adapter_vda5050::schema::{Vda5050State, AgvPosition, BatteryState, SafetyState};
use driver_zenoh_ros2::SouthboundCommand;

// Import the East/West channel types cleanly
use adapter_opc_ua::PeripheralRequest;

use crate::MasterSystemConfig;
use crate::state_manager::{cache_active_goal, retrieve_cached_goal, set_pause_state};

/// Starts the transactional broker loop monitoring internal channels and executing route jumps.
pub async fn start_core_orchestrator(
    config: MasterSystemConfig,
    redis_client: redis::Client,
    mqtt_client: AsyncClient,
    mut northbound_receiver: Receiver<NorthboundEvent>,
    southbound_sender: Sender<SouthboundCommand>,
    peripheral_sender: Sender<PeripheralRequest>, // UPDATED: Mounted East/West sender handle
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("🧠 [Core Engine] Transactional compute loops activated with East/West interlocks.");

    let mut redis_conn = redis_client.get_multiplexed_tokio_connection().await?;

    // TASK 1: Dynamic Outbound VDA5050 State Broadcaster (5 Hz)
    let bcast_client = mqtt_client.clone();
    let bcast_config = config.clone();
    let bcast_redis = redis_client.clone();
    
    tokio::spawn(async move {
        let mut report_timer = interval(Duration::from_millis(200));
        let mut state_conn = match bcast_redis.get_multiplexed_tokio_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                error!("❌ [State Loop] Failed to spin up dedicated telemetry Redis pipeline: {}", err);
                return;
            }
        };
        let mut header_counter: u64 = 0;

        loop {
            report_timer.tick().await;
            header_counter += 1;

            for serial in &bcast_config.target_amr_serials {
                let pose_key = format!("amr:{}:pose", serial);
                let lifecycle_key = format!("amr:{}:lifecycle", serial);

                let coords: std::collections::HashMap<String, String> = state_conn.hgetall(&pose_key).await.unwrap_or_default();
                let lifecycle: std::collections::HashMap<String, String> = state_conn.hgetall(&lifecycle_key).await.unwrap_or_default();

                let x: f64 = coords.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f64 = coords.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let theta: f64 = coords.get("theta").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                
                let is_paused = lifecycle.get("paused").map(|v| v == "true").unwrap_or(false);
                let op_mode = lifecycle.get("operating_mode").cloned().unwrap_or_else(|| "AUTOMATIC".to_string());

                let now_epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let state_frame = Vda5050State {
                    header_id: header_counter,
                    timestamp: now_epoch,
                    version: bcast_config.vda5050_protocol_version.clone(),
                    manufacturer: bcast_config.client_manufacturer.clone(),
                    serial_number: serial.clone(),
                    agv_position: AgvPosition {
                        x, y, theta,
                        position_initialized: true,
                        map_id: bcast_config.warehouse_map_id.clone(),
                    },
                    battery_state: BatteryState {
                        battery_charge: 100.0, 
                        battery_voltage: 24.0,
                        charging_state: "DISCHARGING".to_string(),
                    },
                    operating_mode: if is_paused { "MANUAL".to_string() } else { op_mode },
                    safety_state: SafetyState { e_stop: "NONE".to_string(), field_violation: false },
                    errors: vec![],
                    information: vec![],
                };

                if let Ok(serialized_payload) = serde_json::to_string(&state_frame) {
                    let target_state_topic = format!(
                        "vda5050/{}/{}/{}/state",
                        bcast_config.vda5050_protocol_version,
                        bcast_config.client_manufacturer,
                        serial
                    );
                    
                    let _ = bcast_client.publish(
                        &target_state_topic,
                        rumqttc::QoS::AtLeastOnce,
                        false,
                        serialized_payload.as_bytes()
                    ).await;
                }
            }
        }
    });

    // TASK 2: Continuous Multi-Tenant Command Ingestion Loop
    while let Some(event) = northbound_receiver.recv().await {
        match event {
            NorthboundEvent::OrderReceived { manufacturer: _, serial_number, order_id, x, y, theta } => {
                info!("🧠 [Core Engine] Routing path target [{}] validated for asset [{}]", order_id, serial_number);

                cache_active_goal(&serial_number, x, y, theta, &mut redis_conn).await;

                // --- CONTEXT INTERLOCK SIMULATION ---
                // If the coordinate matches a physical gate zone, execute a synchronous PLC handshake first!
                if x > 20.0 { 
                    warn!("🚧 [Interlock Zone] Target X coordinate ({}) requires clearance through automated gate Door_A1!", x);
                    
                    // Allocate an ephemeral oneshot synchronization portal
                    let (tx, rx) = oneshot::channel();
                    
                    let request = PeripheralRequest::ClearHighSpeedDoor {
                        door_id: "Door_A1".to_string(),
                        responder_tx: tx,
                    };

                    // Send request down to the OPC UA crate worker
                    if peripheral_sender.send(request).await.is_ok() {
                        info!("⏳ [Core Engine] Holding Southbound command trace... awaiting PLC verification signal.");
                        
                        // Block safely until the OPC UA client resolves the future
                        match rx.await {
                            Ok(true) => info!("🔓 [Core Engine] PLC handshakes passed. Gate reported open! Releasing AMR."),
                            _ => {
                                error!("❌ [Core Engine] Interlock rejection! PLC reported hardware fault. Aborting route dispatch.");
                                continue; // Skip routing this command to protect physical assets
                            }
                        }
                    }
                }

                // Everything is clear or cleared—dispatch command down to Zenoh
                let cmd = SouthboundCommand::NavigateToPose { serial_number, x, y, theta };
                let _ = southbound_sender.send(cmd).await;
            }

            NorthboundEvent::InstantActionReceived { manufacturer: _, serial_number, action_id, action_type } => {
                info!("🚨 [Core Engine] Intercepted high-priority execution directive [{}] for asset [{}]", action_type, serial_number);

                match action_type.as_str() {
                    "pause" => {
                        set_pause_state(&serial_number, true, &mut redis_conn).await;
                        let _ = southbound_sender.send(SouthboundCommand::PreemptAndHalt { serial_number }).await;
                    }
                    "resume" => {
                        set_pause_state(&serial_number, false, &mut redis_conn).await;
                        
                        if let Some((target_x, target_y, target_theta)) = retrieve_cached_goal(&serial_number, &mut redis_conn).await {
                            let cmd = SouthboundCommand::NavigateToPose {
                                serial_number,
                                x: target_x,
                                y: target_y,
                                theta: target_theta,
                            };
                            let _ = southbound_sender.send(cmd).await;
                        } else {
                            warn!("⚠️ [Core Engine] Resume token dropped for [{}]: No active path trajectory found inside database.", serial_number);
                        }
                    }
                    "cancelOrder" => {
                        set_pause_state(&serial_number, false, &mut redis_conn).await;
                        let _ = southbound_sender.send(SouthboundCommand::PreemptAndHalt { serial_number }).await;
                    }
                    unhandled => {
                        warn!("ℹ [Core Engine] Received unhandled enterprise action primitive token: '{}' (ID: {})", unhandled, action_id);
                    }
                }
            }
        }
    }

    Ok(())
}
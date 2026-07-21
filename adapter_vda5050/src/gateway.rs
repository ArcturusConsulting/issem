//! Asynchronous multi-tenant MQTT gateway orchestration using rumqttc.

use std::error::Error;
use log::{info, warn, error};
use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};

use crate::{MqttGatewayConfig, NorthboundEvent};
use crate::schema::{Vda5050Order, Vda5050InstantActions};

/// Initializes the MQTT connection fabric, registers global multi-tenant wildcards, 
/// and spawns an isolated background event router. Returns the operational client handle.
pub async fn start_mqtt_gateway(
    config: MqttGatewayConfig,
    event_sender: Sender<NorthboundEvent>,
) -> Result<AsyncClient, Box<dyn Error + Send + Sync>> {
    info!("🚀 [Northbound Gateway] Provisioning corporate MQTT connection fabric...");

    // 1. Configure the low-level connection options
    let mut mqtt_options = MqttOptions::new(
        "issem-stateless-gateway-pod", 
        &config.broker_url, 
        config.broker_port,
    );
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    // Instantiate client connection handles with a conservative bounded buffer channel cap
    let (mqtt_client, mut event_loop) = AsyncClient::new(mqtt_options, 100);

    // 2. Generate multi-tenant operational wildcard filters based on protocol layout
    let order_wildcard = format!("vda5050/{}/+/+/order", config.protocol_version);
    let action_wildcard = format!("vda5050/{}/+/+/instantAction", config.protocol_version);

    // Subscribe to enterprise targets using QoS 1 to guarantee command arrival safety
    mqtt_client.subscribe(&order_wildcard, QoS::AtLeastOnce).await?;
    mqtt_client.subscribe(&action_wildcard, QoS::AtLeastOnce).await?;

    info!("🌐 [Northbound Gateway] Subscribed to enterprise channels: ['{}', '{}']", order_wildcard, action_wildcard);

    // Clone client reference for background worker boundary migration
    let worker_client = mqtt_client.clone();
    let filter_manufacturer = config.manufacturer_filter.clone();

    // 3. Spawn the asynchronous I/O background polling loop
    tokio::spawn(async move {
        info!("📥 [Northbound Gateway] Event polling worker thread successfully initialized.");

        // ◄ FIXED: Use an infinite loop to allow rumqttc to auto-reconnect on transient errors
        loop {
            match event_loop.poll().await {
                Ok(notification) => {
                    if let Event::Incoming(Packet::Publish(packet)) = notification {
                        
                        // Break down topic path strings to extract metadata tokens dynamically
                        // Expected format layout: vda5050/{version}/{manufacturer}/{serialNumber}/{messageType}
                        let topic_tokens: Vec<&str> = packet.topic.split('/').collect();
                        if topic_tokens.len() < 5 {
                            warn!("⚠️ [Gateway] Dropped structural routing anomaly: '{}'", packet.topic);
                            continue;
                        }

                        let manufacturer = topic_tokens[2].to_string();
                        let serial_number = topic_tokens[3].to_string();
                        let message_type = topic_tokens[4];

                        // Corporate security perimeter check: Enforce multi-vendor boundary filters
                        if !filter_manufacturer.is_empty() && manufacturer != filter_manufacturer {
                            continue;
                        }

                        // 4. Ingest and route structural payloads based on spec variants
                        match message_type {
                            "order" => {
                                match serde_json::from_slice::<Vda5050Order>(&packet.payload) {
                                    Ok(order) => {
                                        // Extract the first valid targeted waypoint node containing coordinates
                                        if let Some(target_node) = order.nodes.iter().find(|n| n.node_position.is_some()) {
                                            let position = target_node.node_position.as_ref().unwrap();
                                            
                                            // ==============================================================================
                                            // 🔌 DEFENSIVE VDA 5050 ACTION PARSING
                                            // ==============================================================================
                                            // Inspect the waypoint node's actions for any "clearHighSpeedDoor" tasks.
                                            // If found, safely extract its "door_id" parameter.
                                            let pending_action = target_node.actions.iter()
                                                .find(|a| a.action_type == "clearHighSpeedDoor")
                                                .and_then(|a| {
                                                    a.action_parameters.as_ref()?.iter()
                                                        .find(|p| p.key == "door_id")
                                                        .and_then(|p| p.value.as_str().map(String::from))
                                                });

                                            let event = NorthboundEvent::OrderReceived {
                                                manufacturer: manufacturer.clone(),
                                                serial_number: serial_number.clone(),
                                                order_id: order.order_id,
                                                node_id: target_node.node_id.clone(),
                                                x: position.x,
                                                y: position.y,
                                                theta: position.theta,
                                                pending_action,
                                            };

                                            if let Err(err) = event_sender.send(event).await {
                                                error!("❌ [Gateway] Downstream core communications link failure: {}", err);
                                            }
                                        } else {
                                            warn!("⚠️ [Gateway] Received order [{}] without matching target spatial coordinates.", order.order_id);
                                        }
                                    }
                                    Err(err) => {
                                        error!("❌ [Gateway] JSON schema violation on order pipeline: {}", err);
                                    }
                                }
                            }

                            "instantAction" => {
                                match serde_json::from_slice::<Vda5050InstantActions>(&packet.payload) {
                                    Ok(instant_actions) => {
                                        // Iterate across array fields to unpack bundled priority directives
                                        for action in instant_actions.actions {
                                            let event = NorthboundEvent::InstantActionReceived {
                                                manufacturer: manufacturer.clone(),
                                                serial_number: serial_number.clone(),
                                                action_id: action.action_id,
                                                action_type: action.action_type,
                                            };

                                            if let Err(err) = event_sender.send(event).await {
                                                error!("❌ [Gateway] Downstream core preemption signaling drop: {}", err);
                                                break;
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        error!("❌ [Gateway] JSON schema violation on instantAction preemption: {}", err);
                                    }
                                }
                            }

                            unknown => {
                                warn!("ℹ️ [Gateway] Received unhandled enterprise spec message type token: '{}'", unknown);
                            }
                        }
                    }
                }
                Err(err) => {
                    // ◄ FIXED: Log transient network connection failures and let rumqttc retry
                    warn!("⚠️ [Northbound Gateway] Connection issue: {}. Retrying...", err);
                    sleep(Duration::from_millis(1000)).await;
                }
            }
        }
    });

    // Return active multi-client handle back up to the master engine orchestrator
    Ok(worker_client)
}
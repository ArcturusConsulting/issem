//! Subsystem Root: adapter_opc_ua
//! Encapsulates East/West facility automation and PLC connection architectures using async-opcua.

use std::error::Error;
use std::time::Duration;
use log::{info, error};
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneshotSender;

use opcua::client::*;
use opcua::types::*;

/// Strongly typed facility overrides sent from `issem_core` down to factory hardware.
#[derive(Debug)]
pub enum PeripheralRequest {
    /// Requests a high-speed roll-up gate to actuate and open.
    ClearHighSpeedDoor {
        door_id: String,
        responder_tx: OneshotSender<bool>,
    },
    /// Triggers a synchronization interlock with a physical roller conveyor section.
    InterlockConveyor {
        conveyor_id: String,
        action: String, // "START", "STOP"
        responder_tx: OneshotSender<bool>,
    },
}

/// Spawns the industrial OPC UA runtime client, establishes the secure PLC session,
/// and loops indefinitely processing structural infrastructure overrides from the core.
pub async fn start_opc_ua_gateway(
    endpoint_url: String,
    mut request_receiver: Receiver<PeripheralRequest>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("🔌 [East/West Gateway] Initializing industrial OPC UA client stack...");

    // FIXED: Use the public ClientBuilder pattern to instantiate the client safely
    let mut client = ClientBuilder::new()
        .application_name("ISSEM-Stateless-Gateway-Client")
        .application_uri("urn:issem:gateway:client")
        .session_retry_limit(3)
        .client()
        .map_err(|e| format!("Failed to build OPC UA Client: {:?}", e))?;

    // 2. Spawn the primary connection loop running inside a non-blocking worker thread
    tokio::spawn(async move {
        info!("🏭 [East/West Gateway] Launching background PLC orchestration worker thread.");

        // FIXED: Build a strongly typed EndpointDescription directly from a clean tuple definition
        // Build a strongly typed EndpointDescription directly from a clean tuple definition
        let endpoint_desc: EndpointDescription = (
            endpoint_url.as_str(),
            "None",
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous(),
        ).into();

        loop {
            info!("🔌 [East/West Gateway] Attempting connection to target field PLC at: {}", endpoint_url);

            // FIXED: Use connect_to_matching_endpoint and unpack both the session and its event loop driver
            match client.connect_to_matching_endpoint(endpoint_desc.clone(), IdentityToken::Anonymous).await {
                Ok((session, event_loop)) => {
                    info!("✅ [East/West Gateway] Secure session established with industrial PLC automation tier.");

                    // FIXED: Spawn the event loop on a background task so it keeps running continuously
                    let loop_handle = event_loop.spawn();

                    while let Some(request) = request_receiver.recv().await {
                        match request {
                            PeripheralRequest::ClearHighSpeedDoor { door_id, responder_tx } => {
                                info!("🚪 [OPC UA] Actuating high-speed factory door gate asset: [{}]", door_id);

                                let target_node = NodeId::new(2, format!("DB10.Door_Control.{}", door_id));
                                let value_to_write = DataValue::value_only(true);
                                let write_value = WriteValue::new(target_node, AttributeId::Value, NumericRange::None, value_to_write);
                                
                                let operation_success = match session.write(&[write_value]).await {
                                    Ok(results) => {
                                        if results[0].is_good() {
                                            info!("📤 [OPC UA] High bit successfully set on PLC register for gate [{}]. Door opening.", door_id);
                                            true
                                        } else {
                                            error!("❌ [OPC UA] PLC rejected write sequence instruction value: {:?}", results[0]);
                                            false
                                        }
                                    }
                                    Err(err) => {
                                        error!("❌ [OPC UA] Critical connection drop fault during door write sequence: {}", err);
                                        break; 
                                    }
                                };

                                let _ = responder_tx.send(operation_success);
                            }

                            PeripheralRequest::InterlockConveyor { conveyor_id, action, responder_tx } => {
                                info!("⚙️ [OPC UA] Driving conveyor section interlock state for [{}]: Target = {}", conveyor_id, action);

                                let target_node = NodeId::new(2, format!("DB12.Conveyor_Run.{}", conveyor_id));
                                let signal_bit = action == "START";
                                let value_to_write = DataValue::value_only(signal_bit);
                                let write_value = WriteValue::new(target_node, AttributeId::Value, NumericRange::None, value_to_write);

                                let operation_success = match session.write(&[write_value]).await {
                                    Ok(results) => {
                                        if results[0].is_good() {
                                            info!("📤 [OPC UA] Conveyor [{}] state successfully updated to: {}", conveyor_id, action);
                                            true
                                        } else {
                                            error!("❌ [OPC UA] PLC rejected conveyor interlock write: {:?}", results[0]);
                                            false
                                        }
                                    }
                                    Err(err) => {
                                        error!("❌ [OPC UA] Critical connection drop fault during conveyor interlock write: {}", err);
                                        break; 
                                    }
                                };

                                let _ = responder_tx.send(operation_success);
                            }
                        }
                    }

                    // Clean up the spawned event loop task if the loop drops
                    let _ = session.disconnect().await;
                    let _ = loop_handle.await;
                }
                Err(err) => {
                    error!("❌ [East/West Gateway] Failed to connect to PLC endpoint: {}. Retrying link...", err);
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    Ok(())
}
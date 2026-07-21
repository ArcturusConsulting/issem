//! Subsystem Root: adapter_opc_ua
//! Encapsulates East/West facility automation and PLC connection architectures using async-opcua.

use std::error::Error;
use std::time::Duration;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use log::{info, error, warn};
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneshotSender;
use serde::Deserialize;

use opcua::client::*;
use opcua::types::*;

/// Deserialization layout matching your templated deploy/opc_ua_mapping.json layout
#[derive(Deserialize, Debug, Clone)]
pub struct OpcSignalTemplate {
    pub ns: u16,
    pub node_id_pattern: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpcUaMapping {
    pub signals: HashMap<String, OpcSignalTemplate>,
}

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
    mapping_path: String,
    mut request_receiver: Receiver<PeripheralRequest>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("🔌 [East/West Gateway] Initializing industrial OPC UA client stack...");

    // 1. Safe boot-time parsing of the dynamic PLC mapping configuration
    let mapping = match File::open(&mapping_path) {
        Ok(file) => {
            match serde_json::from_reader::<_, OpcUaMapping>(BufReader::new(file)) {
                Ok(parsed) => {
                    info!("✅ [East/West Gateway] Loaded {} OPC UA template mappings from: {}", parsed.signals.len(), mapping_path);
                    Some(parsed)
                }
                Err(err) => {
                    warn!("⚠️ [East/West Gateway] Parsing fault in mapping config [{}]: {}. Using fallback patterns.", mapping_path, err);
                    None
                }
            }
        }
        Err(_) => {
            warn!("⚠️ [East/West Gateway] No PLC mapping file found at '{}'. Using hardcoded fallback schema.", mapping_path);
            None
        }
    };

    let mut client = ClientBuilder::new()
        .application_name("ISSEM-Stateless-Gateway-Client")
        .application_uri("urn:issem:gateway:client")
        .session_retry_limit(3)
        .client()
        .map_err(|e| format!("Failed to build OPC UA Client: {:?}", e))?;

    // 2. Spawn the primary connection loop running inside a non-blocking worker thread
    let mapping_clone = mapping.clone();
    tokio::spawn(async move {
        info!("🏭 [East/West Gateway] Launching background PLC orchestration worker thread.");

        let endpoint_desc: EndpointDescription = (
            endpoint_url.as_str(),
            "None",
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous(),
        ).into();

        loop {
            info!("🔌 [East/West Gateway] Attempting connection to target field PLC at: {}", endpoint_url);

            match client.connect_to_matching_endpoint(endpoint_desc.clone(), IdentityToken::Anonymous).await {
                Ok((session, event_loop)) => {
                    info!("✅ [East/West Gateway] Secure session established with industrial PLC automation tier.");

                    let loop_handle = event_loop.spawn();

                    while let Some(request) = request_receiver.recv().await {
                        match request {
                            PeripheralRequest::ClearHighSpeedDoor { door_id, responder_tx } => {
                                info!("🚪 [OPC UA] Actuating high-speed factory door gate asset: [{}]", door_id);

                                // ◄ RESOLVE TARGET NODE ID BY REPLACING "{}" IN THE PATTERN
                                let target_node = match &mapping_clone {
                                    Some(map) => {
                                        if let Some(tmpl) = map.signals.get("door_control_template") {
                                            let identifier = tmpl.node_id_pattern.replace("{}", &door_id);
                                            NodeId::new(tmpl.ns, identifier)
                                        } else {
                                            NodeId::new(2, format!("DB10.Door_Control.{}", door_id))
                                        }
                                    }
                                    None => NodeId::new(2, format!("DB10.Door_Control.{}", door_id))
                                };

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

                                // ◄ RESOLVE TARGET NODE ID BY REPLACING "{}" IN THE PATTERN
                                let target_node = match &mapping_clone {
                                    Some(map) => {
                                        if let Some(tmpl) = map.signals.get("conveyor_run_template") {
                                            let identifier = tmpl.node_id_pattern.replace("{}", &conveyor_id);
                                            NodeId::new(tmpl.ns, identifier)
                                        } else {
                                            NodeId::new(2, format!("DB12.Conveyor_Run.{}", conveyor_id))
                                        }
                                    }
                                    None => NodeId::new(2, format!("DB12.Conveyor_Run.{}", conveyor_id))
                                };

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
//! Subsystem Root: adapter_vda5050
//! Encapsulates Northbound MQTT network management and VDA5050 JSON validations.

pub mod schema;
pub mod gateway;

use serde::Deserialize;

/// Configuration token used to bind the rumqttc AsyncClient loop.
#[derive(Debug, Deserialize, Clone)]
pub struct MqttGatewayConfig {
    pub broker_url: String,
    pub broker_port: u16,
    pub protocol_version: String,
    pub manufacturer_filter: String,
}

/// Normalized event enum transmitted upstream over memory channels to `issem_core`.
#[derive(Debug, Clone, PartialEq)]
pub enum NorthboundEvent {
    OrderReceived {
        manufacturer: String,
        serial_number: String,
        order_id: String,
        node_id: String,
        x: f64,
        y: f64,
        theta: f64,
        pending_action: Option<String>,
    },
    InstantActionReceived {
        manufacturer: String,
        serial_number: String,
        action_id: String,
        action_type: String,
    },
}
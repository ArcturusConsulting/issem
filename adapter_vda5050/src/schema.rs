//! Scoped VDA5050 v3.0.0 structural primitives required for ISSEM core loops.

use serde::{Deserialize, Serialize};

// ============================================================================
// INBOUND COALESCED STRUCTURES (FMS -> GATEWAY)
// ============================================================================

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050NodePosition {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub theta: f64,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050ActionParameter {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050Action {
    pub action_id: String,
    pub action_type: String,
    pub action_parameters: Option<Vec<Vda5050ActionParameter>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050Node {
    pub node_id: String,
    pub node_position: Option<Vda5050NodePosition>,
    #[serde(default)]
    pub actions: Vec<Vda5050Action>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050Order {
    pub order_id: String,
    pub nodes: Vec<Vda5050Node>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050InstantActions {
    pub header_id: u64,
    pub timestamp: u64,
    pub version: String,
    pub actions: Vec<Vda5050Action>,
}

// ============================================================================
// OUTBOUND TELEMETRY STRUCTURES (GATEWAY -> FMS)
// ============================================================================

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgvPosition {
    pub x: f64,
    pub y: f64,
    pub theta: f64,
    pub position_initialized: bool,
    pub map_id: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryState {
    pub battery_charge: f64,
    pub battery_voltage: f64,
    pub charging_state: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SafetyState {
    pub e_stop: String,
    pub field_violation: bool,
}

/// Strongly typed outbound telemetry status frame replacing the prototype's manual json! blocks.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vda5050State {
    pub header_id: u64,
    pub timestamp: u64,
    pub version: String,
    pub manufacturer: String,
    pub serial_number: String,
    pub agv_position: AgvPosition,
    pub last_node_id: String,
    pub battery_state: BatteryState,
    pub operating_mode: String,
    pub safety_state: SafetyState,
    pub errors: Vec<String>,
    pub information: Vec<String>,
}
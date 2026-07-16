//! Subsystem Root: issem_core
//! Transactional brain driving multi-tenant coordination and state synchronization.

pub mod state_manager;
pub mod engine;

use serde::Deserialize;

/// Integrated master configuration map pulling fields for all sub-crates.
#[derive(Debug, Deserialize, Clone)]
pub struct MasterSystemConfig {
    pub vda5050_protocol_version: String,
    pub client_manufacturer: String,
    pub warehouse_map_id: String,
    pub target_amr_serials: Vec<String>,
    
    pub mqtt_broker_url: String,
    pub mqtt_broker_port: u16,
    
    pub redis_connection_url: String,
    
    pub zenoh_listen_host: String,
    pub zenoh_listen_port: u16,

    pub opc_ua_plc_url: String,
    pub opc_ua_mapping_path: String,
}

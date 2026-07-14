//! Application Binary: ISSEM Centralized Stateless Orchestration Middleware
//! Orchestrates asynchronous pipeline channel scheduling and loops across workspace crates.

use std::error::Error;
use std::fs::File;
use std::io::Read;
use log::{info, error};

use issem_core::MasterSystemConfig;
use issem_core::state_manager::initialize_fleet_registry;
use issem_core::engine::start_core_orchestrator;

use adapter_vda5050::MqttGatewayConfig;
use adapter_vda5050::gateway::start_mqtt_gateway;

use driver_zenoh_ros2::ZenohDriverConfig;
use driver_zenoh_ros2::telemetry::start_telemetry_uplink;
use driver_zenoh_ros2::command::start_command_downlink;

use adapter_opc_ua::start_opc_ua_gateway;

// Declare our clean license helper module
mod license;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Initialize System Logger
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 Initializing ISSEM Framework (Industrial Stateless Server Orchestration Middleware)...");

    // ==============================================================================
    // 🛡️ MODULAR LICENSING GATEWAY
    // ==============================================================================
    let claims = license::validate_startup_license()?;
    license::spawn_background_validator();

    // ==============================================================================
    // ⚙️ SYSTEM CONFIGURATION & INITIALIZATION SEQUENCE
    // ==============================================================================
    let mut config_file = File::open("config.json")
        .map_err(|_| "Catastrophic Startup Fault: Missing required 'config.json' file in working route path!")?;
    let mut config_raw = String::new();
    config_file.read_to_string(&mut config_raw)?;
    
    let mut master_config: MasterSystemConfig = serde_json::from_str(&config_raw)?;

    // Enforce maximum robot count authorized by the cryptographic license allocation
    if master_config.target_amr_serials.len() > claims.max_amr_fleet {
        error!("❌ Licensing Breach: Requested fleet count ({}) exceeds authorized license limit ({}). System halting.", 
            master_config.target_amr_serials.len(), claims.max_amr_fleet);
        return Err("Licensing Boundary Violation".into());
    }

    if let Ok(env_redis) = std::env::var("ISSEM_REDIS_URL") {
        info!("🔄 On-Premise Override: Applying Redis target state storage URL from environment cluster config.");
        master_config.redis_connection_url = env_redis;
    }
    if let Ok(env_plc) = std::env::var("ISSEM_PLC_ENDPOINT") {
        info!("🔄 On-Premise Override: Applying East/West PLC endpoint path from environment network config.");
        master_config.opc_ua_plc_url = env_plc;
    }

    info!("⚙️ Global deployment config processed successfully. Target fleet count: {}", master_config.target_amr_serials.len());

    // 3. Establish External Shared State Tier (Redis)
    info!("🗄️ Connecting to durable shared storage tier at {}...", master_config.redis_connection_url);
    let redis_client = redis::Client::open(master_config.redis_connection_url.clone())?;
    initialize_fleet_registry(&master_config.target_amr_serials, &redis_client).await?;

    // 4. Initialize Multi-Tenant Southbound Zenoh Socket Portals
    let mut zenoh_config = zenoh::Config::default();
    let zenoh_endpoint = format!("tcp/{}:{}", master_config.zenoh_listen_host, master_config.zenoh_listen_port);
    
    zenoh_config.insert_json5("mode", "\"peer\"")?;
    zenoh_config.insert_json5("listen/endpoints", &format!("[\"{}\"]", zenoh_endpoint))?;
    zenoh_config.insert_json5("scouting/multicast/enabled", "false")?;
    
    info!("🔌 Binding high-speed Zenoh network peer session on {}...", zenoh_endpoint);
    let zenoh_session = zenoh::open(zenoh_config).await?;

    // 5. Allocate Inter-Crate Bounded Communication Pipelines (MPSC)
    let (northbound_tx, northbound_rx) = tokio::sync::mpsc::channel(100);
    let (southbound_tx, southbound_rx) = tokio::sync::mpsc::channel(100);
    let (peripheral_tx, peripheral_rx) = tokio::sync::mpsc::channel(100);

    // 6. Bootstrap Crate Subsystems
    let driver_config = ZenohDriverConfig {
        listen_host: master_config.zenoh_listen_host.clone(),
        listen_port: master_config.zenoh_listen_port,
        target_amr_serials: master_config.target_amr_serials.clone(),
    };
    start_telemetry_uplink(driver_config.clone(), &zenoh_session, redis_client.clone()).await?;
    start_command_downlink(driver_config, zenoh_session, redis_client.clone(), southbound_rx).await?;
    info!("🤖 Southbound driver background actors successfully mounted.");

    start_opc_ua_gateway(master_config.opc_ua_plc_url.clone(), peripheral_rx).await?;
    info!("🏭 East/West OPC UA peripheral background worker active.");

    let gateway_config = MqttGatewayConfig {
        broker_url: master_config.mqtt_broker_url.clone(),
        broker_port: master_config.mqtt_broker_port,
        protocol_version: master_config.vda5050_protocol_version.clone(),
        manufacturer_filter: master_config.client_manufacturer.clone(),
    };
    let mqtt_client = start_mqtt_gateway(gateway_config, northbound_tx).await?;
    info!("🌐 Northbound gateway network event loops active.");

    info!("🧠 Transferring thread control blocks to transactional orchestration core...");
    
    if let Err(err) = start_core_orchestrator(
        master_config,
        redis_client,
        mqtt_client,
        northbound_rx,
        southbound_tx,
        peripheral_tx,
    ).await {
        error!("❌ Catastrophic runtime failure inside orchestrator core loops: {}", err);
        return Err(err);
    }

    Ok(())
}
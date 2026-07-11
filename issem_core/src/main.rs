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

// Mount East/West client dependencies
use adapter_opc_ua::start_opc_ua_gateway;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Initialize System Logger
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    info!("🚀 Initializing ISSEM Framework (Industrial Stateless Server Orchestration Middleware)...");

    // 2. Ingest Integrated Configuration Map
    let mut config_file = File::open("config.json")
        .map_err(|_| "Catastrophic Startup Fault: Missing required 'config.json' file in working route path!")?;
    let mut config_raw = String::new();
    config_file.read_to_string(&mut config_raw)?;
    
    let master_config: MasterSystemConfig = serde_json::from_str(&config_raw)?;
    info!("⚙️ Global deployment config parsed successfully. Target fleet count: {}", master_config.target_amr_serials.len());

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
    
    // Allocate the East/West pipeline queue to feed instructions down to PLCs
    let (peripheral_tx, peripheral_rx) = tokio::sync::mpsc::channel(100);

    // 6. Bootstrap Crate Subsystems
    
    // A. Start Southbound Robotics Driver Workers
    let driver_config = ZenohDriverConfig {
        listen_host: master_config.zenoh_listen_host.clone(),
        listen_port: master_config.zenoh_listen_port,
        target_amr_serials: master_config.target_amr_serials.clone(),
    };
    start_telemetry_uplink(driver_config.clone(), &zenoh_session, redis_client.clone()).await?;
    start_command_downlink(driver_config, zenoh_session, redis_client.clone(), southbound_rx).await?;
    info!("🤖 Southbound driver background actors successfully mounted.");

    // B. Start East/West Industrial Peripheral Gateway Client
    start_opc_ua_gateway(master_config.opc_ua_plc_url.clone(), peripheral_rx).await?;
    info!("🏭 East/West OPC UA peripheral background worker active.");

    // C. Start Northbound Enterprise MQTT Gateway Client Runtime
    let gateway_config = MqttGatewayConfig {
        broker_url: master_config.mqtt_broker_url.clone(),
        broker_port: master_config.mqtt_broker_port,
        protocol_version: master_config.vda5050_protocol_version.clone(),
        manufacturer_filter: master_config.client_manufacturer.clone(),
    };
    let mqtt_client = start_mqtt_gateway(gateway_config, northbound_tx).await?;
    info!("🌐 Northbound gateway network event loops active.");

// D. Execute Core Business Engine Loop
    info!("🧠 Transferring thread control blocks to transactional orchestration core...");
    
    if let Err(err) = start_core_orchestrator(
        master_config,
        redis_client,
        mqtt_client,
        northbound_rx,
        southbound_tx,
        peripheral_tx, // FIXED: Passed the live channel sender straight to the compute core
    ).await {
        error!("❌ Catastrophic runtime failure inside orchestrator core loops: {}", err);
        return Err(err);
    }

    Ok(())
}
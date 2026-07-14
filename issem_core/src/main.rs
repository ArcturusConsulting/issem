//! Application Binary: ISSEM Centralized Stateless Orchestration Middleware
//! Orchestrates asynchronous pipeline channel scheduling and loops across workspace crates.

use std::error::Error;
use std::fs::File;
use std::io::Read;
use log::{info, error, warn};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::Deserialize;

use issem_core::MasterSystemConfig;
use issem_core::state_manager::initialize_fleet_registry;
use issem_core::engine::start_core_orchestrator;

use adapter_vda5050::MqttGatewayConfig;
use adapter_vda5050::gateway::start_mqtt_gateway;

use driver_zenoh_ros2::ZenohDriverConfig;
use driver_zenoh_ros2::telemetry::start_telemetry_uplink;
use driver_zenoh_ros2::command::start_command_downlink;

use adapter_opc_ua::start_opc_ua_gateway;

/// The structural layout of your commercial software license payload
#[derive(Debug, Deserialize)]
struct LicenseClaims {
    iss: String,          // Issuer (e.g., "Furukawa Robotics Consulting")
    sub: String,          // Customer Name
    exp: i64,             // Expiration timestamp
    max_amr_fleet: usize, // Hard ceiling for authorized robot counts
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Initialize System Logger
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 Initializing ISSEM Framework (Industrial Stateless Server Orchestration Middleware)...");

    // ==============================================================================
    // NEW: ENTERPRISE LICENSING GATEWAY BLOCK
    // ==============================================================================
    info!("🔑 Loading cryptographic public key from volume mount...");
    // 1. Map the IO read error to a String
    let public_key_pem = std::fs::read("/app/pki/public.pem")
        .map_err(|e| format!("Failed to read public key file: {}", e))?; 

    info!("🔑 Validating production runtime activation license token...");
    
    // 2. Map file access to a String
    let mut license_file = File::open("license.jwt").map_err(|_| {
        "Licensing Fault: Cryptographic token validation failed! Missing required 'license.jwt' registration block.".to_string()
    })?;
    
    let mut jwt_token = String::new();
    // 3. Map reading of file to a String
    license_file.read_to_string(&mut jwt_token)
        .map_err(|e| format!("Failed to read license file payload: {}", e))?;
    let jwt_token = jwt_token.trim();

    // Decode and verify the cryptographic signature using Ed25519
    // 4. Pass an explicit slice reference &[u8] and map the error to a String
    let decoding_key = DecodingKey::from_ed_pem(&public_key_pem[..])
        .map_err(|e| format!("Cryptographic validation fault: Invalid Public Key format: {}", e))?;
        
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true; // Enforces the token expiration date check automatically

    // 5. Map decoding errors to a String
    let token_data = decode::<LicenseClaims>(&jwt_token, &decoding_key, &validation)
        .map_err(|err| {
            warn!("❌ Cryptographic Signature Check Failed: License altered or signature invalid.");
            format!("JWT Signature validation failed: {}", err)
        })?;

    let claims = token_data.claims;
    info!("✅ License verified successfully for client asset: [{}]. Issued by: {}. Expiry timestamp: {}", claims.sub, claims.iss, claims.exp);
    // 2. Ingest Integrated Configuration Map
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
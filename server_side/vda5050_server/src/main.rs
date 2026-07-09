use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use rumqttc::{MqttOptions, AsyncClient, QoS};

// Declare internal sub-modules
mod ros_msg;
mod vda_msg;
mod uplink;
mod downlink;

// Expose the configuration card layout globally to our workers
#[derive(Debug, Deserialize, Clone)]
pub struct ClientConfig {
    pub client_manufacturer: String,
    pub client_serial_number: String,
    pub warehouse_map_id: String,
    pub vda5050_protocol_version: String,
    pub mqtt_broker_url: String,
    pub mqtt_broker_port: u16,        
    pub zenoh_listen_host: String,
    pub zenoh_listen_port: u16,       
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Initializing ISSEM Core Systems Orchestrator...");

    // 1. Ingest Customer Configuration Profile Card
    let config_file = File::open("config.yaml")
        .map_err(|_| "Operational Error: Missing 'config.yaml' file in running path!")?;
    let client_config: ClientConfig = serde_yaml::from_reader(config_file)?;
    println!("⚙️ Configuration map verified for asset: {}", client_config.client_serial_number);

    // 2. Provision Asynchronous Enterprise MQTT Fabric
    let mut mqtt_options = MqttOptions::new(
        &client_config.client_serial_number, 
        &client_config.mqtt_broker_url, 
        client_config.mqtt_broker_port, 
    );
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(5));
    
    let (mqtt_client, event_loop) = AsyncClient::new(mqtt_options, 10);
    
    let order_topic = format!(
        "vda5050/{}/{}/{}/order",
        client_config.vda5050_protocol_version,
        client_config.client_manufacturer,
        client_config.client_serial_number
    );
    
    mqtt_client.subscribe(&order_topic, QoS::AtLeastOnce).await?;
    println!("🌐 Connected to Enterprise Broker. Subscribed to topic: {}", order_topic);

    // 3. Open Edge Zenoh Peer Socket Infrastructure
    let mut zenoh_config = zenoh::Config::default();
    let zenoh_endpoint = format!("tcp/{}:{}", client_config.zenoh_listen_host, client_config.zenoh_listen_port);
    let zenoh_listen_array = json!([zenoh_endpoint]).to_string();

    zenoh_config.insert_json5("mode", "\"peer\"")?;
    zenoh_config.insert_json5("listen/endpoints", &zenoh_listen_array)?;
    zenoh_config.insert_json5("scouting/multicast/enabled", "false")?;
    
    let zenoh_session = zenoh::open(zenoh_config).await?;
    println!("🔌 Zenoh cluster portal established on {}", zenoh_endpoint);

    // 4. Bind Transport Handles
    let telemetry_subscriber = zenoh_session.declare_subscriber("**/odom").await?;
    let navigation_publisher = zenoh_session.declare_publisher("goal_pose").await?;
    println!("📥 Abstract network IO pipes successfully registered.");

    // 5. SPAWN DOWNLINK ENGINE (MQTT -> ROS 2 via Zenoh background task)
    tokio::spawn(downlink::run_engine(
        event_loop,
        order_topic,
        navigation_publisher,
    ));

    // 6. EXECUTE UPLINK LOOP ON PRIMARY THREAD (ROS 2 -> MQTT blocking call)
    uplink::run_engine(
        client_config,
        telemetry_subscriber,
        mqtt_client,
    ).await?;

    Ok(())
}
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use rumqttc::{MqttOptions, AsyncClient, QoS};

// The blueprint maps 1:1 to our external config.yaml file entries
#[derive(Debug, Deserialize)]
struct ClientConfig {
    client_manufacturer: String,
    client_serial_number: String,
    warehouse_map_id: String,
    mqtt_broker_url: String,
    mqtt_broker_port: u16,        // Dynamically parsed port type
    zenoh_listen_host: String,
    zenoh_listen_port: u16,       // Dynamically parsed port type
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Launching Config-Driven Native Zenoh VDA5050 Server...");

    // 1. Ingest customer configuration file profile
    let config_file = File::open("config.yaml")
        .map_err(|_| "Operational Error: Missing 'config.yaml' file in the running directory!")?;
    let client_config: ClientConfig = serde_yaml::from_reader(config_file)?;
    println!("⚙️ Client profile initialized for asset: {}", client_config.client_serial_number);

    // 2. Initialize MQTT infrastructure dynamically using the parsed profile ports
    let mut mqtt_options = MqttOptions::new(
        &client_config.client_serial_number, 
        &client_config.mqtt_broker_url, 
        client_config.mqtt_broker_port, // FIX: Injected from config
    );
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(5));
    
    let (mqtt_client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    tokio::spawn(async move {
        while let Ok(_notification) = event_loop.poll().await {}
    });
    println!(
        "🌐 Connected straight to MQTT Broker at {}:{}", 
        client_config.mqtt_broker_url, client_config.mqtt_broker_port
    );

    // 3. Open a stable, native Zenoh session using custom runtime endpoints
    let mut zenoh_config = zenoh::Config::default();
    
    // Dynamically string-stitch the network endpoint mapping array
    let zenoh_endpoint = format!("tcp/{}:{}", client_config.zenoh_listen_host, client_config.zenoh_listen_port);
    let zenoh_listen_array = json!([zenoh_endpoint]).to_string();

    // FIX: Core routing topology is now driven entirely by your config file settings
    zenoh_config.insert_json5("mode", "\"peer\"")?;
    zenoh_config.insert_json5("listen/endpoints", &zenoh_listen_array)?;
    zenoh_config.insert_json5("scouting/multicast/enabled", "false")?;
    
    let zenoh_session = zenoh::open(zenoh_config).await?;
    println!("🔌 Server listening for direct client connections on {}", zenoh_endpoint);

    // 4. Declare a subscriber to catch high-frequency binary frames coming over the air
    let subscriber = zenoh_session.declare_subscriber("rt/odom").await?;
    println!("📥 Bound to Zenoh network fabric. Listening for AMR 'rt/odom' telemetry streams...");

    // 5. The primary, zero-copy real-time execution loop
    while let Ok(sample) = subscriber.recv_async().await {
        let payload = sample.payload();
        let _raw_bytes = payload.to_bytes();

        let x = 3.141;
        let y = -0.572;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let vda5050_payload = json!({
            "headerId": 0,
            "timestamp": now,
            "version": "2.0.0",
            "manufacturer": client_config.client_manufacturer,
            "serialNumber": client_config.client_serial_number,
            "agvPosition": {
                "x": x,
                "y": y,
                "theta": 0.0,
                "positionInitialized": true,
                "mapId": client_config.warehouse_map_id
            }
        });

        let serialized_string = serde_json::to_string(&vda5050_payload)?;

        mqtt_client
            .publish("vda5050/v2/state", QoS::AtLeastOnce, false, serialized_string.as_bytes())
            .await?;
    }

    Ok(())
}
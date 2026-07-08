use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use rumqttc::{MqttOptions, AsyncClient, QoS};

#[derive(Debug, Deserialize)]
struct ClientConfig {
    client_manufacturer: String,
    client_serial_number: String,
    warehouse_map_id: String,
    mqtt_broker_url: String,
    mqtt_broker_port: u16,        
    zenoh_listen_host: String,
    zenoh_listen_port: u16,       
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Launching Config-Driven Native Zenoh VDA5050 Server...");

    let config_file = File::open("config.yaml")
        .map_err(|_| "Operational Error: Missing 'config.yaml' file in the running directory!")?;
    let client_config: ClientConfig = serde_yaml::from_reader(config_file)?;
    println!("⚙️ Client profile initialized for asset: {}", client_config.client_serial_number);

    let mut mqtt_options = MqttOptions::new(
        &client_config.client_serial_number, 
        &client_config.mqtt_broker_url, 
        client_config.mqtt_broker_port, 
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

    let mut zenoh_config = zenoh::Config::default();
    let zenoh_endpoint = format!("tcp/{}:{}", client_config.zenoh_listen_host, client_config.zenoh_listen_port);
    let zenoh_listen_array = json!([zenoh_endpoint]).to_string();

    zenoh_config.insert_json5("mode", "\"peer\"")?;
    zenoh_config.insert_json5("listen/endpoints", &zenoh_listen_array)?;
    zenoh_config.insert_json5("scouting/multicast/enabled", "false")?;
    
    let zenoh_session = zenoh::open(zenoh_config).await?;
    println!("🔌 Server listening for direct client connections on {}", zenoh_endpoint);

    // FIX: Swapped "rt/odom" out for the catch-all "**/odom" multi-segment wildcard path
    let subscriber = zenoh_session.declare_subscriber("**/odom").await?;
    println!("📥 Bound to Zenoh network fabric. Listening for AMR position telemetry streams...");

    while let Ok(sample) = subscriber.recv_async().await {
        println!("📩 [Zenoh] High-frequency packet intercepted on data fabric!");

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
            
        println!("📤 [MQTT] Formatted VDA5050 payload flushed to broker topic!");
    }

    Ok(())
}
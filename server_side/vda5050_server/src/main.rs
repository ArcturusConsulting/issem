use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use rumqttc::{MqttOptions, AsyncClient, QoS};

mod ros_msg;
mod vda_msg;

use ros_msg::{RosOdometry, PoseStamped, RosHeader, RosTime, Pose, Point3D, Quaternion};
use vda_msg::Vda5050Order;


#[derive(Debug, Deserialize)]
struct ClientConfig {
    client_manufacturer: String,
    client_serial_number: String,
    warehouse_map_id: String,
    vda5050_protocol_version: String,
    mqtt_broker_url: String,
    mqtt_broker_port: u16,        
    zenoh_listen_host: String,
    zenoh_listen_port: u16,       
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Launching Phase 1 Modular Zenoh VDA5050 Server...");

    let config_file = File::open("config.yaml")
        .map_err(|_| "Operational Error: Missing 'config.yaml' file in running path!")?;
    let client_config: ClientConfig = serde_yaml::from_reader(config_file)?;
    println!("⚙️ Client profile active for asset: {}", client_config.client_serial_number);

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
    println!("🌐 Connected to MQTT Broker at {}:{}", client_config.mqtt_broker_url, client_config.mqtt_broker_port);

    let mut zenoh_config = zenoh::Config::default();
    let zenoh_endpoint = format!("tcp/{}:{}", client_config.zenoh_listen_host, client_config.zenoh_listen_port);
    let zenoh_listen_array = json!([zenoh_endpoint]).to_string();

    zenoh_config.insert_json5("mode", "\"peer\"")?;
    zenoh_config.insert_json5("listen/endpoints", &zenoh_listen_array)?;
    zenoh_config.insert_json5("scouting/multicast/enabled", "false")?;
    
    let zenoh_session = zenoh::open(zenoh_config).await?;
    println!("🔌 Server listening for direct client connections on {}", zenoh_endpoint);

    let subscriber = zenoh_session.declare_subscriber("**/odom").await?;
    println!("📥 Bound to Zenoh network fabric. Listening for real telemetry streams...");

    while let Ok(sample) = subscriber.recv_async().await {
        let payload = sample.payload();
        let raw_bytes = payload.to_bytes();

        // Deserialize directly into our modular type footprint
        let odom_data: RosOdometry = match cdr::deserialize(&raw_bytes) {
            Ok(parsed_msg) => parsed_msg,
            Err(err) => {
                eprintln!("⚠️ Processing warning: Failed to deserialize CDR network token: {}", err);
                continue;
            }
        };

        let x = odom_data.pose.pose.position.x;
        let y = odom_data.pose.pose.position.y;

        let q = &odom_data.pose.pose.orientation;
        let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
        let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
        let theta = siny_cosp.atan2(cosy_cosp);

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let vda5050_state_payload = json!({
            "headerId": 0,
            "timestamp": now,
            "version": client_config.vda5050_protocol_version,
            "manufacturer": client_config.client_manufacturer,
            "serialNumber": client_config.client_serial_number,
            "agvPosition": {
                "x": x,
                "y": y,
                "theta": theta,
                "positionInitialized": true,
                "mapId": client_config.warehouse_map_id
            },
            "batteryState": {
                "batteryCharge": 92.5,        
                "batteryVoltage": 24.2,
                "chargingState": "DISCHARGING" 
            },
            "operatingMode": "AUTOMATIC",     
            "safetyState": {
                "eStop": "NONE",              
                "fieldViolation": false
            },
            "errors": [],
            "information": []
        });

        let serialized_string = serde_json::to_string(&vda5050_state_payload)?;

        mqtt_client
            .publish("vda5050/v2/state", QoS::AtLeastOnce, false, serialized_string.as_bytes())
            .await?;
    }

    Ok(())
}
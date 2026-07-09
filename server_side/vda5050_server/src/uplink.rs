use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use rumqttc::{AsyncClient, QoS};
use zenoh::pubsub::Subscriber;
use zenoh::handlers::fifo::FifoChannelHandler; // Concrete asynchronous queue handler
use zenoh::sample::Sample;
use crate::ClientConfig;
use crate::ros_msg::RosOdometry;

pub async fn run_engine(
    client_config: ClientConfig,
    telemetry_subscriber: Subscriber<FifoChannelHandler<Sample>>, // Fully specified Zenoh 1.x subscriber signature
    mqtt_client: AsyncClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📤 [Uplink] Telemetry Pipeline Engine active and bound to DDS...");

    while let Ok(sample) = telemetry_subscriber.recv_async().await {
        let payload = sample.payload();
        let raw_bytes = payload.to_bytes();

        // Decode incoming raw binary array blocks using our CDR layout card
        let odom_data: RosOdometry = match cdr::deserialize(&raw_bytes) {
            Ok(parsed_msg) => parsed_msg,
            Err(_err) => continue, // Keeps stdout clean during simulator initialization phases
        };

        let x = odom_data.pose.pose.position.x;
        let y = odom_data.pose.pose.position.y;

        // Perform spatial rotation matrix extraction (Quaternion to planar Yaw radians)
        let q = &odom_data.pose.pose.orientation;
        let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
        let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
        let theta = siny_cosp.atan2(cosy_cosp);

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Package stats directly into a fully saturated VDA5050 structural payload
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

        if let Ok(serialized_string) = serde_json::to_string(&vda5050_state_payload) {
            let _ = mqtt_client
                .publish("vda5050/v2/state", QoS::AtLeastOnce, false, serialized_string.as_bytes())
                .await;
        }
    }

    Ok(())
}
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc, Mutex};
use rumqttc::{AsyncClient, QoS};
use zenoh::pubsub::Subscriber;
use zenoh::handlers::fifo::FifoChannelHandler; 
use zenoh::sample::Sample;
use crate::ClientConfig;
use crate::ros_msg::{RosHeader, Pose}; 
use tokio::time::interval;

// Mirror the exact memory footprint of a geometry_msgs/msg/PoseWithCovarianceStamped
#[derive(serde::Deserialize, Clone, Debug)]
pub struct PoseWithCovariance {
    pub pose: Pose,
    // FIXED: Swapped [f64; 36] for [[f64; 6]; 6] to bypass Serde's macro limits
    pub covariance: [[f64; 6]; 6], 
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct PoseWithCovarianceStamped {
    pub header: RosHeader,
    pub pose: PoseWithCovariance,
}

pub async fn run_engine(
    client_config: ClientConfig,
    telemetry_subscriber: Subscriber<FifoChannelHandler<Sample>>, 
    mqtt_client: AsyncClient,
    shared_pose: Arc<Mutex<Pose>>, 
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📤 [Uplink] Telemetry Pipeline Engine active and bound to DDS...");

    // TASK 1: High-speed background cache worker parsing AMCL coordinates
    let shared_pose_worker = shared_pose.clone();
    tokio::spawn(async move {
        while let Ok(sample) = telemetry_subscriber.recv_async().await {
            let payload = sample.payload();
            let raw_bytes = payload.to_bytes();

            // Deserialize using our covariance layout layout card
            if let Ok(amcl_data) = cdr::deserialize::<PoseWithCovarianceStamped>(&raw_bytes) {
                let mut pose_cache = shared_pose_worker.lock().unwrap();
                *pose_cache = amcl_data.pose.pose.clone(); // Storing accurate map coordinates
            }
        }
    });

    // TASK 2: Throttled MQTT Enterprise Publisher (5 Hz)
    let mut report_timer = interval(Duration::from_millis(200)); 
    
    loop {
        report_timer.tick().await;

        let current_pose = {
            let pose_guard = shared_pose.lock().unwrap();
            pose_guard.clone()
        };

        let x = current_pose.position.x;
        let y = current_pose.position.y;

        let q = &current_pose.orientation;
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
            "batteryState": { "batteryCharge": 92.5, "batteryVoltage": 24.2, "chargingState": "DISCHARGING" },
            "operatingMode": "AUTOMATIC",     
            "safetyState": { "eStop": "NONE", "fieldViolation": false },
            "errors": [],
            "information": []
        });

        if let Ok(serialized_string) = serde_json::to_string(&vda5050_state_payload) {
            let _ = mqtt_client
                .publish("vda5050/v2/state", QoS::AtLeastOnce, false, serialized_string.as_bytes())
                .await;
        }
    }
}
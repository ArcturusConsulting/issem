use rumqttc::{Event, Packet, EventLoop};
use zenoh::pubsub::Publisher;
use crate::ros_msg::{PoseStamped, RosHeader, RosTime, Pose, Point3D, Quaternion};
use crate::vda_msg::Vda5050Order;

pub async fn run_engine(
    mut event_loop: EventLoop,
    order_topic_filter: String,
    navigation_publisher: Publisher<'static>,
) {
    println!("📥 [Downlink] Command Translation Engine active and polling...");

    while let Ok(notification) = event_loop.poll().await {
        if let Event::Incoming(Packet::Publish(packet)) = notification {
            if packet.topic == order_topic_filter {
                println!("📥 [MQTT] New corporate fleet route packet received!");

                // Conduct semantic validation schema checks
                let order: Vda5050Order = match serde_json::from_slice(&packet.payload) {
                    Ok(valid_order) => valid_order,
                    Err(err) => {
                        eprintln!("⚠️ [Downlink] Payload structural violation. Dropping packet: {}", err);
                        continue;
                    }
                };

                // Look for executable coordinate paths within the order profile
                if let Some(target_node) = order.nodes.iter().find(|n| n.node_position.is_some()) {
                    let position = target_node.node_position.as_ref().unwrap();
                    println!("🎯 [Downlink] Extracted target waypoint target: ({}, {})", position.x, position.y);

                    // Execute mathematical conversions from planar angles to 3D quaternions
                    let theta = position.theta;
                    let q_z = (theta / 2.0).sin();
                    let q_w = (theta / 2.0).cos();

                    // Map fields straight into our modular ROS 2 blueprint
                    let nav_goal = PoseStamped {
                        header: RosHeader {
                            stamp: RosTime { sec: 0, nanosec: 0 },
                            frame_id: "map".to_string(),
                        },
                        pose: Pose {
                            position: Point3D { x: position.x, y: position.y, z: 0.0 },
                            orientation: Quaternion { x: 0.0, y: 0.0, z: q_z, w: q_w },
                        },
                    };

                    // FIXED: Used explicit turbofish type descriptors to guarantee Little-Endian wire formatting
                    let serialized_goal = match cdr::serialize::<_, _, cdr::CdrLe>(&nav_goal, cdr::Infinite) {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            eprintln!("⚠️ [Downlink] CDR Transformation failure: {}", err);
                            continue;
                        }
                    };

                    // Put the raw data directly down the active Zenoh lane
                    if let Err(err) = navigation_publisher.put(serialized_goal).await {
                        eprintln!("⚠️ [Downlink] Network transport dropped token: {:?}", err);
                    } else {
                        println!("📤 [Zenoh] High-priority 'goal_pose' token successfully routed to AMR!");
                    }
                } else {
                    println!("ℹ️ [Downlink] Dispatched order contained no active positioning coordinates.");
                }
            }
        }
    }
}
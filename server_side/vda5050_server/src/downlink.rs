use rumqttc::{Event, Packet, EventLoop};
use std::sync::Arc;
use std::sync::Mutex;
use crate::ros_msg::{PoseStamped, RosHeader, RosTime, Pose, Point3D, Quaternion};
use crate::vda_msg::{Vda5050Order, Vda5050InstantActions};

pub async fn run_engine(
    mut event_loop: EventLoop,
    order_topic_filter: String,
    action_topic_filter: String,
    zenoh_session: zenoh::Session, 
    shared_pose: Arc<Mutex<Pose>>, 
) {
    println!("📥 [Downlink] Command Translation Engine active and polling incoming streams...");
    
    let navigation_publisher = zenoh_session.declare_publisher("goal_pose").await.unwrap();
    let cached_goal: Arc<Mutex<Option<PoseStamped>>> = Arc::new(Mutex::new(None));

    while let Ok(notification) = event_loop.poll().await {
        if let Event::Incoming(Packet::Publish(packet)) = notification {
            
            if packet.topic == order_topic_filter {
                println!("📥 [MQTT] New corporate fleet route packet received!");
                let order: Vda5050Order = match serde_json::from_slice(&packet.payload) {
                    Ok(valid_order) => valid_order,
                    Err(err) => {
                        eprintln!("⚠️ [Downlink] Payload structural violation: {}", err);
                        continue;
                    }
                };

                if let Some(target_node) = order.nodes.iter().find(|n| n.node_position.is_some()) {
                    let position = target_node.node_position.as_ref().unwrap();
                    println!("🎯 [Downlink] Extracted target waypoint target: ({}, {})", position.x, position.y);

                    let theta = position.theta;
                    let q_z = (theta / 2.0).sin();
                    let q_w = (theta / 2.0).cos();

                    let nav_goal = PoseStamped {
                        header: RosHeader { stamp: RosTime { sec: 0, nanosec: 0 }, frame_id: "map".to_string() },
                        pose: Pose {
                            position: Point3D { x: position.x, y: position.y, z: 0.0 },
                            orientation: Quaternion { x: 0.0, y: 0.0, z: q_z, w: q_w },
                        },
                    };

                    {
                        let mut cache = cached_goal.lock().unwrap();
                        *cache = Some(nav_goal.clone());
                    }

                    let serialized_goal = cdr::serialize::<_, _, cdr::CdrLe>(&nav_goal, cdr::Infinite).unwrap();
                    let _ = navigation_publisher.put(serialized_goal).await;
                    println!("📤 [Zenoh] High-priority 'goal_pose' token successfully routed to AMR.");
                }
            }
            
            else if packet.topic == action_topic_filter {
                println!("🚨 [MQTT] High-Priority Instant Action block intercepted!");
                let instant_actions: Vda5050InstantActions = match serde_json::from_slice(&packet.payload) {
                    Ok(valid_actions) => valid_actions,
                    Err(err) => {
                        eprintln!("⚠️ [Downlink] Instant Action schema violation: {}", err);
                        continue;
                    }
                };

                for action in instant_actions.actions {
                    match action.action_type.as_str() {
                        "pause" => {
                            println!("🛑 [Instant Action] Executing PAUSE via Zero-Distance Goal Preemption...");
                            
                            // Collect the absolute latest map frame coordinate frame
                            let current_map_position = {
                                let pose_guard = shared_pose.lock().unwrap();
                                pose_guard.clone()
                            };
                            
                            let halt_goal = PoseStamped {
                                header: RosHeader { stamp: RosTime { sec: 0, nanosec: 0 }, frame_id: "map".to_string() },
                                pose: current_map_position,
                            };
                            
                            let serialized_halt = cdr::serialize::<_, _, cdr::CdrLe>(&halt_goal, cdr::Infinite).unwrap();
                            let _ = navigation_publisher.put(serialized_halt).await;
                            println!("📤 [Zenoh] Zero-distance goal injected. Navigation track preempted successfully.");
                        },
                        "resume" => {
                            println!("▶️ [Instant Action] Executing RESUME. Restoring cached route targets...");
                            let target_to_restore = {
                                let cache = cached_goal.lock().unwrap();
                                cache.clone()
                            };

                            if let Some(goal) = target_to_restore {
                                let serialized_goal = cdr::serialize::<_, _, cdr::CdrLe>(&goal, cdr::Infinite).unwrap();
                                let _ = navigation_publisher.put(serialized_goal).await;
                                println!("📤 [Zenoh] Cached route frame re-injected. Nav2 cluster resuming autonomous tracking.");
                            }
                        },
                        "cancelOrder" => {
                            println!("🧹 [Instant Action] Executing order cancellation protocols.");
                            {
                                let mut cache = cached_goal.lock().unwrap();
                                *cache = None;
                            }
                            let current_map_position = { let p = shared_pose.lock().unwrap(); p.clone() };
                            let halt_goal = PoseStamped {
                                header: RosHeader { stamp: RosTime { sec: 0, nanosec: 0 }, frame_id: "map".to_string() },
                                pose: current_map_position,
                            };
                            let serialized_halt = cdr::serialize::<_, _, cdr::CdrLe>(&halt_goal, cdr::Infinite).unwrap();
                            let _ = navigation_publisher.put(serialized_halt).await;
                        },
                        other => println!("ℹ️ [Downlink] Received unhandled enterprise action token: '{}'", other),
                    }
                }
            }
        }
    }
}
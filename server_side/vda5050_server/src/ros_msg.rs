use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RosTime {
    pub sec: i32,
    pub nanosec: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RosHeader {
    pub stamp: RosTime,
    pub frame_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Pose {
    pub position: Point3D,
    pub orientation: Quaternion,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PoseWithCovariance {
    pub pose: Pose,
    pub covariance: [[f64; 6]; 6],
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Vector3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Twist {
    pub linear: Vector3D,
    pub angular: Vector3D,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TwistWithCovariance {
    pub twist: Twist,
    pub covariance: [[f64; 6]; 6],
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RosOdometry {
    pub header: RosHeader,
    pub child_frame_id: String,
    pub pose: PoseWithCovariance,
    pub twist: TwistWithCovariance,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PoseStamped {
    pub header: RosHeader,
    pub pose: Pose,
}

// ============================================================================
// NATIVE ROS 2 ACTION CANCELLATION FIELDS (STRATEGY 1 PREEMPTION)
// Models an action_msgs/srv/CancelGoal Request payload
// ============================================================================

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GoalInfo {
    pub uuid: [u8; 16], // 16-byte array. All zeros = Cancel all active goals
    pub stamp: RosTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct CancelGoalRequest {
    pub goal_info: GoalInfo,
}
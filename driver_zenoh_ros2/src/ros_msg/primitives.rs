//! Pure structural representations mapping directly to native ROS 2 IDL wire requirements.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct RosTime {
    pub sec: i32,
    pub nanosec: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct RosHeader {
    pub stamp: RosTime,
    pub frame_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Pose {
    pub position: Point3D,
    pub orientation: Quaternion,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PoseWithCovariance {
    pub pose: Pose,
    #[serde(with = "super::serde_impl::covariance_matrix")]
    pub covariance: [[f64; 6]; 6],
}

/// Mimics `geometry_msgs/msg/PoseWithCovarianceStamped` for AMCL localization updates.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PoseWithCovarianceStamped {
    pub header: RosHeader,
    pub pose: PoseWithCovariance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Vector3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Twist {
    pub linear: Vector3D,
    pub angular: Vector3D,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TwistWithCovariance {
    pub twist: Twist,
    #[serde(with = "super::serde_impl::covariance_matrix")]
    pub covariance: [[f64; 6]; 6],
}

/// Mimics `nav_msgs/msg/Odometry` for high-frequency uplink telemetry processing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct RosOdometry {
    pub header: RosHeader,
    pub child_frame_id: String,
    pub pose: PoseWithCovariance,
    pub twist: TwistWithCovariance,
}

/// Mimics `geometry_msgs/msg/PoseStamped` for TurtleBot 4 target goal injection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PoseStamped {
    pub header: RosHeader,
    pub pose: Pose,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct GoalInfo {
    pub uuid: [u8; 16],
    pub stamp: RosTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct CancelGoalRequest {
    pub goal_info: GoalInfo,
}
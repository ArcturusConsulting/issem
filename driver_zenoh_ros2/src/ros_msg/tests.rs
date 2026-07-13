//! Layout metrics verification and binary round-trip unit testing.

use super::primitives::{RosOdometry, PoseStamped};

#[test]
fn test_covariance_flat_serialization_layout() {
    let mut odom = RosOdometry::default();
    odom.header.frame_id = "odom".to_string();
    odom.child_frame_id = "base_link".to_string();
    
    let mut val = 1.0;
    for i in 0..6 {
        for j in 0..6 {
            odom.pose.covariance[i][j] = val;
            odom.twist.covariance[i][j] = val * 2.0;
            val += 1.0;
        }
    }

    let bytes = odom.to_cdr_bytes().expect("Failed standard CDR encoding verification");

    // Wire verification: check layout scale parameters ensuring continuous byte mapping
    assert!(bytes.len() >= 696, "Byte array length lower than strict minimum DDS footprint threshold");

    let restored_odom = RosOdometry::from_cdr_bytes(&bytes).expect("Failed standard CDR decoding verification");

    assert_eq!(restored_odom.header.frame_id, "odom");
    assert_eq!(restored_odom.child_frame_id, "base_link");
    assert_eq!(restored_odom.pose.covariance[0][0], 1.0);
    assert_eq!(restored_odom.pose.covariance[5][5], 36.0);
    assert_eq!(restored_odom.twist.covariance[0][0], 2.0);
    assert_eq!(restored_odom.twist.covariance[5][5], 72.0);
}

#[test]
fn test_pose_stamped_round_trip() {
    let mut target = PoseStamped::default();
    target.header.frame_id = "map".to_string();
    target.pose.position.x = 12.5;
    target.pose.orientation.w = 1.0;

    let bytes = target.to_cdr_bytes().expect("Failed to serialize PoseStamped");
    let restored = PoseStamped::from_cdr_bytes(&bytes).expect("Failed to deserialize PoseStamped");

    assert_eq!(restored.header.frame_id, "map");
    assert_eq!(restored.pose.position.x, 12.5);
    assert_eq!(restored.pose.orientation.w, 1.0);
}
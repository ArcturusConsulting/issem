//! Custom serialization logic and binary utilities for high-speed CDR conversions.

use super::primitives::{CancelGoalRequest, RosOdometry, PoseStamped};

/// Custom mapping module to handle AMCL and Odometry 36-element covariance matrices
/// as a nested multi-dimensional matrix `[[f64; 6]; 6]` while bypassing
/// Serde's native 32-element macro limitations and enforcing flat CDR binary layouts.
pub mod covariance_matrix {
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::ser::SerializeTuple;
    use serde::{Deserializer, Serializer};
    use std::fmt;

    /// Serializes a 6x6 2D array into a flat 36-element sequential stream without 
    /// sequence length prefixes, matching CDR's fixed-size array constraints (`float64[36]`).
    pub fn serialize<S>(matrix: &[[f64; 6]; 6], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tuple = serializer.serialize_tuple(36)?;
        for row in matrix.iter() {
            for &value in row.iter() {
                tuple.serialize_element(&value)?;
            }
        }
        tuple.end()
    }

    /// Deserializes a flat 36-element continuous sequence back into an ergonomic `[[f64; 6]; 6]` matrix.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<[[f64; 6]; 6], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MatrixVisitor;

        impl<'de> Visitor<'de> for MatrixVisitor {
            type Value = [[f64; 6]; 6];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a flat continuous stream of 36 float64 elements matching float64[36]")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut matrix = [[0.0; 6]; 6];
                for row in 0..6 {
                    for col in 0..6 {
                        matrix[row][col] = seq
                            .next_element()?
                            .ok_or_else(|| A::Error::custom("Unexpected end of stream: missing elements in 36-count matrix"))?;
                    }
                }
                Ok(matrix)
            }
        }

        deserializer.deserialize_tuple(36, MatrixVisitor)
    }
}

// ============================================================================
// BINARY UTILITY EXTENSIONS FOR LITTLE-ENDIAN CDR LAYER HANDSHAKES
// ============================================================================

impl PoseStamped {
    pub fn to_cdr_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let encoded = cdr::serialize::<_, _, cdr::CdrLe>(self, cdr::Infinite)?;
        Ok(encoded)
    }

    pub fn from_cdr_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let decoded = cdr::deserialize::<Self>(bytes)?;
        Ok(decoded)
    }
}

impl RosOdometry {
    pub fn to_cdr_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let encoded = cdr::serialize::<_, _, cdr::CdrLe>(self, cdr::Infinite)?;
        Ok(encoded)
    }

    pub fn from_cdr_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let decoded = cdr::deserialize::<Self>(bytes)?;
        Ok(decoded)
    }
}

impl CancelGoalRequest {
    pub fn to_cdr_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let encoded = cdr::serialize::<_, _, cdr::CdrLe>(self, cdr::Infinite)?;
        Ok(encoded)
    }

    pub fn from_cdr_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let decoded = cdr::deserialize::<Self>(bytes)?;
        Ok(decoded)
    }
}
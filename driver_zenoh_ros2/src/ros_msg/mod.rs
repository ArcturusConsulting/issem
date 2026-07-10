//! Module entry point for ROS 2 IDL message layout specifications.

pub mod primitives;
pub mod serde_impl;

#[cfg(test)]
mod tests;

// Publicly re-export all structures from the primitives file so they match 
// the public visibility interface expected across the ISSEM workspace.
pub use primitives::*;
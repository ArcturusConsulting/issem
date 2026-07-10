# Module: ISSEM Southbound Robotics Driver (`driver_zenoh_ros2`)

This module functions as the isolated Southbound network interface for ISSEM. It encapsulates all low-level Zenoh session runtimes, handles peer-to-peer edge connections over warehouse Wi-Fi, and encodes/decodes native ROS 2 binary message structures using Little-Endian Common Data Representation (CDR).

By decoupling these mechanics into a dedicated crate, the core transactional engine (`issem_core`) is kept entirely free of protocol serialization logic and Zenoh dependency graphs.

---

## 1. Architectural Role & Boundary Layout

```text
 ┌────────────────────────┐         MPSC Channel          ┌─────────────────────┐
 │    adapter_vda5050     ├──────────────────────────────►│     issem_core      │
 └────────────────────────┘    (Parsed VDA Commands)      └──────────┬──────────┘
                                                                     │
                                                                     │ Inbound Command
                                                                     ▼ (Internal Enum)
 ┌────────────────────────┐         Zenoh Protocol        ┌─────────────────────┐
 │    Physical Fleet     │◄──────────────────────────────┤  driver_zenoh_ros2  │
 │ (TurtleBot 4 / Nav2)   ├──────────────────────────────►│ (Southbound Driver) │
 └────────────────────────┘      (Binary Telemetry)       └──────────┬──────────┘
                                                                     │
                                                                     │ Direct High-Speed
                                                                     ▼ Atomic Overwrites
                                                          ┌─────────────────────┐
                                                          │      Redis Pod      │
                                                          └─────────────────────┘
```

### Protocol Translation Mandate
* **Downlink (Command Injection):** Intercepts high-level routing requests from the core, transforms planar angles into spatial 3D quaternions, serializes data into pure DDS wire format, and publishes directly onto scoped Zenoh keyspaces (`{serialNumber}/goal_pose`). This bypasses standard ROS 2 action client queues to mitigate volatile Wi-Fi connection drops.
* **Uplink (High-Speed Catch):** Listens to continuous high-frequency robotics telemetry (`**/amcl_pose`, `**/odom`), extracts linear and angular metrics, converts orientation vectors back to standard planar radians (theta), and writes updates directly to the external Redis cluster.

---

## 2. Technical Workaround: 36-Element Covariance Flattening

Standard ROS 2 messages handling localization uncertainties (e.g., `geometry_msgs/msg/PoseWithCovarianceStamped`) represent structural covariance as a flat `float64[36]` fixed-size array. 

Standard Serde implementations fail for this requirement due to a hardcoded 32-element limitation on array macros. To maintain an ergonomic 2D matrix layout (`[[f64; 6]; 6]`) inside our codebase while satisfying exact DDS wire configurations, this crate implements a custom tuple-serialization loop inside `ros_msg::covariance_matrix`.

This system strips out standard sequence length headers, ensuring raw, sequential 288-byte data arrays stream onto the wire smoothly:

```rust
// Ergonomic representation within Rust:
pub struct PoseWithCovariance {
    pub pose: Pose,
    #[serde(with = "covariance_matrix")]
    pub covariance: [[f64; 6]; 6], // Serializes cleanly to flat float64[36]
}
```

---

## 3. Public API Boundary Contracts

### A. Core Configuration Layout
```rust
pub struct ZenohDriverConfig {
    pub listen_host: String,
    pub listen_port: u16,
    pub target_amr_serials: Vec<String>,
}
```

### B. Inbound Boundary Channel Commands
The internal command structure handled via asynchronous channel pipes:
```rust
pub enum SouthboundCommand {
    NavigateToPose {
        serial_number: String,
        x: f64,
        y: f64,
        theta: f64,
    },
    PreemptAndHalt {
        serial_number: String,
    },
}
```

### C. Execution Entrypoints
```rust
/// Launches the background telemetry ingestion loop mapping incoming Zenoh payloads to Redis.
pub async fn start_telemetry_uplink(
    config: ZenohDriverConfig,
    redis_client: redis::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Launches the downlink driver parsing internal commands into live target frames.
pub async fn start_command_downlink(
    config: ZenohDriverConfig,
    mut command_receiver: tokio::sync::mpsc::Receiver<SouthboundCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
```

---

## 4. Shared State Registry Mappings (Redis)

Telemetry workers stream localization arrays straight to the global memory tier to ensure zero data loss during cloud-native pod migrations.

* **Target Namespace:** `amr:{serialNumber}:pose`
* **Data Structure:** Hash

| Hash Key Field | Storage Type | Unit / Representation | Writer Source |
| :--- | :--- | :--- | :--- |
| `x` | `f64` | Meters (Map Frame coordinate) | `driver_zenoh_ros2` |
| `y` | `f64` | Meters (Map Frame coordinate) | `driver_zenoh_ros2` |
| `theta` | `f64` | Planar Yaw Radians ([-pi, pi]) | `driver_zenoh_ros2` |
| `last_updated`| `u64` | Epoch Timestamp (Milliseconds) | `driver_zenoh_ros2` |

---

## 5. Local Subsystem Verification

To run compilation health checks, verify byte sequence layouts, and execute serialization loop unit validations for this crate independently of the broader network infrastructure, run:

```bash
cargo test -p driver_zenoh_ros2
```
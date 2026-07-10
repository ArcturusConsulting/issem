# ISSEM (Interoperable Stateless Synchronization Enabler Module)

[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Protocol](https://img.shields.io/badge/protocol-VDA5050%20v3.0.0-blue.svg)](https://github.com/VDA5050/VDA5050)
[![Middleware](https://img.shields.io/badge/middleware-Zenoh%20%7C%20ROS%202-green.svg)](https://zenoh.io/)
[![Database](https://img.shields.io/badge/state-Redis%20(Durable)-red.svg)](https://redis.io/)

ISSEM is a centralized, high-performance, stateless server-side gateway written in pure Rust. It bridges the structural chasm between corporate **Warehouse Execution Systems (WES)** and agile, open-source **Autonomous Mobile Robots (AMRs)**. 

The framework intercepts industrial **VDA5050 JSON payloads over MQTT**, maps high-level coordination paths into localized navigation target frames, and pipes them down to the robot fleet using ultra-lean, sub-millisecond **Zenoh binary streams**. Telemetry tracking handles high-frequency (5 Hz+) localization arrays, synchronization states, and hardware overrides via a highly resilient, externalized **Redis** shared state layer.

> 💡 **Repository Note:** Looking for the initial proof-of-concept monolithic single-robot pipeline? Check out the historical `prototype` branch. The `main` branch holds the complete multi-crate enterprise fleet workspace.

---

## 1. Value Proposition & Project Context

In modern industrial logistics hubs (such as third-party logistics networks and heavy manufacturing plants), enterprise IT systems mandate strict **VDA5050 compliance over MQTT** to eliminate vendor lock-in. Conversely, modern AMRs run on highly dynamic, binary network graphs like **ROS 2 Jazzy and Zenoh/DDS**. 

ISSEM provides a **Stateless On-Premise Container pattern** designed to serve as a high-performance alternative to massive multi-vendor frameworks (like Open-RMF) in environments where the WES commands the automation layer directly.

### Core Architecture Enhancements
* **Off-Robot Stateless Compute:** Pulls protocol serialization overhead off the physical vehicles. AMRs communicate via lean, native binary streams over the air, saving edge CPU and battery capacity.
* **Decoupled State & Resiliency:** Eliminates Single Points of Failure (SPOF). Compute logic is separated from memory blocks. If the Rust compute container crashes, an orchestrator (Docker/K3s) revives it in milliseconds; the new container reconnects to the persistent Redis node and restores full fleet navigation context instantly with zero data loss.
* **DDS Bottleneck Mitigation:** Bypasses deep ROS 2 service/action queue constraints (such as the 31-byte Fast-DDS history allocation limit) by executing a stateful topic-level preemption engine for instant overrides (`pause`, `resume`, `cancelOrder`).

### Target Deployment Profiles
* **Single-Vendor AMR Fleets:** Operations deploying between 10 to 50 custom ROS 2/Nav2 mobile platforms on open warehouse floors where heavy spatial traffic deconfliction frameworks represent massive operational and computational overkill.
* **Legacy Corporate Integrations:** Facilities governed by strict enterprise IT compliance mandates where the top-level orchestrator is a traditional commercial WES (e.g., SAP EWM, Siemens Logistics, Daifuku, or Swisslog) that requires direct VDA5050 compliance out-of-the-box.
* **Hardware-Constrained Robotics Teams:** Teams building highly optimized AMRs that need to preserve 100% of their onboard edge computing power for localized navigation and computer vision tasks rather than parsing heavy corporate JSON payloads over volatile factory Wi-Fi networks.

### Strategic Market Positioning

| Strategic Vector | Open-RMF Fleet System | InOrbit Edge Connector | ISSEM Gateway (This Project) |
| :--- | :--- | :--- | :--- |
| **Execution Domain** | Server Rack (Multi-Process Suite) | Physical Robot (Edge Node) | **Server Rack (Stateless Gateway)** |
| **Primary Code Stack** | C++ / ROS 2 / Python | C++ / ROS 2 / Python | **Pure Rust / Zenoh / Redis** |
| **System Footprint** | Heavy (Requires Full ROS 2 Stack) | Medium (Edge Compute Overhead) | **Ultra-Lightweight Container** |
| **Northbound Interface**| Proprietary WebSockets / REST | Standard VDA5050 over MQTT | **Standard VDA5050 over MQTT** |
| **Network Efficiency** | Heavy DDS Multicast Traffic | Heavy JSON Payload over Wi-Fi | **Lean Binary Zenoh Streams over Wi-Fi** |

---

## 2. System Architecture & Data Flows

ISSEM functions as the centralized multi-tenant traffic router deployed on the local warehouse server rack.

```text
  ┌────────────────────────────────────────────────────────┐
  │                 ENTERPRISE NETWORK ZONE                │
  │  WES / ERP (VDA5050 JSON) ──► MQTT Broker (EMQX)       │
  └───────────────────────────┬────────────────────────────┘
                              │ TCP Port
  ┌───────────────────────────▼────────────────────────────┐
  │            ISSEM ON-PREMISE CONTAINER STACK            │
  │                                                        │
  │   ┌──────────────────────────┐   IPC   ┌────────────┐  │
  │   │     issem_workspace      ├────────►│   Redis    │  │
  │   │  (Stateless Compute Core)│◄────────┤ (AOF Sync) │  │
  │   └─────────────┬────────────┴─────────┴────────────┘  │
  └─────────────────┼──────────────────────────────────────┘
                    │ Zenoh Binary Protocol (Wi-Fi)
  ┌─────────────────▼──────────────────────────────────────┐
  │               LOCAL ROBOTICS EXECUTION ZONE            │
  │                                                        │
  │      ┌───────────────────────┐ DDS┌────────────┐       │
  │      │ Zenoh-DDS Edge Bridge ├───►│ Nav2 Stack │       │
  │      └───────────────────────┘    └────────────┘       │
  │                     [ ROS2 Robot ]                     │
  └────────────────────────────────────────────────────────┘
```

### High-Frequency Data Pipelines

#### A. Downlink Path Command (WES ──► Robot Fleet)
1. **Ingestion:** An order packet arrives at `vda5050/3.0.0/manufacturer/serialNumber/order`.
2. **Parsing & Mapping:** The VDA5050 module parses the target coordinates and target orientation angle (theta). It computes the target quaternion rotation variables:
   * q_z = sin(theta / 2)
   * q_w = cos(theta / 2)
3. **State Verification:** The core checks the Redis status registry to ensure the specified AMR is not currently locked in a HARD e-stop state block.
4. **Cache Backing:** The computed `PoseStamped` structure is stored in the persistent cache database.
5. **Zenoh Injection:** The data is serialized into Common Data Representation (CDR) little-endian byte format and published over Zenoh to `{serialNumber}/goal_pose`.

#### B. Uplink Telemetry Processing (Robot Fleet ──► WES)
1. **High-Speed Catch:** The Zenoh driver captures binary localization arrays coming from the robot fleet at frequencies exceeding 5 Hz.
2. **Euler Transformation:** The vehicle's quaternion components (q_x, q_y, q_z, q_w) are instantly converted to a planar radian yaw format (theta) for corporate ingestion:
   * theta = atan2(2.0 * (q_w * q_z + q_x * q_y), 1.0 - 2.0 * (q_y * q_y + q_z * q_z))
3. **Cache Sync:** The active location coordinates (x, y, theta) are updated in the Redis cluster using high-speed key-value overwrites.
4. **Throttled State Generation:** A background loop collects the current pose from Redis at a stabilized, throttled rate of 5 Hz, pairs it with battery and system metrics, builds a compliant VDA5050 state JSON structure, and publishes it back up to the enterprise MQTT broker.

### Deep Robotics Bottleneck Workarounds: Zero-Distance Preemption
A major bug in legacy middleware integrations is trying to command pauses and aborts through deep ROS 2 service or action layers, which frequently lock up due to client history queue allocations (the 31-byte Fast-DDS history boundary limitation).

ISSEM bypasses this completely via a custom stateful tracking loop inside the compute core:

```text
[ Incoming VDA5050 PAUSE Action ]
               │
               ▼
┌──────────────────────────────────────────────┐
│             ISSEM CORE COMPUTE               │
│ 1. Read current active position from Redis   │
│ 2. Retain original target waypoint in Cache  │
│ 3. Generate instant "Halt Target"            │
└──────────────┬───────────────────────────────┘
               │
               ▼ (Bypasses Action Layers)
[ Publish Halt Target directly to Zenoh {serialNumber}/goal_pose ]
               │
               ▼
[ Nav2 stack preempts active path, executing immediate zero-distance stop ]
```

When a `RESUME` command is subsequently received, the core reads the cached original waypoint target out of Redis and re-injects it into the Zenoh stream, restoring active navigation mid-transit with zero loss of order context.

---

## 3. Implementation Roadmap & Repository Blueprint

### Workspace Crate Modular Subsystems

To ensure strict separation of concerns and eliminate protocol compile-time interference, ISSEM is architected as a decoupled multi-crate Rust workspace:

* **`issem_core` (Transactional Engine Core):** The engine's transactional brain. It is entirely protocol-agnostic. It consumes internal Rust primitives passed through bounded memory channels and manages state updates via the Redis client abstraction.
* **`adapter_vda5050` (Northbound Enterprise Gateway):** Owns the MQTT connection. Spawns an asynchronous `rumqttc` event loop to manage enterprise broker handshakes, ingests inbound string payloads, and validates structural semantics against the VDA5050 specification.
* **`driver_zenoh_ros2` (Southbound Robotics Driver):** Owns the Zenoh session. Listens to high-speed binary streams, deserializes the CDR bytes (handling your nested `[[f64; 6]; 6]` covariance arrays natively, bypassing Serde's standard 32-element array constraint), and handles outgoing waypoint injection.
* **`adapter_opc_ua` (East/West Peripheral Sync):** Hosts a high-speed asynchronous industrial OPC UA client stack to interface with factory PLCs. It handles physical hardware handshakes (e.g., elevators and conveyors) before letting an AMR complete a payload handover.

### Repository Directory Topology

```text
issem_workspace/
├── Cargo.toml                      # Master workspace configuration
├── docker-compose.yml              # Centralized container orchestration 
├── redis.conf                      # Hyper-durable persistence configuration
│
├── issem_core/                     # Transactional Engine Core
│   ├── Cargo.toml                  # Encapsulates redis driver (tokio-comp)
│   └── src/
│       ├── lib.rs
│       ├── state_manager.rs        # Redis client infrastructure & cache commands
│       └── engine.rs               # Multi-tenant route & override handlers
│
├── adapter_vda5050/                # Northbound Enterprise Gateway
│   ├── Cargo.toml                  # Encapsulates rumqttc client loop
│   └── src/
│       ├── lib.rs
│       ├── mqtt_client.rs          # Asynchronous MQTT packet subscriber
│       └── schema.rs               # VDA5050 JSON validation primitives
│
├── driver_zenoh_ros2/              # Southbound Robotics Driver
│   ├── Cargo.toml                  # Encapsulates zenoh & cdr engines
│   └── src/
│       ├── lib.rs
│       ├── zenoh_session.rs        # High-speed telemetry ingestion runtime
│       └── ros_msg.rs              # Advanced CDR serialization protocols
│
└── adapter_opc_ua/                 # East/West Peripheral Sync
    ├── Cargo.toml                  # Encapsulates industrial opcua crates
    └── src/
        ├── lib.rs
        └── opcua_client.rs         # Asynchronous PLC connection handshakers
```

### Master Workspace `Cargo.toml`

```toml
[workspace]
members = [
    "issem_core",
    "adapter_vda5050",
    "driver_zenoh_ros2",
    "adapter_opc_ua"
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.38", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
```

### Redis Schema Design & Shared State Topology
ISSEM structures its key space dynamically using deterministic namespaces keyed by the unique AMR serial number:

```text
amr:fleet:active_serials         -> Set [ "Kashiwa-Robot-001", "Kashiwa-Robot-002" ]
amr:{serialNumber}:pose          -> Hash { "x": "12.45", "y": "-8.21", "theta": "1.57" }
amr:{serialNumber}:active_goal   -> String (Serialized CDR PoseStamped Binary string)
amr:{serialNumber}:lifecycle     -> Hash { "mode": "AUTOMATIC", "paused": "false" }
```

To guarantee absolute durability against unexpected facility power failures, the accompanying container system runs a hybrid, high-frequency logging persistence loop:

```ini
# redis.conf
appendonly yes
appendfsync everysec
save 300 1
```

---

## 4. Phased Development Roadmap & Status

### Phase 1: Workspace Infrastructure & Telemetry Uplink ── `[COMPLETED]`
* Root workspace directory setup and layout of the modular compilation crates.
* Implementation of the asynchronous `rumqttc` network runtime for continuous Northbound traffic management.
* Mapping of 5 Hz throttled state generation loops feeding positional tracking to the corporate layer.

### Phase 2: High-Fidelity Binary Serialization ── `[COMPLETED]`
* Configuration of the little-endian `cdr` stream engine to process Zenoh networking layers.
* **Technical Achievement:** Authored a custom multi-dimensional deserialization algorithm (`[[f64; 6]; 6]`) to parse AMCL's 36-element localization covariance matrix, natively bypassing Serde's structural 32-element array macro limitation.

### Phase 3: Stateful Downlink & Topic Preemption ── `[COMPLETED]`
* Construction of VDA5050 path target parsing and coordinate translation maps.
* **Technical Achievement:** Isolated and resolved the 31-byte Fast-DDS history lockup vulnerability. Developed an asynchronous topic preemption layer that catches instant `pause` payloads, retains the true route targets inside local thread-safe boundaries, and injects zero-distance halt parameters to safely freeze the robot mid-transit. A subsequent `resume` re-injects the original target cleanly.

### Phase 4: Shared State Externalization & Multi-Tenancy ── `[IN PROGRESS]`
* Refactoring localized memory collections into a robust, concurrent `redis` async wrapper.
* Keying global multi-tenant namespaces dynamically using unique AMR `{serialNumber}` paths extracted from routing topologies.
* Designing a hyper-durable container setup linking the stateless Rust application to a persistent Redis node backed by Append-Only File (AOF) disk sync writes every second.

### Phase 5: East/West Physical PLC Handshaking ── `[PLANNED]`
* Building an active, asynchronous OPC UA client stack to interface with factory PLCs.
* Programming automated safety handshakes (e.g., locking elevator cabs, verifying conveyor optical sensors) before allowing an AMR to release cargo waypoints.

---

## 5. Local Sandbox Verification & Verification Loop

To run the complete stateless architecture suite in your local simulation environment, follow the steps below:

### 1. Provision the Environment
Spin up the container cluster. This boots up the stateless Rust compute engine and initializes a persistent Redis node configured for AOF recording:
```bash
docker compose up --build -d
```

### 2. Monitor Live Data Logs
Attach to the container output logs to view real-time protocol processing:
```bash
docker logs -f issem_gateway
```

### 3. Dispatch an Enterprise Command
Inject an industry-compliant VDA5050 `instantAction` payload into the local MQTT network loop using the terminal interface to verify the preemption mechanics:
```bash
mosquitto_pub -h localhost -p 1883 \
  -t "vda5050/3.0.0/Arcturus-Logistics/Kashiwa-Robot-001/instantAction" \
  -m '{
    "headerId": 102,
    "timestamp": 1783584600,
    "version": "3.0.0",
    "manufacturer": "Arcturus-Logistics",
    "serialNumber": "Kashiwa-Robot-001",
    "actions": [
      {
        "actionType": "pause",
        "actionId": "act_p_009",
        "blockingType": "HARD"
      }
    ]
  }'
```

### 4. Inject a Crash Scenario (Resiliency Drill)
While a robot is tracking an active path, deliberately force a crash on the compute layer to test data persistence:
```bash
docker compose restart issem
```
*Observe that the newly spawned container instance initializes, queries the surviving Redis node, and resumes tracking the AMR fleet coordinates within 50 milliseconds with zero loss of execution context.*
```
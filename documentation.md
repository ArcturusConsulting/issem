# Zenoh VDA5050 Workspace Documentation

This repository establishes a decoupled, ultra-low-latency bridge between an internal robot operating ecosystem (**ROS 2 / DDS**) and an enterprise industrial fleet management fabric (**VDA5050 / MQTT**).

---

## Technical Architecture & Data Flow

### 1. Component Breakdown

* **Robot Emulation Layer (`ROS 2` / `Gazebo` / `Nav2`)**
    * Operates locally on the Automated Mobile Robot (AMR).
    * Handles high-frequency sensor fusion, mapping, and local path planning.
    * Continuously publishes raw `nav_msgs/msg/Odometry` data frames within its isolated DDS domain.
* **Network Transport Layer (`zenoh-bridge-ros2dds`)**
    * Deployed directly on the AMR as a standalone Zenoh Client.
    * Intercepts local DDS traffic straight from loopback memory, completely bypassing heavy Wi-Fi multicast network flooding.
    * Pipes the filtered data over a stable, point-to-point TCP connection directly to the server infrastructure.
* **Translation Engine (`vda5050_server`)**
    * A high-performance standalone Rust Peer Binary running an asynchronous `tokio` runtime worker loop on the factory edge server.
    * Dynamically pulls environment variables and ports from an external `config.yaml` card.
    * Hosts the TCP listener socket, catches frames over the network using a multi-segment wildcard (`**/odom`), and parses raw telemetry frames natively into industrial JSON payloads.
* **Enterprise Integration Fabric (`MQTT` / `Mosquitto Broker`)**
    * Acts as the centralized global enterprise message bus.
    * Accepts stringified JSON packets published by the server onto standardized industrial topic trees for real-time consumption by top-tier Warehouse Execution Systems (WES).

### 2. End-to-End Data Flow Sequence

| Step | Layer | Mechanism | Data Format |
| :--- | :--- | :--- | :--- |
| **1. Telemetry Generation** | AMR Internal | Nav2 / AMCL publishes localization updates to the local DDS layer. | Native DDS C++ Struct |
| **2. Local Ingestion** | Network Bridge | Zenoh Bridge intercepts DDS frames and wraps them into zero-copy network tokens. | Binary Stream |
| **3. Transport Uplink** | Network Pipe | The bridge streams tokens as a TCP Client directly to the edge server port (`7448`). | Compressed Binary |
| **4. Ingest & Map** | Edge Server | Rust Server picks up frames asynchronously via `recv_async()`, loads configuration states, and builds a VDA5050 JSON payload. | `serde_json::Value` |
| **5. Global Publish** | Enterprise Bus | The server flushes the structured string over to the Mosquitto broker on topic `vda5050/v2/state`. | VDA5050 JSON String |

---

## Bidirectional VDA5050 Implementation Roadmap

To achieve complete, production-grade bidirectional compliance with the [Official VDA5050 Schema Ecosystem](https://github.com/VDA5050/VDA5050/tree/main/json_schemas), development is organized into a four-phased engineering rollout.

### Phase 1: Deep Ingestion & State Serialization (Uplink Expansion)
* **Objective:** Transition from mock coordinate wrappers to absolute telemetry tracking.
* **Execution:**
    * Integrate a CDR (Common Data Representation) Deserializer (`cdr` crate) into the Rust server loop to decode the raw binary bytes from `sample.payload()` straight into matching Rust structs for `nav_msgs/msg/Odometry`.
    * Expand the outbound JSON encoder to fully saturate the mandatory fields of the VDA5050 State Schema (`state.json`), including `batteryState` (voltage/percentage), `operatingMode` (AUTOMATIC/MANUAL), and `safetyState` (E-stop status).

### Phase 2: Inbound Order Ingestion (Downlink Path)
* **Objective:** Allow the enterprise fleet manager to command the robot using VDA5050 standard graph paths.
* **Execution:**
    * Introduce a parallel async Tokio task in `vda5050_server` that subscribes to the MQTT topic `vda5050/v2/order`.
    * Implement strict structural validation of incoming paths using the VDA5050 Order Schema (`order.json`).
    * Build a Graph Translation Engine that parses incoming VDA5050 nodes and edges (coordinates, orientations, and trajectories) and translates them into a sequence of native ROS 2 Nav2 `FollowWaypoints` or `MapsToPose` Action Goals.
    * Expose these goals back to the AMR by declaring a Zenoh Publisher on the server that maps to the robot's action server interfaces.

### Phase 3: Operational Control & Lifecycle Safety
* **Objective:** Implement heartbeat monitoring and immediate, high-priority intervention maneuvers.
* **Execution:**
    * Implement the VDA5050 Connection Schema (`connection.json`) to handle daemon heartbeats. Configure the MQTT client with a Last Will and Testament (LWT) packet so that if an AMR drops off the Wi-Fi network unexpectedly, the broker immediately flags the asset state as `OFFLINE`.
    * Incorporate the VDA5050 Instant Actions Schema (`instantAction.json`) to process immediate override commands (`pause`, `resume`, `cancelOrder`). Map these commands directly to the ROS 2 Nav2 lifecycle manager services to instantly halt or release the robot's physical drive motors.

### Phase 4: Concurrency & Multi-Robot Fleet Scaling
* **Objective:** Scale the single-server instance to coordinate dozens of physical AMRs simultaneously.
* **Execution:**
    * Refactor the `vda5050_server` state tracker from localized variables to a thread-safe global collection, such as a Concurrent Hash Map (`DashMap` crate).
    * Key the map by the unique `serialNumber` extracted from incoming network paths. This allows a single running server gateway to maintain isolated tracking frames, pending path queues, and active connection lifecycles for an entire multi-robot fleet concurrently.

---

## Installed Middleware

Execute the following setup sequence to ensure all core system dependencies are fully provisioned:

```bash
# Install Open-RMF core development headers for ROS 2 Jazzy
sudo apt install -y ros-jazzy-rmf-dev

# Install Mosquitto MQTT Broker and testing client interfaces
sudo apt install -y mosquitto mosquitto-clients

# Install Python 3 MQTT Client libraries for ecosystem diagnostic scripts
sudo apt install -y python3-paho-mqtt
```

## Testing & Execution Guide

To run a full end-to-end telemetry system integration test, initialize the following environment processes across separate terminal tabs in this exact chronological order:

### 1. Start the System Infrastructure
* **Terminal 1: The MQTT Broker**
    ```bash
    mosquitto
    ```
* **Terminal 2: The Gazebo Virtual Warehouse World**
    ```bash
    # Execute inside your active ROS 2 navigation workspace
    ros2 launch turtlebot4_gz_bringup turtlebot4_gz.launch.py world:=warehouse model:=lite localization:=true nav2:=true
    ```

### 2. Synchronize Localization and Launch Applications
* **Terminal 3: Initialize Robot Pose (AMCL Map Alignment)**
    ```bash
    ros2 topic pub --once /initialpose geometry_msgs/msg/PoseWithCovarianceStamped "{header: {frame_id: 'map'}, pose: {pose: {position: {x: 0.814, y: -0.655, z: 0.0}, orientation: {w: 1.0}}}}"
    ```
* **Terminal 4: Start the VDA5050 Translation Server (TCP Listener)**
    ```bash
    cd ~/zenoh_vda5050_workspace/server_side/vda5050_server
    ./target/release/vda5050_server
    ```
* **Terminal 5: Fire Up the AMR Client Tunnel (TCP Client)**
    ```bash
    cd ~/zenoh_vda5050_workspace/amr_side
    ./zenoh-bridge-ros2dds client -e tcp/127.0.0.1:7448
    ```

### 3. Verify Live Global Output Stream
* **Terminal 6: Monitor Industrial Outbound Telemetry**
    ```bash
    mosquitto_sub -t "vda5050/v2/state" -v
    ```

---

## Useful Commands

### Environment Reset & Process Eviction
```bash
# Terminate any ghost Gazebo simulation engines running in the background
pkill -f gz

# Terminate lingering ROS 2 nodes or graph daemons
pkill -f ros2
ros2 daemon stop

# Force-stop any conflicting background Mosquitto instances
sudo killall mosquitto
```

### Manual Node Interventions & Test Dispatching
```bash
# Reset / Overwrite the robot's physical profile on the map
ros2 topic pub --once /initialpose geometry_msgs/msg/PoseWithCovarianceStamped "{header: {frame_id: 'map'}, pose: {pose: {position: {x: 0.814, y: -0.655, z: 0.0}, orientation: {w: 1.0}}}}"

# Dispatch a mock industrial path routing order straight to the Fleet Broker
mosquitto_pub -t "vda5050/v2/Arcturus/Robot001/order" -m '{
  "orderId": "order_2026_07_07",
  "nodes": [
    {
      "nodeId": "warehouse_waypoint_alpha",
      "nodePosition": {
        "x": 3.0,
        "y": 2.0
      }
    }
  ]
}'
```
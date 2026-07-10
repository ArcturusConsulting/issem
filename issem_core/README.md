# Module: ISSEM Transactional Core (`issem_core`)

This module functions as the centralized, protocol-agnostic transactional engine of ISSEM. It coordinates state transitions, executes multi-tenant fleet tracking maps, and acts as the structural bridge between the Northbound Enterprise Gateway and the Southbound Robotics Driver.

---

## 1. Architectural Runtime Mapping

```text
 ┌────────────────────────────────────────────────────────┐
 │                    ADAPTER_VDA5050                     │
 └────────────────────────┬───────────────────────────────┘
                          │
                          │ NorthboundEvent (MPSC)
                          ▼
 ┌────────────────────────────────────────────────────────┐
 │                       ISSEM_CORE                       │
 │                                                        │
 │   ┌─────────────────┐       ┌──────────────────────┐   │
 │   │  Engine Router  ├──────►│ 5Hz State Publisher  │   │
 │   └────────┬────────┘       └──────────┬───────────┘   │
 └────────────┼───────────────────────────┼───────────────┘
              │                           │
              ├──────────────┐            │ Direct MQTT Publish
              │              │            │ (Via Shared Client)
              ▼              ▼            ▼
      ┌──────────────┐┌──────────────┐┌───────────────────┐
      │  Redis Pod   ││  MPSC Tx     ││   Corporate WES   │
      │ (State Tier) ││ (Southbound) ││  (Broker Fabric)  │
      └──────────────┘└──────┬───────┘└───────────────────┘
                             │
                             ▼
 ┌────────────────────────────────────────────────────────┐
 │                   DRIVER_ZENOH_ROS2                    │
 └────────────────────────────────────────────────────────┘
```

### Core Execution Responsibilities
* **Order Processing:** Intercepts incoming `OrderReceived` primitives, maps them into atomic target configurations, writes the original target frame cache down to Redis (`amr:{serialNumber}:active_goal`), and emits a downstream navigation event.
* **Topic Preemption Handling:** Intercepts critical `pause`, `resume`, and `cancelOrder` action tokens. For a `pause` action, it completely bypasses standard deep ROS 2 action server queues, fires an instantaneous `PreemptAndHalt` event to the robotics channel, and toggles the dynamic state bit inside the Redis engine workspace.
* **Throttled Telemetry Broadcast:** Spins up a localized `tokio::time::interval` worker locking at 5 Hz. This loop reads the spatial metrics (`x`, `y`, `theta`) written to Redis by the Southbound driver, marries them with system data configurations, and builds a fully formatted VDA5050 `Vda5050State` payload to publish back up to the enterprise MQTT layer.

---

## 2. Shared State Topology Reference (Redis)

This crate actively tracks multi-tenant execution constraints across separate database fields using namespaced AMR serial parameters.

### A. Lifecycle Configuration Cache
* **Target Key Format:** `amr:{serialNumber}:lifecycle`
* **Data Structure:** Hash

| Hash Key | Data Type | Permitted Values | Architectural Context |
| :--- | :--- | :--- | :--- |
| `paused` | `string` | `"true"`, `"false"` | Dictates whether target navigation loops are currently blocked. |
| `operating_mode` | `string` | `"AUTOMATIC"`, `"MANUAL"` | Injected into the outgoing VDA5050 state frames. |

### B. Route Path Target Cache
* **Target Key Format:** `amr:{serialNumber}:active_goal`
* **Data Structure:** String

Stores a raw JSON string mapping the absolute latest high-level waypoint goal sent by the corporate net. If a `resume` directive drops in after a hard pause event, the core extracts this cache directly to re-inject the route without requesting a duplicate transmission from the corporate WES.

---

## 3. Integrated Inter-Crate Communication Channels

| Channel Direction | Primitive Type | Linked Subsystem Crate | Purpose |
| :--- | :--- | :--- | :--- |
| **Inbound Receiver** (`rx`) | `NorthboundEvent` | `adapter_vda5050` | Stream of pre-validated corporate orders and emergency pause action flags. |
| **Outbound Sender** (`tx`) | `SouthboundCommand` | `driver_zenoh_ros2` | High-speed trajectories and zero-distance stop directives passed over the air. |

---

## 4. Subsystem Verification Verification

To run structural compilation runs and check thread boundary layouts for the transactional application container, execute:

```bash
cargo check -p issem_core
```
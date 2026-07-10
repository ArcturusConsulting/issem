# Module: ISSEM Northbound Enterprise Gateway (`adapter_vda5050`)

This module functions as the isolated Northbound network interface for ISSEM. It encapsulates the asynchronous MQTT connection runtime using `rumqttc`, handles industrial broker connectivity state, and intercepts heavy corporate JSON data streams. 

By offloading validation, string splitting, and JSON deserialization entirely to this crate, the core orchestration engine (`issem_core`) stays fully protocol-agnostic and free of text-parsing overhead.

---

## 1. Architectural Role & Boundary Layout

```text
 ┌────────────────────────┐         MQTT Protocol         ┌─────────────────────┐
 │     Enterprise WES     ├──────────────────────────────►│   adapter_vda5050   │
 │ (SAP EWM, EMQX, etc.)  │◄──────────────────────────────┤ (Northbound Gateway)│
 └────────────────────────┘        (VDA5050 JSON)         └──────────┬──────────┘
                                                                     │
                                                                     │ MPSC Channel 
                                                                     │ (Parsed Structs)
                                                                     ▼
                                                          ┌─────────────────────┐
                                                          │     issem_core      │
                                                          │  (Compute Engine)   │
                                                          └─────────────────────┘
```

### Multi-Tenant Token Routing Mandate
Rather than creating individual gateway instances per vehicle, this module hooks into a global multi-tenant subscription matrix using protocol wildcards:

```text
vda5050/3.0.0/+/+/order
vda5050/3.0.0/+/+/instantAction
```

When an enterprise packet drops into the broker fabric, the gateway intercepts the raw byte array, extracts the dynamic routing identifiers (`{manufacturer}` and `{serialNumber}`) directly from the incoming topic tokens, structurally checks the JSON schema, and fires normalized primitives down a thread-safe channel to the engine.

---

## 2. Public API Boundary Contracts

### A. Core Configuration Layout
```rust
pub struct MqttGatewayConfig {
    pub broker_url: String,
    pub broker_port: u16,
    pub protocol_version: String,
    pub manufacturer_filter: String,
}
```

### B. Normalized Event Variant (Outbound Bridge Output)
The structured types transmitted upstream to the compute core:
```rust
pub enum NorthboundEvent {
    OrderReceived {
        manufacturer: String,
        serial_number: String,
        order_id: String,
        x: f64,
        y: f64,
        theta: f64,
    },
    InstantActionReceived {
        manufacturer: String,
        serial_number: String,
        action_id: String,
        action_type: String, // "pause", "resume", "cancelOrder"
    },
}
```

### C. Execution Entrypoint
```rust
/// Launches the background rumqttc AsyncClient loop, maps wildcard topics, 
/// runs structural syntax validation, and pipes parsed operations downstream.
pub async fn start_mqtt_gateway(
    config: MqttGatewayConfig,
    event_sender: tokio::sync::mpsc::Sender<NorthboundEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
```

---

## 3. Inter-Crate Communication Channels

To protect transaction sequences from being dropped during temporary networking brownouts, communications rely on a bounded asynchronous memory pipeline:

| Channel Type | Producer (`tx`) | Consumer (`rx`) | Payload Type | Operational Context |
| :--- | :--- | :--- | :--- | :--- |
| `tokio::sync::mpsc::channel(100)` | `adapter_vda5050` | `issem_core` | `NorthboundEvent` | Fast-path transportation of pre-validated multi-tenant orders and high-priority preemption action triggers. |

---

## 4. Local Subsystem Verification

To verify semantic structures, check structural compilation blocks, and validate local deserialization match configurations independently of the broader edge robotics clusters, run:

```bash
cargo check -p adapter_vda5050
```
# Module: ISSEM East/West Peripheral Gateway (`adapter_opc_ua`)

This module serves as the isolated East/West automation interface for ISSEM. It encapsulates an asynchronous OPC UA client stack to handle physical hardware handshakes with factory PLCs, managing infrastructure such as fast-acting doors, elevators, and roller conveyors.

---

## 1. Architectural Interaction Model

```text
 ┌────────────────────────────────────────────────────────┐
 │                       ISSEM_CORE                       │
 │                   (Compute Orchestrator)               │
 └────────────────────────┬───────────────────────────────┘
                          │
                          │ PeripheralRequest (MPSC Channel)
                          ▼
 ┌────────────────────────────────────────────────────────┐
 │                    ADAPTER_OPC_UA                      │
 │                                                        │
 │   ┌─────────────────┐       ┌──────────────────────┐   │
 │   │ Request Monitor ├──────►│ Asynchronous Client  │   │
 │   └─────────────────┘       └──────────┬───────────┘   │
 └────────────────────────────────────────┼───────────────┘
                                          │
                                          │ OPC UA Binary Protocol
                                          ▼
                         ┌────────────────────────────────┐
                         │      Facility Automation       │
                         │    (Conveyor/Door/Lift PLCs)   │
                         └────────────────────────────────┘
```

### Handshake Isolation Mandate
OPC UA connection profiles require certificate handling, cryptographic session validation, and continuous node polling. By keeping this network footprint entirely inside this library crate, the orchestrator core remains decoupled from industrial fieldbus tracking variables.

---

## 2. Public API Boundary Contracts

### A. Peripheral Request Payload
The strongly-typed variants sent from `issem_core` to control physical warehouse infrastructure:
```rust
pub enum PeripheralRequest {
    /// Requests a high-speed roll-up gate to actuate.
    ClearHighSpeedDoor {
        door_id: String,
        responder_tx: tokio::sync::oneshot::Sender<bool>,
    },
    /// Triggers an interlocking sequence with a physical roller conveyor.
    InterlockConveyor {
        conveyor_id: String,
        action: String, // "START", "STOP"
        responder_tx: tokio::sync::oneshot::Sender<bool>,
    },
}
```

### B. Execution Entrypoint
```rust
/// Spawns the internal OPC UA runtime client, connects to the targeted field PLC,
/// and processes oncoming peripheral requests from the core MPSC queue.
pub async fn start_opc_ua_gateway(
    endpoint_url: String,
    mut request_receiver: tokio::sync::mpsc::Receiver<PeripheralRequest>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
```

---

## 3. Inter-Crate Communication Channels

| Channel Type | Producer (`tx`) | Consumer (`rx`) | Purpose |
| :--- | :--- | :--- | :--- |
| `tokio::sync::mpsc::channel` | `issem_core` | `adapter_opc_ua` | Streams structural execution blocks requesting physical asset overrides. |
| `tokio::sync::oneshot::channel` | `adapter_opc_ua` | `issem_core` | Asynchronously returns boolean validation blocks confirming the PLC operation succeeded. |

---

## 4. Subsystem Verification

To verify semantic structures and compile industrial types independently of the broader cluster runtime components, run:

```bash
cargo check -p adapter_opc_ua
```
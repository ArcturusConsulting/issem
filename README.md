![ISSEM Logo](ISSEM.png)
# ISSEM (Interoperative Semantic Synchronization Enabling Module)
## Blasing Fast VDA 5050 / ROS2 / OPC UA Adapter

ISSEM is a centralized, high-performance, stateless server-side gateway written in pure Rust. It bridges the chasm between corporate **Warehouse Execution Systems (WES)**, agile, open-source **Autonomous Mobile Robots (AMRs)**, and physical **Operational Technology (OT) infrastructure** such as elevators, conveyor lines, and automatic doors.

The framework intercepts industrial **VDA5050 JSON payloads over MQTT**, maps high-level coordination paths into localized navigation target frames, and pipes them down to the robot fleet using ultra-lean, sub-millisecond **Zenoh binary streams**. Concurrently, it captures high-frequency telemetry (5 Hz+), manages dynamic license admission leases, and handles physical hardware interlocks (e.g., automated gates) via an externalized **Redis** shared state layer and a built-in, non-blocking **OPC UA dynamic adapter**.

> 💡 **Repository Note:** Looking for the initial proof-of-concept monolithic single-robot pipeline? Check out the historical `prototype` branch. The `main` branch holds the complete multi-crate enterprise fleet workspace.

---

## 1. Value Proposition

In modern industrial logistics hubs (such as third-party logistics networks and heavy manufacturing plants in Japan), enterprise IT systems mandate strict **VDA5050 compliance over MQTT** to eliminate vendor lock-in. Conversely, modern AMRs run on highly dynamic, binary network graphs like **ROS 2 Jazzy and Zenoh/DDS**. 

To meet the strict industrial mandate of **「止まらない現場」 (Tomaranai Gemba — The Floor That Never Stops)**, ISSEM provides a **Centralized K3s Edge Orchestration pattern** designed to serve as a high-availability, low-footprint alternative to massive multi-vendor frameworks (like Open-RMF) in environments where the WES commands the automation layer directly.

### Core Architecture Enhancements

* **Off-Robot Stateless Compute:** Pulls protocol serialization overhead off the physical vehicles. AMRs communicate via lean, native binary streams over the air, saving edge CPU and battery capacity.
* **Decoupled East/West OT Interlocking (OPC UA Gateway):** Incorporates a dedicated, non-blocking `adapter_opc_ua` subsystem. This acts as a highly resilient, stateless physical translation layer, executing real-time physical handshakes (e.g., opening high-speed doors, querying elevator states, triggering conveyor interlocks) without overloading the core asynchronous task loop or violating IT/OT security boundaries.
* **Automated High Availability (HA) via K3s:** Eliminates Single Points of Failure (SPOF). Compute logic is entirely decoupled from memory blocks. If a physical edge server node suffers a hardware fault, the K3s cluster automatically migrates the ISSEM pod to a surviving node in seconds. The pod instantly reconnects to the persistent Redis storage layer, resuming fleet navigation context with zero data loss.
* **DDS Bottleneck Mitigation:** Bypasses deep ROS 2 service/action queue constraints (such as the 31-byte Fast-DDS history allocation limit) by executing a stateful topic-level preemption engine for instant overrides (`pause`, `resume`, `cancelOrder`).

### Target Deployment Profiles

* **Single-Vendor AMR Fleets:** Operations deploying between 10 to 50 custom ROS 2/Nav2 mobile platforms on open warehouse floors where heavy spatial traffic deconfliction frameworks represent massive operational and computational overkill.
* **Legacy Corporate Integrations:** Facilities governed by strict enterprise IT compliance mandates where the top-level orchestrator is a traditional commercial WES (e.g., SAP EWM, Siemens Logistics, Daifuku, or Swisslog) that requires direct VDA5050 compliance out-of-the-box.
* **Hardware-Constrained Robotics Teams:** Teams building highly optimized AMRs that need to preserve 100% of their onboard edge computing power for localized navigation and computer vision tasks rather than parsing heavy corporate JSON payloads over volatile factory Wi-Fi networks.

---

## 2. System Architecture & Data Flows

ISSEM functions as the centralized multi-tenant traffic router deployed on a localized K3s server cluster on the warehouse floor.

```text
  ┌────────────────────────────────────────────────────────┐
  │                 ENTERPRISE NETWORK ZONE                │
  │  WES / ERP (VDA5050 JSON) ──► MQTT Broker (EMQX)       │
  └───────────────────────────┬────────────────────────────┘
                              │ TCP Port 1883 / 8883
  ┌───────────────────────────▼────────────────────────────┐      ┌──────────────────────────┐
  │              ISSEM K3S EDGE ORCHESTRATION STACK        │      │    PHYSICAL PLC ZONE     │
  │                                                        │      │ (Elevators, Doors, etc.) │
  │   ┌─────────────────────────┐    IPC   ┌────────────┐  │      │                          │
  │   │     issem-core Pod      ├─────────►│ Redis Pod  │  │      │   ┌──────────────────┐   │
  │   │ ┌─────────────────────┐ │◄─────────┤ (AOF Sync) │  │      │   │   Omron/Siemens  │   │
  │   │ │  adapter_opc_ua     ├─┼──────────┼────────────┼──┼─────►│   │   OPC UA Server  │   │
  │   │ └─────────────────────┘ │          └────────────┘  │      │   └──────────────────┘   │
  │   └─────────────┬───────────┘                          │      └──────────────────────────┘
  └─────────────────┼──────────────────────────────────────┘        
                    │ Zenoh Binary Protocol (Wi-Fi)                 
                    │                                               
  ┌─────────────────▼──────────────────────────────────────┐        
  │               LOCAL ROBOTICS EXECUTION ZONE            │        
  │                                                        │        
  │     ┌───────────────────────┐DDS ┌────────────┐        │        
  │     │ Zenoh-DDS Edge Bridge ├───►│ Nav2 Stack │        │        
  │     └───────────────────────┘    └────────────┘        │        
  │                      [ ROS2 robots ]                   │        
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
   theta = atan2(2.0 * (q_w * q_z + q_x * q_y), 1.0 - 2.0 * (q_y * q_y + q_z * q_z))
3. **Cache Sync:** The active location coordinates (x, y, theta) are updated in the Redis cluster using high-speed key-value overwrites.
4. **Throttled State Generation:** A background loop collects the current pose from Redis at a stabilized, throttled rate of 5 Hz, pairs it with battery and system metrics, builds a compliant VDA5050 state JSON structure, and publishes it back up to the enterprise MQTT broker.

#### C. East/West Facility Interlocking (WES ──► ISSEM ──► PLC)
1. **Action Intercept:** When an incoming VDA 5050 order specifies a localized physical action attached to a node (such as `"actionType": "clearHighSpeedDoor"` with `"door_id": "Door_A1"`), the MQTT gateway parses the action payload.
2. **Core Suspension:** The core orchestrator intercepts the event, temporarily caches the robot's navigation progress, and sends an internal `PeripheralRequest` over an asynchronous Tokio MPSC channel.
3. **Dynamic Node Resolution:** The `adapter_opc_ua` module parses the request, references the local `deploy/opc_ua_mapping.json` layout configuration, and dynamically maps the logical target name (`"Door_A1"`) to the physical on-premise PLC register (`ns=2;s=DB10.Door_Control.Door_A1`).
4. **Physical Handshake:** The adapter writes `true` directly to the PLC register. It waits asynchronously for the verification signal from the hardware, safely coordinates with the robot's driving thread via a Tokio oneshot channel, and releases the AMR once physical clearance is secured.

---

## 3. Deep Robotics Bottleneck Workarounds

### Zero-Distance Preemption
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

### Safe IT/OT Device Decoupling
Because industrial PLC networks must remain separated from enterprise IT layers, ISSEM maintains strict physical and semantic boundaries:
1. **The WES is Hardware-Agnostic:** WES simply issues high-level VDA 5050 string tokens (e.g. `"Door_A1"`) over MQTT.
2. **The Gateway is Strongly Typed:** The gateway acts as a static dictionary wrapper. It isolates the physical registers to a local config file (`opc_ua_mapping.json`), keeping the application 100% stateless while guarding OT networks from arbitrary write commands.

---

## 4. On-Premise Configuration & Local Quickstart

ISSEM relies on localized external files to map physical assets and global application targets.
Download and use the `deploy/` directory.

### Configuration Layouts

#### Config File Path Setting (`deploy/issem-chart/values.yaml`)
Within `hostPaths`, set the correct paths of `configJson` and `opcUaMappingJson`in your local environment.
#### ISSEM Image Selection (`deploy/argo-application.yaml`)
Within `spec`, specify the tag of the image you want to use by setting `targetRevision` and the `value` of `helm`.
#### Global Deployment Configuration (`deploy/config.json`)
The global master config maps application channels, network interfaces, and targets.
#### Industrial PLC Hardware Mapping (`deploy/opc_ua_mapping.json`)
Allows on-site engineers to dynamically update PLC register templates without recompiling the Rust codebase.

---

## 5. Implementation Roadmap & Repository Blueprint

### Workspace Crate Modular Subsystems

To ensure strict separation of concerns and eliminate protocol compile-time interference, ISSEM is architected as a decoupled multi-crate Rust workspace:

* **`issem_core` (Transactional Engine Core):** The engine's transactional brain. It is entirely protocol-agnostic. It consumes internal Rust primitives passed through bounded memory channels and manages state updates via the Redis client abstraction.
* **`adapter_vda5050` (Northbound Enterprise Gateway):** Owns the MQTT connection. Spawns an asynchronous `rumqttc` event loop to manage enterprise broker handshakes, ingests inbound string payloads, and validates structural semantics against the VDA5050 specification.
* **`driver_zenoh_ros2` (Southbound Robotics Driver):** Owns the Zenoh session. Listens to high-speed binary streams, deserializes the CDR bytes (handling nested `[[f64; 6]; 6]` covariance arrays natively, bypassing Serde's standard 32-element array constraint), and handles outgoing waypoint injection.
* **`adapter_opc_ua` (East/West Peripheral Sync):** Hosts a high-speed asynchronous industrial OPC UA client stack to interface with factory PLCs. It handles physical hardware handshakes (e.g., automated safety gates and conveyor lines) before letting an AMR complete a payload handover.

### Redis Schema Design & Shared State Topology
ISSEM structures its key space dynamically using deterministic namespaces keyed by the unique AMR serial number:

```text
amr:fleet:active_serials         -> Set [ "Kashiwa-Robot-001", "Kashiwa-Robot-002" ]
amr:{serialNumber}:pose          -> Hash { "x": "12.45", "y": "-8.21", "theta": "1.57" }
amr:{serialNumber}:active_goal   -> String (Serialized CDR PoseStamped Binary string)
amr:{serialNumber}:lifecycle     -> Hash { "mode": "AUTOMATIC", "paused": "false" }
```

To guarantee absolute durability against unexpected facility power failures, the accompanying container system runs a hybrid, high-frequency logging persistence loop inside the K3s storage volume:

```ini
# redis.conf
appendonly yes
appendfsync everysec
save 300 1
```
---

## 6. Production Deployment & Cluster Installation

This section details the step-by-step setup required to provision a clean enterprise host edge server rack running a standard Linux distribution (e.g., Ubuntu LTS) from absolute scratch.

### 1. Provision the Edge Kubernetes Engine (K3s)
Install the lightweight, production-grade Kubernetes runtime directly onto the host server node. This script automatically configures container runtimes, networking layers, and local storage providers:

```bash
# Download and install K3s
curl -sfL https://get.k3s.io | sh -

# Verify the local node transitions to a 'Ready' state
sudo k3s kubectl get nodes
```

### 2. Deploy the GitOps Controller (Argo CD)
Install the continuous delivery operator inside an isolated management namespace within the cluster:

```bash
# Create the dedicated namespace
sudo k3s kubectl create namespace argocd

# Execute the deployment using the server-side validation flag
sudo k3s kubectl apply --server-side -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml

# Monitor deployment until all pods report a 'Running' status
sudo k3s kubectl get pods -n argocd --watch
```

### 3. Initialize the Master GitOps Control Application
Apply the root application manifest from your deployment folder to initiate the cluster pull engine:

```bash
sudo k3s kubectl apply -f [PATH_TO_THE_DIRECTORY/]deploy/argo-application.yaml
```

### 4. Create Local Trusted HTTPS Connection (Optional but recommended for local UI)

To avoid browser self-signed TLS warnings, generate locally trusted certificates using `mkcert` and inject them into the Argo CD secret store:

```bash
# Install mkcert and local trust store dependencies
sudo apt update && sudo apt install -y libnss3-tools mkcert
mkcert -install

# Generate certificates for localhost
mkcert 127.0.0.1 localhost

# Replace default Argo CD secret with trusted TLS certificate
sudo k3s kubectl delete secret argocd-secret -n argocd --ignore-not-found
sudo k3s kubectl create secret tls argocd-secret -n argocd \
  --cert=127.0.0.1+1.pem \
  --key=127.0.0.1+1-key.pem

# Restart Argo CD server to apply changes
sudo k3s kubectl -n argocd rollout restart deployment argocd-server
sudo k3s kubectl -n argocd rollout status deployment argocd-server
```

### 5. Access the Local Management Console
To monitor application health states visually, retrieve the secure access token and expose the dashboard layout:

```bash
# 1. Retrieve the auto-generated admin password (username: admin)
sudo k3s kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath="{.data.password}" | base64 -d; echo

# 2. Establish a secure port-forward tunnel to access the UI locally
sudo k3s kubectl port-forward svc/argocd-server -n argocd 8080:443
```
Open a browser tab and navigate to `https://localhost:8080`, enter the Username (`admin`) and the retrieved password to view the running container tree.

---

## 7. Useful commands
### Applying changes in argo-application.yaml
```bash
# 1. Apply the updated manifest
sudo k3s kubectl apply -f [PATH_TO_THE_DIRECTORY/]deploy/argo-application.yaml

# 2. Tell Argo CD to refresh instantly (use the app name defined in your metadata.name above)
sudo k3s kubectl patch application issem-gateway -n argocd --type merge \
  -p '{"metadata":{"annotations":{"argocd.argoproj.io/refresh":"hard"}}}'
```

### Checking logs of the container
```bash
sudo k3s kubectl logs -f deployment/issem-gateway -n default --tail=50
```

### Uninstalling k3s (for resetting from scratch)
```bash
# 1. Run the official uninstaller
sudo /usr/local/bin/k3s-uninstall.sh

# 2. Obliterate lingering network interfaces, configurations, and cache directories
sudo rm -rf /etc/rancher /var/lib/rancher /var/lib/kubelet /run/k3s ~/.kube
```

### Inject a Node Failure (Resiliency Drill)
Simulate a catastrophic hardware rack failure by deleting the running application pod mid-transit:
```bash
sudo k3s kubectl delete pod -l app=issem-core
```
*Observe that the cluster controller handles container failover immediately. A fresh instance initializes on an available thread slot, hits the live Redis storage cache, and resumes handling active AMR coordinates within 50 milliseconds with zero loss of execution history.*

---

## 8. Local Sandbox Verification & Verification Loop

To run the complete stateless cloud-native suite in your local sandbox cluster environment, follow the steps below:

### 1. Provision the Cluster Manifests
Apply the declarative configurations to your running K3s engine. This builds your stateless compute pod and orchestrates your durable Redis storage mount:
```bash
sudo k3s kubectl apply -f deploy/
```

### 2. Monitor Cluster Lifecycle
Verify the initialization status of your distributed container pods:
```bash
sudo k3s kubectl get pods
```

### 3. Monitor Live Data Logs
Attach to the application runtime log stream to watch the translation loops:
```bash
sudo k3s kubectl logs -l app=issem-core --follow
```

### 4. Dispatch an Enterprise Command
Inject an industry-compliant VDA5050 `instantAction` payload into your broker network to test the preemption mechanics:
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
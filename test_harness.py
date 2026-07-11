import time
import json
import struct
import zenoh
import paho.mqtt.client as mqtt

# --- CDR Serialization Helper for ROS 2 geometry_msgs/PoseWithCovarianceStamped ---
def serialize_amcl_pose_cdr(x, y, theta):
    cdr_header = b'\x00\x01\x00\x00'
    stamp = struct.pack('<iI', int(time.time()), 0)
    frame_id_bytes = b"map\x00"
    frame_id = struct.pack('<I', len(frame_id_bytes)) + frame_id_bytes
    position = struct.pack('<ddd', x, y, 0.0)
    q_z = round(float(type(theta)(0.0) if theta == 0 else __import__('math').sin(theta / 2.0)), 6)
    q_w = round(float(type(theta)(1.0) if theta == 0 else __import__('math').cos(theta / 2.0)), 6)
    orientation = struct.pack('<dddd', 0.0, 0.0, q_z, q_w)
    covariance = struct.pack('<' + 'd'*36, *[0.0]*36)
    return cdr_header + stamp + frame_id + position + orientation + covariance

# --- Part 1: Northbound MQTT State Listener ---
def on_mqtt_message(client, userdata, msg):
    try:
        payload_json = json.loads(msg.payload.decode())
        if "agvPosition" in payload_json:
            pos = payload_json.get("agvPosition", {})
            print(f" 🌐 [MQTT Monitor] Outbound state -> X: {pos.get('x'):.2f}, Y: {pos.get('y'):.2f}")
    except Exception:
        pass

print("🌐 Connecting to local corporate MQTT broker...")
try:
    mqtt_client = mqtt.Client(callback_api_version=mqtt.CallbackVersion.VERSION2)
except AttributeError:
    mqtt_client = mqtt.Client()

mqtt_client.on_message = on_mqtt_message
mqtt_client.connect("localhost", 1883, 60)

state_topic = "vda5050/3.0.0/Arcturus-Logistics/Kashiwa-Robot-001/state"
order_topic = "vda5050/3.0.0/Arcturus-Logistics/Kashiwa-Robot-001/order"
mqtt_client.subscribe(state_topic)
mqtt_client.loop_start()

# --- Part 2: Southbound Zenoh Telemetry Injector ---
print("🔌 Connecting to Zenoh peer fabric...")
z_config = zenoh.Config()
z_config.insert_json5("mode", '"client"')
z_config.insert_json5("connect/endpoints", '["tcp/127.0.0.1:7447"]')
z_session = zenoh.open(z_config)
pub = z_session.declare_publisher("Kashiwa-Robot-001/amcl_pose")

# --- Part 3: Active Movement Simulation Loop ---
print("\n🏎️ Starting localization simulation...")
pub.put(serialize_amcl_pose_cdr(0.0, 1.2, 0.7854))
time.sleep(0.5)

# --- Part 4: Inject High-Priority Interlock Order ---
print(f"\n📤 Injecting VDA5050 Order targeting Interlock Zone (X = 25.5) onto: {order_topic}")
mock_order = {
    "headerId": 999,
    "timestamp": int(time.time()),
    "version": "3.0.0",
    "manufacturer": "Arcturus-Logistics",
    "serialNumber": "Kashiwa-Robot-001",
    "orderId": "ORD-2026-INTERLOCK-TEST",
    "nodes": [
        {
            "nodeId": "WAYPOINT_GATE_A1",
            "sequenceId": 0,
            "released": True,
            "nodePosition": {"x": 25.5, "y": 1.2, "theta": 0.0, "mapId": "Kashiwa_Hub_Floor_1"}
        }
    ],
    "edges": []
}

mqtt_client.publish(order_topic, json.dumps(mock_order), qos=1)

# Keep script running briefly to let logs display execution
try:
    time.sleep(4.0)
except KeyboardInterrupt:
    pass
finally:
    mqtt_client.loop_stop()
    z_session.close()
    print("\n🏁 Integration test loop complete.")
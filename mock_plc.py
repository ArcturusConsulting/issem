# mock_plc.py
import asyncio
import logging
from asyncua import Server, ua

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("MockPLC")

async def main():
    server = Server()
    await server.init()
    server.set_endpoint("opc.tcp://127.0.0.1:4840/")
    
    # Set up namespace mapping
    uri = "http://issem.mock.plc"
    idx = await server.register_namespace(uri)
    logger.info(f"Registered namespace '{uri}' at index {idx}")

    objects = server.nodes.objects

    # FIXED: Name matches exactly what Rust sends: "DB10.Door_Control.Door_A1"
    door_node_id = ua.NodeId("DB10.Door_Control.Door_A1", idx)
    door_var = await objects.add_variable(door_node_id, "Door_Gate_001", False)
    
    # FIXED: Grant open write access using standard asyncua helpers
    await door_var.set_writable(True)

    # FIXED: Map the conveyor matching what the core sends (e.g. using the asset serial)
    conveyor_node_id = ua.NodeId("DB12.Conveyor_Run.Kashiwa-Robot-001", idx)
    conveyor_var = await objects.add_variable(conveyor_node_id, "Conveyor_Section_001", False)
    await conveyor_var.set_writable(True)

    logger.info("⚡ Mock PLC Server initialized nodes. Starting network loop...")
    async with server:
        while True:
            await asyncio.sleep(1)
            door_val = await door_var.get_value()
            conveyor_val = await conveyor_var.get_value()
            if door_val or conveyor_val:
                logger.info(f"[PLC State Change Detected] Door_A1 Register: {door_val} | Conveyor Register: {conveyor_val}")

if __name__ == "__main__":
    asyncio.run(main())
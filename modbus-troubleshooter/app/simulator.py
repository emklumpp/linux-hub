"""
Lightweight Modbus TCP slave for testing the troubleshooter without OpenPLC.

Serves all four register types pre-seeded with sample data so you can exercise
connection tests and register reads/writes. Pure Python (pymodbus) — no extra
image, no architecture concerns on a Raspberry Pi.

Run standalone:  python simulator.py
Env overrides:   SIM_HOST (default 0.0.0.0), SIM_PORT (default 502)
"""
import logging
import os

from pymodbus.datastore import (
    ModbusSequentialDataBlock,
    ModbusServerContext,
    ModbusSlaveContext,
)
from pymodbus.server import StartTcpServer

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger("modbus-simulator")

BLOCK_SIZE = 200


def build_context() -> ModbusServerContext:
    """Create a single-slave context seeded with recognizable sample values."""
    store = ModbusSlaveContext(
        di=ModbusSequentialDataBlock(0, [i % 2 for i in range(BLOCK_SIZE)]),        # discrete inputs
        co=ModbusSequentialDataBlock(0, [0] * BLOCK_SIZE),                          # coils
        hr=ModbusSequentialDataBlock(0, list(range(BLOCK_SIZE))),                   # holding registers
        ir=ModbusSequentialDataBlock(0, [i * 2 for i in range(BLOCK_SIZE)]),        # input registers
        zero_mode=True,  # address N maps to index N (no off-by-one)
    )
    return ModbusServerContext(slaves=store, single=True)


def main() -> None:
    host = os.getenv("SIM_HOST", "0.0.0.0")
    port = int(os.getenv("SIM_PORT", "502"))
    logger.info(f"Starting Modbus TCP simulator on {host}:{port} "
                f"(hr/ir/di/co blocks of {BLOCK_SIZE})")
    StartTcpServer(context=build_context(), address=(host, port))


if __name__ == "__main__":
    main()

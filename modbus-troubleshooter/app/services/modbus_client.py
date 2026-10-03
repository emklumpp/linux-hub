"""
Modbus Client Service for device communication.

Device configuration is persisted in SQLite; register read/write results are
recorded to the transaction log so they show up in reports and statistics.
"""
import asyncio
import logging
from datetime import datetime
from typing import List, Optional

from pymodbus.client import ModbusSerialClient, ModbusTcpClient

from config import settings
from api.schemas import DeviceConfig, DeviceResponse, RegisterReadResponse, RegisterWriteResponse
from models.database import Device, record_transaction, session_scope

logger = logging.getLogger(__name__)


class ModbusClientService:
    """Service for managing Modbus client connections and operations"""

    def __init__(self):
        logger.info("ModbusClientService initialized")

    # --- Device management (persisted) -------------------------------------

    async def list_devices(self) -> List[DeviceResponse]:
        """List all configured devices."""
        with session_scope() as db:
            rows = db.query(Device).order_by(Device.id).all()
            return [DeviceResponse.model_validate(row) for row in rows]

    async def add_device(self, device: DeviceConfig) -> DeviceResponse:
        """Add a new device configuration."""
        with session_scope() as db:
            row = Device(
                name=device.name,
                host=device.host,
                port=device.port,
                slave_id=device.slave_id,
                protocol=device.protocol,
                description=device.description,
            )
            db.add(row)
            db.flush()
            db.refresh(row)
            logger.info(f"Added device {row.name} with ID {row.id}")
            return DeviceResponse.model_validate(row)

    async def get_device(self, device_id: int) -> Optional[DeviceResponse]:
        """Get device by ID."""
        with session_scope() as db:
            row = db.get(Device, device_id)
            return DeviceResponse.model_validate(row) if row else None

    async def delete_device(self, device_id: int) -> bool:
        """Delete a device configuration."""
        with session_scope() as db:
            row = db.get(Device, device_id)
            if not row:
                return False
            db.delete(row)
            logger.info(f"Deleted device ID {device_id}")
            return True

    # --- Modbus client factory ---------------------------------------------

    def _create_client(self, host: str, port: int, protocol: str = "tcp"):
        """Create a Modbus client based on protocol."""
        if protocol == "tcp":
            return ModbusTcpClient(
                host=host,
                port=port,
                timeout=settings.MODBUS_TIMEOUT,
                retries=settings.MODBUS_RETRY_COUNT,
            )
        elif protocol == "rtu":
            return ModbusSerialClient(
                port=settings.SERIAL_PORT,
                baudrate=settings.SERIAL_BAUDRATE,
                bytesize=settings.SERIAL_BYTESIZE,
                parity=settings.SERIAL_PARITY,
                stopbits=settings.SERIAL_STOPBITS,
                timeout=settings.MODBUS_TIMEOUT,
            )
        raise ValueError(f"Unsupported protocol: {protocol}")

    # --- Register operations -----------------------------------------------

    async def read_registers(
        self,
        host: str,
        port: int,
        slave_id: int,
        register_type: str,
        start_address: int,
        count: int,
    ) -> RegisterReadResponse:
        """Read registers from a Modbus device."""
        start_time = datetime.now()
        client = None

        try:
            client = self._create_client(host, port)
            loop = asyncio.get_event_loop()
            connected = await loop.run_in_executor(None, client.connect)

            if not connected:
                return self._read_failure(
                    host, port, register_type, start_address, count,
                    f"Failed to connect to {host}:{port}", 0,
                )

            readers = {
                "coil": lambda: client.read_coils(start_address, count, slave=slave_id),
                "discrete_input": lambda: client.read_discrete_inputs(start_address, count, slave=slave_id),
                "holding_register": lambda: client.read_holding_registers(start_address, count, slave=slave_id),
                "input_register": lambda: client.read_input_registers(start_address, count, slave=slave_id),
            }
            if register_type not in readers:
                return self._read_failure(
                    host, port, register_type, start_address, count,
                    f"Unsupported register type: {register_type}", 0,
                )

            result = await loop.run_in_executor(None, readers[register_type])
            response_time = self._elapsed_ms(start_time)

            if result.isError():
                return self._read_failure(
                    host, port, register_type, start_address, count,
                    f"Modbus error: {result}", response_time,
                )

            if register_type in ("coil", "discrete_input"):
                values = [int(v) for v in result.bits[:count]]
            else:
                values = [int(v) for v in result.registers]

            record_transaction(
                operation=f"read_{register_type}", success=True, host=host, port=port,
                response_time_ms=response_time,
                details={"start_address": start_address, "count": count},
            )
            return RegisterReadResponse(
                success=True,
                message="Registers read successfully",
                register_type=register_type,
                start_address=start_address,
                count=count,
                values=values,
                timestamp=datetime.now(),
                response_time_ms=response_time,
            )

        except Exception as e:
            logger.error(f"Error reading registers: {e}")
            return self._read_failure(
                host, port, register_type, start_address, count,
                f"Exception: {e}", self._elapsed_ms(start_time),
            )
        finally:
            if client:
                client.close()

    async def write_registers(
        self,
        host: str,
        port: int,
        slave_id: int,
        register_type: str,
        start_address: int,
        values: List[int],
    ) -> RegisterWriteResponse:
        """Write values to Modbus registers."""
        start_time = datetime.now()
        client = None

        try:
            client = self._create_client(host, port)
            loop = asyncio.get_event_loop()
            connected = await loop.run_in_executor(None, client.connect)

            if not connected:
                return self._write_failure(
                    host, port, register_type, start_address,
                    f"Failed to connect to {host}:{port}", 0,
                )

            if register_type == "coil":
                if len(values) == 1:
                    op = lambda: client.write_coil(start_address, bool(values[0]), slave=slave_id)
                else:
                    op = lambda: client.write_coils(start_address, [bool(v) for v in values], slave=slave_id)
            elif register_type == "holding_register":
                if len(values) == 1:
                    op = lambda: client.write_register(start_address, values[0], slave=slave_id)
                else:
                    op = lambda: client.write_registers(start_address, values, slave=slave_id)
            else:
                return self._write_failure(
                    host, port, register_type, start_address,
                    f"Unsupported register type: {register_type}", 0,
                )

            result = await loop.run_in_executor(None, op)
            response_time = self._elapsed_ms(start_time)

            if result.isError():
                return self._write_failure(
                    host, port, register_type, start_address,
                    f"Modbus error: {result}", response_time,
                )

            record_transaction(
                operation=f"write_{register_type}", success=True, host=host, port=port,
                response_time_ms=response_time,
                details={"start_address": start_address, "values_written": len(values)},
            )
            return RegisterWriteResponse(
                success=True,
                message="Registers written successfully",
                register_type=register_type,
                start_address=start_address,
                values_written=len(values),
                timestamp=datetime.now(),
                response_time_ms=response_time,
            )

        except Exception as e:
            logger.error(f"Error writing registers: {e}")
            return self._write_failure(
                host, port, register_type, start_address,
                f"Exception: {e}", self._elapsed_ms(start_time),
            )
        finally:
            if client:
                client.close()

    # --- Helpers ------------------------------------------------------------

    @staticmethod
    def _elapsed_ms(start_time: datetime) -> float:
        return (datetime.now() - start_time).total_seconds() * 1000

    def _read_failure(self, host, port, register_type, start_address, count, message, response_time):
        record_transaction(
            operation=f"read_{register_type}", success=False, host=host, port=port,
            response_time_ms=response_time,
            details={"start_address": start_address, "count": count}, error_message=message,
        )
        return RegisterReadResponse(
            success=False,
            message=message,
            register_type=register_type,
            start_address=start_address,
            count=count,
            timestamp=datetime.now(),
            response_time_ms=response_time,
        )

    def _write_failure(self, host, port, register_type, start_address, message, response_time):
        record_transaction(
            operation=f"write_{register_type}", success=False, host=host, port=port,
            response_time_ms=response_time,
            details={"start_address": start_address}, error_message=message,
        )
        return RegisterWriteResponse(
            success=False,
            message=message,
            register_type=register_type,
            start_address=start_address,
            values_written=0,
            timestamp=datetime.now(),
            response_time_ms=response_time,
        )

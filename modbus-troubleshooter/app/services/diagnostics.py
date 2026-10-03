"""
Diagnostic Service for connection testing and reporting.

Connection tests are executed live against the device; all transactions
(tests, reads, writes) are read back from the SQLite transaction log for
reports and statistics.
"""
import asyncio
import json
import logging
from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional

from pymodbus.client import ModbusSerialClient, ModbusTcpClient

from config import settings
from api.schemas import ConnectionTestResponse, DeviceDiagnostic, DiagnosticReport
from models.database import TransactionLog, record_transaction, session_scope

logger = logging.getLogger(__name__)


class DiagnosticService:
    """Service for diagnostic operations and reporting"""

    def __init__(self):
        self.start_time = datetime.now()
        logger.info("DiagnosticService initialized")

    async def test_connection(
        self,
        host: str,
        port: int,
        slave_id: int,
        protocol: str = "tcp",
        timeout: int = 3,
    ) -> ConnectionTestResponse:
        """Test connectivity to a Modbus device."""
        start_time = datetime.now()
        client = None

        try:
            if protocol == "tcp":
                client = ModbusTcpClient(host=host, port=port, timeout=timeout)
            elif protocol == "rtu":
                client = ModbusSerialClient(
                    port=settings.SERIAL_PORT,
                    baudrate=settings.SERIAL_BAUDRATE,
                    timeout=timeout,
                )
            else:
                return ConnectionTestResponse(
                    success=False,
                    message=f"Unsupported protocol: {protocol}",
                    timestamp=datetime.now(),
                )

            loop = asyncio.get_event_loop()
            connected = await loop.run_in_executor(None, client.connect)

            if not connected:
                return self._test_failure(
                    host, port, slave_id, protocol,
                    f"Failed to connect to {host}:{port}", None,
                )

            # Read a register to verify two-way communication.
            result = await loop.run_in_executor(
                None,
                lambda: client.read_holding_registers(0, 1, slave=slave_id),
            )
            response_time = self._elapsed_ms(start_time)

            if result.isError():
                return self._test_failure(
                    host, port, slave_id, protocol,
                    f"Connected but communication failed: {result}", response_time,
                )

            record_transaction(
                operation="connection_test", success=True, host=host, port=port,
                response_time_ms=response_time,
                details={"slave_id": slave_id, "protocol": protocol},
            )
            return ConnectionTestResponse(
                success=True,
                message=f"Successfully connected to {host}:{port}",
                response_time_ms=response_time,
                timestamp=datetime.now(),
                details={
                    "host": host,
                    "port": port,
                    "slave_id": slave_id,
                    "protocol": protocol,
                    "test_read_successful": True,
                },
            )

        except Exception as e:
            logger.error(f"Connection test error: {e}")
            return self._test_failure(
                host, port, slave_id, protocol,
                f"Connection test failed: {e}", self._elapsed_ms(start_time),
            )
        finally:
            if client:
                client.close()

    async def generate_report(self, device_id: Optional[int] = None) -> DiagnosticReport:
        """Generate a comprehensive diagnostic report for the last 24 hours."""
        now = datetime.now()
        period_start = now - timedelta(hours=24)

        # Select plain column values so the rows stay usable after the
        # session closes (ORM instances would detach and raise on access).
        with session_scope() as db:
            rows = (
                db.query(
                    TransactionLog.host,
                    TransactionLog.port,
                    TransactionLog.success,
                    TransactionLog.response_time_ms,
                    TransactionLog.error_message,
                )
                .filter(TransactionLog.timestamp >= period_start)
                .all()
            )

        total_transactions = len(rows)
        total_errors = sum(1 for r in rows if not r.success)

        # Group by device (host:port)
        device_stats: Dict[str, Dict[str, Any]] = {}
        for r in rows:
            device_key = f"{r.host or 'unknown'}:{r.port or 0}"
            stats = device_stats.setdefault(
                device_key,
                {"total": 0, "successful": 0, "failed": 0, "response_times": [], "errors": []},
            )
            stats["total"] += 1
            if r.success:
                stats["successful"] += 1
            else:
                stats["failed"] += 1
                if r.error_message:
                    stats["errors"].append(r.error_message)
            if r.response_time_ms is not None:
                stats["response_times"].append(r.response_time_ms)

        devices = []
        for idx, (device_key, stats) in enumerate(device_stats.items(), 1):
            success_rate = (stats["successful"] / stats["total"] * 100) if stats["total"] else 0
            avg_response = (
                sum(stats["response_times"]) / len(stats["response_times"])
                if stats["response_times"] else 0
            )
            devices.append(DeviceDiagnostic(
                device_id=idx,
                device_name=device_key,
                status="online" if success_rate > 50 else "offline",
                last_seen=now if stats["successful"] > 0 else None,
                total_requests=stats["total"],
                successful_requests=stats["successful"],
                failed_requests=stats["failed"],
                success_rate=success_rate,
                avg_response_time_ms=avg_response,
                errors=list(set(stats["errors"]))[:5],
            ))

        online_count = sum(1 for d in devices if d.status == "online")
        summary = (
            f"Diagnostic report for the last 24 hours. "
            f"{online_count} of {len(devices)} devices online. "
            f"{total_transactions} total transactions with {total_errors} errors."
        )

        return DiagnosticReport(
            generated_at=now,
            period_start=period_start,
            period_end=now,
            total_devices=len(devices),
            online_devices=online_count,
            offline_devices=len(devices) - online_count,
            total_transactions=total_transactions,
            total_errors=total_errors,
            devices=devices,
            summary=summary,
        )

    async def get_logs(self, limit: int = 100, offset: int = 0) -> List[Dict[str, Any]]:
        """Retrieve transaction logs, newest first."""
        with session_scope() as db:
            rows = (
                db.query(TransactionLog)
                .order_by(TransactionLog.timestamp.desc())
                .offset(offset)
                .limit(limit)
                .all()
            )
            return [self._log_to_dict(r) for r in rows]

    async def get_statistics(self) -> Dict[str, Any]:
        """Get overall application statistics."""
        now = datetime.now()
        last_24h = now - timedelta(hours=24)

        with session_scope() as db:
            from models.database import Device

            total_transactions = db.query(TransactionLog).count()
            total_errors = (
                db.query(TransactionLog).filter(TransactionLog.success.is_(False)).count()
            )
            last_24h_transactions = (
                db.query(TransactionLog).filter(TransactionLog.timestamp >= last_24h).count()
            )
            last_24h_errors = (
                db.query(TransactionLog)
                .filter(TransactionLog.timestamp >= last_24h, TransactionLog.success.is_(False))
                .count()
            )
            response_times = [
                rt for (rt,) in db.query(TransactionLog.response_time_ms)
                .filter(TransactionLog.response_time_ms.isnot(None)).all()
            ]
            total_devices = db.query(Device).count()

        success_rate = (
            (total_transactions - total_errors) / total_transactions * 100
            if total_transactions else 0
        )
        avg_response = sum(response_times) / len(response_times) if response_times else 0

        return {
            "total_devices": total_devices,
            "total_transactions": total_transactions,
            "total_errors": total_errors,
            "success_rate": success_rate,
            "avg_response_time_ms": avg_response,
            "uptime_seconds": int((now - self.start_time).total_seconds()),
            "last_24h_transactions": last_24h_transactions,
            "last_24h_errors": last_24h_errors,
            "service_start_time": self.start_time.isoformat(),
        }

    # --- Helpers ------------------------------------------------------------

    @staticmethod
    def _elapsed_ms(start_time: datetime) -> float:
        return (datetime.now() - start_time).total_seconds() * 1000

    @staticmethod
    def _log_to_dict(row: TransactionLog) -> Dict[str, Any]:
        return {
            "id": row.id,
            "timestamp": row.timestamp.isoformat() if row.timestamp else None,
            "device_id": row.device_id,
            "host": row.host,
            "port": row.port,
            "operation": row.operation,
            "success": row.success,
            "response_time_ms": row.response_time_ms,
            "details": json.loads(row.details) if row.details else None,
            "error_message": row.error_message,
        }

    def _test_failure(self, host, port, slave_id, protocol, message, response_time):
        record_transaction(
            operation="connection_test", success=False, host=host, port=port,
            response_time_ms=response_time,
            details={"slave_id": slave_id, "protocol": protocol}, error_message=message,
        )
        return ConnectionTestResponse(
            success=False,
            message=message,
            response_time_ms=response_time,
            timestamp=datetime.now(),
            details={"host": host, "port": port, "slave_id": slave_id, "protocol": protocol},
        )

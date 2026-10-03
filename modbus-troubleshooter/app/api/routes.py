"""
API routes for the troubleshooting tool
"""
from fastapi import APIRouter, HTTPException, Depends
from typing import List, Optional
import logging

from .schemas import (
    DeviceConfig,
    DeviceResponse,
    ConnectionTestRequest,
    ConnectionTestResponse,
    RegisterReadRequest,
    RegisterReadResponse,
    RegisterWriteRequest,
    RegisterWriteResponse,
    DiagnosticReport
)
from services.modbus_client import ModbusClientService
from services.plc_interface import PLCInterface
from services.diagnostics import DiagnosticService

logger = logging.getLogger(__name__)
router = APIRouter()

# Service instances (in production, use dependency injection)
modbus_service = ModbusClientService()
plc_service = PLCInterface()
diagnostic_service = DiagnosticService()


# Device Management Endpoints
@router.get("/devices", response_model=List[DeviceResponse])
async def list_devices():
    """List all configured devices"""
    try:
        devices = await modbus_service.list_devices()
        return devices
    except Exception as e:
        logger.error(f"Error listing devices: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/devices", response_model=DeviceResponse)
async def add_device(device: DeviceConfig):
    """Add a new device configuration"""
    try:
        new_device = await modbus_service.add_device(device)
        return new_device
    except Exception as e:
        logger.error(f"Error adding device: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/devices/{device_id}", response_model=DeviceResponse)
async def get_device(device_id: int):
    """Get device details by ID"""
    try:
        device = await modbus_service.get_device(device_id)
        if not device:
            raise HTTPException(status_code=404, detail="Device not found")
        return device
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting device: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.delete("/devices/{device_id}")
async def delete_device(device_id: int):
    """Delete a device configuration"""
    try:
        success = await modbus_service.delete_device(device_id)
        if not success:
            raise HTTPException(status_code=404, detail="Device not found")
        return {"message": "Device deleted successfully"}
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error deleting device: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# Connection Testing Endpoints
@router.post("/test/connection", response_model=ConnectionTestResponse)
async def test_connection(request: ConnectionTestRequest):
    """Test connectivity to a Modbus device"""
    try:
        result = await diagnostic_service.test_connection(
            host=request.host,
            port=request.port,
            slave_id=request.slave_id,
            protocol=request.protocol,
            timeout=request.timeout
        )
        return result
    except Exception as e:
        logger.error(f"Error testing connection: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# Register Operations
@router.post("/registers/read", response_model=RegisterReadResponse)
async def read_registers(request: RegisterReadRequest):
    """Read registers from a Modbus device"""
    try:
        result = await modbus_service.read_registers(
            host=request.host,
            port=request.port,
            slave_id=request.slave_id,
            register_type=request.register_type,
            start_address=request.start_address,
            count=request.count
        )
        return result
    except Exception as e:
        logger.error(f"Error reading registers: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/registers/write", response_model=RegisterWriteResponse)
async def write_registers(request: RegisterWriteRequest):
    """Write values to Modbus registers"""
    try:
        result = await modbus_service.write_registers(
            host=request.host,
            port=request.port,
            slave_id=request.slave_id,
            register_type=request.register_type,
            start_address=request.start_address,
            values=request.values
        )
        return result
    except Exception as e:
        logger.error(f"Error writing registers: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# PLC Integration
@router.get("/plc/status")
async def get_plc_status():
    """Get OpenPLC runtime status"""
    try:
        status = await plc_service.get_status()
        return status
    except Exception as e:
        logger.error(f"Error getting PLC status: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/plc/start")
async def start_plc():
    """Start OpenPLC runtime"""
    try:
        result = await plc_service.start()
        return {"message": "PLC started successfully", "result": result}
    except Exception as e:
        logger.error(f"Error starting PLC: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/plc/stop")
async def stop_plc():
    """Stop OpenPLC runtime"""
    try:
        result = await plc_service.stop()
        return {"message": "PLC stopped successfully", "result": result}
    except Exception as e:
        logger.error(f"Error stopping PLC: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# Diagnostics and Reporting
@router.get("/diagnostics/report", response_model=DiagnosticReport)
async def generate_diagnostic_report(device_id: Optional[int] = None):
    """Generate a comprehensive diagnostic report"""
    try:
        report = await diagnostic_service.generate_report(device_id)
        return report
    except Exception as e:
        logger.error(f"Error generating diagnostic report: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/logs")
async def get_logs(limit: int = 100, offset: int = 0):
    """Retrieve transaction logs"""
    try:
        logs = await diagnostic_service.get_logs(limit=limit, offset=offset)
        return {"logs": logs, "total": len(logs)}
    except Exception as e:
        logger.error(f"Error retrieving logs: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/stats")
async def get_statistics():
    """Get application statistics"""
    try:
        stats = await diagnostic_service.get_statistics()
        return stats
    except Exception as e:
        logger.error(f"Error getting statistics: {e}")
        raise HTTPException(status_code=500, detail=str(e))

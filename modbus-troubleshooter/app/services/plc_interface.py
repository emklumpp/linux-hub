"""
OpenPLC Interface Service
"""
import asyncio
import httpx
import logging
from typing import Dict, Any

from config import settings

logger = logging.getLogger(__name__)


class PLCInterface:
    """Service for interfacing with OpenPLC runtime"""
    
    def __init__(self):
        self.base_url = f"http://{settings.OPENPLC_HOST}:{settings.OPENPLC_WEB_PORT}"
        self.modbus_url = f"{settings.OPENPLC_HOST}:{settings.OPENPLC_PORT}"
        logger.info(f"PLCInterface initialized for {self.base_url}")
    
    async def get_status(self) -> Dict[str, Any]:
        """Get OpenPLC runtime status"""
        try:
            async with httpx.AsyncClient(timeout=settings.OPENPLC_TIMEOUT) as client:
                # Try to ping the OpenPLC web interface
                response = await client.get(f"{self.base_url}/")
                
                return {
                    "online": response.status_code == 200,
                    "web_interface": self.base_url,
                    "modbus_endpoint": self.modbus_url,
                    "status": "running" if response.status_code == 200 else "offline"
                }
        except Exception as e:
            logger.error(f"Error getting PLC status: {e}")
            return {
                "online": False,
                "web_interface": self.base_url,
                "modbus_endpoint": self.modbus_url,
                "status": "offline",
                "error": str(e)
            }
    
    async def start(self) -> Dict[str, Any]:
        """Start OpenPLC runtime (if supported by API)"""
        try:
            async with httpx.AsyncClient(timeout=settings.OPENPLC_TIMEOUT) as client:
                # Note: OpenPLC v3 may require authentication
                # This is a placeholder - adjust based on your OpenPLC setup
                response = await client.post(f"{self.base_url}/start-plc")
                
                return {
                    "success": response.status_code == 200,
                    "message": "PLC start command sent",
                    "status_code": response.status_code
                }
        except Exception as e:
            logger.error(f"Error starting PLC: {e}")
            return {
                "success": False,
                "message": f"Error: {str(e)}"
            }
    
    async def stop(self) -> Dict[str, Any]:
        """Stop OpenPLC runtime (if supported by API)"""
        try:
            async with httpx.AsyncClient(timeout=settings.OPENPLC_TIMEOUT) as client:
                # Note: OpenPLC v3 may require authentication
                # This is a placeholder - adjust based on your OpenPLC setup
                response = await client.post(f"{self.base_url}/stop-plc")
                
                return {
                    "success": response.status_code == 200,
                    "message": "PLC stop command sent",
                    "status_code": response.status_code
                }
        except Exception as e:
            logger.error(f"Error stopping PLC: {e}")
            return {
                "success": False,
                "message": f"Error: {str(e)}"
            }
    
    async def read_plc_registers(self, start_address: int, count: int) -> Dict[str, Any]:
        """Read registers from OpenPLC via Modbus"""
        from services.modbus_client import ModbusClientService
        
        modbus_service = ModbusClientService()
        result = await modbus_service.read_registers(
            host=settings.OPENPLC_HOST,
            port=settings.OPENPLC_PORT,
            slave_id=1,
            register_type="holding_register",
            start_address=start_address,
            count=count
        )
        
        return {
            "success": result.success,
            "values": result.values,
            "message": result.message
        }

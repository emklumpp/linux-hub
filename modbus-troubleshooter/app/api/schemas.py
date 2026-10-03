"""
Pydantic schemas for request/response validation
"""
from pydantic import BaseModel, ConfigDict, Field, field_validator
from typing import List, Optional, Literal
from datetime import datetime


# Device Configuration
class DeviceConfig(BaseModel):
    name: str = Field(..., description="Device name")
    host: str = Field(..., description="Device IP address or hostname")
    port: int = Field(default=502, ge=1, le=65535, description="Modbus TCP port")
    slave_id: int = Field(default=1, ge=0, le=247, description="Modbus slave ID")
    protocol: Literal["tcp", "rtu"] = Field(default="tcp", description="Protocol type")
    description: Optional[str] = Field(None, description="Device description")


class DeviceResponse(DeviceConfig):
    id: int
    created_at: datetime
    last_tested: Optional[datetime] = None
    status: str = "unknown"  # online, offline, unknown

    model_config = ConfigDict(from_attributes=True)


# Connection Testing
class ConnectionTestRequest(BaseModel):
    host: str
    port: int = 502
    slave_id: int = 1
    protocol: Literal["tcp", "rtu"] = "tcp"
    timeout: int = Field(default=3, ge=1, le=30)


class ConnectionTestResponse(BaseModel):
    success: bool
    message: str
    response_time_ms: Optional[float] = None
    timestamp: datetime
    details: Optional[dict] = None


# Register Operations
class RegisterReadRequest(BaseModel):
    host: str
    port: int = 502
    slave_id: int = 1
    register_type: Literal["coil", "discrete_input", "holding_register", "input_register"]
    start_address: int = Field(..., ge=0, le=65535)
    count: int = Field(..., ge=1, le=125)
    
    @field_validator('count')
    @classmethod
    def validate_count(cls, v, info):
        """Validate count based on register type"""
        register_type = info.data.get('register_type')
        if register_type in ['coil', 'discrete_input'] and v > 2000:
            raise ValueError('Maximum 2000 coils/discrete inputs per read')
        elif register_type in ['holding_register', 'input_register'] and v > 125:
            raise ValueError('Maximum 125 registers per read')
        return v


class RegisterReadResponse(BaseModel):
    success: bool
    message: str
    register_type: str
    start_address: int
    count: int
    values: Optional[List[int]] = None
    timestamp: datetime
    response_time_ms: float


class RegisterWriteRequest(BaseModel):
    host: str
    port: int = 502
    slave_id: int = 1
    register_type: Literal["coil", "holding_register"]
    start_address: int = Field(..., ge=0, le=65535)
    values: List[int]
    
    @field_validator('values')
    @classmethod
    def validate_values(cls, v, info):
        """Validate values based on register type"""
        register_type = info.data.get('register_type')
        if register_type == 'coil':
            if not all(val in [0, 1] for val in v):
                raise ValueError('Coil values must be 0 or 1')
        elif register_type == 'holding_register':
            if not all(0 <= val <= 65535 for val in v):
                raise ValueError('Register values must be between 0 and 65535')
        return v


class RegisterWriteResponse(BaseModel):
    success: bool
    message: str
    register_type: str
    start_address: int
    values_written: int
    timestamp: datetime
    response_time_ms: float


# Diagnostic Report
class DeviceDiagnostic(BaseModel):
    device_id: int
    device_name: str
    status: str
    last_seen: Optional[datetime]
    total_requests: int
    successful_requests: int
    failed_requests: int
    success_rate: float
    avg_response_time_ms: float
    errors: List[str] = []


class DiagnosticReport(BaseModel):
    generated_at: datetime
    period_start: datetime
    period_end: datetime
    total_devices: int
    online_devices: int
    offline_devices: int
    total_transactions: int
    total_errors: int
    devices: List[DeviceDiagnostic]
    summary: str


# Logging
class TransactionLog(BaseModel):
    id: int
    timestamp: datetime
    device_id: Optional[int]
    operation: str
    success: bool
    response_time_ms: float
    details: Optional[dict]
    error_message: Optional[str]

    model_config = ConfigDict(from_attributes=True)


# Statistics
class Statistics(BaseModel):
    total_devices: int
    total_transactions: int
    total_errors: int
    success_rate: float
    avg_response_time_ms: float
    uptime_seconds: int
    last_24h_transactions: int
    last_24h_errors: int

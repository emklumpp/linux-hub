"""
Application configuration using Pydantic Settings
"""
from pydantic_settings import BaseSettings, SettingsConfigDict
from typing import List


class Settings(BaseSettings):
    """Application settings"""

    model_config = SettingsConfigDict(
        env_file=".env",
        case_sensitive=True,
        extra="ignore",
    )

    # Application
    APP_NAME: str = "Modbus Troubleshooter"
    VERSION: str = "1.0.0"
    DEBUG: bool = False
    HOST: str = "0.0.0.0"
    PORT: int = 8000
    
    # Logging
    LOG_LEVEL: str = "INFO"
    
    # Database
    DATABASE_URL: str = "sqlite:///data/troubleshooter.db"
    
    # OpenPLC Configuration
    OPENPLC_HOST: str = "openplc"
    OPENPLC_PORT: int = 502
    OPENPLC_WEB_PORT: int = 8080
    OPENPLC_TIMEOUT: int = 10
    
    # Modbus Configuration
    MODBUS_TCP_PORT: int = 502
    MODBUS_TIMEOUT: int = 3
    MODBUS_RETRY_COUNT: int = 3
    MODBUS_BYTE_ORDER: str = "big"  # or "little"
    MODBUS_WORD_ORDER: str = "big"  # or "little"
    
    # Serial/RTU Configuration
    SERIAL_PORT: str = "/dev/ttyUSB0"
    SERIAL_BAUDRATE: int = 9600
    SERIAL_BYTESIZE: int = 8
    SERIAL_PARITY: str = "N"  # N=None, E=Even, O=Odd
    SERIAL_STOPBITS: int = 1
    
    # CORS
    ALLOWED_ORIGINS: List[str] = ["*"]
    
    # Security
    SECRET_KEY: str = "your-secret-key-change-in-production"
    API_KEY_HEADER: str = "X-API-Key"
    
    # Data retention
    LOG_RETENTION_DAYS: int = 30
    MAX_LOG_SIZE_MB: int = 100


# Create global settings instance
settings = Settings()

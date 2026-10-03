"""
Database models, initialization, and session helpers (SQLite).
"""
import json
import logging
from contextlib import contextmanager
from datetime import datetime
from typing import Any, Optional

from sqlalchemy import (
    Boolean,
    Column,
    DateTime,
    Float,
    Integer,
    String,
    Text,
    create_engine,
)
from sqlalchemy.orm import declarative_base, sessionmaker

from config import settings

logger = logging.getLogger(__name__)

Base = declarative_base()


class Device(Base):
    """Device configuration model"""
    __tablename__ = "devices"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String(100), nullable=False)
    host = Column(String(255), nullable=False)
    port = Column(Integer, default=502)
    slave_id = Column(Integer, default=1)
    protocol = Column(String(10), default="tcp")
    description = Column(Text, nullable=True)
    created_at = Column(DateTime, default=datetime.now)
    updated_at = Column(DateTime, default=datetime.now, onupdate=datetime.now)
    last_tested = Column(DateTime, nullable=True)
    status = Column(String(20), default="unknown")


class TransactionLog(Base):
    """Transaction log model"""
    __tablename__ = "transaction_logs"

    id = Column(Integer, primary_key=True, index=True)
    timestamp = Column(DateTime, default=datetime.now, index=True)
    device_id = Column(Integer, nullable=True)
    host = Column(String(255), nullable=True, index=True)
    port = Column(Integer, nullable=True)
    operation = Column(String(50), nullable=False)
    success = Column(Boolean, default=False)
    response_time_ms = Column(Float, nullable=True)
    details = Column(Text, nullable=True)
    error_message = Column(Text, nullable=True)


class DiagnosticSnapshot(Base):
    """Periodic diagnostic snapshots"""
    __tablename__ = "diagnostic_snapshots"

    id = Column(Integer, primary_key=True, index=True)
    timestamp = Column(DateTime, default=datetime.now, index=True)
    total_devices = Column(Integer, default=0)
    online_devices = Column(Integer, default=0)
    total_transactions = Column(Integer, default=0)
    total_errors = Column(Integer, default=0)
    success_rate = Column(Float, default=0.0)
    avg_response_time_ms = Column(Float, default=0.0)


# Engine + session factory
engine = create_engine(
    settings.DATABASE_URL,
    connect_args={"check_same_thread": False} if "sqlite" in settings.DATABASE_URL else {},
)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


async def init_db():
    """Create database tables if they don't exist."""
    try:
        Base.metadata.create_all(bind=engine)
        logger.info("Database tables ready")
    except Exception as e:
        logger.error(f"Error creating database tables: {e}")
        raise


def get_db():
    """FastAPI dependency that yields a database session."""
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


@contextmanager
def session_scope():
    """Provide a transactional scope around a series of operations."""
    db = SessionLocal()
    try:
        yield db
        db.commit()
    except Exception:
        db.rollback()
        raise
    finally:
        db.close()


def record_transaction(
    operation: str,
    success: bool,
    host: Optional[str] = None,
    port: Optional[int] = None,
    device_id: Optional[int] = None,
    response_time_ms: Optional[float] = None,
    details: Optional[dict] = None,
    error_message: Optional[str] = None,
) -> None:
    """Persist a single transaction log entry."""
    try:
        with session_scope() as db:
            db.add(TransactionLog(
                device_id=device_id,
                host=host,
                port=port,
                operation=operation,
                success=success,
                response_time_ms=response_time_ms,
                details=json.dumps(details) if details is not None else None,
                error_message=error_message,
            ))
    except Exception as e:
        # Logging a transaction must never break the request that triggered it.
        logger.error(f"Failed to record transaction: {e}")

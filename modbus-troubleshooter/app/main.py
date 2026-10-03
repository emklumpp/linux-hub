"""
Main application entry point for Modbus/PLC Troubleshooting Tool
"""
from contextlib import asynccontextmanager
import logging

from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse
from fastapi.templating import Jinja2Templates
from fastapi.middleware.cors import CORSMiddleware

from api.routes import router as api_router
from config import settings
from models.database import init_db

# Configure logging
logging.basicConfig(
    level=settings.LOG_LEVEL,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application startup/shutdown lifecycle."""
    logger.info("Starting Modbus/PLC Troubleshooting Tool...")
    await init_db()
    logger.info(f"Application started on {settings.HOST}:{settings.PORT}")
    yield
    logger.info("Shutting down application...")


# Initialize FastAPI app
app = FastAPI(
    title="Modbus/PLC Troubleshooting Tool",
    description="A comprehensive tool for diagnosing Modbus and PLC communication issues",
    version="1.0.0",
    docs_url="/docs",
    redoc_url="/redoc",
    lifespan=lifespan,
)

# CORS middleware. Credentials are disabled so a wildcard origin is valid;
# lock ALLOWED_ORIGINS down to specific hosts if you enable credentials.
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.ALLOWED_ORIGINS,
    allow_credentials=False,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Templates
templates = Jinja2Templates(directory="templates")

# Include API routes
app.include_router(api_router, prefix="/api")


@app.get("/", response_class=HTMLResponse)
async def root(request: Request):
    """Root endpoint - renders main UI"""
    return templates.TemplateResponse(
        "index.html",
        {"request": request, "title": "Modbus Troubleshooter"}
    )


@app.get("/dashboard", response_class=HTMLResponse)
async def dashboard(request: Request):
    """Full diagnostic dashboard UI"""
    return templates.TemplateResponse(
        "dashboard.html",
        {"request": request, "title": "Modbus Troubleshooter"}
    )


@app.get("/health")
async def health_check():
    """Health check endpoint for Docker"""
    return {
        "status": "healthy",
        "service": "modbus-troubleshooter",
        "version": "1.0.0"
    }


@app.get("/info")
async def info():
    """Application information"""
    return {
        "name": "Modbus/PLC Troubleshooting Tool",
        "version": "1.0.0",
        "description": "Diagnostic tool for Modbus and PLC communications",
        "features": [
            "Modbus TCP/RTU support",
            "OpenPLC integration",
            "Real-time diagnostics",
            "Connection testing",
            "Register read/write operations",
            "Historical logging"
        ]
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=settings.HOST,
        port=settings.PORT,
        reload=settings.DEBUG
    )

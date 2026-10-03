#!/bin/bash

# Modbus Troubleshooting Tool Setup Script
# For Raspberry Pi and ARM devices

set -e

echo "================================================"
echo "Modbus/PLC Troubleshooting Tool Setup"
echo "================================================"
echo ""

# Check if running on Raspberry Pi
if [[ $(uname -m) =~ ^(arm|aarch64) ]]; then
    echo "✓ Detected ARM architecture (Raspberry Pi compatible)"
else
    echo "⚠ Warning: Not running on ARM architecture"
    echo "  This script is optimized for Raspberry Pi"
fi

echo ""

# Check for Docker
echo "Checking dependencies..."
if ! command -v docker &> /dev/null; then
    echo "❌ Docker not found. Installing Docker..."
    curl -fsSL https://get.docker.com -o get-docker.sh
    sudo sh get-docker.sh
    sudo usermod -aG docker $USER
    rm get-docker.sh
    echo "✓ Docker installed"
else
    echo "✓ Docker found"
fi

# Check for Docker Compose
if ! command -v docker-compose &> /dev/null; then
    echo "❌ Docker Compose not found. Installing..."
    sudo apt-get update
    sudo apt-get install -y docker-compose
    echo "✓ Docker Compose installed"
else
    echo "✓ Docker Compose found"
fi

echo ""

# Create necessary directories
echo "Creating directories..."
mkdir -p data/logs
mkdir -p openplc/config
mkdir -p app/templates
echo "✓ Directories created"

echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "Creating .env file from template..."
    cp .env.example .env 2>/dev/null || cat > .env << 'EOF'
# Application Configuration
APP_NAME=Modbus Troubleshooter
DEBUG=false
LOG_LEVEL=INFO

# Database
DATABASE_URL=sqlite:///data/troubleshooter.db

# OpenPLC Configuration
OPENPLC_HOST=openplc
OPENPLC_PORT=502
OPENPLC_WEB_PORT=8080

# Modbus Configuration
MODBUS_TIMEOUT=3

# Serial Configuration
SERIAL_PORT=/dev/ttyUSB0
SERIAL_BAUDRATE=9600

# Security
SECRET_KEY=$(openssl rand -hex 32)
EOF
    echo "✓ .env file created"
else
    echo "✓ .env file already exists"
fi

echo ""

# Detect serial ports
echo "Detecting serial ports..."
if ls /dev/ttyUSB* 1> /dev/null 2>&1; then
    echo "Found USB serial ports:"
    ls -l /dev/ttyUSB* | awk '{print "  " $NF}'
    echo ""
    echo "Update SERIAL_PORT in .env if needed"
elif ls /dev/ttyAMA* 1> /dev/null 2>&1; then
    echo "Found Raspberry Pi serial ports:"
    ls -l /dev/ttyAMA* | awk '{print "  " $NF}'
    echo ""
    echo "Update SERIAL_PORT in .env if needed"
else
    echo "⚠ No serial ports detected"
    echo "  Connect your USB-to-Serial adapter and run this script again"
fi

echo ""

# Add user to dialout group for serial access
if groups $USER | grep -q dialout; then
    echo "✓ User already in dialout group"
else
    echo "Adding user to dialout group for serial access..."
    sudo usermod -aG dialout $USER
    echo "✓ User added to dialout group"
    echo "⚠ You need to log out and log back in for this to take effect"
fi

echo ""
echo "================================================"
echo "Setup Complete!"
echo "================================================"
echo ""
echo "Next steps:"
echo "1. Review and edit .env file: nano .env"
echo "2. Start services: docker-compose up -d"
echo "3. Access web UI: http://$(hostname -I | awk '{print $1}'):8000"
echo "4. Access OpenPLC: http://$(hostname -I | awk '{print $1}'):8080"
echo ""
echo "Useful commands:"
echo "  docker-compose up -d      # Start services"
echo "  docker-compose down       # Stop services"
echo "  docker-compose logs -f    # View logs"
echo "  docker-compose ps         # Check status"
echo ""

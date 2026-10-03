#!/usr/bin/env python3
"""
Test script for Modbus connection
Run this to verify your setup is working
"""

import sys
from pymodbus.client import ModbusTcpClient
from pymodbus.exceptions import ModbusException

def test_connection(host="localhost", port=502, slave_id=1):
    """Test Modbus TCP connection"""
    print(f"Testing connection to {host}:{port} (Slave ID: {slave_id})")
    print("-" * 50)
    
    try:
        # Create client
        client = ModbusTcpClient(host=host, port=port, timeout=5)
        
        # Connect
        print("Connecting...")
        if not client.connect():
            print("❌ Failed to connect")
            return False
        
        print("✓ Connected successfully")
        
        # Try to read holding registers
        print("\nReading holding registers 0-9...")
        result = client.read_holding_registers(0, 10, slave=slave_id)
        
        if result.isError():
            print(f"❌ Error reading registers: {result}")
            client.close()
            return False
        
        print(f"✓ Successfully read {len(result.registers)} registers")
        print(f"  Values: {result.registers}")
        
        # Try to read coils
        print("\nReading coils 0-9...")
        result = client.read_coils(0, 10, slave=slave_id)
        
        if result.isError():
            print(f"⚠ Error reading coils: {result}")
        else:
            print(f"✓ Successfully read {len(result.bits[:10])} coils")
            print(f"  Values: {result.bits[:10]}")
        
        # Close connection
        client.close()
        print("\n" + "=" * 50)
        print("✓ Connection test PASSED")
        print("=" * 50)
        return True
        
    except Exception as e:
        print(f"\n❌ Exception occurred: {e}")
        print("=" * 50)
        print("❌ Connection test FAILED")
        print("=" * 50)
        return False

if __name__ == "__main__":
    # Parse command line arguments
    host = sys.argv[1] if len(sys.argv) > 1 else "localhost"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 502
    slave_id = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    
    print("Modbus Connection Test Tool")
    print("=" * 50)
    print(f"Host: {host}")
    print(f"Port: {port}")
    print(f"Slave ID: {slave_id}")
    print()
    
    success = test_connection(host, port, slave_id)
    sys.exit(0 if success else 1)

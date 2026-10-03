#!/bin/bash

# Modbus Scanner Configuration
IP="10.232.20.139"
PORT="5310"
REGISTER="44"
FOUND=0
OUTPUT_FILE="modbus_scan_$(date +%Y%m%d_%H%M%S).txt"

# Create output file with header
cat > "$OUTPUT_FILE" << EOF
Modbus Scanner Results
==================================
IP: $IP
Port: $PORT
Register: $REGISTER
Scan Started: $(date)
==================================

EOF

echo "Modbus Scanner"
echo "=================================="
echo "Output file: $OUTPUT_FILE"
echo ""

# Scan addresses 1-247
for addr in {1..247}; do
    # Run modpoll and capture output
    OUTPUT=$(modpoll -m tcp -a $addr -r $REGISTER -0 -t 4:int -i -p $PORT $IP 2>&1)
    
    # Check if device responded (look for brackets in output)
    if echo "$OUTPUT" | grep -q "\["; then
        ((FOUND++))
        
        echo "[+] Device $FOUND: Address $addr is ACTIVE"
        echo "[+] Device $FOUND: Address $addr is ACTIVE" >> "$OUTPUT_FILE"
        
        # Run detailed query and save to file
        modpoll -m tcp -a $addr -r $REGISTER -0 -t 4:int -i -p $PORT $IP >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo ""
    else
        echo "[ ] Address $addr - No response"
        echo "[ ] Address $addr - No response" >> "$OUTPUT_FILE"
    fi
done

echo ""
echo "=================================="
echo "Scan complete! Found $FOUND device(s)"
echo "Results saved to: $OUTPUT_FILE"
echo "=================================="

# Add summary to file
cat >> "$OUTPUT_FILE" << EOF

==================================
Scan Summary
Total devices found: $FOUND
Scan completed: $(date)
==================================
EOF


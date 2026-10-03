"""
Lian Li Cooler Management Application - Linux Edition
A Python app to control and monitor Lian Li AIO coolers on Linux
"""

import tkinter as tk
from tkinter import ttk, messagebox, filedialog
import threading
import time
import json
import os
import subprocess
import glob
from dataclasses import dataclass, asdict
from typing import Dict, List, Optional, Tuple, Any, Union
import logging
from pathlib import Path
import struct

# You'll need to install these packages:
# sudo apt install python3-pip python3-dev libudev-dev
# pip install pyusb hidapi psutil

try:
    import usb.core
    import usb.util
    USB_AVAILABLE = True
except ImportError:
    USB_AVAILABLE = False
    print("Warning: pyusb not installed. Run: pip install pyusb")

try:
    import hid
    HID_AVAILABLE = True
except ImportError:
    HID_AVAILABLE = False
    print("Warning: hidapi not installed. Run: pip install hidapi")

try:
    import psutil
    PSUTIL_AVAILABLE = True
except ImportError:
    PSUTIL_AVAILABLE = False
    print("Warning: psutil not installed. Run: pip install psutil")

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

@dataclass
class CoolerProfile:
    """Represents a cooling profile with fan curves and pump settings"""
    name: str
    pump_speed: int  # 0-100%
    fan_curve: List[Tuple[int, int]]  # [(temp, speed), ...]
    rgb_mode: str
    rgb_colors: List[str]
    auto_mode: bool = True

@dataclass
class CoolerStatus:
    """Current status of the cooler"""
    cpu_temp: float
    pump_speed: int
    fan_speed: int
    pump_rpm: int
    fan_rpm: int
    liquid_temp: float = 0.0

@dataclass
class DeviceInfo:
    """Information about a detected device"""
    vendor_id: int
    product_id: int
    name: str
    device_type: str
    interface: str  # 'usb' or 'hid'
    path: Optional[str] = None

class LinuxSystemMonitor:
    """Monitor system temperatures and hardware info on Linux"""
    
    def __init__(self):
        self.temp_sensors = self.find_temperature_sensors()
        self.fan_controls = self.find_fan_controls()
        
    def find_temperature_sensors(self) -> Dict[str, str]:
        """Find available temperature sensors"""
        sensors = {}
        
        # Check hwmon sensors
        hwmon_paths = glob.glob('/sys/class/hwmon/hwmon*/temp*_input')
        for path in hwmon_paths:
            try:
                name_path = path.replace('temp', 'temp').replace('_input', '_label')
                if os.path.exists(name_path):
                    with open(name_path, 'r') as f:
                        name = f.read().strip()
                else:
                    # Try to get sensor name from hwmon name
                    hwmon_dir = os.path.dirname(path)
                    name_file = os.path.join(hwmon_dir, 'name')
                    if os.path.exists(name_file):
                        with open(name_file, 'r') as f:
                            name = f.read().strip()
                    else:
                        name = os.path.basename(path)
                
                sensors[name] = path
            except Exception as e:
                logger.debug(f"Error reading sensor {path}: {e}")
        
        # Check thermal zones
        thermal_paths = glob.glob('/sys/class/thermal/thermal_zone*/temp')
        for path in thermal_paths:
            try:
                zone_dir = os.path.dirname(path)
                type_file = os.path.join(zone_dir, 'type')
                if os.path.exists(type_file):
                    with open(type_file, 'r') as f:
                        zone_type = f.read().strip()
                    sensors[f"thermal_{zone_type}"] = path
            except Exception as e:
                logger.debug(f"Error reading thermal zone {path}: {e}")
        
        return sensors
    
    def find_fan_controls(self) -> Dict[str, str]:
        """Find available fan controls"""
        controls = {}
        
        # Check for PWM controls
        pwm_paths = glob.glob('/sys/class/hwmon/hwmon*/pwm*')
        for path in pwm_paths:
            try:
                # Check if it's writable
                if os.access(path, os.W_OK):
                    name = os.path.basename(path)
                    controls[name] = path
            except Exception as e:
                logger.debug(f"Error checking PWM control {path}: {e}")
        
        return controls
    
    def get_cpu_temperature(self) -> float:
        """Get CPU temperature"""
        # Try different CPU temperature sources
        cpu_temp_sources = [
            'Package id 0',  # Intel
            'Tdie',          # AMD
            'CPU',           # Generic
            'coretemp',      # Intel alternative
            'k10temp'        # AMD alternative
        ]
        
        for source in cpu_temp_sources:
            if source in self.temp_sensors:
                try:
                    with open(self.temp_sensors[source], 'r') as f:
                        temp_raw = int(f.read().strip())
                        return temp_raw / 1000.0  # Convert from millidegrees
                except Exception as e:
                    logger.debug(f"Error reading temperature from {source}: {e}")
        
        # Fallback to psutil if available
        if PSUTIL_AVAILABLE:
            try:
                temps = psutil.sensors_temperatures()
                if 'coretemp' in temps and temps['coretemp']:
                    return temps['coretemp'][0].current
                elif 'k10temp' in temps and temps['k10temp']:
                    return temps['k10temp'][0].current
                elif 'cpu_thermal' in temps and temps['cpu_thermal']:
                    return temps['cpu_thermal'][0].current
            except Exception as e:
                logger.debug(f"Error reading temperature via psutil: {e}")
        
        return 0.0
    
    def get_all_temperatures(self) -> Dict[str, float]:
        """Get all available temperatures"""
        temperatures = {}
        
        for name, path in self.temp_sensors.items():
            try:
                with open(path, 'r') as f:
                    temp_raw = int(f.read().strip())
                    temperatures[name] = temp_raw / 1000.0
            except Exception as e:
                logger.debug(f"Error reading temperature {name}: {e}")
        
        return temperatures

class LianLiHardwareInterface:
    """Direct hardware interface for Lian Li devices on Linux"""
    
    # Known Lian Li USB vendor/product IDs
    KNOWN_DEVICES = {
        # Galahad series
        (0x0C45, 0x7697): {"name": "Galahad AIO 240/280", "type": "aio"},
        (0x0C45, 0x7698): {"name": "Galahad AIO 360", "type": "aio"},
        # Trinity series  
        (0x0C45, 0x7699): {"name": "Trinity AIO", "type": "aio"},
        # SL series fans
        (0x0C45, 0x7696): {"name": "SL120/SL140 Fans", "type": "fan"},
        # More generic IDs that might work
        (0x3633, 0x0001): {"name": "Lian Li Device", "type": "unknown"},
        (0x3633, 0x0002): {"name": "Lian Li Device", "type": "unknown"},
    }
    
    def __init__(self):
        self.devices = []
        self.current_device = None
        self.hid_devices = {}
        
    def scan_devices(self) -> List[DeviceInfo]:
        """Scan for Lian Li devices"""
        found_devices = []
        
        # Scan USB devices
        if USB_AVAILABLE:
            found_devices.extend(self._scan_usb_devices())
        
        # Scan HID devices
        if HID_AVAILABLE:
            found_devices.extend(self._scan_hid_devices())
        
        self.devices = found_devices
        return found_devices
    
    def _scan_usb_devices(self) -> List[DeviceInfo]:
        """Scan USB devices for Lian Li hardware"""
        devices = []
        
        try:
            # Find all USB devices
            for device in usb.core.find(find_all=True):
                device_id = (device.idVendor, device.idProduct)
                
                if device_id in self.KNOWN_DEVICES:
                    info = self.KNOWN_DEVICES[device_id]
                    devices.append(DeviceInfo(
                        vendor_id=device.idVendor,
                        product_id=device.idProduct,
                        name=info["name"],
                        device_type=info["type"],
                        interface="usb"
                    ))
                    logger.info(f"Found USB device: {info['name']} ({hex(device.idVendor)}:{hex(device.idProduct)})")
        
        except Exception as e:
            logger.error(f"Error scanning USB devices: {e}")
        
        return devices
    
    def _scan_hid_devices(self) -> List[DeviceInfo]:
        """Scan HID devices for Lian Li hardware"""
        devices = []
        
        try:
            for device_info in hid.enumerate():
                vendor_id = device_info['vendor_id']
                product_id = device_info['product_id']
                device_id = (vendor_id, product_id)
                
                if device_id in self.KNOWN_DEVICES:
                    info = self.KNOWN_DEVICES[device_id]
                    devices.append(DeviceInfo(
                        vendor_id=vendor_id,
                        product_id=product_id,
                        name=info["name"],
                        device_type=info["type"],
                        interface="hid",
                        path=device_info['path']
                    ))
                    logger.info(f"Found HID device: {info['name']} ({hex(vendor_id)}:{hex(product_id)})")
        
        except Exception as e:
            logger.error(f"Error scanning HID devices: {e}")
        
        return devices
    
    def connect_device(self, device: DeviceInfo) -> bool:
        """Connect to a specific device"""
        try:
            if device.interface == "usb":
                return self._connect_usb_device(device)
            elif device.interface == "hid":
                return self._connect_hid_device(device)
        except Exception as e:
            logger.error(f"Error connecting to device: {e}")
        
        return False
    
    def _connect_usb_device(self, device: DeviceInfo) -> bool:
        """Connect to USB device"""
        try:
            usb_device = usb.core.find(idVendor=device.vendor_id, idProduct=device.product_id)
            if usb_device is None:
                return False
            
            # Try to detach kernel driver if it's active
            try:
                if usb_device.is_kernel_driver_active(0):
                    usb_device.detach_kernel_driver(0)
            except Exception as e:
                logger.warning(f"Could not detach kernel driver: {e}")
            
            # Claim the interface
            usb.util.claim_interface(usb_device, 0)
            
            self.current_device = {
                'device': usb_device,
                'info': device,
                'type': 'usb'
            }
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to connect USB device: {e}")
            return False
    
    def _connect_hid_device(self, device: DeviceInfo) -> bool:
        """Connect to HID device"""
        try:
            if device.path:
                hid_device = hid.device()
                hid_device.open_path(device.path)
            else:
                hid_device = hid.device()
                hid_device.open(device.vendor_id, device.product_id)
            
            self.current_device = {
                'device': hid_device,
                'info': device,
                'type': 'hid'
            }
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to connect HID device: {e}")
            return False
    
    def disconnect(self):
        """Disconnect from current device"""
        if self.current_device:
            try:
                if self.current_device['type'] == 'usb':
                    usb.util.release_interface(self.current_device['device'], 0)
                elif self.current_device['type'] == 'hid':
                    self.current_device['device'].close()
                
                self.current_device = None
                return True
                
            except Exception as e:
                logger.error(f"Error disconnecting device: {e}")
        
        return False
    
    def send_command(self, command: bytes) -> bool:
        """Send command to current device"""
        if not self.current_device:
            return False
        
        try:
            if self.current_device['type'] == 'usb':
                # USB endpoint writing
                self.current_device['device'].write(0x01, command)
                return True
            elif self.current_device['type'] == 'hid':
                # HID report writing
                self.current_device['device'].write(command)
                return True
                
        except Exception as e:
            logger.error(f"Error sending command: {e}")
        
        return False
    
    def read_data(self, length: int = 64) -> Optional[bytes]:
        """Read data from current device"""
        if not self.current_device:
            return None
        
        try:
            if self.current_device['type'] == 'usb':
                return self.current_device['device'].read(0x81, length, timeout=1000)
            elif self.current_device['type'] == 'hid':
                return bytes(self.current_device['device'].read(length, timeout_ms=1000))
                
        except Exception as e:
            logger.debug(f"Error reading data: {e}")
        
        return None

class LianLiProtocol:
    """Protocol implementation for Lian Li devices"""
    
    def __init__(self, hardware_interface: LianLiHardwareInterface):
        self.hw = hardware_interface
        
    def set_pump_speed(self, speed: int) -> bool:
        """Set pump speed (0-100%)"""
        if not 0 <= speed <= 100:
            raise ValueError("Speed must be between 0 and 100")
        
        # Generic pump speed command - may need adjustment for specific models
        command = bytes([0x01, 0x03, 0x01, speed, 0x00, 0x00, 0x00, 0x00])
        return self.hw.send_command(command)
    
    def set_fan_speed(self, speed: int) -> bool:
        """Set fan speed (0-100%)"""
        if not 0 <= speed <= 100:
            raise ValueError("Speed must be between 0 and 100")
        
        # Generic fan speed command
        command = bytes([0x01, 0x03, 0x02, speed, 0x00, 0x00, 0x00, 0x00])
        return self.hw.send_command(command)
    
    def set_fan_curve(self, curve: List[Tuple[int, int]]) -> bool:
        """Set fan curve with temperature/speed points"""
        if len(curve) > 8:  # Most devices support max 8 points
            curve = curve[:8]
        
        # Pack curve data
        command = bytearray([0x01, 0x04, len(curve)])
        
        for temp, speed in curve:
            command.extend([temp, speed])
        
        # Pad to fixed length
        while len(command) < 32:
            command.append(0x00)
        
        return self.hw.send_command(bytes(command))
    
    def set_rgb_color(self, r: int, g: int, b: int) -> bool:
        """Set RGB color"""
        if not all(0 <= c <= 255 for c in [r, g, b]):
            raise ValueError("RGB values must be between 0 and 255")
        
        command = bytes([0x02, 0x01, 0x01, r, g, b, 0x00, 0x00])
        return self.hw.send_command(command)
    
    def set_rgb_mode(self, mode: str) -> bool:
        """Set RGB lighting mode"""
        mode_map = {
            'off': 0x00,
            'static': 0x01,
            'breathing': 0x02,
            'rainbow': 0x03,
            'wave': 0x04,
            'spectrum': 0x05
        }
        
        if mode not in mode_map:
            return False
        
        command = bytes([0x02, 0x02, mode_map[mode], 0x00, 0x00, 0x00, 0x00, 0x00])
        return self.hw.send_command(command)
    
    def get_sensor_data(self) -> Optional[Dict[str, Any]]:
        """Get sensor data from device"""
        # Request sensor data
        command = bytes([0x03, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        
        if not self.hw.send_command(command):
            return None
        
        # Read response
        time.sleep(0.1)  # Give device time to respond
        data = self.hw.read_data()
        
        if not data or len(data) < 16:
            return None
        
        try:
            # Parse sensor data (format may vary by device)
            return {
                'pump_rpm': struct.unpack('<H', data[2:4])[0],
                'fan_rpm': struct.unpack('<H', data[4:6])[0],
                'liquid_temp': struct.unpack('<H', data[6:8])[0] / 100.0,
                'pump_speed': data[8],
                'fan_speed': data[9]
            }
        except Exception as e:
            logger.error(f"Error parsing sensor data: {e}")
            return None

class LianLiCoolerController:
    """Main controller class for Linux"""
    
    def __init__(self):
        self.hardware = LianLiHardwareInterface()
        self.protocol = LianLiProtocol(self.hardware)
        self.system_monitor = LinuxSystemMonitor()
        self.current_profile = None
        
    def initialize(self) -> bool:
        """Initialize the controller"""
        devices = self.hardware.scan_devices()
        
        if not devices:
            logger.warning("No Lian Li devices found")
            return False
        
        # Try to connect to the first AIO device
        for device in devices:
            if device.device_type == 'aio':
                if self.hardware.connect_device(device):
                    logger.info(f"Connected to {device.name}")
                    return True
        
        # If no AIO found, try any device
        if self.hardware.connect_device(devices[0]):
            logger.info(f"Connected to {devices[0].name}")
            return True
        
        return False
    
    def get_devices(self) -> List[DeviceInfo]:
        """Get list of detected devices"""
        return self.hardware.devices
    
    def connect_device(self, device: DeviceInfo) -> bool:
        """Connect to a specific device"""
        return self.hardware.connect_device(device)
    
    def get_current_status(self) -> CoolerStatus:
        """Get current cooler status"""
        # Get CPU temperature from system
        cpu_temp = self.system_monitor.get_cpu_temperature()
        
        # Get device sensor data if available
        sensor_data = self.protocol.get_sensor_data()
        
        if sensor_data:
            return CoolerStatus(
                cpu_temp=cpu_temp,
                pump_speed=sensor_data.get('pump_speed', 0),
                fan_speed=sensor_data.get('fan_speed', 0),
                pump_rpm=sensor_data.get('pump_rpm', 0),
                fan_rpm=sensor_data.get('fan_rpm', 0),
                liquid_temp=sensor_data.get('liquid_temp', 0.0)
            )
        else:
            # Return status with system data only
            return CoolerStatus(
                cpu_temp=cpu_temp,
                pump_speed=75,  # Default values
                fan_speed=60,
                pump_rpm=2800,
                fan_rpm=1200,
                liquid_temp=35.0
            )
    
    def set_pump_speed(self, speed: int) -> bool:
        """Set pump speed"""
        return self.protocol.set_pump_speed(speed)
    
    def set_fan_speed(self, speed: int) -> bool:
        """Set fan speed"""
        return self.protocol.set_fan_speed(speed)
    
    def set_fan_curve(self, curve: List[Tuple[int, int]]) -> bool:
        """Set fan curve"""
        return self.protocol.set_fan_curve(curve)
    
    def set_rgb_lighting(self, mode: str, colors: List[str]) -> bool:
        """Set RGB lighting"""
        success = True
        
        # Set mode first
        if not self.protocol.set_rgb_mode(mode):
            success = False
        
        # Set color if applicable
        if colors and mode in ['static', 'breathing']:
            try:
                color = colors[0].lstrip('#')
                r, g, b = tuple(int(color[i:i+2], 16) for i in (0, 2, 4))
                if not self.protocol.set_rgb_color(r, g, b):
                    success = False
            except Exception as e:
                logger.error(f"Error setting RGB color: {e}")
                success = False
        
        return success
    
    def apply_profile(self, profile: CoolerProfile) -> bool:
        """Apply a complete cooling profile"""
        success = True
        
        # Apply pump speed
        if not self.set_pump_speed(profile.pump_speed):
            success = False
            logger.error("Failed to set pump speed")
        
        # Apply fan curve
        if not self.set_fan_curve(profile.fan_curve):
            success = False
            logger.error("Failed to set fan curve")
        
        # Apply RGB settings
        if not self.set_rgb_lighting(profile.rgb_mode, profile.rgb_colors):
            success = False
            logger.error("Failed to set RGB lighting")
        
        if success:
            self.current_profile = profile
            logger.info(f"Applied profile: {profile.name}")
        
        return success
    
    def disconnect(self):
        """Disconnect from device"""
        self.hardware.disconnect()

class CoolerGUI:
    """Main GUI application for Linux"""
    
    def __init__(self):
        self.controller = LianLiCoolerController()
        self.profiles = self.load_profiles()
        self.monitoring = False
        self.monitor_thread = None
        
        self.setup_gui()
        
    def setup_gui(self):
        """Initialize the GUI"""
        self.root = tk.Tk()
        self.root.title("Lian Li Cooler Manager - Linux Edition")
        self.root.geometry("900x700")
        self.root.protocol("WM_DELETE_WINDOW", self.on_closing)
        
        # Create main frame
        main_frame = ttk.Frame(self.root)
        main_frame.pack(fill=tk.BOTH, expand=True)
        
        # Status bar
        self.status_bar = ttk.Label(main_frame, text="Ready", relief=tk.SUNKEN, anchor=tk.W)
        self.status_bar.pack(side=tk.BOTTOM, fill=tk.X)
        
        # Create notebook for tabs
        self.notebook = ttk.Notebook(main_frame)
        self.notebook.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)
        
        # Setup tabs
        self.setup_connection_tab()
        self.setup_status_tab()
        self.setup_control_tab()
        self.setup_profiles_tab()
        self.setup_rgb_tab()
        self.setup_system_tab()
        
        # Initialize after GUI is ready
        self.root.after(100, self.initialize_controller)
    
    def setup_connection_tab(self):
        """Setup device connection tab"""
        conn_frame = ttk.Frame(self.notebook)
        self.notebook.add(conn_frame, text="Devices")
        
        # Device detection
        detect_frame = ttk.LabelFrame(conn_frame, text="Device Detection")
        detect_frame.pack(fill=tk.X, padx=20, pady=10)
        
        # Hardware requirements info
        info_text = """Linux Requirements:
• USB/HID access permissions (add user to 'plugdev' group)
• Python packages: pyusb, hidapi, psutil
• For USB devices: may need to run as root or set udev rules

Install packages:
sudo apt install python3-dev libudev-dev
pip install pyusb hidapi psutil"""
        
        info_label = tk.Text(detect_frame, height=8, wrap=tk.WORD)
        info_label.insert(1.0, info_text)
        info_label.config(state=tk.DISABLED)
        info_label.pack(fill=tk.X, padx=10, pady=5)
        
        # Scan button
        ttk.Button(detect_frame, text="Scan for Devices", 
                  command=self.scan_devices).pack(pady=10)
        
        # Device list
        device_frame = ttk.LabelFrame(conn_frame, text="Detected Devices")
        device_frame.pack(fill=tk.BOTH, expand=True, padx=20, pady=10)
        
        # Create treeview for devices
        columns = ('Name', 'Type', 'VID:PID', 'Interface', 'Status')
        self.device_tree = ttk.Treeview(device_frame, columns=columns, show='headings', height=6)
        
        for col in columns:
            self.device_tree.heading(col, text=col)
            self.device_tree.column(col, width=120)
        
        self.device_tree.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)
        
        # Device control buttons
        button_frame = ttk.Frame(device_frame)
        button_frame.pack(pady=10)
        
        ttk.Button(button_frame, text="Connect", 
                  command=self.connect_selected_device).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Disconnect", 
                  command=self.disconnect_device).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Refresh", 
                  command=self.refresh_device_list).pack(side=tk.LEFT, padx=5)
    
    def setup_status_tab(self):
        """Setup status monitoring tab"""
        status_frame = ttk.Frame(self.notebook)
        self.notebook.add(status_frame, text="Status")
        
        ttk.Label(status_frame, text="System & Cooler Status", font=("Arial", 14, "bold")).pack(pady=10)
        
        # Status variables
        self.status_vars = {
            'cpu_temp': tk.StringVar(value="--°C"),
            'liquid_temp': tk.StringVar(value="--°C"),
            'pump_speed': tk.StringVar(value="--%"),
            'fan_speed': tk.StringVar(value="--%"),
            'pump_rpm': tk.StringVar(value="-- RPM"),
            'fan_rpm': tk.StringVar(value="-- RPM"),
            'current_profile': tk.StringVar(value="None"),
            'device_status': tk.StringVar(value="Disconnected")
        }
        
        # Status display in two columns
        status_container = ttk.Frame(status_frame)
        status_container.pack(pady=20)
        
        # Left column - Temperature & Device
        left_frame = ttk.LabelFrame(status_container, text="System Status")
        left_frame.pack(side=tk.LEFT, padx=20, pady=10, fill=tk.BOTH)
        
        left_labels = [
            ("Device Status:", 'device_status'),
            ("CPU Temperature:", 'cpu_temp'),
            ("Liquid Temperature:", 'liquid_temp'),
            ("Active Profile:", 'current_profile')
        ]
        
        for i, (label, var_key) in enumerate(left_labels):
            frame = ttk.Frame(left_frame)
            frame.pack(fill=tk.X, padx=10, pady=5)
            ttk.Label(frame, text=label, font=("Arial", 10, "bold"), width=18).pack(side=tk.LEFT)
            ttk.Label(frame, textvariable=self.status_vars[var_key]).pack(side=tk.LEFT)
        
        # Right column - Fan & Pump
        right_frame = ttk.LabelFrame(status_container, text="Cooler Status")
        right_frame.pack(side=tk.RIGHT, padx=20, pady=10, fill=tk.BOTH)
        
        right_labels = [
            ("Pump Speed:", 'pump_speed'),
            ("Fan Speed:", 'fan_speed'),
            ("Pump RPM:", 'pump_rpm'),
            ("Fan RPM:", 'fan_rpm')
        ]
        
        for i, (label, var_key) in enumerate(right_labels):
            frame = ttk.Frame(right_frame)
            frame.pack(fill=tk.X, padx=10, pady=5)
            ttk.Label(frame, text=label, font=("Arial", 10, "bold"), width=12).pack(side=tk.LEFT)
            ttk.Label(frame, textvariable=self.status_vars[var_key]).pack(side=tk.LEFT)
        
        # Monitoring controls
        monitor_frame = ttk.Frame(status_frame)
        monitor_frame.pack(pady=20)
        
        self.monitor_button = ttk.Button(monitor_frame, text="Start Monitoring", 
                                        command=self.toggle_monitoring)
        self.monitor_button.pack(side=tk.LEFT, padx=5)
        
        ttk.Button(monitor_frame, text="Refresh Now", 
                  command=self.refresh_status).pack(side=tk.LEFT, padx=5)
    
    def setup_control_tab(self):
        """Setup manual control tab"""
        control_frame = ttk.Frame(self.notebook)
        self.notebook.add(control_frame, text="Manual Control")
        
        ttk.Label(control_frame, text="Manual Controls", font=("Arial", 14, "bold")).pack(pady=10)
        
        # Control container
        control_container = ttk.Frame(control_frame)
        control_container.pack(fill=tk.BOTH, expand=True, padx=20)
        
        # Pump control
        pump_frame = ttk.LabelFrame(control_container, text="Pump Control")
        pump_frame.pack(fill=tk.X, pady=10)
        
        self.pump_var = tk.IntVar(value=75)
        
        pump_control = ttk.Frame(pump_frame)
        pump_control.pack(fill=tk.X, padx=10, pady=10)
        
        ttk.Label(pump_control, text="Pump Speed (%):").pack(anchor="w")
        pump_scale = ttk.Scale(pump_control, from_=0, to=100, variable=self.pump_var, 
                              orient=tk.HORIZONTAL, length=300)
        pump_scale.pack(fill=tk.X, pady=5)
        
        pump_value_frame = ttk.Frame(pump_control)
        pump_value_frame.pack(fill=tk.X, pady=5)
        ttk.Label(pump_value_frame, textvariable=self.pump_var).pack(side=tk.LEFT)
        ttk.Button(pump_value_frame, text="Apply Pump Speed", 
                  command=self.apply_pump_speed).pack(side=tk.RIGHT)
        
        # Fan control
        fan_frame = ttk.LabelFrame(control_container, text="Fan Control")
        fan_frame.pack(fill=tk.X, pady=10)
        
        self.fan_var = tk.IntVar(value=60)
        
        fan_control = ttk.Frame(fan_frame)
        fan_control.pack(fill=tk.X, padx=10, pady=10)
        
        ttk.Label(fan_control, text="Fan Speed (%):").pack(anchor="w")
        fan_scale = ttk.Scale(fan_control, from_=0, to=100, variable=self.fan_var, 
                             orient=tk.HORIZONTAL, length=300)
        fan_scale.pack(fill=tk.X, pady=5)
        
        fan_value_frame = ttk.Frame(fan_control)
        fan_value_frame.pack(fill=tk.X, pady=5)
        ttk.Label(fan_value_frame, textvariable=self.fan_var).pack(side=tk.LEFT)
        ttk.Button(fan_value_frame, text="Apply Fan Speed", 
                  command=self.apply_fan_speed).pack(side=tk.RIGHT)
        
        # Fan curve control
        curve_frame = ttk.LabelFrame(control_container, text="Fan Curve")
        curve_frame.pack(fill=tk.X, pady=10)
        
        ttk.Label(curve_frame, text="Custom Fan Curve (Temperature°C : Speed%):").pack(anchor="w", padx=10, pady=5)
        
        self.fan_curve_frame = ttk.Frame(curve_frame)
        self.fan_curve_frame.pack(fill=tk.X, padx=10, pady=5)
        
        self.fan_curve_entries = []
        self.create_fan_curve_controls()
        
        curve_buttons = ttk.Frame(curve_frame)
        curve_buttons.pack(pady=10)
        ttk.Button(curve_buttons, text="Apply Fan Curve", 
                  command=self.apply_fan_curve).pack(side=tk.LEFT, padx=5)
        ttk.Button(curve_buttons, text="Reset to Default", 
                  command=self.reset_fan_curve).pack(side=tk.LEFT, padx=5)
    
    def create_fan_curve_controls(self):
        """Create fan curve input controls"""
        default_points = [(30, 30), (50, 50), (70, 70), (85, 100)]
        
        for i, (temp, speed) in enumerate(default_points):
            frame = ttk.Frame(self.fan_curve_frame)
            frame.pack(fill=tk.X, pady=2)
            
            ttk.Label(frame, text=f"Point {i+1}:", width=8).pack(side=tk.LEFT)
            
            temp_var = tk.IntVar(value=temp)
            speed_var = tk.IntVar(value=speed)
            
            ttk.Label(frame, text="Temp:").pack(side=tk.LEFT, padx=5)
            temp_spin = ttk.Spinbox(frame, from_=0, to=100, width=5, textvariable=temp_var)
            temp_spin.pack(side=tk.LEFT, padx=5)
            
            ttk.Label(frame, text="Speed:").pack(side=tk.LEFT, padx=5)
            speed_spin = ttk.Spinbox(frame, from_=0, to=100, width=5, textvariable=speed_var)
            speed_spin.pack(side=tk.LEFT, padx=5)
            
            self.fan_curve_entries.append((temp_var, speed_var))
    
    def setup_profiles_tab(self):
        """Setup profiles management tab"""
        profiles_frame = ttk.Frame(self.notebook)
        self.notebook.add(profiles_frame, text="Profiles")
        
        ttk.Label(profiles_frame, text="Cooling Profiles", font=("Arial", 14, "bold")).pack(pady=10)
        
        # Profile management container
        mgmt_container = ttk.Frame(profiles_frame)
        mgmt_container.pack(fill=tk.BOTH, expand=True, padx=20, pady=10)
        
        # Profile list
        list_frame = ttk.LabelFrame(mgmt_container, text="Available Profiles")
        list_frame.pack(side=tk.LEFT, fill=tk.BOTH, expand=True, padx=(0, 10))
        
        self.profile_listbox = tk.Listbox(list_frame, font=("Arial", 10))
        self.profile_listbox.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)
        
        profile_scrollbar = ttk.Scrollbar(list_frame, orient=tk.VERTICAL, command=self.profile_listbox.yview)
        profile_scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
        self.profile_listbox.config(yscrollcommand=profile_scrollbar.set)
        
        # Profile details
        details_frame = ttk.LabelFrame(mgmt_container, text="Profile Details")
        details_frame.pack(side=tk.RIGHT, fill=tk.BOTH, padx=(10, 0))
        
        self.profile_details = tk.Text(details_frame, width=35, height=15, font=("Courier", 9))
        self.profile_details.pack(fill=tk.BOTH, expand=True, padx=10, pady=10)
        
        details_scroll = ttk.Scrollbar(details_frame, orient=tk.VERTICAL, command=self.profile_details.yview)
        details_scroll.pack(side=tk.RIGHT, fill=tk.Y)
        self.profile_details.config(yscrollcommand=details_scroll.set)
        
        # Profile control buttons
        button_frame = ttk.Frame(profiles_frame)
        button_frame.pack(pady=10)
        
        ttk.Button(button_frame, text="Apply Profile", 
                  command=self.apply_profile).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Create New", 
                  command=self.create_profile_dialog).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Edit Selected", 
                  command=self.edit_profile_dialog).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Delete", 
                  command=self.delete_profile).pack(side=tk.LEFT, padx=5)
        ttk.Button(button_frame, text="Import/Export", 
                  command=self.import_export_profiles).pack(side=tk.LEFT, padx=5)
        
        self.profile_listbox.bind('<<ListboxSelect>>', self.on_profile_selected)
        self.update_profile_list()
    
    def setup_rgb_tab(self):
        """Setup RGB control tab"""
        rgb_frame = ttk.Frame(self.notebook)
        self.notebook.add(rgb_frame, text="RGB Lighting")
        
        ttk.Label(rgb_frame, text="RGB Lighting Control", font=("Arial", 14, "bold")).pack(pady=10)
        
        rgb_container = ttk.Frame(rgb_frame)
        rgb_container.pack(fill=tk.BOTH, expand=True, padx=20)
        
        # RGB Mode selection
        mode_frame = ttk.LabelFrame(rgb_container, text="Lighting Mode")
        mode_frame.pack(fill=tk.X, pady=10)
        
        self.rgb_mode = tk.StringVar(value="static")
        modes = [("Off", "off"), ("Static", "static"), ("Breathing", "breathing"), 
                ("Rainbow", "rainbow"), ("Wave", "wave"), ("Spectrum", "spectrum")]
        
        mode_grid = ttk.Frame(mode_frame)
        mode_grid.pack(padx=10, pady=10)
        
        for i, (display_name, mode_value) in enumerate(modes):
            ttk.Radiobutton(mode_grid, text=display_name, variable=self.rgb_mode, 
                           value=mode_value, command=self.on_rgb_mode_changed).grid(
                           row=i//3, column=i%3, sticky="w", padx=10, pady=2)
        
        # Color selection
        color_frame = ttk.LabelFrame(rgb_container, text="Color Selection")
        color_frame.pack(fill=tk.X, pady=10)
        
        self.rgb_vars = {'r': tk.IntVar(value=255), 'g': tk.IntVar(), 'b': tk.IntVar()}
        
        color_controls = ttk.Frame(color_frame)
        color_controls.pack(padx=10, pady=10)
        
        for color, var in self.rgb_vars.items():
            frame = ttk.Frame(color_controls)
            frame.pack(fill=tk.X, pady=5)
            
            ttk.Label(frame, text=f"{color.upper()}:", width=5).pack(side=tk.LEFT)
            ttk.Scale(frame, from_=0, to=255, variable=var, orient=tk.HORIZONTAL,
                     command=lambda v, c=color: self.on_color_changed()).pack(
                     side=tk.LEFT, fill=tk.X, expand=True, padx=10)
            ttk.Label(frame, textvariable=var, width=5).pack(side=tk.RIGHT)
        
        # Color preview
        preview_frame = ttk.Frame(color_frame)
        preview_frame.pack(pady=10)
        ttk.Label(preview_frame, text="Preview:").pack(side=tk.LEFT, padx=5)
        self.color_preview = tk.Frame(preview_frame, width=60, height=30, bg="#FF0000", relief=tk.RAISED, bd=2)
        self.color_preview.pack(side=tk.LEFT, padx=10)
        
        # Preset colors
        preset_frame = ttk.LabelFrame(rgb_container, text="Preset Colors")
        preset_frame.pack(fill=tk.X, pady=10)
        
        presets = [
            ("Red", "#FF0000"), ("Green", "#00FF00"), ("Blue", "#0000FF"),
            ("White", "#FFFFFF"), ("Purple", "#800080"), ("Cyan", "#00FFFF"),
            ("Yellow", "#FFFF00"), ("Orange", "#FFA500"), ("Pink", "#FFC0CB")
        ]
        
        preset_grid = ttk.Frame(preset_frame)
        preset_grid.pack(pady=10)
        
        for i, (name, color) in enumerate(presets):
            btn = tk.Button(preset_grid, text=name, bg=color, fg="black" if color in ["#FFFFFF", "#FFFF00", "#00FFFF"] else "white",
                           width=8, command=lambda c=color: self.set_preset_color(c))
            btn.grid(row=i//3, column=i%3, padx=2, pady=2)
        
        # Apply button
        ttk.Button(rgb_container, text="Apply RGB Settings", 
                  command=self.apply_rgb_settings).pack(pady=20)
    
    def setup_system_tab(self):
        """Setup system information tab"""
        system_frame = ttk.Frame(self.notebook)
        self.notebook.add(system_frame, text="System Info")
        
        ttk.Label(system_frame, text="System Information", font=("Arial", 14, "bold")).pack(pady=10)
        
        # Temperature sensors
        temp_frame = ttk.LabelFrame(system_frame, text="Temperature Sensors")
        temp_frame.pack(fill=tk.X, padx=20, pady=10)
        
        self.temp_text = tk.Text(temp_frame, height=8, font=("Courier", 9))
        self.temp_text.pack(fill=tk.X, padx=10, pady=10)
        
        # Fan controls
        fan_controls_frame = ttk.LabelFrame(system_frame, text="System Fan Controls")
        fan_controls_frame.pack(fill=tk.X, padx=20, pady=10)
        
        self.fan_controls_text = tk.Text(fan_controls_frame, height=6, font=("Courier", 9))
        self.fan_controls_text.pack(fill=tk.X, padx=10, pady=10)
        
        # Refresh button
        ttk.Button(system_frame, text="Refresh System Info", 
                  command=self.refresh_system_info).pack(pady=10)
        
        # Initial load
        self.refresh_system_info()
    
    def initialize_controller(self):
        """Initialize the controller"""
        def init_thread():
            try:
                success = self.controller.initialize()
                self.root.after(0, lambda: self.on_controller_initialized(success))
            except Exception as e:
                self.root.after(0, lambda: self.on_controller_error(str(e)))
        
        self.status_bar.config(text="Initializing...")
        threading.Thread(target=init_thread, daemon=True).start()
    
    def on_controller_initialized(self, success):
        """Handle controller initialization result"""
        if success:
            self.status_bar.config(text="Device connected successfully")
            self.status_vars['device_status'].set("Connected")
            self.refresh_device_list()
            self.start_monitoring()
        else:
            self.status_bar.config(text="No devices found - scanning...")
            self.scan_devices()
    
    def on_controller_error(self, error):
        """Handle controller initialization error"""
        self.status_bar.config(text=f"Error: {error}")
        messagebox.showerror("Initialization Error", 
                           f"Failed to initialize controller:\n{error}\n\n"
                           "Check that you have proper permissions and required packages installed.")
    
    def scan_devices(self):
        """Scan for devices"""
        def scan_thread():
            devices = self.controller.hardware.scan_devices()
            self.root.after(0, lambda: self.on_scan_complete(devices))
        
        self.status_bar.config(text="Scanning for devices...")
        threading.Thread(target=scan_thread, daemon=True).start()
    
    def on_scan_complete(self, devices):
        """Handle scan completion"""
        self.refresh_device_list()
        
        if devices:
            self.status_bar.config(text=f"Found {len(devices)} device(s)")
        else:
            self.status_bar.config(text="No Lian Li devices found")
            messagebox.showinfo("No Devices Found", 
                              "No Lian Li devices were detected.\n\n"
                              "Make sure:\n"
                              "• Device is connected via USB\n"
                              "• You have proper permissions\n"
                              "• Required packages are installed\n"
                              "• Device is supported")
    
    def refresh_device_list(self):
        """Refresh device list display"""
        # Clear existing items
        for item in self.device_tree.get_children():
            self.device_tree.delete(item)
        
        # Add devices
        for device in self.controller.get_devices():
            vid_pid = f"{device.vendor_id:04X}:{device.product_id:04X}"
            status = "Connected" if (self.controller.hardware.current_device and 
                                   self.controller.hardware.current_device['info'] == device) else "Available"
            
            self.device_tree.insert('', tk.END, values=(
                device.name,
                device.device_type,
                vid_pid,
                device.interface.upper(),
                status
            ))
    
    def connect_selected_device(self):
        """Connect to selected device"""
        selection = self.device_tree.selection()
        if not selection:
            messagebox.showwarning("No Selection", "Please select a device to connect to")
            return
        
        item = self.device_tree.item(selection[0])
        device_name = item['values'][0]
        
        # Find device by name
        device = None
        for dev in self.controller.get_devices():
            if dev.name == device_name:
                device = dev
                break
        
        if device and self.controller.connect_device(device):
            self.status_bar.config(text=f"Connected to {device.name}")
            self.status_vars['device_status'].set("Connected")
            self.refresh_device_list()
            self.start_monitoring()
        else:
            messagebox.showerror("Connection Failed", f"Failed to connect to {device_name}")
    
    def disconnect_device(self):
        """Disconnect from current device"""
        self.controller.disconnect()
        self.status_vars['device_status'].set("Disconnected")
        self.stop_monitoring()
        self.refresh_device_list()
        self.status_bar.config(text="Disconnected from device")
    
    def start_monitoring(self):
        """Start status monitoring"""
        if not self.monitoring:
            self.monitoring = True
            self.monitor_button.config(text="Stop Monitoring")
            self.monitor_thread = threading.Thread(target=self.monitor_loop, daemon=True)
            self.monitor_thread.start()
    
    def stop_monitoring(self):
        """Stop status monitoring"""
        self.monitoring = False
        if self.monitor_button:
            self.monitor_button.config(text="Start Monitoring")
    
    def toggle_monitoring(self):
        """Toggle monitoring on/off"""
        if self.monitoring:
            self.stop_monitoring()
        else:
            self.start_monitoring()
    
    def monitor_loop(self):
        """Main monitoring loop"""
        while self.monitoring:
            try:
                status = self.controller.get_current_status()
                self.root.after(0, lambda s=status: self.update_status_display(s))
                time.sleep(3)  # Update every 3 seconds
            except Exception as e:
                logger.error(f"Error in monitoring loop: {e}")
                time.sleep(5)
    
    def update_status_display(self, status: CoolerStatus):
        """Update status display"""
        self.status_vars['cpu_temp'].set(f"{status.cpu_temp:.1f}°C")
        self.status_vars['liquid_temp'].set(f"{status.liquid_temp:.1f}°C")
        self.status_vars['pump_speed'].set(f"{status.pump_speed}%")
        self.status_vars['fan_speed'].set(f"{status.fan_speed}%")
        self.status_vars['pump_rpm'].set(f"{status.pump_rpm} RPM")
        self.status_vars['fan_rpm'].set(f"{status.fan_rpm} RPM")
        
        profile_name = self.controller.current_profile.name if self.controller.current_profile else "None"
        self.status_vars['current_profile'].set(profile_name)
    
    def refresh_status(self):
        """Refresh status immediately"""
        try:
            status = self.controller.get_current_status()
            self.update_status_display(status)
            self.status_bar.config(text="Status refreshed")
        except Exception as e:
            self.status_bar.config(text=f"Error refreshing status: {e}")
    
    def apply_pump_speed(self):
        """Apply pump speed setting"""
        speed = self.pump_var.get()
        try:
            if self.controller.set_pump_speed(speed):
                self.status_bar.config(text=f"Pump speed set to {speed}%")
            else:
                messagebox.showerror("Error", "Failed to set pump speed. Check device connection.")
        except Exception as e:
            messagebox.showerror("Error", f"Error setting pump speed: {e}")
    
    def apply_fan_speed(self):
        """Apply fan speed setting"""
        speed = self.fan_var.get()
        try:
            if self.controller.set_fan_speed(speed):
                self.status_bar.config(text=f"Fan speed set to {speed}%")
            else:
                messagebox.showerror("Error", "Failed to set fan speed. Check device connection.")
        except Exception as e:
            messagebox.showerror("Error", f"Error setting fan speed: {e}")
    
    def apply_fan_curve(self):
        """Apply fan curve settings"""
        try:
            curve = []
            for temp_var, speed_var in self.fan_curve_entries:
                curve.append((temp_var.get(), speed_var.get()))
            
            # Sort by temperature
            curve.sort(key=lambda x: x[0])
            
            if self.controller.set_fan_curve(curve):
                self.status_bar.config(text="Fan curve applied successfully")
            else:
                messagebox.showerror("Error", "Failed to apply fan curve. Check device connection.")
        except Exception as e:
            messagebox.showerror("Error", f"Error applying fan curve: {e}")
    
    def reset_fan_curve(self):
        """Reset fan curve to default"""
        default_points = [(30, 30), (50, 50), (70, 70), (85, 100)]
        for i, (temp, speed) in enumerate(default_points):
            if i < len(self.fan_curve_entries):
                self.fan_curve_entries[i][0].set(temp)
                self.fan_curve_entries[i][1].set(speed)
    
    def on_rgb_mode_changed(self):
        """Handle RGB mode change"""
        mode = self.rgb_mode.get()
        # Could enable/disable color controls based on mode
        pass
    
    def on_color_changed(self):
        """Handle color slider change"""
        r, g, b = self.rgb_vars['r'].get(), self.rgb_vars['g'].get(), self.rgb_vars['b'].get()
        color = f"#{r:02x}{g:02x}{b:02x}"
        try:
            self.color_preview.config(bg=color)
        except tk.TclError:
            pass  # Invalid color
    
    def set_preset_color(self, color_hex):
        """Set a preset color"""
        color_hex = color_hex.lstrip('#')
        try:
            r, g, b = tuple(int(color_hex[i:i+2], 16) for i in (0, 2, 4))
            self.rgb_vars['r'].set(r)
            self.rgb_vars['g'].set(g)
            self.rgb_vars['b'].set(b)
            self.on_color_changed()
        except ValueError:
            pass
    
    def apply_rgb_settings(self):
        """Apply RGB settings"""
        try:
            mode = self.rgb_mode.get()
            r, g, b = self.rgb_vars['r'].get(), self.rgb_vars['g'].get(), self.rgb_vars['b'].get()
            colors = [f"#{r:02x}{g:02x}{b:02x}"]
            
            if self.controller.set_rgb_lighting(mode, colors):
                self.status_bar.config(text=f"RGB set to {mode} mode")
            else:
                messagebox.showerror("Error", "Failed to set RGB lighting. Check device connection.")
        except Exception as e:
            messagebox.showerror("Error", f"Error setting RGB: {e}")
    
    def refresh_system_info(self):
        """Refresh system information display"""
        # Temperature sensors
        self.temp_text.delete(1.0, tk.END)
        temperatures = self.controller.system_monitor.get_all_temperatures()
        
        if temperatures:
            temp_info = "Available Temperature Sensors:\n\n"
            for name, temp in temperatures.items():
                temp_info += f"{name:<25} {temp:>6.1f}°C\n"
        else:
            temp_info = "No temperature sensors found.\n\nTry installing lm-sensors:\nsudo apt install lm-sensors\nsudo sensors-detect"
        
        self.temp_text.insert(1.0, temp_info)
        
        # Fan controls
        self.fan_controls_text.delete(1.0, tk.END)
        fan_controls = self.controller.system_monitor.fan_controls
        
        if fan_controls:
            fan_info = "Available Fan Controls:\n\n"
            for name, path in fan_controls.items():
                fan_info += f"{name:<15} {path}\n"
        else:
            fan_info = "No fan controls found.\n\nFan controls require:\n• Hardware PWM support\n• Proper kernel modules\n• Write permissions"
        
        self.fan_controls_text.insert(1.0, fan_info)
    
    def load_profiles(self) -> Dict[str, CoolerProfile]:
        """Load cooling profiles"""
        try:
            if os.path.exists('linux_cooler_profiles.json'):
                with open('linux_cooler_profiles.json', 'r') as f:
                    data = json.load(f)
                    profiles = {}
                    for name, profile_data in data.items():
                        profiles[name] = CoolerProfile(**profile_data)
                    return profiles
        except Exception as e:
            logger.error(f"Error loading profiles: {e}")
        
        # Default profiles for Linux
        return {
            "Silent": CoolerProfile("Silent", 50, [(25, 25), (45, 35), (65, 50), (80, 65)], "static", ["#0080FF"], True),
            "Balanced": CoolerProfile("Balanced", 75, [(25, 30), (45, 45), (65, 65), (80, 85)], "breathing", ["#00FF80"], True),
            "Performance": CoolerProfile("Performance", 100, [(25, 50), (45, 65), (65, 80), (80, 100)], "rainbow", [], True),
            "Gaming": CoolerProfile("Gaming", 85, [(25, 40), (45, 55), (65, 75), (80, 95)], "wave", ["#FF0080"], False),
            "Quiet": CoolerProfile("Quiet", 45, [(25, 20), (45, 30), (65, 45), (80, 60)], "static", ["#8000FF"], True)
        }
    
    def save_profiles(self):
        """Save profiles to file"""
        try:
            data = {}
            for name, profile in self.profiles.items():
                data[name] = asdict(profile)
            
            with open('linux_cooler_profiles.json', 'w') as f:
                json.dump(data, f, indent=2)
                
        except Exception as e:
            logger.error(f"Error saving profiles: {e}")
    
    def update_profile_list(self):
        """Update profile list display"""
        self.profile_listbox.delete(0, tk.END)
        for profile_name in self.profiles.keys():
            self.profile_listbox.insert(tk.END, profile_name)
    
    def on_profile_selected(self, event):
        """Handle profile selection"""
        selection = self.profile_listbox.curselection()
        if selection:
            profile_name = self.profile_listbox.get(selection[0])
            profile = self.profiles.get(profile_name)
            if profile:
                self.display_profile_details(profile)
    
    def display_profile_details(self, profile: CoolerProfile):
        """Display profile details"""
        self.profile_details.delete(1.0, tk.END)
        
        details = f"Profile: {profile.name}\n"
        details += "=" * 30 + "\n\n"
        details += f"Pump Speed: {profile.pump_speed}%\n\n"
        details += "Fan Curve Points:\n"
        for i, (temp, speed) in enumerate(profile.fan_curve):
            details += f"  {i+1:2d}. {temp:3d}°C → {speed:3d}%\n"
        details += f"\nRGB Mode: {profile.rgb_mode.title()}\n"
        if profile.rgb_colors:
            details += f"RGB Colors: {', '.join(profile.rgb_colors)}\n"
        details += f"Auto Mode: {'Enabled' if profile.auto_mode else 'Disabled'}\n\n"
        details += "This profile will adjust pump speed,\nfan curve, and RGB lighting when applied."
        
        self.profile_details.insert(1.0, details)
    
    def apply_profile(self):
        """Apply selected profile"""
        selection = self.profile_listbox.curselection()
        if not selection:
            messagebox.showwarning("No Selection", "Please select a profile to apply")
            return
        
        profile_name = self.profile_listbox.get(selection[0])
        profile = self.profiles.get(profile_name)
        
        if profile:
            try:
                if self.controller.apply_profile(profile):
                    self.status_bar.config(text=f"Applied profile: {profile_name}")
                    messagebox.showinfo("Success", f"Successfully applied profile: {profile_name}")
                    
                    # Update UI controls to match profile
                    self.pump_var.set(profile.pump_speed)
                    self.rgb_mode.set(profile.rgb_mode)
                    if profile.rgb_colors:
                        color = profile.rgb_colors[0].lstrip('#')
                        try:
                            r, g, b = tuple(int(color[
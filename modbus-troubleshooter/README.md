# Modbus/PLC Troubleshooting Tool

A Python/FastAPI tool for diagnosing Modbus (TCP/RTU) communication and PLC
connectivity issues, packaged with Docker Compose for deployment on a Raspberry
Pi. It ships with a lightweight built-in Modbus simulator as a test target, with
a full OpenPLC runtime available as an optional profile.

## Features

- **Modbus TCP/RTU** connection testing and register read/write (coils,
  discrete inputs, holding registers, input registers)
- **Built-in Modbus simulator** to test against out of the box — plus optional
  **OpenPLC** integration when you need a real PLC runtime
- **Web dashboard** with live stats, device management, testing tools, and reports
- **REST API** with interactive docs at `/docs`
- **Persistent history** — devices and every transaction are stored in SQLite
- **Docker Compose** stack, lean by default (troubleshooter + simulator) with
  OpenPLC / MQTT / Grafana behind opt-in profiles

## Stack & ports

| Service | Container | Host port | Starts by default? | Purpose |
|---------|-----------|-----------|--------------------|---------|
| troubleshooter | `modbus-troubleshooter` | 8000 | ✅ yes | This app (UI + API) |
| simulator | `modbus-simulator` | 5020 → 502 | ✅ yes | Lightweight Modbus TCP target |
| openplc | `openplc-runtime` | 8080, 502 | ⛔ `openplc` profile | Full PLC runtime + web UI |
| mqtt | `mqtt-broker` | 1883, 9001 | ⛔ `extras` profile | Event broker (optional) |
| grafana | `troubleshooter-grafana` | 3000 | ⛔ `extras` profile | Visualization (optional) |

By default the stack is just the **troubleshooter + simulator** — pure Python,
no extra image pull, and no ARM concerns. Opt in to the heavier pieces:

```bash
docker compose --profile openplc up -d   # add the full OpenPLC runtime
docker compose --profile extras  up -d   # add MQTT + Grafana
```

### Test target: simulator vs OpenPLC

- **Simulator** (default) — a tiny pymodbus TCP slave (`app/simulator.py`) that
  reuses the app image. It answers reads/writes on all four register types
  (holding registers seeded 0,1,2,…; input registers 0,2,4,…). Ideal for
  exercising connectivity, reads, writes, logging, and reports.
- **OpenPLC** (`--profile openplc`) — the real runtime: ladder logic, its own web
  UI at `:8080`, and the `/api/plc/*` endpoints. Heavier (~1 GB image) and must
  have an ARM build for your Pi. Use it when you need actual PLC behavior, not
  just a Modbus responder.

Inside the Docker network the troubleshooter reaches them as host `simulator`
(or `openplc`) on port `502`.

## Quick start

**Prerequisites:** a Raspberry Pi (3/4/5 or any ARM/x86 host), Docker, and the
Docker Compose plugin. (`scripts/setup.sh` can install these and set up
permissions for you.)

```bash
# 1. Configure
cp .env.example .env
nano .env                 # set SECRET_KEY and GRAFANA_PASSWORD

# 2. Build and start the core stack (troubleshooter + simulator)
docker compose up -d --build     # older Docker: docker-compose up -d --build

# 3. Check status
docker compose ps

# (optional) add the full OpenPLC runtime, or MQTT + Grafana, later:
# docker compose --profile openplc up -d
# docker compose --profile extras  up -d
```

> Note: on older installs the command is `docker-compose` (with a hyphen). Both
> work; this README uses the modern `docker compose`.

### Access points

Replace `PI_IP` with your Pi's address (`hostname -I`), or use `localhost` on the Pi.

| URL | What |
|-----|------|
| `http://PI_IP:8000/` | Simple testing UI (`index.html`) |
| `http://PI_IP:8000/dashboard` | Full dashboard (`dashboard.html`) |
| `http://PI_IP:8000/docs` | Interactive API documentation |
| `http://PI_IP:8000/health` | Health check |
| `http://PI_IP:8080` | OpenPLC web interface — only with `--profile openplc` (login `openplc` / `openplc`) |
| `http://PI_IP:3000` | Grafana — only with `--profile extras` (login `admin` / `GRAFANA_PASSWORD`) |

### First connection test

The simulator is ready as soon as the stack is up — test it from the
troubleshooter (the app reaches it on the Docker network as `simulator:502`):

```bash
curl -X POST http://localhost:8000/api/test/connection \
  -H "Content-Type: application/json" \
  -d '{"host":"simulator","port":502,"slave_id":1,"protocol":"tcp"}'
```

(If you enabled `--profile openplc` instead, start its runtime in the OpenPLC web
UI first, then use `"host":"openplc"`.)

## Web interface

- **`/`** — a single-page form for a quick connection test and register read/write.
- **`/dashboard`** — a tabbed dashboard:
  - **Overview** — KPIs, transaction/response-time charts, recent activity
  - **Devices** — add, list, and remove saved devices
  - **Testing** — connection tests and register read/write with live feedback
  - **Diagnostics** — generate a 24-hour report (success rates, errors, timing)

Both UIs call the same REST API described below.

## API reference

Base path: `/api`

**Devices**
- `GET /api/devices` — list saved devices
- `POST /api/devices` — add a device
- `GET /api/devices/{id}` — get one device
- `DELETE /api/devices/{id}` — remove a device

**Testing & registers**
- `POST /api/test/connection` — test connectivity to a device
- `POST /api/registers/read` — read coils/inputs/registers
- `POST /api/registers/write` — write coils or holding registers

**PLC (OpenPLC)**
- `GET /api/plc/status` — OpenPLC runtime status
- `POST /api/plc/start` / `POST /api/plc/stop` — start/stop the runtime

**Monitoring**
- `GET /api/diagnostics/report` — 24-hour diagnostic report
- `GET /api/logs?limit=100&offset=0` — transaction history
- `GET /api/stats` — aggregate statistics

### Examples

```bash
# Read 10 holding registers from the simulator (seeded 0,1,2,…)
curl -X POST http://localhost:8000/api/registers/read \
  -H "Content-Type: application/json" \
  -d '{"host":"simulator","port":502,"slave_id":1,
       "register_type":"holding_register","start_address":0,"count":10}'

# Write coils
curl -X POST http://localhost:8000/api/registers/write \
  -H "Content-Type: application/json" \
  -d '{"host":"simulator","port":502,"slave_id":1,
       "register_type":"coil","start_address":0,"values":[1,0,1,1]}'

# Diagnostic report
curl http://localhost:8000/api/diagnostics/report
```

## Configuration

Settings come from environment variables (see `.env.example`), read by the app
via pydantic-settings. Common ones:

```bash
LOG_LEVEL=INFO
DATABASE_URL=sqlite:///data/troubleshooter.db   # relative to /app -> ./data on host
OPENPLC_HOST=openplc
OPENPLC_PORT=502
MODBUS_TIMEOUT=3
MODBUS_RETRY_COUNT=3
SERIAL_PORT=/dev/ttyUSB0                         # only used for protocol=rtu
SERIAL_BAUDRATE=9600
SECRET_KEY=change-me                             # change before non-test use
GRAFANA_PASSWORD=admin
```

### Modbus RTU / serial

Serial device passthrough is **commented out** in `docker-compose.yml` so the
stack starts without a USB adapter attached. To use RTU:

1. Plug in your USB-to-serial adapter and find it: `ls -l /dev/tty*`
2. Set `SERIAL_PORT` (and baud rate) in `.env`.
3. Uncomment the `devices:` and `group_add:` block under the `troubleshooter`
   service in `docker-compose.yml`, matching your device path.
4. `docker compose up -d` to recreate the container.

### Persistence

The SQLite database lives at `./data/troubleshooter.db` on the host (mounted
into the container) and survives restarts. Tables:

- `devices` — saved device configurations
- `transaction_logs` — every connection test, read, and write
- `diagnostic_snapshots` — reserved for periodic snapshots

To reset all history, stop the stack and delete `./data/troubleshooter.db`.

## Troubleshooting

**Services won't start**
```bash
docker compose logs                 # inspect errors
docker compose up -d --build        # rebuild after code changes
```

**Can't reach the web UI** — confirm it's up from the Pi itself:
```bash
curl http://localhost:8000/health
docker compose ps
```

**Can't connect to OpenPLC** — make sure the runtime is started in the OpenPLC
web UI, then check `docker compose logs openplc`.

**Serial port access** — add your user to `dialout` and re-login:
```bash
sudo usermod -aG dialout $USER
ls -l /dev/ttyUSB0                   # verify the path matches docker-compose.yml
```

**Connection timeouts** — raise `MODBUS_TIMEOUT` in `.env`, verify the slave ID,
and confirm network reachability to the target device.

## Local development (without Docker)

```bash
cd app
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
uvicorn main:app --reload           # http://localhost:8000
```

`scripts/test_connection.py` is a standalone Modbus TCP smoke-test. Against the
running simulator (mapped to host port 5020):
```bash
python scripts/test_connection.py localhost 5020 1
```

## Project structure

```
modbus-troubleshooter/
├── docker-compose.yml          # Full stack: troubleshooter + openplc + mqtt + grafana
├── .env.example                # Copy to .env and edit
├── mqtt/config/mosquitto.conf  # MQTT broker config
├── scripts/
│   ├── setup.sh                # Optional: install Docker, set up permissions
│   └── test_connection.py      # Standalone connection smoke-test
└── app/
    ├── Dockerfile              # Python 3.11 slim, multi-stage
    ├── requirements.txt        # Runtime dependencies
    ├── main.py                 # FastAPI app (routes, lifespan)
    ├── simulator.py            # Lightweight Modbus TCP test target
    ├── config.py               # Settings (pydantic-settings)
    ├── api/
    │   ├── routes.py           # REST endpoints
    │   └── schemas.py          # Request/response models
    ├── services/
    │   ├── modbus_client.py    # Modbus client + device persistence
    │   ├── plc_interface.py    # OpenPLC integration
    │   └── diagnostics.py      # Connection tests, reports, statistics
    ├── models/
    │   └── database.py         # SQLAlchemy models + session helpers
    └── templates/
        ├── index.html          # Simple UI  (served at /)
        └── dashboard.html      # Dashboard  (served at /dashboard)
```

## Notes before going beyond testing

This stack is set up for **local testing**. Before exposing it more widely:
change `SECRET_KEY` and the Grafana/OpenPLC passwords, put it behind an HTTPS
reverse proxy, lock down `ALLOWED_ORIGINS`, and don't leave the MQTT broker open
to untrusted networks (`allow_anonymous` is on for convenience).

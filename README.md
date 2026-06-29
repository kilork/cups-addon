# CUPS Print Server — Home Assistant Addon

[![GitHub Release](https://img.shields.io/github/v/release/kilork/cups-addon?style=for-the-badge)](https://github.com/kilork/cups-addon/releases)
[![Build Status](https://img.shields.io/github/actions/workflow/status/kilork/cups-addon/builder.yaml?branch=main&style=for-the-badge)](https://github.com/kilork/cups-addon/actions)
[![aarch64](https://img.shields.io/badge/aarch64-yes-green?style=for-the-badge)](https://github.com/kilork/cups-addon)
[![amd64](https://img.shields.io/badge/amd64-yes-green?style=for-the-badge)](https://github.com/kilork/cups-addon)
[![License: Apache 2.0](https://img.shields.io/badge/Apache%202.0-000?style=for-the-badge)](LICENSE-APACHE)
[![License: MIT](https://img.shields.io/badge/MIT-000?style=for-the-badge)](LICENSE-MIT)
[![License: Unlicense](https://img.shields.io/badge/Unlicense-000?style=for-the-badge)](LICENSE-UNLICENSE)

A Home Assistant addon providing a CUPS print server with REST API, MQTT auto-discovery, and USB printer support.

Built on top of [arest/cups-addon](https://github.com/arest/cups-addon) by Andrea Restello — the original CUPS addon for Home Assistant.

## Features

- **Network Printing** — Share printers across your network via IPP, LPD, AirPrint
- **USB Printer Support** — Plug-and-play USB pass-through for local printers
- **Samsung M2020 Support** — SpliX driver built from git master with `SpecialBandWidth` fix
- **REST API** — Printer status as JSON on a configurable port (default `8000`)
- **MQTT Auto-Discovery** — Automatically creates a `sensor.cups_printer` in Home Assistant
- **Web Interface** — Full CUPS admin panel at `http://<ha-ip>:631`
- **Persistent Storage** — Printers and config survive restarts and updates

## Quick Start

1. Add the repository: `https://github.com/kilork/cups-addon`
2. Install **CUPS Print Server**
3. Configure (optional): `admin_username`, `admin_password`
4. Start the addon
5. Open the CUPS web UI at `http://<ha-ip>:631` and add your printer

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `admin_username` | `admin` | CUPS web UI admin username |
| `admin_password` | `admin` | CUPS web UI admin password |
| `api_port` | `8000` | Port for the REST API |
| `printer_driver_deb` | `""` | Path to a `.deb` with extra printer drivers |
| `mqtt_host` | `""` | MQTT broker (auto-detected via supervisor if empty) |
| `mqtt_port` | `1883` | MQTT broker port |
| `mqtt_username` | `""` | MQTT username (auto-detected if empty) |
| `mqtt_password` | `""` | MQTT password (auto-detected if empty) |

## REST API

The addon exposes a Rust HTTP server (axum) on the configured `api_port`:

```bash
# Printer status
curl http://<ha-ip>:8000/api/status | jq

# Health check
curl http://<ha-ip>:8000/health
```

### Example response

```json
{
  "cups": { "is_running": true, "version": "CUPS 2.4.19" },
  "printers": [
    {
      "name": "Samsung_M2020_Series",
      "state": "idle",
      "is_accepting_jobs": true,
      "is_enabled": true,
      "make_and_model": "Samsung M2020 Series",
      "device_uri": "usb://Samsung/M2020%20Series...",
      "state_reasons": [],
      "jobs_in_queue": 0
    }
  ],
  "jobs_completed_total": 42
}
```

## MQTT Auto-Discovery

If you have the **Mosquitto MQTT broker** addon installed, the addon automatically detects it and publishes Home Assistant discovery topics:

| Topic | Description |
|-------|-------------|
| `homeassistant/sensor/cups_printer/config` | Entity configuration (retained) |
| `homeassistant/sensor/cups_printer/state` | Current printer state (every 30s) |
| `homeassistant/sensor/cups_printer/attributes` | Full printer status JSON |

A `sensor.cups_printer` appears in HA automatically — no `configuration.yaml` edits needed.

Alternatively, configure a RESTful sensor manually:

```yaml
sensor:
  - platform: rest
    name: CUPS Printer
    resource: http://<ha-ip>:8000/api/status
    value_template: "{{ value_json.printers[0].state }}"
    json_attributes_path: "$"
    json_attributes:
      - printers
      - cups
      - jobs_completed_total
    scan_interval: 30
```

## USB Printer Support

USB printers need pass-through. The addon already has:

```yaml
usb: true
devices:
  - /dev/bus/usb
apparmor: false
```

For Samsung M2020 specifically, the addon includes:
- **SpliX git master** — `rastertoqpdl` with M2020 bandwidth fix
- **USB quirks override** — Prevents device reset errors
- **Correct PPD** — Generated from Splix's `samsung.drv.in` with `SpecialBandWidth: True`

## Architecture

```
┌─────────────────────────────────────────────────┐
│  CUPS Print Server (Alpine)                     │
│                                                  │
│  ┌──────────┐   ┌──────────────┐                │
│  │  CUPSd   │   │  cups-api    │                │
│  │  :631    │   │  (Rust/axum) │                │
│  │          │   │  :8000       │                │
│  └────┬─────┘   └──────┬───────┘                │
│       │                │                        │
│       ▼                ▼                        │
│  ┌──────────┐   ┌──────────────┐                │
│  │ lpstat,  │   │  Supervisor  │                │
│  │ filters, │   │  API / MQTT  │                │
│  │ backends │   │  discovery   │                │
│  └──────────┘   └──────────────┘                │
└─────────────────────────────────────────────────┘
```

## Development

```bash
# Clone
git clone https://github.com/kilork/cups-addon.git
cd cups-addon

# Build locally (requires Docker buildx)
docker buildx bake -f cups/bake.hcl

# Or build a specific architecture
docker buildx build --platform linux/aarch64 -t cups-addon:dev ./cups
```

## Acknowledgments

- [SpliX](https://github.com/OpenPrinting/splix) — Open-source SPL printer driver
- [CUPS](https://www.cups.org/) — The printing system
- [arest/cups-addon](https://github.com/arest/cups-addon) — Original addon this was forked from

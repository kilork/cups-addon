# Changelog

## [1.9.0] — 2026-07-02

- **AirPrint/mDNS Discovery** — Apple devices (iPhone, iPad, Mac) can now
  auto-discover the CUPS printer on the local network
- Add `avahi`, `avahi-tools`, `dbus` packages to Docker image
- Start `dbus-daemon --system` and `avahi-daemon -D` during container setup
- Add DNS-SD directives to `cupsd.conf` (`ServerName %H`, `BrowseLocalProtocols dnssd`,
  `BrowseDNSSDSubTypes _cups,_print`)
- Add `avahi_running` status field to API health endpoint

## [1.8.0] — 2026-07-01

- cups-api manages cupsd lifecycle (PID 1 restructure)
- Setup moved from bash to Rust (`setup()` function)
- Boot script reduced to single `exec /opt/cups-api/cups-api`

## [1.7.5] — 2026-06-30

- Fix MQTT auth header (Bearer token)

## [1.7.4] — 2026-06-30

- MQTT auto-discovery working via `services: [mqtt:want]`

## [1.7.3] — 2026-06-30

- Fix missing re_desc regex definition in status parser

## [1.7.2] — 2026-06-30

- Fix MQTT auto-discovery supervisor API integration

## [1.7.1] — 2026-06-30

- Fix CUPS version and make/model parsing in status endpoint

## [1.7.0] — 2026-06-30

- First MQTT auto-discovery (Rust + rumqttc)

## [1.6.2] — 2026-06-29

- Configurable API port via `api_port` option (default 8000)

## [1.6.1] — 2026-06-29

- Build fix: add cups-api to MONITORED_FILES

## [1.6.0] — 2026-06-29

- Rust cups-api binary (axum HTTP server replacing Python)
- Multi-stage Docker build with static musl binary

## [1.5.2] — 2026-06-29

- Production cleanup: remove SYS_MODULE, debug logging, verbose diagnostics

## [1.5.1] — 2026-06-29

- Fix PPD propagation bug: SpecialBandWidth was never in CUPS runtime PPD

## [1.5.0] — 2026-06-29

- Build Splix from git master (Alpine package 2.0.0 too old — lacks M2020 support)
- Samsung M2020 printer driver (SpliX commit 206e283)

## [1.4.3] — 2026-06-29

- USB quirks override for Samsung M2020 (`no-reattach unidir` for USB ID 0x04e8:0x3321)

## [1.4.2] — 2026-06-28

- Fix USB printing: remove conflicting usblp kernel module

## [1.4.1] — 2026-06-28

- Debug logging and filter diagnostics

## [1.4.0] — 2026-06-28

- USB access fixes for printer passthrough

## [1.3.3] — 2026-06-28

- Added splix driver support for Samsung printers

## [1.2.1] — 2026-05-17

- Fix: remove hardcoded aarch64 default from BUILD_FROM ARG

## [1.2.0] — 2026-05-15

- Fix printer list not persisting across container restarts: replace file-level
  CUPS config symlinks with a directory-level symlink (/etc/cups → /share/cups/config)
  to prevent CUPS atomic file writes from breaking the symlink and writing to
  ephemeral storage

## [1.1.1] — 2026-05-15

- Fix build failure on Alpine 3.23 (HA OS 2026.5): remove unavailable packages hplip, foomatic-db, foomatic-db-ppds

## [1.1.0] — 2026-05-15

- Upgrade to CUPS 3.0
- Fix CUPS printer config persistence to HA shared directory (/share/cups)
- Fix build_from config with default BUILD_FROM arg for reliable Docker builds

## [1.0.0] — 2026-03-30

- Add Canon MF4412 (UFR II) printer driver support
- Install printer drivers (HP, Gutenprint, Foomatic) in container setup

## [0.9.0] — 2026-01-25

- Persist PPD documents across restarts

## [0.8.0] — 2025-05-08

- Move everything into a subfolder and add repository YAML
- Update docs for manual installation

## [0.7.0] — 2025-03-18

- Add Epson printer drivers
- Persist configuration and printer lists

## [0.6.0] — 2025-03-15

- Remove basic authentication
- Initial CUPS add-on release

#!/usr/bin/with-contenv bash

# ═════════════════════════════════════════════════════════════
# CUPS Print Server — Boot sequence
# ═════════════════════════════════════════════════════════════
VERSION="1.5.0"
echo "────────────────────────────────────────────────────────────"
echo "  CUPS Print Server v${VERSION}"
echo "  $(uname -o) / $(uname -m)"
echo "  $(date -Iseconds)"
echo "────────────────────────────────────────────────────────────"

# ═════════════════════════════════════════════════════════════
# Remove conflicting usblp kernel module
# ═════════════════════════════════════════════════════════════
# The usblp module creates /dev/usb/lp* devices that conflict
# with libusb (used by the CUPS USB backend). When usblp has
# the USB interface claimed, libusb's device reset fails
# (LIBUSB_ERROR_NOT_FOUND / -5), causing the print job to
# complete in CUPS without the printer actually printing.
if lsmod 2>/dev/null | grep -q usblp; then
    echo "[boot] Detected conflicting usblp kernel module — removing..."
    if modprobe -r usblp 2>/dev/null; then
        echo "[boot]   usblp removed successfully."
    else
        echo "[boot]   WARNING: could not remove usblp (SYS_MODULE may be needed)"
        echo "[boot]   USB printing may fail with 'Device reset failed, code: -5'"
    fi
else
    echo "[boot] usblp kernel module not loaded — good."
fi

# ─────────────────────────────────────────────────────────────
# Create CUPS data directories in the persistent HA share
# ─────────────────────────────────────────────────────────────
echo "[boot] Creating CUPS data directories..."
mkdir -p /share/cups/cache
mkdir -p /share/cups/logs
mkdir -p /share/cups/state
mkdir -p /share/cups/config
mkdir -p /share/cups/config/ppd
mkdir -p /share/cups/config/ssl

# Set proper permissions
echo "[boot] Setting permissions on /share/cups..."
chown -R root:lp /share/cups
chmod -R 775 /share/cups

# ─────────────────────────────────────────────────────────────
# Write a fresh cupsd.conf (this is static config we own)
# ─────────────────────────────────────────────────────────────
echo "[boot] Writing /share/cups/config/cupsd.conf..."
cat > /share/cups/config/cupsd.conf << 'EOL'
# Listen on all interfaces
Listen 0.0.0.0:631

# Allow access from local network
<Location />
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

# Admin access (no authentication)
<Location /admin>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

# Job management permissions
<Location /jobs>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Location>

<Limit Send-Document Send-URI Hold-Job Release-Job Restart-Job Purge-Jobs Set-Job-Attributes Create-Job-Subscription Renew-Subscription Cancel-Subscription Get-Notifications Reprocess-Job Cancel-Current-Job Suspend-Current-Job Resume-Job Cancel-My-Jobs Close-Job CUPS-Move-Job CUPS-Get-Document>
  Order allow,deny
  Allow localhost
  Allow 10.0.0.0/8
  Allow 172.16.0.0/12
  Allow 192.168.0.0/16
</Limit>

# Enable web interface
WebInterface Yes

# Logging — set to debug for troubleshooting print failures;
# revert to info once printing works.
LogLevel debug

# Default settings
DefaultAuthType None
JobSheets none,none
PreserveJobHistory No
EOL

# Migrate legacy data from /data/cups to /share/cups if present
if [ -d /data/cups/config ] && [ ! -f /share/cups/config/.migrated ]; then
    echo "Migrating CUPS data from /data/cups to /share/cups..."
    cp -r /data/cups/config/printers.conf /share/cups/config/ 2>/dev/null || true
    cp -r /data/cups/config/ppd/* /share/cups/config/ppd/ 2>/dev/null || true
    cp -r /data/cups/config/ssl/* /share/cups/config/ssl/ 2>/dev/null || true
    cp -r /data/cups/config/cupsd.conf /share/cups/config/ 2>/dev/null || true
    cp -r /data/cups/cache/* /share/cups/cache/ 2>/dev/null || true
    cp -r /data/cups/logs/* /share/cups/logs/ 2>/dev/null || true
    cp -r /data/cups/state/* /share/cups/state/ 2>/dev/null || true
    touch /share/cups/config/.migrated
    echo "Migration complete."
fi

# ─────────────────────────────────────────────────────────────
# Replace /etc/cups with a directory-level symlink so that
# CUPS atomic file writes (write .N, rename .O, rename .N)
# operate inside the persistent storage instead of replacing
# individual file symlinks with ephemeral real files.
#
# Background: CUPS saves printers.conf atomically — it writes
# printers.conf.N, renames printers.conf→printers.conf.O, then
# renames printers.conf.N→printers.conf. With file-level
# symlinks the first rename() replaces the symlink itself with
# a real file in the container's ephemeral layer, so all
# subsequent writes bypass the persistent share. After a
# container restart the real file is gone and the old
# (empty/stale) printers.conf in /share/cups/ is used again.
#
# A directory symlink avoids this because rename() only touches
# files inside the resolved target directory, leaving /etc/cups
# as a symlink intact.
# ─────────────────────────────────────────────────────────────

if [ -d /etc/cups ] && [ ! -L /etc/cups ]; then
    echo "Replacing /etc/cups directory with symlink to /share/cups/config..."

    # Copy any default config files from the package-installed
    # /etc/cups/ (e.g. cups-files.conf) that don't yet exist in
    # the persistent storage.
    for item in /etc/cups/*; do
        [ -e "$item" ] || continue
        base="$(basename "$item")"
        # Skip files/dirs we manage ourselves or that may be
        # stale from a previous file-level symlink approach.
        case "$base" in
            cupsd.conf|printers.conf|printers.conf.O|ppd|ssl)
                continue
                ;;
        esac
        if [ ! -e "/share/cups/config/$base" ]; then
            cp -r "$item" "/share/cups/config/$base"
            echo "  Copied default $base to persistent storage."
        fi
    done

    # Safeguard: make sure printers.conf exists in the
    # persistent location before we switch over.
    touch /share/cups/config/printers.conf

    rm -rf /etc/cups
    ln -sf /share/cups/config /etc/cups
    echo "/etc/cups → /share/cups/config"
else
    # Already a symlink or does not exist — just ensure it.
    rm -rf /etc/cups
    ln -sf /share/cups/config /etc/cups
fi

# Verify printers.conf exists in the persistent location
if [ ! -f /share/cups/config/printers.conf ]; then
    touch /share/cups/config/printers.conf
fi

# Install user-supplied printer driver .deb (e.g. Canon UFR II for MF4412)
DRIVER_DEB=$(jq -r '.printer_driver_deb // empty' /data/options.json 2>/dev/null)
if [ -n "$DRIVER_DEB" ]; then
    DRIVER_PATH="/share/${DRIVER_DEB}"
    if [ -f "$DRIVER_PATH" ]; then
        echo "Installing printer driver from ${DRIVER_PATH}..."
        EXTRACT_DIR=$(mktemp -d)
        dpkg -x "$DRIVER_PATH" "$EXTRACT_DIR"
        # Copy CUPS filters
        if [ -d "${EXTRACT_DIR}/usr/lib/cups/filter" ]; then
            cp -r "${EXTRACT_DIR}/usr/lib/cups/filter/." /usr/lib/cups/filter/
            chmod 755 /usr/lib/cups/filter/*
        fi
        # Copy shared libraries
        if [ -d "${EXTRACT_DIR}/usr/lib" ]; then
            find "${EXTRACT_DIR}/usr/lib" -name "*.so*" -exec cp {} /usr/lib/ \;
        fi
        # Copy PPD files
        if [ -d "${EXTRACT_DIR}/usr/share/cups/model" ]; then
            cp -r "${EXTRACT_DIR}/usr/share/cups/model/." /usr/share/cups/model/
        fi
        rm -rf "$EXTRACT_DIR"
        echo "Printer driver installed."
    else
        echo "Warning: printer_driver_deb set to '${DRIVER_DEB}' but /share/${DRIVER_DEB} was not found."
    fi
fi

# ═════════════════════════════════════════════════════════════
# Package verification
# ═════════════════════════════════════════════════════════════
echo "[boot] Package verification:"
for pkg in splix cups cups-filters ghostscript gutenprint epson-inkjet-printer-escpr; do
    if apk list -I "$pkg" 2>/dev/null | grep -q "$pkg"; then
        echo "  [ok]  $pkg — installed"
    else
        echo "  [??]  $pkg — not found"
    fi
done

# Check for the critical splix filter binary
if command -v rastertoqpdl &>/dev/null; then
    echo "  [ok]  rastertoqpdl — available ($(which rastertoqpdl))"
    # Check that all shared libraries are resolved
    LDD_OUTPUT=$(ldd $(which rastertoqpdl) 2>&1)
    UNRESOLVED=$(echo "$LDD_OUTPUT" | grep -i "not found" || true)
    if [ -n "$UNRESOLVED" ]; then
        echo "  [!!]  rastertoqpdl — missing shared libraries:"
        echo "$UNRESOLVED" | sed 's/^/        /'
    else
        echo "  [ok]  rastertoqpdl — all shared libraries resolved"
    fi
else
    echo "  [!!]  rastertoqpdl — MISSING (Samsung M2020 printing will fail)"
fi

# ═════════════════════════════════════════════════════════════
# PPD discovery
# ═════════════════════════════════════════════════════════════
PPD_COUNT=$(find /usr/share/cups/model -name "*.ppd" -o -name "*.ppd.gz" 2>/dev/null | wc -l)
echo "[boot] PPD files found: ${PPD_COUNT}"
SAMSUNG_PPDS=$(find /usr/share/cups/model -path "*/samsung/*.ppd" 2>/dev/null)
if [ -n "$SAMSUNG_PPDS" ]; then
    echo "[boot] Samsung PPDs:"
    for ppd in $SAMSUNG_PPDS; do
        nickname=$(grep "^\*NickName:" "$ppd" 2>/dev/null | sed 's/.*"\(.*\)"/  \1/')
        if [ -n "$nickname" ]; then
            echo "  $ppd →$nickname"
        else
            echo "  $ppd"
        fi
    done
fi

# ═════════════════════════════════════════════════════════════
# Network information
# ═════════════════════════════════════════════════════════════
echo "[boot] Network:"
echo "  Hostname: $(hostname 2>/dev/null || echo 'unknown')"
IP_ADDR=$(ip -4 -o addr show scope global 2>/dev/null | awk '{print $4}' | head -3)
if [ -n "$IP_ADDR" ]; then
    echo "$IP_ADDR" | while IFS= read -r addr; do
        echo "  IP:       ${addr%/*}"
    done
else
    echo "  IP:       (no global IP assigned yet)"
fi

# ═════════════════════════════════════════════════════════════
# Start CUPS and wait for readiness
# ═════════════════════════════════════════════════════════════
echo "[boot] Starting CUPS daemon..."
/usr/sbin/cupsd -f &
CUPS_PID=$!

# Poll until CUPS is ready (up to 15 seconds)
CUPS_READY=false
for i in $(seq 1 15); do
    if lpstat -r 2>/dev/null; then
        CUPS_READY=true
        break
    fi
    sleep 1
done

if [ "$CUPS_READY" = true ]; then
    echo "[boot] CUPS is running and accepting requests."
    echo "[boot] Available drivers (Samsung):"
    lpinfo -m 2>/dev/null | grep -i samsung | head -10 || echo "  (none listed yet)"
    echo "[boot] Available drivers (total): $(lpinfo -m 2>/dev/null | wc -l) models"
    echo ""
    echo "[boot] ── CUPS error log (last 20 lines) ──"
    sleep 1
    tail -20 /share/cups/logs/error_log 2>/dev/null || echo "  (error_log not yet written)"
    echo "[boot] ─────────────────────────────────────"
    echo ""
    echo "╔══════════════════════════════════════════════════════╗"
    echo "║  CUPS Print Server v${VERSION} is READY              ║"
    echo "║  Web UI:  http://<your-ha-ip>:631                   ║"
    echo "║  Logs:    /share/cups/logs/error_log                ║"
    echo "╚══════════════════════════════════════════════════════╝"
    echo ""
else
    echo "[boot] WARNING: CUPS did not respond to lpstat within 15s."
    echo "[boot] The daemon is still starting in the background."
fi

# Hand back to the foreground CUPS process
wait $CUPS_PID
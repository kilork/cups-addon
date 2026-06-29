FROM ghcr.io/home-assistant/base:latest

# Add env
ENV LANG=C.UTF-8
# Set CUPS environment variables for data persistence (HA shared directory)
ENV CUPS_DATADIR=/share/cups
ENV CUPS_CACHEDIR=/share/cups/cache
ENV CUPS_LOGDIR=/share/cups/logs
ENV CUPS_STATEDIR=/share/cups/state
ENV CUPS_SERVERROOT=/share/cups/config

# Setup base
# Note: hplip, foomatic-db, and foomatic-db-ppds were removed from Alpine 3.23
# and are no longer available in stable repositories.
# Gutenprint covers most printer models as a replacement.
RUN \
    echo "http://dl-cdn.alpinelinux.org/alpine/v$(cat /etc/alpine-release | cut -d'.' -f1,2)/main" > /etc/apk/repositories && \
    echo "http://dl-cdn.alpinelinux.org/alpine/v$(cat /etc/alpine-release | cut -d'.' -f1,2)/community" >> /etc/apk/repositories && \
    apk update && \
    apk add --no-cache \
        cups \
        cups-filters \
        ghostscript \
        libjpeg-turbo \
        net-snmp \
        epson-inkjet-printer-escpr \
        gutenprint \
        gcompat \
        libstdc++ \
        dpkg \
        jq && \
    \
    # Build Splix from git master (Alpine package 2.0.0 is too old —
    # lacks Samsung M2020 support added in Oct 2025 commit 206e283)
    BUILD_DEPS="g++ make cups-dev git" && \
    apk add --no-cache $BUILD_DEPS && \
    git clone --depth 1 https://github.com/OpenPrinting/splix.git /tmp/splix && \
    cd /tmp/splix && \
    make DISABLE_JBIG=1 && \
    cp optimized/rastertoqpdl /usr/lib/cups/filter/ && \
    cp optimized/pstoqpdl /usr/lib/cups/filter/ 2>/dev/null; \
    chmod 755 /usr/lib/cups/filter/rastertoqpdl && \
    cd / && \
    rm -rf /tmp/splix && \
    apk del $BUILD_DEPS

# Copy data
COPY rootfs /

# Expose CUPS web interface port
EXPOSE 631

HEALTHCHECK \
    CMD lpstat -r || exit 1

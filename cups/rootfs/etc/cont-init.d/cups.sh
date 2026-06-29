#!/usr/bin/with-contenv bash
# CUPS Print Server — setup is handled by cups-api, this just hands off.
exec /opt/cups-api/cups-api

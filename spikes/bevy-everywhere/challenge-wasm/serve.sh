#!/bin/sh
# Serves the spike dir on 127.0.0.1:8766 with a request log (shows whether a
# .meta probe or a second GLB fetch ever hits the network).
SPIKE=$(cd "$(dirname "$0")" && pwd)
cd "$SPIKE"
exec python3 -m http.server 8766 --bind 127.0.0.1 >> logs/http.log 2>&1

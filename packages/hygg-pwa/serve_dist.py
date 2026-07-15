#!/usr/bin/env python3
"""Static SPA server for the hygg-pwa dist/ bundle, bound to 0.0.0.0:8080.
Serves real files; falls back to index.html for client-side routes; sets the
wasm MIME type so the module instantiates.

Access logs are written to the project's ./data/logs/hygg-pwa/ directory (one
directory per service), rotated daily and retained for 30 days. Override the
base log directory with HYGG_LOG_DIR."""
import http.server
import logging
import os
import socketserver
import sys
from logging.handlers import TimedRotatingFileHandler

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "dist")
# Project root is two levels up from packages/hygg-pwa/, so logs land under
# ./data/logs regardless of the working directory the server is launched from.
PROJECT_ROOT = os.path.dirname(os.path.dirname(HERE))
LOG_BASE = os.environ.get("HYGG_LOG_DIR") or os.path.join(PROJECT_ROOT, "data", "logs")
LOG_DIR = os.path.join(LOG_BASE, "hygg-pwa")


def build_logger():
    os.makedirs(LOG_DIR, exist_ok=True)
    logger = logging.getLogger("hygg-pwa")
    logger.setLevel(logging.INFO)
    logger.propagate = False
    fmt = logging.Formatter("%(asctime)s %(message)s", "%Y-%m-%dT%H:%M:%S%z")
    # Daily rotation, 30 days retained (yesterday..-30d plus today's active file).
    file_handler = TimedRotatingFileHandler(
        os.path.join(LOG_DIR, "access.log"),
        when="midnight",
        interval=1,
        backupCount=30,
        encoding="utf-8",
    )
    file_handler.setFormatter(fmt)
    console = logging.StreamHandler(sys.stdout)
    console.setFormatter(fmt)
    logger.addHandler(file_handler)
    logger.addHandler(console)
    return logger


LOGGER = build_logger()


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=ROOT, **k)

    def send_head(self):
        path = self.path.split("?")[0].lstrip("/")
        full = os.path.join(ROOT, path)
        # SPA fallback: unknown, extension-less paths -> index.html
        if path and not os.path.isfile(full) and "." not in os.path.basename(path):
            self.path = "/index.html"
        return super().send_head()

    def end_headers(self):
        # The HTML shell and service worker are NOT content-hashed, so they must
        # always revalidate — otherwise the browser keeps serving an old
        # index.html that points at stale wasm and never picks up a new build
        # (there's no service worker on a plain-HTTP LAN origin to force it).
        # Hashed assets (wasm/js/css) are immutable and cache forever.
        p = self.path.split("?")[0]
        base = os.path.basename(p)
        if p in ("/", "/index.html") or base in ("index.html", "sw.js", "manifest.webmanifest"):
            self.send_header("Cache-Control", "no-cache")
        elif "." in base:
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        super().end_headers()

    # Route request/error logging through the rotating file logger.
    def log_message(self, fmt, *args):
        LOGGER.info("%s - %s", self.address_string(), fmt % args)


Handler.extensions_map.update({
    ".wasm": "application/wasm",
    ".js": "text/javascript",
    ".webmanifest": "application/manifest+json",
})


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True


with Server(("0.0.0.0", 8080), Handler) as httpd:
    LOGGER.info("hygg-pwa serving dist/ at http://0.0.0.0:8080 (logs: %s)", LOG_DIR)
    httpd.serve_forever()

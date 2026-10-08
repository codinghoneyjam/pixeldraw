# <META - FILE SUMMARY - Draw Tool v2 launcher: free-port server + auto-open browser>
"""One-command launcher for the Draw Tool v2 editor.

The editor is plain ESM, so it cannot be opened over file:// (Chromium blocks ES
module imports cross-origin). This script removes that footgun: it picks a free
loopback port, serves this directory, and opens the default browser.

Usage:
    python run.py                 # serve + auto-open browser (default)
    python run.py --port 8000     # try a specific port first
    python run.py --no-browser    # serve only, print the URL

Everything is stdlib-only. Ctrl+C shuts the server down.
"""
from __future__ import annotations

import argparse
import socket
import sys
import threading
import webbrowser
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DEFAULT_PORTS = (8000, 8001, 8002, 8080, 8888)

# <META - ROLE : Check if is port free | L30-43>
def is_port_free(port: int, host: str = "127.0.0.1") -> bool:
    """True when nothing is already bound to `port`.

    Deliberately does NOT set SO_REUSEADDR. On Windows that flag lets a probe bind
    straight over a live listener, which would report a busy port as free and make
    the launcher hand back a port it cannot actually serve on. Erring toward
    "busy" only costs an extra port hop.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind((host, port))
        except OSError:
            return False
    return True

# <META - ROLE : Execute pick port | L46-57>
def pick_port(preferred: int | None, host: str = "127.0.0.1") -> int:
    """Return `preferred` when free, else the first free default port, else any free port."""
    if preferred is not None:
        if is_port_free(preferred, host):
            return preferred
        print(f"[draw-tool] port {preferred} is busy, looking for another one")
    for port in DEFAULT_PORTS:
        if is_port_free(port, host):
            return port
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind((host, 0))
        return int(sock.getsockname()[1])

class QuietHandler(SimpleHTTPRequestHandler):
    """SimpleHTTPRequestHandler without the per-request stderr spam."""

    # Windows' mimetypes registry maps .mjs to text/plain, which strict ES-module
    # checking rejects. Pin JS extensions to a JS MIME type.
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".mjs": "text/javascript",
    }

    # <META - ROLE : Execute log message | L63-64>
    def log_message(self, format: str, *args: object) -> None:
        pass

# <META - ROLE : Execute serve | L67-71>
def serve(port: int, host: str) -> ThreadingHTTPServer:
    handler = partial(QuietHandler, directory=str(ROOT))
    httpd = ThreadingHTTPServer((host, port), handler)
    httpd.daemon_threads = True
    return httpd

# <META - ROLE : Execute open browser later | L74-76>
def open_browser_later(url: str, delay: float = 0.6) -> None:
    """Open the browser once the server is actually accepting connections."""
    threading.Timer(delay, lambda: webbrowser.open(url)).start()

# <META - ROLE : Execute main | L79-110>
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="run.py",
        description="Serve the Draw Tool v2 editor and open it in your browser.",
    )
    parser.add_argument("--port", type=int, default=None, help="preferred port (default: auto)")
    parser.add_argument("--host", default="127.0.0.1", help="bind address (default: 127.0.0.1)")
    parser.add_argument("--no-browser", action="store_true", help="do not open a browser")
    args = parser.parse_args(argv)

    if not (ROOT / "index.html").is_file():
        print(f"[draw-tool] ERROR: index.html not found in {ROOT}", file=sys.stderr)
        return 1

    port = pick_port(args.port, args.host)
    url = f"http://{args.host}:{port}/"
    httpd = serve(port, args.host)

    print("[draw-tool] Draw Tool v2 is running", flush=True)
    print(f"[draw-tool]   {url}", flush=True)
    print("[draw-tool]   Ctrl+C to stop", flush=True)
    if not args.no_browser:
        open_browser_later(url)

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n[draw-tool] stopped")
    finally:
        httpd.shutdown()
        httpd.server_close()
    return 0

if __name__ == "__main__":
    raise SystemExit(main())

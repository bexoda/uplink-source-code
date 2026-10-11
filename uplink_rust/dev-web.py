"""Static file server for the browser build: `python dev-web.py <dir>` on http://127.0.0.1:8765.

Plain `python -m http.server` takes MIME types from the Windows registry, which on many machines
says .js is text/plain; browsers then refuse to load the page's JavaScript module.
"""
import functools
import http.server
import sys


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".wasm": "application/wasm",
    }


http.server.ThreadingHTTPServer(
    ("127.0.0.1", 8765), functools.partial(Handler, directory=sys.argv[1])
).serve_forever()

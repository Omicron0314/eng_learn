"""Loopback-only CORS fixture. No provider requests or user data."""
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        if self.path == '/allow':
            self.send_header('Access-Control-Allow-Origin', 'http://127.0.0.1:4319')
        self.end_headers()
        self.wfile.write(b'{"fixture":true}')

HTTPServer(('127.0.0.1', 4320), Handler).serve_forever()

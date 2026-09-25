#!/usr/bin/env python3

import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query)

        print(f"\nGET {parsed.path}")

        if params:
            print("Parameters:")
            for name, values in params.items():
                for value in values:
                    print(f"  {name} = {value}")
        else:
            print("No parameters")

        # Current UTC time as Unix timestamp (seconds since 1970-01-01 UTC)
        timestamp = int(time.time())

        print(f"UTC timestamp: {timestamp}")

        body = f"{timestamp}\n".encode()

        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()

        self.wfile.write(body)


server = HTTPServer(("0.0.0.0", 8080), Handler)

print("Listening on port 8080...")
server.serve_forever()

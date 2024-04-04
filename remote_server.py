from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import sys

class PayloadHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        print("triggered")
        # Read request body (assuming payload is JSON)
        content_length = int(self.headers.get('Content-Length', 0))
        payload_data = self.rfile.read(content_length).decode()
        try:
            payload = json.loads(payload_data)
            print(f"Received payload: {payload}")
            self.send_response(200)
            self.send_header("Content-type", "text/plain")
            self.end_headers()
            self.wfile.write(b"HTTP OK from server a")
        except json.JSONDecodeError:
            print(f"Error parsing payload: {payload_data}")
            self.send_error(400, "Invalid JSON payload")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python server.py <port>")
        sys.exit(1)

    print("starting server...")
    port = int(sys.argv[1])
    with HTTPServer(("", port), PayloadHandler) as httpd:
        print(f"Serving on port {port}")
        httpd.serve_forever()

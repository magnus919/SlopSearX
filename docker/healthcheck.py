"""Health check script for Docker HEALTHCHECK."""
import sys
import urllib.request

port = 8080
try:
    url = "http://127.0.0.1:" + str(port) + "/health"
    resp = urllib.request.urlopen(url)
    sys.exit(0 if resp.status == 200 else 1)
except Exception:
    sys.exit(1)

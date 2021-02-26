#!/usr/bin/env python3

###
# Simple https server for serving json, images and DASH
##

import http.server
import ssl

handler = http.server.SimpleHTTPRequestHandler
handler.extensions_map = {
    '.manifest': 'text/cache-manifest',
    '.html': 'text/html',
    ''
    '.png': 'image/png',
    '.jpg': 'image/jpg',
    '.jpeg': 'image/jpeg',
    '.svg': 'image/svg+xml',
    '.css': 'text/css',
    '.js': 'application/x-javascript',
    '.mpd': 'application/dash+xml',  # Adaptive streaming / DASH
    '': 'application/octet-stream',  # Default
}

server_address = ('0.0.0.0', 4443)
httpd = http.server.HTTPServer(server_address, handler)
httpd.socket = ssl.wrap_socket(httpd.socket,
                               server_side=True,
                               certfile='localhost.pem',
                               ssl_version=ssl.PROTOCOL_TLS)
httpd.serve_forever()

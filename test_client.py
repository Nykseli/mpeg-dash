#!/usr/bin/env python3

import http.client
import ssl
import threading

CONNECTIONS = 1000
THREADS = []

# Do not verify the cert for now
no_verif = ssl._create_unverified_context()

def get_call():
    for _ in range(10):
        c = http.client.HTTPSConnection("localhost", 8443, context=no_verif )
        c.request("GET", "/test_data/bunny/stream.mpd")
        response = c.getresponse()
        if response.status != 200:
            print("CONNECTION FAILED")
            print(response.status, response.reason, "\n")
        data = response.read()
        assert len(data) > 0


for i in range(CONNECTIONS):
    THREADS.append(threading.Thread(target=get_call))

for t in THREADS:
    t.start()

for t in THREADS:
    t.join()

#!/usr/bin/env python3
"""Exercise real browser chrome in disposable profiles; never use the user's profile."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time


def rpc(binary, env, method, value):
    out = subprocess.run([binary, 'core', method], input=json.dumps(value), text=True,
                         capture_output=True, env=env, check=True)
    result = json.loads(out.stdout)
    if not result.get('ok'):
        raise RuntimeError(result)
    return result['data']


class Marionette:
    def __init__(self, port):
        self.sock = socket.create_connection(('127.0.0.1', port), timeout=3)
        self.stream = self.sock.makefile('rb')
        self.read()
        self.index = 0

    def read(self):
        prefix = b''
        while not prefix.endswith(b':'):
            chunk = self.stream.read(1)
            if not chunk:
                raise RuntimeError('browser disconnected')
            prefix += chunk
        return json.loads(self.stream.read(int(prefix[:-1])))

    def send(self, name, value):
        self.index += 1
        data = json.dumps([0, self.index, name, value]).encode()
        self.sock.sendall(str(len(data)).encode() + b':' + data)
        result = self.read()
        if result[2]:
            raise RuntimeError(result[2])
        return result[3]

    def close(self):
        self.stream.close()
        self.sock.close()


def rgb(value, background=(1, 1, 1)):
    if value.startswith('#'):
        return tuple(int(value[i:i+2], 16) / 255 for i in (1, 3, 5))
    parts = value[value.index('(')+1:value.index(')')].replace(',', ' ').replace('/', ' ').split()
    if value.startswith('color(srgb '):
        channels = [float(x) for x in parts[1:4]]
        alpha = float(parts[4]) if len(parts) > 4 else 1
    else:
        channels = [float(x) / 255 for x in parts[:3]]
        alpha = float(parts[3]) if len(parts) > 3 else 1
    return tuple(c * alpha + b * (1-alpha) for c, b in zip(channels, background))


def luminance(color):
    values = [c / 12.92 if c <= .04045 else ((c+.055)/1.055)**2.4 for c in color]
    return sum(c*w for c, w in zip(values, (.2126, .7152, .0722)))


if __name__ == '__main__':
    import runpy
    runpy.run_path(str(Path(__file__).with_name('browser-native-accent.py')), run_name='__main__')

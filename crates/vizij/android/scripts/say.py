"""A `say` through the device's open local bridge, followed to its end.

    say.py <port> <text>

Connects to ws://127.0.0.1:<port> (`adb forward` reaches the app's), invokes
`say` with <text>, and reads the device's feed until the run's status key
leaves running. Prints the utterance the face reports as speaking, the
visemes it shows, and the run's outcome; exits 0 when the run succeeded with
the utterance reported and a viseme other than silence shown, 1 otherwise.

The standard library alone: the CI runner's Python has no WebSocket package.
"""

import base64
import json
import os
import socket
import struct
import sys
import time

DEADLINE_SECS = 60
SILENCE = "sil"
# arora-behavior's `Status` variants, as the run's status key carries them.
SUCCESS = "766e9e9a-446d-4e46-83e6-14b7ca101169"
RUNNING = "acd79ec6-0c44-401a-82f8-5da5422d3eec"


class Socket:
    """A WebSocket client over one TCP connection: text frames out, masked
    as RFC 6455 requires of a client; text frames in, reassembled."""

    def __init__(self, port):
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=10)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            (
                "GET / HTTP/1.1\r\n"
                f"Host: 127.0.0.1:{port}\r\n"
                "Upgrade: websocket\r\nConnection: Upgrade\r\n"
                f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = self.sock.recv(1)
            if not chunk:
                raise ConnectionError("closed during the handshake")
            head += chunk
        status = head.split(b"\r\n", 1)[0]
        if b" 101 " not in status:
            raise ConnectionError(f"handshake refused: {status!r}")

    def send(self, message):
        payload = json.dumps(message).encode()
        mask = os.urandom(4)
        n = len(payload)
        if n < 126:
            header = struct.pack("!BB", 0x81, 0x80 | n)
        elif n < 1 << 16:
            header = struct.pack("!BBH", 0x81, 0x80 | 126, n)
        else:
            header = struct.pack("!BBQ", 0x81, 0x80 | 127, n)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(header + mask + masked)

    def close(self):
        """A close frame (status 1000), then the connection."""
        self.sock.sendall(bytes([0x88, 0x82]) + b"\0\0\0\0" + struct.pack("!H", 1000))
        self.sock.close()

    def _exact(self, n):
        data = b""
        while len(data) < n:
            chunk = self.sock.recv(n - len(data))
            if not chunk:
                raise ConnectionError("closed")
            data += chunk
        return data

    def receive(self):
        """The next JSON message; control frames are answered or skipped."""
        message = b""
        while True:
            first, second = self._exact(2)
            opcode = first & 0x0F
            n = second & 0x7F
            if n == 126:
                (n,) = struct.unpack("!H", self._exact(2))
            elif n == 127:
                (n,) = struct.unpack("!Q", self._exact(8))
            payload = self._exact(n)
            if opcode == 0x8:
                raise ConnectionError("closed by the device")
            if opcode == 0x9:
                self.sock.sendall(bytes([0x8A, 0x80 | len(payload)]) + b"\0\0\0\0" + payload)
                continue
            if opcode == 0xA:
                continue
            message += payload
            if first & 0x80:
                return json.loads(message)


def text_of(value):
    return value.get("str") if isinstance(value, dict) else None


def variant_of(status):
    return (status or {}).get("enum", {}).get("variant_id")


def main():
    port, text = int(sys.argv[1]), " ".join(sys.argv[2:])
    ws = Socket(port)
    ws.send({"type": "list_methods"})
    while (reply := ws.receive())["type"] != "list_methods_resp":
        pass
    names = [method["path"] for method in reply["methods"]]
    if "say" not in names:
        print(f"say: not among the device's methods {names}")
        return 1

    ws.send({"type": "invoke", "method": "say", "args": {"text": {"str": text}}, "request_id": "say"})
    while (reply := ws.receive())["type"] != "invoke_resp":
        pass
    if not reply.get("success"):
        print(f"say: the invoke failed: {reply}")
        return 1
    fields = reply["value"]["keyvalue"]["fields"]
    status_key = fields["status"]["value"]["str"]
    print(f"say: run {fields['run']['value']['str']}, status at {status_key}")

    spoken, visemes, status = set(), [], None
    deadline = time.monotonic() + DEADLINE_SECS
    while time.monotonic() < deadline:
        message = ws.receive()
        if message["type"] != "values_changed":
            continue
        for key, value in message["values"].items():
            if key == status_key:
                status = value
            elif key.endswith("/standard/vizij/speech") and text_of(value):
                spoken.add(text_of(value))
            elif key.endswith("/standard/vizij/viseme"):
                shape = text_of(value)
                if shape and (not visemes or visemes[-1] != shape):
                    visemes.append(shape)
        if status is not None and variant_of(status) != RUNNING:
            break
    ws.close()
    print(f"say: speech reported {sorted(spoken)}")
    print(f"say: visemes {' '.join(visemes)}")
    print(f"say: status {json.dumps(status)}")
    succeeded = variant_of(status) == SUCCESS
    shown = any(shape != SILENCE for shape in visemes)
    return 0 if succeeded and text in spoken and shown else 1


if __name__ == "__main__":
    sys.exit(main())

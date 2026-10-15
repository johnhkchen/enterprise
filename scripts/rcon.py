#!/usr/bin/env python3
"""Send commands to the dev server over RCON and print the replies.

    python3 scripts/rcon.py "time set day" "enterprise lockers 7 -60 7"

The dev server (just server) enables RCON on localhost:25598 with password "dev";
see core/run/server.properties, written by `just server-setup`.
"""

import socket
import struct
import sys

HOST, PORT, PASSWORD = "127.0.0.1", 25598, "dev"


def packet(sock: socket.socket, request_id: int, kind: int, body: str) -> None:
    data = struct.pack("<ii", request_id, kind) + body.encode() + b"\x00\x00"
    sock.sendall(struct.pack("<i", len(data)) + data)


def read(sock: socket.socket) -> tuple[int, str]:
    size = struct.unpack("<i", _exactly(sock, 4))[0]
    data = _exactly(sock, size)
    request_id = struct.unpack("<i", data[:4])[0]
    return request_id, data[8:-2].decode(errors="replace")


MARKER = 0x7E57


def command(sock: socket.socket, request_id: int, text: str) -> str:
    """Runs a command and returns its whole reply. The server splits a long reply into 4 KB packets with no end
    mark, so a request of an unknown kind follows it: the server answers that only once the reply is all sent, and
    everything before its answer is the reply. (Reading one packet left the rest to be taken for the next reply.)"""
    packet(sock, request_id, 2, text.removeprefix("/"))
    # Only once the reply has begun: the server reads one packet per read, and drops a client whose two came together.
    got_id, body = read(sock)
    parts = [body] if got_id == request_id else []
    packet(sock, MARKER, 0, "")
    while True:
        got_id, body = read(sock)
        if got_id == MARKER:
            return "".join(parts)
        if got_id == request_id:
            parts.append(body)


def _exactly(sock: socket.socket, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("server closed the connection")
        buf += chunk
    return buf


def main() -> None:
    with socket.create_connection((HOST, PORT), timeout=10) as sock:
        packet(sock, 1, 3, PASSWORD)
        if read(sock)[0] == -1:
            sys.exit("RCON login failed")
        for i, text in enumerate(sys.argv[1:], start=2):
            print(f"> {text}\n{command(sock, i, text)}")


if __name__ == "__main__":
    main()

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
        for i, command in enumerate(sys.argv[1:], start=2):
            packet(sock, i, 2, command.removeprefix("/"))
            print(f"> {command}\n{read(sock)[1]}")


if __name__ == "__main__":
    main()

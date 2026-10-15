"""Compositor-delivered pointer and keyboard input through wlroots virtual input protocols."""
import os
import socket
import struct
import time

BUTTON_LEFT = 0x110
KEY_ESC, KEY_TAB, KEY_ENTER, KEY_SPACE = 1, 15, 28, 57
KEYMAP = (b'xkb_keymap { xkb_keycodes { include "evdev+aliases(qwerty)" }; '
          b'xkb_types { include "complete" }; xkb_compat { include "complete" }; '
          b'xkb_symbols { include "pc+us+inet(evdev)" }; };\0')


def encode_string(value):
    data = value.encode() + b"\0"
    return struct.pack("<I", len(data)) + data + b"\0" * (-len(data) % 4)


class VirtualInput:
    def __init__(self, width, height, runtime_dir, display):
        self.width, self.height = width, height
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.settimeout(10)
        try:
            self.connect(os.path.join(runtime_dir, display))
        except BaseException:
            self.socket.close()
            raise

    def connect(self, path):
        self.socket.connect(path)
        self.buffer, self.next_id = b"", 2
        registry = self.allocate()
        self.send(1, 1, struct.pack("<I", registry))
        globals_ = {}

        def announce(target, opcode, body):
            if target == registry and opcode == 0:
                name, length = struct.unpack("<II", body[:8])
                globals_[body[8:7 + length].decode()] = name
        self.roundtrip(announce)
        missing = {"wl_seat", "zwlr_virtual_pointer_manager_v1",
                   "zwp_virtual_keyboard_manager_v1"} - set(globals_)
        if missing:
            raise ValueError("compositor lacks virtual input globals: " + ", ".join(sorted(missing)))

        def bind(interface):
            target = self.allocate()
            self.send(registry, 0, struct.pack("<I", globals_[interface]) + encode_string(interface)
                      + struct.pack("<II", 1, target))
            return target
        seat, pointers, keyboards = (bind(name) for name in (
            "wl_seat", "zwlr_virtual_pointer_manager_v1", "zwp_virtual_keyboard_manager_v1"))
        self.pointer, self.keyboard = self.allocate(), self.allocate()
        self.send(pointers, 0, struct.pack("<II", seat, self.pointer))
        self.send(keyboards, 0, struct.pack("<II", seat, self.keyboard))
        keymap = os.memfd_create("resina-keymap")
        try:
            os.write(keymap, KEYMAP)
            self.send(self.keyboard, 0, struct.pack("<II", 1, len(KEYMAP)), keymap)
        finally:
            os.close(keymap)
        self.roundtrip()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def allocate(self):
        self.next_id += 1
        return self.next_id - 1

    def send(self, target, opcode, payload=b"", descriptor=None):
        message = struct.pack("<II", target, (8 + len(payload)) << 16 | opcode) + payload
        if descriptor is None:
            self.socket.sendall(message)
        else:
            self.socket.sendmsg([message], [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                             struct.pack("<i", descriptor))])

    def receive(self, size):
        while len(self.buffer) < size:
            chunk = self.socket.recv(65536)
            if not chunk:
                raise ValueError("compositor closed the virtual input connection")
            self.buffer += chunk

    def roundtrip(self, handler=None):
        callback = self.allocate()
        self.send(1, 0, struct.pack("<I", callback))
        while True:
            self.receive(8)
            target, word = struct.unpack("<II", self.buffer[:8])
            self.receive(word >> 16)
            body, self.buffer = self.buffer[8:word >> 16], self.buffer[word >> 16:]
            if target == 1 and word & 0xFFFF == 0:
                failed, code, length = struct.unpack("<III", body[:12])
                message = body[12:11 + length].decode(errors="replace")
                raise ValueError(f"compositor rejected virtual input: error {code} on object {failed}: {message}")
            if target == callback:
                return
            if handler:
                handler(target, word & 0xFFFF, body)

    def timestamp(self):
        return int(time.monotonic() * 1000) & 0xFFFFFFFF

    def move(self, x, y):
        if not (0 <= x < self.width and 0 <= y < self.height):
            raise ValueError("pointer target lies outside the output")
        self.send(self.pointer, 1, struct.pack("<IIIII", self.timestamp(), round(x), round(y),
                                               self.width, self.height))
        self.send(self.pointer, 4)
        self.roundtrip()

    def button(self, pressed, button=BUTTON_LEFT):
        self.send(self.pointer, 2, struct.pack("<III", self.timestamp(), button, int(pressed)))
        self.send(self.pointer, 4)
        self.roundtrip()

    def key(self, code, pressed):
        self.send(self.keyboard, 1, struct.pack("<III", self.timestamp(), code, int(pressed)))
        self.roundtrip()

    def close(self):
        if self.socket.fileno() < 0:
            return
        try:
            self.send(self.pointer, 8)
            self.send(self.keyboard, 3)
            self.roundtrip()
        finally:
            self.socket.close()

"""Serve the XDG Desktop Portal Settings backend interface with values set over D-Bus.

The real `xdg-desktop-portal` frontend loads this backend through `XDG_DESKTOP_PORTAL_DIR`, so clients still read
`org.freedesktop.portal.Settings` from the frontend. The values start as "no preference" and change only through
`org.resina.PortalSettings.Set`, which emits `SettingChanged` like a desktop backend does.
"""
import argparse
from pathlib import Path
import subprocess
import sys
import time

APPEARANCE = "org.freedesktop.appearance"
BUS_NAME = "org.freedesktop.impl.portal.desktop.resinatest"
FRONTEND = "org.freedesktop.portal.Desktop"
PORTAL_ID = "resinatest"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
SETTINGS = "org.freedesktop.impl.portal.Settings"
CONTROL_PATH = "/org/resina/PortalSettings"
CONTROL = "org.resina.PortalSettings"
PROPERTIES = "org.freedesktop.DBus.Properties"
NOT_FOUND = "org.freedesktop.portal.Error.NotFound"
INVALID = "org.freedesktop.DBus.Error.InvalidArgs"
UNKNOWN = "org.freedesktop.DBus.Error.UnknownMethod"
# The portal types these keys as uint32: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html
VALUES = {"color-scheme": (0, 1, 2), "contrast": (0, 1), "reduced-motion": (0, 1)}
VERSION = 1


def matches(namespace, patterns):
    if not patterns or "" in patterns:
        return True
    return any(namespace == pattern or (pattern.endswith(".*") and namespace.startswith(pattern[:-1]))
               for pattern in patterns)


class Settings:
    def __init__(self, keys):
        unknown = sorted(set(keys) - set(VALUES))
        if unknown or not keys:
            raise ValueError(f"provided keys must be a non-empty subset of {sorted(VALUES)}, not {list(keys)}")
        self.values = {APPEARANCE: {key: 0 for key in keys}}

    def read_all(self, patterns):
        return {namespace: {key: ("u", value) for key, value in keys.items()}
                for namespace, keys in self.values.items() if matches(namespace, patterns)}

    def read(self, namespace, key):
        try:
            return "u", self.values[namespace][key]
        except KeyError:
            raise LookupError(f"{namespace} {key} is not provided") from None

    def set(self, namespace, key, variant):
        if key not in self.values.get(namespace, {}):
            raise LookupError(f"{namespace} {key} is not provided")
        signature, value = variant
        if signature != "u" or value not in VALUES[key]:
            raise ValueError(f"{namespace} {key} takes a uint32 in {VALUES[key]}, not {signature} {value!r}")
        if self.values[namespace][key] == value:
            return False
        self.values[namespace][key] = value
        return True

    def handle(self, path, interface, member, signature, body):
        """Answer one method call as (reply, signals): reply is ("return", signature, body) or ("error", name, text)."""
        try:
            if (path, interface, member, signature) == (PORTAL_PATH, SETTINGS, "ReadAll", "as"):
                return ("return", "a{sa{sv}}", (self.read_all(body[0]),)), []
            if (path, interface, member, signature) == (PORTAL_PATH, SETTINGS, "Read", "ss"):
                return ("return", "v", (self.read(*body),)), []
            if (path, interface, member, signature) == (PORTAL_PATH, PROPERTIES, "Get", "ss"):
                if body != (SETTINGS, "version"):
                    raise LookupError(f"no property {body[1]} on {body[0]}")
                return ("return", "v", (("u", VERSION),)), []
            if (path, interface, member, signature) == (PORTAL_PATH, PROPERTIES, "GetAll", "s"):
                return ("return", "a{sv}", ({"version": ("u", VERSION)} if body[0] == SETTINGS else {},)), []
            if (path, interface, member, signature) == (CONTROL_PATH, CONTROL, "Set", "ssv"):
                changed = self.set(*body)
                return ("return", "", ()), [body] if changed else []
        except LookupError as error:
            return ("error", NOT_FOUND, str(error)), []
        except ValueError as error:
            return ("error", INVALID, str(error)), []
        return ("error", UNKNOWN, f"{interface}.{member}({signature}) is not served at {path}"), []


def portal_directory(directory):
    """Write the frontend's portal definition and configuration selecting only this backend for Settings."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    (directory / f"{PORTAL_ID}.portal").write_text(
        f"[portal]\nDBusName={BUS_NAME}\nInterfaces={SETTINGS};\n", encoding="utf-8")
    (directory / "portals.conf").write_text(f"[preferred]\n{SETTINGS}={PORTAL_ID}\n", encoding="utf-8")
    return directory


def start(environment, keys, log):
    """Start the backend on the environment's session bus and return it once it owns its name."""
    process = subprocess.Popen([sys.executable, __file__, "--keys", *keys], env=environment, stdout=subprocess.PIPE,
                               stderr=log, text=True, start_new_session=True)
    line = process.stdout.readline().strip()
    process.stdout.close()
    if line != "ready":
        process.kill()
        process.wait()
        raise ValueError(f"the test Settings backend did not start: {line or 'no output'}")
    return process


def owned(environment, name):
    result = subprocess.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus", "--object-path",
                             "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.NameHasOwner", name],
                            env=environment, capture_output=True, text=True, timeout=10)
    return result.stdout.strip() == "(true,)"


def start_frontend(portal, environment, directory, log, timeout=15):
    """Start the frontend with only this backend and return it once it owns the portal name.

    Waiting without calling the portal keeps the bus from activating a packaged frontend that would not use it.
    """
    process = subprocess.Popen([str(portal)], env=dict(environment, XDG_DESKTOP_PORTAL_DIR=str(directory)),
                               stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    deadline = time.monotonic() + timeout
    while not owned(environment, FRONTEND):
        if process.poll() is not None or time.monotonic() > deadline:
            process.kill()
            process.wait()
            raise ValueError(f"the xdg-desktop-portal frontend did not take {FRONTEND} "
                             f"(exit status {process.returncode})")
        time.sleep(.1)
    return process


def set_value(environment, key, value):
    subprocess.run(["gdbus", "call", "--session", "--dest", BUS_NAME, "--object-path", CONTROL_PATH,
                    "--method", f"{CONTROL}.Set", APPEARANCE, key, f"<uint32 {value}>"],
                   env=environment, check=True, capture_output=True, timeout=10)


def serve(keys):
    from jeepney import DBusAddress, HeaderFields, MessageType, new_error, new_method_return, new_signal
    from jeepney.bus_messages import message_bus
    from jeepney.io.blocking import open_dbus_connection

    settings = Settings(keys)
    connection = open_dbus_connection(bus="SESSION")
    do_not_queue, primary_owner = 4, 1
    reply = connection.send_and_get_reply(message_bus.RequestName(BUS_NAME, do_not_queue))
    if reply.body != (primary_owner,):
        print(f"{BUS_NAME} is already owned", flush=True)
        return 1
    print("ready", flush=True)
    emitter = DBusAddress(PORTAL_PATH, interface=SETTINGS)
    while True:
        message = connection.receive()
        if message.header.message_type != MessageType.method_call:
            continue
        fields = message.header.fields
        (kind, first, second), signals = settings.handle(
            fields.get(HeaderFields.path), fields.get(HeaderFields.interface), fields.get(HeaderFields.member),
            fields.get(HeaderFields.signature, ""), message.body)
        connection.send(new_method_return(message, first, second) if kind == "return"
                        else new_error(message, first, "s", (second,)))
        for body in signals:
            connection.send(new_signal(emitter, "SettingChanged", "ssv", body))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keys", nargs="+", choices=sorted(VALUES), required=True,
                        help=f"{APPEARANCE} keys to provide")
    return serve(parser.parse_args().keys)


if __name__ == "__main__":
    sys.exit(main())

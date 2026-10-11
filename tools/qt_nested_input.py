"""Explicit XTest injection into a measured nested Wayland client, never hardware evidence."""
import ctypes
import ctypes.util
import time


def exercise(app, window, parent_display, parent_window, wait, observe, check):
    from PySide6.QtCore import QEvent, QObject, Qt
    if app.platformName() != "wayland" or window.devicePixelRatio() != 1:
        raise ValueError("parent input requires an actual Wayland Qt client at DPR 1")
    xlib = ctypes.CDLL(ctypes.util.find_library("X11"))
    xtest = ctypes.CDLL(ctypes.util.find_library("Xtst"))
    xlib.XOpenDisplay.argtypes = [ctypes.c_char_p]
    xlib.XOpenDisplay.restype = ctypes.c_void_p
    xlib.XSetInputFocus.argtypes = [ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
    xlib.XSync.argtypes = [ctypes.c_void_p,ctypes.c_int]
    xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xlib.XKeysymToKeycode.argtypes = [ctypes.c_void_p,ctypes.c_ulong]
    xlib.XKeysymToKeycode.restype = ctypes.c_uint
    xtest.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong]
    xtest.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
    xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
    display = xlib.XOpenDisplay(parent_display.encode())
    if not display:
        raise ValueError("cannot open parent X11 display")
    records = []
    class Observer(QObject):
        def eventFilter(self, watched, event):
            if event.type() in (QEvent.MouseMove,QEvent.MouseButtonPress,QEvent.MouseButtonRelease):
                records.append((event.type(),event.spontaneous(),event.position().x(),event.position().y()))
            if event.type() in (QEvent.KeyPress,QEvent.KeyRelease):
                records.append((event.type(),event.spontaneous(),event.key(),event.nativeScanCode()))
            return False
    observer = Observer(window)
    window.installEventFilter(observer)
    held_button, held_key = False, 0
    def move(x,y):
        if not xtest.XTestFakeMotionEvent(display,-1,round(x),round(y),0):
            raise ValueError("parent pointer injection failed")
        xlib.XSync(display,False)
    def delivered(start, kind):
        wait(app,lambda:any(record[0] == kind for record in records[start:]),"spontaneous compositor input")
        result = next(record for record in records[start:] if record[0] == kind)
        if not result[1]:
            raise ValueError("injected input bypassed native Wayland delivery")
        return result
    try:
        xlib.XSetInputFocus(display,parent_window,2,0)
        xlib.XSync(display,False)
        origin = None
        for y in range(100,601,100):
            for x in range(100,801,100):
                start = len(records)
                move(x,y)
                deadline = time.monotonic()+.1
                while time.monotonic()<deadline:
                    app.processEvents()
                fresh = [record for record in records[start:] if record[0] == QEvent.MouseMove and record[1]]
                if fresh:
                    origin = (x-fresh[-1][2],y-fresh[-1][3])
                    break
            if origin is not None:
                break
        if origin is None:
            raise ValueError("no spontaneous client pointer coordinates in the nested output")
        # Restore the checked authored configuration and input lifecycle first.
        window.availability(True)
        check()
        baseline = observe()["effects"]
        start = len(records)
        move(origin[0]+120,origin[1]+80)
        if delivered(start,QEvent.MouseMove)[2:] != (120,80):
            raise ValueError("measured parent/client coordinate mapping changed")
        check()
        for down in (True,False):
            start = len(records)
            if not xtest.XTestFakeButtonEvent(display,1,down,0):
                raise ValueError("parent pointer button injection failed")
            held_button = down
            xlib.XSync(display,False)
            delivered(start,QEvent.MouseButtonPress if down else QEvent.MouseButtonRelease)
            result = observe()
            if down and not result["state"]["hold"]:
                raise ValueError("compositor press did not arm the integrated control")
        if observe()["effects"] != baseline+1 or observe()["state"]["hold"]:
            raise ValueError("compositor pointer release did not produce exactly one effect")
        check()
        window.focusCommand()
        wait(app,lambda:observe()["state"]["focused"],"native compositor keyboard focus")
        check()
        code = xlib.XKeysymToKeycode(display,0x20)
        if not code:
            raise ValueError("parent Space key has no native code")
        for down in (True,False):
            start = len(records)
            if not xtest.XTestFakeKeyEvent(display,code,down,0):
                raise ValueError("parent keyboard injection failed")
            held_key = code if down else 0
            xlib.XSync(display,False)
            result = delivered(start,QEvent.KeyPress if down else QEvent.KeyRelease)
            if result[2] != Qt.Key_Space or not result[3]:
                raise ValueError("missing native Space scan code")
        if observe()["effects"] != baseline+2 or observe()["state"]["hold"]:
            raise ValueError("compositor Space release did not produce exactly one effect")
        check()
        print("Integrated control: parent XTest traversed Weston/wl_pointer and wl_keyboard into spontaneous Qt events and exact-once product effects (injected input).")
    finally:
        if held_button:
            xtest.XTestFakeButtonEvent(display,1,False,0)
        if held_key:
            xtest.XTestFakeKeyEvent(display,held_key,False,0)
        xlib.XSync(display,False)
        xlib.XCloseDisplay(display)
        window.removeEventFilter(observer)

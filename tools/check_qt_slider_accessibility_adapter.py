"""Verify the continuous native Slider bridge on actual AT-SPI, not a full control.

Value.Text is separately reported as unsupported on unmodified Qt 6.11.2.
--require-value-text fails closed when complete native intrinsic semantics are
required. Fixture paint and presentation ownership are not component evidence.
"""
import argparse
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from check_slint_native_availability import ACCESSIBLE, PROPERTIES, call, integer_values, payload, references, string_value
from check_qt_native_availability import ACTION


def require(condition, message):
    if not condition:
        raise ValueError(message)


def action_names(reply):
    match = re.fullmatch(r"array\s*\[\s*(.*?)\s*\]", payload(reply), re.S)
    require(match is not None, 'expected native action array')
    pattern = r'struct\s*\{\s*string "([^"\\\n]*)"\s*string "[^"\\\n]*"\s*string "[^"\\\n]*"\s*\}'
    require(not re.sub(pattern, "", match[1]).strip(), 'invalid native action tuple')
    names = re.findall(pattern, match[1])
    require(len(names) == 2 and set(names) == {'Increase', 'Decrease'}, 'expected exactly Increase/Decrease actions: ' + str(names))
    return names


def host(args):
    from PySide6.QtCore import QTimer, QUrl, qVersion
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlComponent, QQmlEngine
    from PySide6.QtQuick import QQuickWindow
    require(qVersion() == '6.11.2', 'requires pinned Qt 6.11.2')
    app = QGuiApplication([])
    engine = QQmlEngine()
    engine.addImportPath(str(args.module_path.resolve()))
    component = QQmlComponent(engine)
    source = '''import QtQuick
import Resina.Runtime 0.1
import "QML_DIRECTORY"
Window {
 id: root; width:460; height:220; visible:true
 property string mode: "initial"
 property int effects: 0
 property int proposals: 0
 property int revision: 1
 property var published: ({revision:"fixture-1",stepSize:.05,valuePolicy:{kind:"continuous"},
   accessibility:{schemaVersion:"0.1.0",role:"slider",name:"Volume fixture",description:"Adapter qualification only.",
   value:{schemaVersion:"0.1.0",minimum:0,maximum:1,value:.4},valueText:"40 percent",
   orientation:"ORIENTATION",state:{enabled:true,readOnly:false,focused:false},focusable:true,
   actions:[{kind:"setValue",available:true},{kind:"increase",available:true},{kind:"decrease",available:true}],relationships:[]}})
 function refresh() {
   if (!loader.item) return;
   const p = JSON.parse(JSON.stringify(published));
   p.accessibility.state.enabled = mode !== "disabled";
   p.accessibility.state.readOnly = mode === "readOnly";
   p.accessibility.actions.forEach(a => a.available = p.accessibility.state.enabled && !p.accessibility.state.readOnly);
   if (!loader.item.publish(JSON.stringify(p))) throw Error(loader.item.lastError);
   if (loader.item.accessibleValueText() !== p.accessibility.valueText) throw Error("public QAccessible value text mismatch");
   loader.item.sourceAvailable = mode !== "withdrawn";
 }
 function report(kind) {
   console.log("RESINA_SLIDER_FIXTURE " + JSON.stringify({kind:kind,mode:mode,effects:effects,proposals:proposals,
     publication:loader.item && loader.item.publication.length ? JSON.parse(loader.item.publication) : null}));
 }
 onModeChanged: {
   if (mode === "destroyed") { loader.active = false; report("configured"); return; }
   if (mode === "recreated") { loader.active = true; }
   else if (mode === "invalid") {
      if (loader.item.publish('{"bad":true}')) throw Error("accepted invalid publication");
   } else refresh();
   report("configured");
 }
 Loader {
   id: loader; x:40; y:40; width:280; height:100
   sourceComponent: Component {
     ResinaSliderAccessibility {
       width:280; height:100
       onAdjustmentRequested: function(source) {
         const intent = JSON.parse(source);
         root.proposals++;
         if (intent.revision !== root.published.revision || intent.candidateBase.value !== root.published.accessibility.value.value)
           throw Error("stale presentation intent");
         const next = JSON.parse(JSON.stringify(root.published));
         const v = intent.candidate.value;
         next.accessibility.value = {schemaVersion:v.schemaVersion,minimum:v.minimum,maximum:v.maximum,value:v.value};
         next.accessibility.valueText = Math.round(v.value * 100) + " percent";
         next.revision = "fixture-" + (++root.revision);
         // Publish the complete new semantic value/text before the fixture effect.
         if (!publish(JSON.stringify(next))) throw Error(lastError);
         if (accessibleValueText() !== next.accessibility.valueText) throw Error("public QAccessible value text stale");
         root.published = next;
         if (intent.candidate.changed) root.effects++;
         root.report("adopted");
       }
       Text { text:parent.semantics ? parent.semantics.name + ": " + parent.semantics.valueText : "Withdrawn"; Accessible.ignored:true }
     }
   }
   onLoaded: { root.refresh(); root.report("loaded"); }
 }
 Item { x:350;y:40;width:70;height:40
   Accessible.role:Accessible.Button; Accessible.name:"Unrelated Qt button"; Accessible.focusable:true
   Text { text:"Qt fixture"; Accessible.ignored:true }
 }
 Component.onCompleted: { refresh(); report("ready"); }
}'''.replace('QML_DIRECTORY', (ROOT / 'backends/quickshell/resina-qml/qml').as_uri()).replace('ORIENTATION', args.orientation)
    component.setData(source.encode(), QUrl())
    window = component.create()
    require(window is not None, str(component.errors()))
    os.set_blocking(sys.stdin.fileno(), False)
    buffer = ''
    def commands():
        nonlocal buffer
        chunk = os.read(sys.stdin.fileno(), 4096).decode()
        buffer += chunk
        while '\n' in buffer:
            line, buffer = buffer.split('\n', 1)
            window.setProperty('mode', json.loads(line)['mode'])
    timer = QTimer()
    def poll():
        try:
            commands()
        except BlockingIOError:
            pass
        except Exception as error:
            print(str(error), file=sys.stderr, flush=True)
            app.exit(1)
    timer.timeout.connect(poll)
    timer.start(10)
    return app.exec()


def session(args):
    address = string_value(call('org.a11y.Bus', '/org/a11y/bus', 'org.a11y.Bus.GetAddress'))
    for name in ('IsEnabled', 'ScreenReaderEnabled'):
        call('org.a11y.Bus', '/org/a11y/bus', PROPERTIES + '.Set', 'string:org.a11y.Status', 'string:' + name, 'variant:boolean:true')
    with tempfile.TemporaryFile(mode='w+') as log:
        process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), '--host', '--module-path', str(args.module_path.resolve()), '--orientation', args.orientation], stdin=subprocess.PIPE, stdout=log, stderr=log, text=True)
        try:
            def records():
                log.seek(0)
                return [json.loads(line.partition('RESINA_SLIDER_FIXTURE ')[2]) for line in log.read().splitlines() if 'RESINA_SLIDER_FIXTURE ' in line]
            def until(predicate, message):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    require(process.poll() is None, 'native fixture exited')
                    if predicate():
                        return
                    time.sleep(.02)
                log.seek(0)
                raise ValueError(message + ': ' + log.read()[-2000:])
            until(lambda: any(r['kind'] == 'ready' for r in records()), 'missing native publication')
            def tree():
                pending = references(call('org.a11y.atspi.Registry', '/org/a11y/atspi/accessible/root', ACCESSIBLE + '.GetChildren', address=address))
                require(len(pending) <= 1, 'requires one isolated native application')
                if not pending:
                    return []
                visited, sliders, buttons = set(), [], []
                while pending:
                    node = pending.pop(0)
                    require(node not in visited and len(visited) < 20, 'cyclic or excessive native tree')
                    visited.add(node)
                    role = integer_values(call(*node, ACCESSIBLE + '.GetRole', address=address))[0]
                    children = references(call(*node, ACCESSIBLE + '.GetChildren', address=address))
                    name = string_value(call(*node, PROPERTIES + '.Get', 'string:' + ACCESSIBLE, 'string:Name', address=address))
                    if role == 51:
                        require(not children and name == 'Volume fixture', 'Slider leaf or name mismatch')
                        sliders.append(node)
                    if name == 'Unrelated Qt button':
                        require(role == 43, 'factory changed unrelated Qt role')
                        buttons.append(node)
                    pending.extend(children)
                require(len(buttons) == 1, 'unrelated Qt item missing')
                return sliders
            until(lambda: len(tree()) == 1, 'Slider not discoverable')
            node = tree()[0]
            def get(name):
                return payload(call(*node, PROPERTIES + '.Get', 'string:org.a11y.atspi.Value', 'string:' + name, address=address))
            from PySide6.QtCore import QCoreApplication
            from PySide6.QtDBus import QDBusConnection, QDBusMessage, QDBusVariant
            client_app = QCoreApplication([])
            value_bus = QDBusConnection.connectToBus(address, 'resina-slider-check')
            require(value_bus.isConnected(), 'cannot connect native numeric client')
            def numeric(name):
                # dbus-send's printed doubles lose precision. Decode the actual
                # D-Bus double through QtDBus instead of comparing rounded text.
                message = QDBusMessage.createMethodCall(node[0],node[1],PROPERTIES,'Get')
                message.setArguments(['org.a11y.atspi.Value',name])
                reply = value_bus.call(message)
                require(reply.type() != QDBusMessage.ErrorMessage, 'native numeric query failed: ' + reply.errorMessage())
                values = reply.arguments()
                require(len(values) == 1 and isinstance(values[0],QDBusVariant), 'expected native numeric variant')
                value = values[0].variant()
                require(type(value) is float, 'expected actual D-Bus double')
                return value
            def state():
                words = integer_values(call(*node, ACCESSIBLE + '.GetState', address=address), array=True)
                return {'horizontal': bool(words[0] & (1 << 14)), 'vertical': bool(words[0] & (1 << 29)),
                        'enabled':bool(words[0] & (1 << 8)), 'focused':bool(words[0] & (1 << 12)),
                        'readOnly':bool(words[1] & (1 << 11))}
            require(state()['horizontal'] == (args.orientation == 'horizontal') and state()['vertical'] == (args.orientation == 'vertical'), 'orientation lost in real AT-SPI')
            require(not state()['focused'], 'fixture must prove actions without keyboard focus')
            require(numeric('MinimumValue') == 0 and numeric('MaximumValue') == 1 and numeric('MinimumIncrement') == .05 and numeric('CurrentValue') == .4, 'numeric native mapping mismatch')
            description = string_value(call(*node, PROPERTIES + '.Get', 'string:' + ACCESSIBLE, 'string:Description', address=address))
            require(description == 'Adapter qualification only.', 'value text replaced native description')
            text_support = False
            text_error = None
            try:
                text = string_value(call(*node, PROPERTIES + '.Get', 'string:org.a11y.atspi.Value', 'string:Text', address=address))
            except (RuntimeError, ValueError) as error:
                text_error = str(error)
            else:
                require(text == '40 percent', 'native localized value text mismatch')
                text_support = True
            if not text_support:
                log.seek(0)
                require('does not implement "GetText"' in log.read(), 'Value.Text failure was not the documented Qt bridge limitation')
            def set_value(value):
                call(*node, PROPERTIES + '.Set', 'string:org.a11y.atspi.Value', 'string:CurrentValue', 'variant:double:' + str(value), address=address)
            def check_effects(effects, proposals, value, discoverable=True):
                until(lambda: records()[-1]['effects'] == effects and records()[-1]['proposals'] == proposals, 'wrong effect or proposal count')
                require(numeric('CurrentValue') == value, 'native value publication mismatch')
                p = records()[-1]['publication']
                require(p['accessibility']['value']['value'] == value and p['accessibility']['valueText'] == str(round(value * 100)) + ' percent', 'value and text were not coherently published')
                require(tree() == ([node] if discoverable else []), 'native readiness or stable node identity mismatch')
            set_value(.65)
            check_effects(1, 1, .65)
            if text_support:
                require(string_value(call(*node, PROPERTIES + '.Get', 'string:org.a11y.atspi.Value', 'string:Text', address=address)) == '65 percent', 'localized value text stale after native write')
            set_value(.65)
            check_effects(1, 2, .65)
            set_value(2)
            time.sleep(.08)
            check_effects(1, 2, .65)
            names = action_names(call(*node, ACTION + '.GetActions', address=address))
            require(set(names) == {'Increase', 'Decrease'}, 'missing native adjustment actions: ' + str(names))
            def action(name):
                call(*node, ACTION + '.DoAction', 'int32:' + str(names.index(name)), address=address)
            action('Increase')
            check_effects(2, 3, .7000000000000001)
            action('Decrease')
            check_effects(3, 4, .65)
            def mode(name):
                process.stdin.write(json.dumps({'mode':name}) + '\n')
                process.stdin.flush()
                until(lambda: records()[-1]['mode'] == name and records()[-1]['kind'] == 'configured', 'host mode was not committed')
            guarded = []
            for name in ('readOnly', 'disabled', 'withdrawn'):
                mode(name)
                if name == 'readOnly':
                    require(state()['readOnly'], 'native read-only state missing')
                if name in ('disabled', 'withdrawn'):
                    require(not state()['enabled'], 'unavailable native source still enabled')
                set_value(.8)
                action('Increase')
                time.sleep(.08)
                check_effects(3, 4, .65, discoverable=name != 'withdrawn')
                guarded.append(name)
            mode('invalid')
            until(lambda: not tree(), 'invalid publication remained discoverable')
            try:
                set_value(.8)
            except (RuntimeError, ValueError):
                pass
            time.sleep(.08)
            require(records()[-1]['effects'] == 3 and records()[-1]['proposals'] == 4, 'invalid publication delivered intent')
            mode('initial')
            until(lambda: tree() == [node], 'valid republication changed cached node')
            mode('destroyed')
            until(lambda: not tree(), 'destroyed item remained discoverable')
            try:
                set_value(.8)
            except (RuntimeError, ValueError):
                pass
            time.sleep(.08)
            require(records()[-1]['effects'] == 3 and records()[-1]['proposals'] == 4, 'stale AT object produced effect')
            mode('recreated')
            until(lambda: len(tree()) == 1, 'new item failed factory/cache qualification')
            replacement = tree()[0]
            require(replacement != node, 'Qt cache reused destroyed native identity')
            node = replacement
            set_value(.8)
            check_effects(4, 5, .8)
            report = {'scope':'continuous Slider accessibility adapter; not a complete native control', 'hostVersion':'6.11.2',
                      'orientation':args.orientation,'nativeNumericAndActionsVerified':True,'guardedStates':guarded,
                      'destructionAndRecreationVerified':True,'unrelatedQtFactoryPreserved':True,'effects':4,
                      'nativeValueTextSupported':text_support,'nativeValueTextError':text_error,
                      'completeNativeIntrinsicSemantics':text_support}
            require(not args.require_value_text or text_support, 'complete native semantics required; Qt AT-SPI Value.Text is unsupported: ' + str(text_error))
            return report
        except Exception as error:
            log.seek(0)
            raise ValueError(str(error) + '\nNative host: ' + log.read()[-2500:]) from error
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--module-path', type=Path, required=True)
    parser.add_argument('--orientation', choices=('horizontal','vertical'), default='horizontal')
    parser.add_argument('--require-value-text', action='store_true')
    parser.add_argument('--session', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--host', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        require((args.module_path / 'Resina/Runtime/qmldir').is_file(), 'compiled runtime module missing')
        if args.host:
            return host(args)
        require(sys.platform == 'linux' and bool(os.environ.get('DISPLAY') or os.environ.get('WAYLAND_DISPLAY')), 'requires Linux and a real native display')
        if args.session:
            report = session(args)
        else:
            with tempfile.TemporaryDirectory(prefix='resina-slider-atspi-') as runtime:
                environment = dict(os.environ)
                if environment.get('WAYLAND_DISPLAY'):
                    display = Path(environment['WAYLAND_DISPLAY'])
                    if not display.is_absolute():
                        display = Path(environment.get('XDG_RUNTIME_DIR','')) / display
                    require(display.is_socket(), 'requires actual compositor socket')
                    environment['WAYLAND_DISPLAY'] = str(display)
                environment.update(XDG_RUNTIME_DIR=runtime, XDG_CACHE_HOME=runtime, QT_LINUX_ACCESSIBILITY_ALWAYS_ON='1', QT_QUICK_BACKEND='software', QT_QPA_PLATFORM='wayland' if environment.get('WAYLAND_DISPLAY') else 'xcb')
                process = subprocess.Popen(['dbus-run-session','--',sys.executable,str(Path(__file__).resolve()),'--session','--module-path',str(args.module_path.resolve()),'--orientation',args.orientation,*(['--require-value-text'] if args.require_value_text else [])], env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
                try:
                    output, diagnostics = process.communicate(timeout=50)
                    require(process.returncode == 0, 'native session failed: ' + diagnostics[-3500:])
                    reports = [json.loads(line.removeprefix('RESINA_SLIDER_ADAPTER ')) for line in output.splitlines() if line.startswith('RESINA_SLIDER_ADAPTER ')]
                    require(len(reports) == 1, 'expected one adapter report')
                    report = reports[0]
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.communicate(timeout=3)
        print(('RESINA_SLIDER_ADAPTER ' if args.session else '') + json.dumps(report, sort_keys=True))
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print('Qt native Slider adapter check failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())

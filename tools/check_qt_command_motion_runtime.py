import argparse
import copy
import json
import io
import math
import os
from pathlib import Path
import sys
import time

from check_command_motion_backend import equation_sample
from check_spring_trajectory_backend import trajectory_mismatch
from check_headless_backend import mismatch
from check_quickshell_paint_runtime import compare_images, read_backend_output
from check_qt_command_control_runtime import wait
from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot
import conformance_font


def authored_motion(source, phase):
    theme = json.loads(source['content']['themeSource'])
    family = theme['materialAssignments']['control'][source['body']['surface']['materialRole'].split('.')[1]]
    reduced = source['content']['environment']['accessibilityPreferences']['reducedMotion']
    policy = 'reducedMotion' if reduced else 'castImmediate' if family == 'cast' else 'spring'
    target = {'bodyMix': 0, 'depthScale': 1} if phase == 'rest' else source['body']['commandAppearance']['profiles'][family][phase]
    return family, policy, target


def motion_mismatch(actual, source, metadata):
    family, policy, target = authored_motion(source, actual['command']['phase'])
    if actual['command']['paint']['body']['materialFamily'] != family:
        return '/command/materialFamily'
    if actual['policy'] != policy or actual['target'] != target:
        return '/policy/target'
    for name, lower, upper in (('bodyMix', -1, 1), ('depthScale', 0, 2)):
        expected = equation_sample(metadata['channels'][name], target[name], metadata['time'], policy != 'spring')
        difference = trajectory_mismatch(actual[name], expected)
        if difference:
            return '/' + name + difference
        value = expected['state']['position']
        projection = 'lowerBound' if value < lower else 'upperBound' if value > upper else 'none'
        if actual[name + 'Projection'] != projection:
            return '/' + name + 'Projection'
        if mismatch(actual['command']['response'][name], min(upper, max(lower, value))):
            return '/command/response/' + name
    return None


def check_native_paint(window, control, source, args):
    from PIL import Image, ImageDraw
    from PySide6.QtCore import QObject, QPointF
    # Freeze the already demonstrated native driver for one exact frame readback.
    # No synthetic elapsed time or replacement paint is introduced.
    drivers = [child for child in control.findChildren(QObject)
               if 'FrameAnimation' in child.metaObject().className()]
    if len(drivers) != 1 or not drivers[0].property('running'):
        raise ValueError('moving paint oracle requires the actual running native frame driver')
    driver = drivers[0]
    driver.setProperty('running', False)
    try:
        before = json.loads(window.observation())
        metadata = json.loads(before['motion'])
        snapshot = json.loads(before['snapshot'])
        frame = control.property('currentFrame')
        origin = frame.mapToScene(QPointF(0, 0))
        dpr = window.devicePixelRatio()
        image = window.grabWindow()
        if json.loads(window.observation())['motion'] != before['motion']:
            raise ValueError('native motion changed during frozen readback')
        path = args.output_dir / 'moving-paint-oracle.png'
        if not image.save(str(path)):
            raise ValueError('native sampled oracle capture failed')
        body = copy.deepcopy(source['body'])
        appearance = body.pop('commandAppearance')
        surrounding = body.pop('surroundingColor')
        body.update(schemaVersion='0.1.0', size=snapshot['label']['size'],
                    theme={'schemaVersion':'0.1.0', **{key:source['content'][key]
                          for key in ('themeSource','externalSources','environment')}})
        body['surface']['states'] = snapshot['paint']['paint']['body']['states']
        request = {'schemaVersion':'0.1.0', 'surface':{'schemaVersion':'0.1.0','body':body,
                   'surroundingColor':surrounding}, 'commandAppearance':appearance,
                   'channels':metadata['channels'], 'time':metadata['time']}
        oracle = read_backend_output([str(args.raster.resolve()), '-', str(image.width()), str(image.height()),
                                     str(-origin.x()), str(-origin.y()), str(dpr), '4'], json.dumps(request).encode())
        with Image.open(io.BytesIO(oracle)) as paint, Image.open(path) as captured:
            expected = Image.new('RGBA', paint.size, 'white')
            expected.alpha_composite(paint.convert('RGBA'))
            actual = captured.convert('RGBA')
            # Native glyph placement/content are independently verified by the
            # complete label checks. Compare every pixel outside that label.
            label = snapshot['label']['labelBounds']
            rectangle = (math.floor((origin.x()+label['x'])*dpr)-1,
                         math.floor((origin.y()+label['y'])*dpr)-1,
                         math.ceil((origin.x()+label['x']+label['width'])*dpr)+1,
                         math.ceil((origin.y()+label['y']+label['height'])*dpr)+1)
            ImageDraw.Draw(expected).rectangle(rectangle, fill='white')
            ImageDraw.Draw(actual).rectangle(rectangle, fill='white')
            compare_images(expected, actual)
    finally:
        driver.setProperty('running', True)


def check_bindings(engine, app, channels):
    from PySide6.QtCore import QCoreApplication, QEvent, QUrl
    from PySide6.QtQml import QQmlComponent
    component = QQmlComponent(engine)
    component.setData(b'import Resina.Runtime\nCommandSnapshotResolver {}', QUrl())
    resolver = component.create()
    if resolver is None:
        raise ValueError(str(component.errors()))
    for name in ('publication', 'motion', 'frame'):
        if resolver.metaObject().property(resolver.metaObject().indexOfProperty(name)).isWritable():
            raise ValueError('native frame cache became writable')
    raw = copy.deepcopy(channels)
    raw['bodyMix']['initial'] = {'position': 1.4, 'velocity': 3}
    raw['depthScale']['initial'] = {'position': -0.4, 'velocity': -2}
    target = {'bodyMix': -.09, 'depthScale': .25}
    for policy in ('spring', 'castImmediate', 'reducedMotion'):
        for elapsed in (0, .25):
            value = json.loads(resolver.retargetMotion(json.dumps({'channels': raw, 'target': target,
                                                                 'policy': policy, 'time': elapsed})))
            if 'error' in value:
                raise ValueError(value['error'])
            for name in ('bodyMix', 'depthScale'):
                expected = equation_sample(raw[name], target[name], elapsed, policy != 'spring')
                if trajectory_mismatch(value['previous'][name], expected) or mismatch(value['channels'][name]['initial'], expected['state']):
                    raise ValueError('retarget clipped raw state or manufactured velocity')
    invalid = ['[]', ' ' * 65537, json.dumps(channels)[:-1] + ',"bodyMix":{}}',
               json.dumps({**channels, 'unknown': True})]
    for path, bad in ((('bodyMix','dynamics','mass'), 0), (('depthScale','dynamics','schemaVersion'), 'future'),
                      (('bodyMix','initial','position'), float('nan'))):
        value = copy.deepcopy(channels)
        value[path[0]][path[1]][path[2]] = bad
        invalid.append(json.dumps(value))
    for source in invalid:
        if not json.loads(resolver.validateMotionChannels(source)).get('error'):
            raise ValueError('invalid native motion channels were accepted')
    request = {'channels': channels, 'target': target, 'policy': 'spring', 'time': .2}
    for member, bad in (('time', -1), ('policy', 'unknown'), ('target', {'bodyMix': 2, 'depthScale': 1})):
        if not json.loads(resolver.retargetMotion(json.dumps({**request, member: bad}))).get('error'):
            raise ValueError('invalid native retarget was accepted')
    component.setData(b'import Resina.Runtime\nCommandClock {}', QUrl())
    clock = component.create()
    if not json.loads(clock.sample()).get('error'):
        raise ValueError('unstarted native clock accepted sampling')
    if json.loads(clock.restart()) != {'previousTime': 0}:
        raise ValueError('initial clock restart fabricated elapsed time')
    start = time.monotonic()
    wait(app, lambda: time.monotonic() - start > .02, 'native clock elapsed')
    elapsed = json.loads(clock.sample())['time']
    previous = json.loads(clock.restart())['previousTime']
    if not 0 < elapsed <= previous or abs(previous - (time.monotonic() - start)) > .02:
        raise ValueError('native clock restart lost its monotonic epoch')
    clock.invalidate()
    if not json.loads(clock.sample()).get('error'):
        raise ValueError('invalidated native clock retained a sample')
    component.setData(b'import QtQuick\nQtObject {}', QUrl())
    frame = component.create()
    records = []
    resolver.frameChanged.connect(lambda: records.append((resolver.property('publication'), resolver.property('motion'), resolver.property('frame'))))
    resolver.publishFrame('complete', 'sample', frame)
    if records != [('complete', 'sample', frame)]:
        raise ValueError('frame notification preceded coherent cache commit')
    def retire():
        if resolver.property('publication'):
            resolver.publishSnapshot('')
    resolver.publicationChanged.connect(retire)
    resolver.publishFrame('replacement', 'next', frame)
    if resolver.property('publication') or resolver.property('motion') or resolver.property('frame'):
        raise ValueError('reentrant cache retirement retained a frame')
    resolver.publicationChanged.disconnect(retire)
    resolver.publishFrame('complete', 'sample', frame)
    frame.deleteLater()
    QCoreApplication.sendPostedEvents(None, QEvent.DeferredDelete)
    if resolver.property('publication') or resolver.property('motion') or resolver.property('frame'):
        raise ValueError('destroyed drawing frame retained cache publication')
    resolver.deleteLater()
    clock.deleteLater()


def main():
    parser = argparse.ArgumentParser(description='Verify guarded Qt command frames against independent spring equations.')
    parser.add_argument('--module-path', required=True, type=Path)
    parser.add_argument('--font-dir', required=True, type=Path)
    parser.add_argument('--raster', required=True, type=Path)
    parser.add_argument('--output-dir', required=True, type=Path)
    args = parser.parse_args()
    os.environ.setdefault('QT_QPA_PLATFORM', 'offscreen')
    os.environ.setdefault('QT_QUICK_BACKEND', 'software')
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QPluginLoader, QObject, Qt, QUrl
        from PySide6.QtGui import QGuiApplication, QKeyEvent
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        from PySide6.QtCore import qVersion
        if qVersion() != '6.11.2':
            raise ValueError('requires Qt 6.11.2')
        app = QGuiApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve() / 'Resina/Runtime/libResina_Runtime.so'))
        if plugin.instance() is None:
            raise ValueError(plugin.errorString())
        conformance_font.register_qt(args.font_dir)
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve()))
        component = QQmlComponent(engine, QUrl.fromLocalFile(str(ROOT / 'conformance/quickshell/command-control-fixture.qml')))
        window = component.create()
        if window is None:
            raise ValueError('; '.join(error.toString() for error in component.errors()))
        control = window.findChild(QObject, 'commandControl')
        window.requestActivate()
        wait(app, lambda: window.isActive() and window.isExposed(), 'active native window')
        source = request_for('light', 'Reconnect')
        source['content']['environment']['accessibilityPreferences']['reducedMotion'] = False
        channels = json.loads((ROOT / 'conformance/ir/command-motion-request.json').read_text())['channels']
        check_bindings(engine, app, channels)
        observed = []
        violations = []

        def observation():
            return json.loads(window.observation())

        def observe_frame():
            result = observation()
            if not result['snapshot'] or not result['motion']:
                return
            try:
                snapshot = json.loads(result['snapshot'])
                metadata = json.loads(result['motion'])
                current = copy.deepcopy(source)
                current.update(activation=result['state'], hovered=result['hovered'], focusable=result['state']['enabled'])
                hold = result['state']['hold']
                pressed = bool(hold and (hold['kind'] == 'key' or hold['inside']))
                signals = ['rest'] if not result['hovered'] and not pressed else []
                if result['hovered']: signals.append('hover')
                if result['state']['focused']: signals.append('focused')
                if pressed: signals.append('pressed')
                if not result['state']['enabled']:
                    if signals == ['rest']: signals = []
                    signals.append('disabled')
                states = {'schemaVersion':'0.1.0','states':signals or ['rest']}
                verify_snapshot(snapshot, current, states, metadata['motion'])
                difference = motion_mismatch(metadata['motion'], current, metadata)
                if difference:
                    raise ValueError('native motion differs from independent equations at ' + difference)
                frame = control.property('currentFrame')
                if frame is None or frame.property('snapshot') != result['snapshot'] or frame.property('motion') != result['motion']:
                    raise ValueError('native publication selected a different drawing frame')
                if json.loads(result['target']) != snapshot['hitRegion']:
                    raise ValueError('sampled native target differs from complete snapshot')
                observed.append((time.monotonic(), result, metadata))
            except Exception as error:
                violations.append(str(error))

        control.snapshotChanged.connect(observe_frame)
        control.motionChanged.connect(observe_frame)
        window.setProperty('motionChannels', json.dumps(channels))
        window.setProperty('configuration', json.dumps(source))
        wait(app, lambda: observation()['ready'] or observation()['error'], 'sampled preparation')
        if observation()['error']:
            raise ValueError(observation()['error'])
        window.availability(True)
        window.focusCommand()
        wait(app, lambda: observation()['ready'] and observation()['state']['focused'], 'focused moving frame')
        stable = observation()['target']
        baseline = len(observed)
        QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyPress, Qt.Key_Space, Qt.NoModifier))
        try:
            wait(app, lambda: observation()['ready'] and len(observed) >= baseline + 3, 'multiple native pressed frames')
        except ValueError:
            result = observation()
            raise ValueError(f"pressed frame timeout: ready={result['ready']}, error={result['error']}, frames={len(observed)-baseline}, policy={json.loads(result['motion'])['motion']['policy'] if result['motion'] else None}, violations={violations[:1]}")
        args.output_dir.mkdir(parents=True, exist_ok=True)
        first_image = window.grabWindow()
        if not first_image.save(str(args.output_dir / 'moving-pressed.png')):
            raise ValueError('native motion capture failed')
        first_time = json.loads(observation()['motion'])['time']
        wait(app, lambda: observation()['ready'] and json.loads(observation()['motion'])['time'] > first_time + .4, 'later visible pressed sample')
        later_image = window.grabWindow()
        if later_image == first_image or not later_image.save(str(args.output_dir / 'moving-pressed-later.png')):
            raise ValueError('native sampled paint did not visibly advance')
        check_native_paint(window, control, source, args)
        QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyRelease, Qt.Key_Space, Qt.NoModifier))
        baseline = len(observed)
        wait(app, lambda: observation()['ready'] and len(observed) >= baseline + 3, 'retargeted native release frames')
        if observation()['effects'] != 1 or observation()['target'] != stable:
            raise ValueError('moving command changed reservation or activation effect')
        if not any(any(abs(frame[2]['channels'][name]['initial']['velocity']) > 0 for name in ('bodyMix','depthScale')) for frame in observed[baseline:]):
            raise ValueError('native release retarget lost retained velocity')
        if violations:
            raise ValueError(violations[0])
        if not any(frame[2]['motion']['policy'] == 'spring' and not frame[2]['motion']['bodyMix']['settled'] for frame in observed):
            raise ValueError('no actual unsettled spring publication')
        # Same target and retained channels define one clock epoch; elapsed values
        # advance with actual monotonic time, never a fabricated frame delta.
        pairs = [(a,b) for a,b in zip(observed, observed[1:])
                 if a[2]['channels'] == b[2]['channels'] and a[2]['motion']['target'] == b[2]['motion']['target']]
        if not pairs or not any(b[2]['time'] > a[2]['time'] for a,b in pairs):
            raise ValueError('native sampled clock did not advance')
        for a,b in pairs:
            if abs((b[2]['time'] - a[2]['time']) - (b[0] - a[0])) > .25:
                raise ValueError('sample clock does not track actual monotonic elapsed time')
        # Both channels must settle before the driver stops. The original public
        # fixture has deliberately slow arithmetic coefficients, not calibration.
        def settled():
            result = observation()
            if result['error']:
                raise ValueError(result['error'])
            sample = json.loads(result['motion'])['motion'] if result['motion'] else None
            return result['ready'] and sample and sample['bodyMix']['settled'] and sample['depthScale']['settled']
        deadline = time.monotonic() + 20
        while not settled() and time.monotonic() < deadline:
            app.processEvents()
        if not settled():
            raise ValueError('native motion never reached both settled channels')
        if not any(frame[2]['motion']['bodyMix']['settled'] and not frame[2]['motion']['depthScale']['settled'] for frame in observed):
            raise ValueError('fixture did not prove one settled channel leaves the other scheduled')
        last = observation()['motion']
        stop = time.monotonic() + .15
        while time.monotonic() < stop:
            app.processEvents()
        if observation()['motion'] != last:
            raise ValueError('native frame scheduling continued after both channels settled')
        # Explicit reduced-motion and Cast policy use immediate frames.
        for policy in ('reducedMotion', 'castImmediate'):
            source = request_for('dark', 'Reconnect')
            source['content']['environment']['accessibilityPreferences']['reducedMotion'] = policy == 'reducedMotion'
            if policy == 'castImmediate':
                theme = json.loads(source['content']['themeSource'])
                theme['materialAssignments']['control']['interactive'] = 'cast'
                source['content']['themeSource'] = json.dumps(theme)
            window.setProperty('configuration', json.dumps(source))
            wait(app, lambda: observation()['ready'] or observation()['error'], 'immediate policy replacement')
            if observation()['error'] or json.loads(observation()['motion'])['motion']['policy'] != policy:
                raise ValueError('native immediate policy was not derived from current portable input')
            QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyPress, Qt.Key_Space, Qt.NoModifier))
            wait(app, lambda: observation()['ready'], 'immediate pressed publication')
            sample = json.loads(observation()['motion'])['motion']
            if not sample['bodyMix']['settled'] or not sample['depthScale']['settled']:
                raise ValueError('immediate native preference retained motion')
            QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyRelease, Qt.Key_Space, Qt.NoModifier))
            wait(app, lambda: observation()['ready'], 'immediate release publication')
        for name in ('light', 'dark'):
            source = request_for(name, 'Reconnect')
            source['content']['environment']['accessibilityPreferences']['reducedMotion'] = False
            source['content']['environment']['accessibilityPreferences']['reducedTransparency'] = True
            theme = json.loads(source['content']['themeSource'])
            theme['materialAssignments']['control']['interactive'] = 'frost'
            source['content']['themeSource'] = json.dumps(theme)
            window.setProperty('configuration', json.dumps(source))
            wait(app, lambda: observation()['ready'] or bool(observation()['error']), 'native Frost replacement')
            if observation()['error']:
                raise ValueError(observation()['error'])
            window.focusCommand()
            wait(app, lambda: observation()['ready'] and observation()['state']['focused'], 'native Frost focus')
            baseline = len(observed)
            QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyPress, Qt.Key_Space, Qt.NoModifier))
            wait(app, lambda: observation()['ready'] and len(observed) >= baseline + 3, 'native Frost pressed frames')
            sample = json.loads(observation()['motion'])['motion']
            if sample['policy'] != 'spring' or sample['command']['paint']['body']['materialFamily'] != 'frost':
                raise ValueError('native Frost path did not retain its bound family and policy')
            QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyRelease, Qt.Key_Space, Qt.NoModifier))
            wait(app, lambda: observation()['ready'], 'native Frost release')
        for invalid in ('[]', json.dumps(channels)[:-1] + ',"bodyMix":{}}', json.dumps({**channels,'unknown':True})):
            window.setProperty('motionChannels', invalid)
            wait(app, lambda: bool(observation()['error']), 'invalid motion profile withdrawal')
            result = observation()
            if result['ready'] or result['snapshot'] or result['motion'] or result['target'] or window.invoke():
                raise ValueError('invalid motion profile retained production channels')
            window.setProperty('motionChannels', json.dumps(channels))
            wait(app, lambda: observation()['ready'], 'explicit motion recovery')
        source = request_for('light', 'Reconnect')
        source['content']['environment']['accessibilityPreferences']['reducedMotion'] = False
        window.setProperty('configuration', json.dumps(source))
        wait(app, lambda: observation()['ready'], 'spring profile restored')
        # Endpoint preflight alone is insufficient: this raw initial sample
        # saturates to black and must fail the existing content contrast guard.
        invalid_sample = copy.deepcopy(channels)
        invalid_sample['bodyMix']['initial']['position'] = -1
        window.setProperty('motionChannels', json.dumps(invalid_sample))
        wait(app, lambda: bool(observation()['error']), 'actual intermediate contrast rejection')
        result = observation()
        if result['ready'] or result['snapshot'] or result['motion'] or result['target'] or window.invoke():
            raise ValueError('failed intermediate paint retained production channels')
        window.setProperty('motionChannels', json.dumps(channels))
        wait(app, lambda: observation()['ready'], 'guarded motion recovered')
        raw = copy.deepcopy(channels)
        raw['bodyMix']['initial'] = {'position': 4, 'velocity': -.2}
        raw['depthScale']['initial'] = {'position': 4, 'velocity': -.3}
        # Slow this explicit overshoot fixture so native raster preparation at
        # larger DPR cannot consume its beyond-bound interval before input.
        for name in ('bodyMix', 'depthScale'):
            raw[name]['dynamics']['stiffness'] /= 100
            raw[name]['dynamics']['damping'] /= 10
        window.setProperty('motionChannels', json.dumps(raw))
        wait(app, lambda: observation()['ready'] or bool(observation()['error']), 'projected native raw profile')
        result = observation()
        if result['error']:
            raise ValueError(result['error'])
        sample = json.loads(result['motion'])['motion']
        if sample['bodyMixProjection'] != 'upperBound' or sample['depthScaleProjection'] != 'upperBound':
            raise ValueError('raw native profile did not retain explicit projection markers')
        window.focusCommand()
        wait(app, lambda: observation()['ready'] and observation()['state']['focused'], 'raw native focus')
        QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyPress, Qt.Key_Space, Qt.NoModifier))
        wait(app, lambda: observation()['ready'], 'overshoot retarget publication')
        metadata = json.loads(observation()['motion'])
        if any(metadata['channels'][name]['initial']['position'] <= upper or metadata['channels'][name]['initial']['velocity'] == 0 for name, upper in (('bodyMix', 1), ('depthScale', 2))):
            raise ValueError('native target change clipped overshoot or discarded velocity: ' + str(metadata['channels']))
        QCoreApplication.sendEvent(window, QKeyEvent(QEvent.KeyRelease, Qt.Key_Space, Qt.NoModifier))
        wait(app, lambda: observation()['ready'], 'raw release publication')
        window.setProperty('revokeOnReady', True)
        window.renew()
        try:
            wait(app, lambda: observation()['ready'] and not observation()['state']['enabled'], 'reentrant sampled publication revocation')
        except ValueError:
            result = observation()
            raise ValueError('reentrant motion revocation: ' + str({key:result[key] for key in ('ready','error','state','effects')}))
        if observation()['publicationRevocation'] is not True or json.loads(observation()['snapshot'])['accessibility']['state']['enabled']:
            raise ValueError('reentrant motion publication retained stale enabled semantics')
        window.rotateCarrier(10)
        wait(app, lambda: bool(observation()['error']) and not observation()['target'] and not observation()['snapshot'], 'motion transform withdrawal')
        window.rotateCarrier(0)
        window.renew()
        wait(app, lambda: observation()['ready'], 'motion transform recovery')
        window.showCommand(False)
        wait(app, lambda: not observation()['snapshot'] and not observation()['target'], 'hidden motion retirement')
        if observation()['motion'] or observation()['ready']:
            raise ValueError('hidden motion retained native publication')
        window.showCommand(True)
        wait(app, lambda: observation()['ready'], 'visible motion requalification')
        if violations:
            raise ValueError(violations[0])
        print('Qt command motion: coherent native frames, independent equations, keyboard activation and stable reservation verified')
        window.close()
        return 0
    except Exception as error:
        print('Qt command motion failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())

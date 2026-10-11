import argparse
import json
from pathlib import Path
import sys

from check_qt_command_runtime import ROOT, agrees


def require(condition, message):
    if not condition:
        raise ValueError(message)


def checked(controller, value):
    require(controller.setChecked(json.dumps({'checked':value})), 'external checked update failed: ' + controller.property('lastError'))


def state(controller):
    return json.loads(controller.property('publication'))


def check_case(component, case):
    controller = component.create()
    require(controller is not None, 'cannot instantiate Rust ToggleController')
    try:
        initial = case['request']['state']
        for event in ({'kind':'availability','enabled':initial['enabled']},
                      {'kind':'focus','focused':initial['focused']}):
            require(controller.dispatch(json.dumps(event)), 'cannot seed Toggle activation')
        hold = initial['hold']
        if hold:
            events = ([{'kind':'pointerDown','id':hold['id'],'inside':True},
                       {'kind':'pointerMove','id':hold['id'],'inside':hold['inside']}]
                      if hold['kind'] == 'pointer' else [{'kind':'keyDown','key':hold['key'],'repeat':False}])
            for event in events:
                require(controller.dispatch(json.dumps(event)), 'cannot seed Toggle hold')
        checked(controller, case['request']['checked'])
        require(agrees(state(controller)['activation']['state'], initial), 'checked replacement lost the live gesture')
        publications, intents, captures = [], [], []
        controller.publicationChanged.connect(lambda: publications.append(state(controller)))
        controller.activated.connect(lambda value:intents.append((value,state(controller))))
        controller.captureRequested.connect(lambda identity, acquire:captures.append({'kind':'acquire' if acquire else 'release','id':identity}))
        require(controller.dispatch(json.dumps(case['request']['event'])), 'valid Toggle event rejected')
        expected = case['expected']
        require(agrees(publications,[expected]) and agrees(state(controller),expected), 'Toggle native publication differs from ' + case['name'])
        expected_intents = [(expected['checked'],expected)] if expected['activation']['activate'] else []
        require(agrees(intents,expected_intents), 'Toggle intent was missing, duplicated or delivered before the joint commit')
        expected_capture = expected['activation']['capture']
        require(agrees(captures,[expected_capture] if expected_capture else []), 'Toggle capture differs from shared activation')
        for name in ('publication','lastError'):
            require(not controller.metaObject().property(controller.metaObject().indexOfProperty(name)).isWritable(), 'Toggle state became independently writable')
    finally:
        controller.deleteLater()


def check_sequences(component):
    controller = component.create()
    require(controller is not None, 'cannot instantiate Toggle sequence owner')
    effects = []
    controller.activated.connect(lambda value:effects.append(value))
    try:
        require(controller.dispatch('{"kind":"availability","enabled":true}'), 'Toggle enable failed')
        require(controller.dispatch('{"kind":"focus","focused":true}'), 'Toggle focus failed')
        require(controller.dispatch('{"kind":"keyDown","key":"space","repeat":false}'), 'Toggle Space hold failed')
        checked(controller,True)
        require(controller.dispatch('{"kind":"keyUp","key":"space"}'), 'Toggle Space release failed')
        require(effects == [False] and state(controller)['checked'] is False, 'release toggled the down-time checked value')
        require(controller.dispatch('{"kind":"keyDown","key":"space","repeat":false}'), 'Toggle semantic hold failed')
        require(controller.dispatch('{"kind":"invoke"}'), 'Toggle semantic invocation failed')
        require(controller.dispatch('{"kind":"keyUp","key":"space"}'), 'Toggle superseded release failed')
        require(effects == [False,True] and state(controller)['activation']['state']['hold'] is None, 'superseded Toggle release replayed intent')
        before = controller.property('publication')
        for source in ('[]','{"checked":false,"checked":true}','{"checked":1}','{"checked":null}',
                       '{"checked":true,"unknown":true}', ' ' * 65537):
            require(not controller.setChecked(source) and bool(controller.property('lastError')), 'invalid checked replacement was accepted')
            require(controller.property('publication') == before and effects == [False,True], 'invalid checked source changed publication/effect')
        for source in ('[]','{"kind":"invoke","kind":"invoke"}','{"kind":"invoke","unknown":true}', ' ' * 65537):
            require(not controller.dispatch(source) and bool(controller.property('lastError')), 'invalid Toggle event was accepted')
            require(controller.property('publication') == before and effects == [False,True], 'invalid event changed Toggle publication/effect')
        reentry = []
        def during_publication():
            reentry.extend([controller.dispatch('{"kind":"invoke"}'), controller.setChecked('{"checked":true}')])
        controller.publicationChanged.connect(during_publication)
        require(controller.dispatch('{"kind":"invoke"}'), 'Toggle failed recovery')
        require(reentry == [False,False] and effects == [False,True,False], 'Toggle recursive publication mutated state or replayed an effect')
        controller.publicationChanged.disconnect(during_publication)
        def revoke(value):
            require(state(controller)['checked'] == value, 'product effect preceded selection commit')
            require(controller.dispatch('{"kind":"availability","enabled":false}'), 'product callback could not revoke Toggle availability')
        controller.activated.connect(revoke)
        require(controller.dispatch('{"kind":"invoke"}'), 'available Toggle invocation failed')
        require(controller.dispatch('{"kind":"invoke"}'), 'stale Toggle invocation rejected the valid unavailable event')
        require(effects == [False,True,False,True] and state(controller)['checked'] is True, 'stale invocation replayed or disabled selection was erased')
    finally:
        controller.deleteLater()


def main():
    parser = argparse.ArgumentParser(description='Verify the compiled Qt Toggle owner against shared binary activation cases (injected QObject events).')
    parser.add_argument('--module-path', required=True, type=Path)
    args = parser.parse_args()
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QPluginLoader, QUrl, qVersion
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6 import QtQuick
        require(qVersion() == '6.11.2', 'requires Qt 6.11.2')
        app = QCoreApplication([])
        plugin = QPluginLoader(str(args.module_path.resolve(strict=True) / 'Resina/Runtime/libResina_Runtime.so'))
        require(plugin.instance() is not None, plugin.errorString())
        engine = QQmlEngine()
        engine.addImportPath(str(args.module_path.resolve(strict=True)))
        component = QQmlComponent(engine)
        component.setData(b'import Resina.Runtime\nToggleController {}', QUrl())
        for case in json.loads((ROOT / 'conformance/interaction/toggle-activation-cases.json').read_text()):
            if 'expected' in case:
                check_case(component,case)
                QCoreApplication.sendPostedEvents(None,QEvent.DeferredDelete)
        check_sequences(component)
        QCoreApplication.sendPostedEvents(None,QEvent.DeferredDelete)
        print('Compiled Qt Toggle joint publication, live checked replacement, capture and intent delivery match shared cases; rejection and reentrancy passed (injected QObject events)')
        return 0
    except Exception as error:
        print('Compiled Qt Toggle failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())

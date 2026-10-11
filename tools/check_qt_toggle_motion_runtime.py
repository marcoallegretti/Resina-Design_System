"""Verify integrated native Toggle motion with independent equations and CPU readback."""
import argparse
import copy
import json
import os
from pathlib import Path
import sys

from check_qt_toggle_control_runtime import ROOT,request_for,verify_paint
from check_qt_command_control_runtime import wait
from check_command_motion_backend import equation_sample
from check_spring_trajectory_backend import trajectory_mismatch
from check_schemas import validator_for
from check_headless_backend import mismatch


def fixture_channels():
    dynamics=json.loads((ROOT/'conformance/motion/toggle-travel-cases.json').read_text())['dynamics']
    dynamics.update(stiffness=4,damping=4)
    def channel(position, velocity=0, rate=1):
        actual=copy.deepcopy(dynamics)
        actual['stiffness']*=rate*rate;actual['damping']*=rate
        return {'dynamics':actual,'initial':{'position':position,'velocity':velocity}}
    return {'track':{'bodyMix':channel(0),'depthScale':channel(1,rate=1.3)},
            'thumb':{'bodyMix':channel(0,rate=.8),'depthScale':channel(1,rate=1.1)},
            'travel':channel(.75)}


def verify_motion(snapshot,metadata,source):
    sample=metadata['motion'];channels=metadata['channels'];time=metadata['time']
    reduced=source['content']['environment']['accessibilityPreferences']['reducedMotion']
    theme=json.loads(source['content']['themeSource'])
    for part in ('track','thumb'):
        value=sample[part]
        validator_for('schemas/toggle-part-motion-ir.schema.json').validate(value)
        if value['partPaint'] != snapshot[part]:raise ValueError('complete motion/paint publication disagrees')
        family=snapshot[part]['paint']['body']['materialFamily']
        role=source[part]['surface']['materialRole'].split('.')[1]
        if family != theme['materialAssignments']['control'][role]:raise ValueError('actual material lost theme binding')
        selected=source[part]['checkedColorRole'] if snapshot[part]['checked'] else source[part]['surface']['colorRole']
        if snapshot[part]['paint']['body']['colorRole'] != selected:raise ValueError('motion delayed current checked pigment role')
        policy='reducedMotion' if reduced else 'castImmediate' if family=='cast' else 'spring'
        phase=next((state for state in ('disabled','pressed','hover') if state in snapshot[part]['paint']['body']['states']['states']),'rest')
        target={'bodyMix':0,'depthScale':1} if phase=='rest' else source[part]['interactionAppearance']['profiles'][family][phase]
        if value['target'] != target or value['policy'] != policy:raise ValueError('motion target/policy differs from existing contract')
        for name,lower,upper in (('bodyMix',-1,1),('depthScale',0,1)):
            expected=equation_sample(channels[part][name],target[name],time,policy!='spring')
            if trajectory_mismatch(value[name],expected):raise ValueError('independent part trajectory disagrees')
            position=value[name]['state']['position']
            projection='lowerBound' if position<lower else 'upperBound' if position>upper else 'none'
            if value[name+'Projection'] != projection or value['partPaint']['response'][name] != min(upper,max(lower,position)):
                raise ValueError('part paint projection disagrees with retained trajectory')
    travel=sample['travel'];family=snapshot['thumb']['paint']['body']['materialFamily']
    policy='reducedMotion' if reduced else 'castImmediate' if family=='cast' else 'spring'
    target=1 if snapshot['layout']['checked'] else 0
    expected=equation_sample(channels['travel'],target,time,policy!='spring')
    if travel['target'] != target or travel['policy'] != policy or trajectory_mismatch(travel['trajectory'],expected):
        raise ValueError('independent checked travel trajectory disagrees')
    raw=travel['trajectory']['state']['position'];progress=max(0,min(1,raw))
    projection='offEndpoint' if raw<0 else 'onEndpoint' if raw>1 else 'none'
    off=snapshot['layout']['offThumbBounds'];on=snapshot['layout']['onThumbBounds']
    bounds=off if progress==0 else on if progress==1 else {**off,'x':off['x']+(on['x']-off['x'])*progress}
    if travel['projection'] != projection or travel['thumbBounds'] != bounds:
        raise ValueError('checked native placement differs from bounded travel: '+repr((travel['thumbBounds'],bounds,travel['trajectory']['state'],expected['state'])))


def check_native_bindings(engine,channels):
    from PySide6.QtCore import QUrl
    from PySide6.QtQml import QQmlComponent
    component=QQmlComponent(engine)
    component.setData(b'''import QtQml
import Resina.Runtime 0.1
QtObject {
 property ToggleSnapshotResolver resolver: ToggleSnapshotResolver {}
 function validate(source) { return resolver.validateMotionChannels(source); }
 function retarget(source) { return resolver.retargetMotion(source); }
}''',QUrl())
    native=component.create()
    if native is None:raise ValueError(str(component.errors()))
    if json.loads(native.validate(json.dumps(channels))).get('channels') != channels:
        raise ValueError('native motion validator did not retain explicit authored dynamics/state')
    for source in ('[]','{}','{"track":[],"thumb":{},"travel":{}}',' '*65537,
                   json.dumps({**channels,'unexpected':True}),json.dumps(channels).replace('"mass": 1','"mass": 1,"mass": 1',1)):
        if set(json.loads(native.validate(source))) != {'error'}:raise ValueError('malformed motion profile accepted')
    for policy in ('spring','castImmediate','reducedMotion'):
        targets={'track':{'target':{'bodyMix':-.12,'depthScale':.8},'policy':policy},
                 'thumb':{'target':{'bodyMix':.08,'depthScale':.9},'policy':policy},
                 'travel':{'target':1,'policy':policy}}
        source={'channels':channels,'targets':targets,'time':.37}
        result=json.loads(native.retarget(json.dumps(source)))
        if 'error' in result:raise ValueError(result['error'])
        for part in ('track','thumb'):
            for name in ('bodyMix','depthScale'):
                expected=equation_sample(channels[part][name],targets[part]['target'][name],source['time'],policy!='spring')['state']
                if mismatch(result['channels'][part][name]['initial'],expected):raise ValueError('native part retarget discarded raw state')
        expected=equation_sample(channels['travel'],1,source['time'],policy!='spring')['state']
        if mismatch(result['channels']['travel']['initial'],expected):raise ValueError('native travel retarget discarded raw state')
        for key,value in (('time',-1),('targets',[])):
            invalid={**source,key:value}
            if set(json.loads(native.retarget(json.dumps(invalid)))) != {'error'}:raise ValueError('invalid native retarget accepted')
    native.deleteLater()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('module-path','font','raster','output-dir'):parser.add_argument('--'+name,required=True,type=Path)
    args=parser.parse_args()
    os.environ.setdefault('QT_QPA_PLATFORM','offscreen');os.environ.setdefault('QT_QUICK_BACKEND','software')
    try:
        from PySide6.QtCore import QObject,QPluginLoader,QUrl,Qt
        from PySide6.QtGui import QGuiApplication,QFontDatabase
        from PySide6.QtQml import QQmlComponent,QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        from PySide6.QtTest import QTest
        app=QGuiApplication([])
        plugin=QPluginLoader(str(args.module_path.resolve()/'Resina/Runtime/libResina_Runtime.so'))
        if plugin.instance() is None:raise ValueError(plugin.errorString())
        if QFontDatabase.applicationFontFamilies(QFontDatabase.addApplicationFont(str(args.font.resolve()))) != ['DejaVu Sans']:
            raise ValueError('requires explicit DejaVu Sans')
        engine=QQmlEngine();engine.addImportPath(str(args.module_path.resolve()))
        component=QQmlComponent(engine,QUrl.fromLocalFile(str(ROOT/'conformance/quickshell/toggle-control-fixture.qml')))
        window=component.create()
        if window is None:raise ValueError(str(component.errors()))
        control=window.findChild(QObject,'toggleControl')
        window.requestActivate();wait(app,lambda:window.isActive() and window.isExposed(),'active native motion host')
        args.output_dir.mkdir(parents=True,exist_ok=True)
        def observe():return json.loads(window.observation())
        def check():
            wait(app,lambda:observe()['ready'] or bool(observe()['error']),'complete native motion publication')
            result=observe()
            if result['error'] or not result['ready']:raise ValueError(result['error'])
            snapshot=json.loads(result['snapshot']);metadata=json.loads(result['motion'])
            if snapshot['accessibility']['state']['checked'] != result['checked'] or snapshot['layout']['checked'] != result['checked']:
                raise ValueError('moving frame delayed current selection semantics')
            if json.loads(result['target']) != snapshot['hitRegion']:raise ValueError('motion resized native target')
            verify_motion(snapshot,metadata,source)
            return result,snapshot,metadata
        def native_oracle(name):
            drivers=[child for child in control.findChildren(QObject) if 'FrameAnimation' in child.metaObject().className()]
            if len(drivers)!=1:raise ValueError('must own one native motion driver')
            driver=drivers[0];running=driver.property('running');driver.setProperty('running',False)
            try:
                result,snapshot,metadata=check()
                verify_paint(window,control,source,snapshot,args.output_dir/name,args.raster,metadata)
                if observe()['motion'] != result['motion']:raise ValueError('frame changed during native readback')
            finally:driver.setProperty('running',running)
        window.availability(True)
        channels=fixture_channels()
        check_native_bindings(engine,channels)
        for theme in ('light','dark'):
            for direction in ('ltr','rtl'):
                source=request_for(theme,direction=direction)
                source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
                window.setProperty('motionChannels',json.dumps(channels))
                window.setProperty('configuration',json.dumps(source))
                result,snapshot,metadata=check()
                if metadata['motion']['travel']['trajectory']['settled']:raise ValueError('fixture never exposed actual travel')
                native_oracle(f'{theme}-{direction}-off.png')
                baseline=result['effects'];stable=result['target']
                if not window.invoke():raise ValueError('moving native Toggle rejected live invocation')
                result,snapshot,metadata=check()
                if not result['checked'] or result['effects']!=baseline+1 or result['target']!=stable:
                    raise ValueError('motion lost immediate checked activation or stable input')
                if metadata['channels']['travel']['initial']['velocity']==0:raise ValueError('retarget discarded raw travel velocity')
                native_oracle(f'{theme}-{direction}-on.png')
                window.setChecked(False);check()
        # Actual family binding and immediate Cast are verified on complete frames.
        for family in ('cast','frost','elastomer'):
            for theme in ('light','dark'):
                source=request_for(theme);source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
                authored=json.loads(source['content']['themeSource'])
                authored['materialAssignments']['control'].update(passive=family,interactive=family)
                source['content']['themeSource']=json.dumps(authored)
                window.setProperty('motionChannels',json.dumps(channels))
                if family=='frost':
                    # Normal transparency selects a translucent body and must remain rejected by this opaque profile.
                    window.setProperty('configuration',json.dumps(source))
                    wait(app,lambda:bool(observe()['error']),'unmodified translucent Frost rejection')
                    if 'opaque body' not in observe()['error'] or observe()['ready'] or observe()['snapshot'] or observe()['target']:
                        raise ValueError('translucent Frost was silently normalized into opaque paint')
                    # A distinct authored scene explicitly requests reduced transparency; runtime receives it unchanged.
                    source=copy.deepcopy(source)
                    source['content']['environment']['accessibilityPreferences']['reducedTransparency']=True
                window.setProperty('configuration',json.dumps(source))
                result,snapshot,metadata=check()
                if family=='cast' and not metadata['motion']['travel']['trajectory']['settled']:
                    raise ValueError('Cast did not select immediate travel')
                native_oracle(f'{family}-{theme}.png')
        # Wrapped, scaled complete glyphs and focused press motion use the same verified frame.
        for text,direction in (('Automatische Aktualisierungen aktivieren','ltr'),('التحديثات التلقائية\u2029Automatic updates','rtl')):
            source=request_for('light',text,direction)
            source['content']['environment']['textScale']=2
            source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
            window.setProperty('configuration',json.dumps(source))
            result,snapshot,metadata=check()
            if snapshot['label']['labelBounds']['height'] <= snapshot['label']['typography']['fontSize']['value']*snapshot['label']['typography']['lineHeight']:
                raise ValueError('native scaled-label fixture did not wrap')
            native_oracle('wrapped-'+direction+'.png')
        window.focusCommand();wait(app,lambda:observe()['state']['focused'],'actual focus before sampled press')
        QTest.keyPress(window,Qt.Key_Space)
        result,snapshot,metadata=check();begin=metadata['time']
        wait(app,lambda:observe()['ready'] and json.loads(observe()['motion'])['time']>begin+.3,'intermediate native press response')
        native_oracle('focused-moving-press.png')
        QTest.keyRelease(window,Qt.Key_Space);check();window.setChecked(False);check()
        # Raw overshoot is retained while native placement saturates at an allocated endpoint.
        source=request_for('light');source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
        overshoot=fixture_channels();overshoot['travel']['initial']={'position':1.5,'velocity':2}
        overshoot['travel']['dynamics'].update(stiffness=.04,damping=.4)
        window.setProperty('motionChannels',json.dumps(overshoot));window.setProperty('configuration',json.dumps(source))
        result,snapshot,metadata=check()
        travel=metadata['motion']['travel']
        if travel['projection']!='onEndpoint' or travel['trajectory']['settled'] or travel['trajectory']['state']['velocity']==0:
            raise ValueError('native travel lost projected overshoot or manufactured settling')
        native_oracle('overshoot-endpoint.png')
        window.setChecked(True);result,snapshot,metadata=check()
        if metadata['channels']['travel']['initial']['position']<=1 or metadata['channels']['travel']['initial']['velocity']==0:
            raise ValueError('native retarget fed a bounded coordinate back into travel')
        window.setChecked(False);check()
        # Independent channel settling: scheduling cannot stop when travel alone settles.
        source=copy.deepcopy(source)
        source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
        settling=fixture_channels()
        for part in ('track','thumb'):
            for name in ('bodyMix','depthScale'):
                settling[part][name]['dynamics'].update(stiffness=100,damping=20)
        settling['thumb']['bodyMix']['initial']['position']=.05
        settling['travel']['initial']={'position':0,'velocity':0}
        window.setProperty('motionChannels',json.dumps(settling))
        window.setProperty('configuration',json.dumps(source))
        result,snapshot,metadata=check()
        initial_time=metadata['time']
        if not metadata['motion']['travel']['trajectory']['settled'] or metadata['motion']['thumb']['bodyMix']['settled']:
            raise ValueError('settling fixture did not separate travel from body response')
        wait(app,lambda:observe()['ready'] and json.loads(observe()['motion'])['time']>initial_time,'continued native sampling after travel settled')
        wait(app,lambda:observe()['ready'] and all(json.loads(observe()['motion'])['motion'][part][name]['settled'] for part in ('track','thumb') for name in ('bodyMix','depthScale')),'all independent channels settled')
        drivers=[child for child in control.findChildren(QObject) if 'FrameAnimation' in child.metaObject().className()]
        if drivers[0].property('running'):raise ValueError('driver did not stop after every channel settled')
        # Exact reduced-motion policy for every channel, preserving visible authored targets.
        source=copy.deepcopy(source);source['content']['environment']['accessibilityPreferences']['reducedMotion']=True
        window.setProperty('configuration',json.dumps(source))
        result,snapshot,metadata=check()
        if any(not metadata['motion'][part][name]['settled'] for part in ('track','thumb') for name in ('bodyMix','depthScale')) or not metadata['motion']['travel']['trajectory']['settled']:
            raise ValueError('reduced motion did not settle every channel immediately')
        # Endpoint-valid sources can fail at a sampled pigment; no stale channel may survive.
        source=request_for('light');source['content']['environment']['accessibilityPreferences']['reducedMotion']=False
        window.setProperty('configuration',json.dumps(source));check()
        unsafe=fixture_channels();unsafe['thumb']['bodyMix']['initial']['position']=.99
        window.setProperty('motionChannels',json.dumps(unsafe))
        wait(app,lambda:bool(observe()['error']),'actual sampled contrast guard')
        if 'contrast' not in observe()['error'] or observe()['ready'] or observe()['snapshot'] or observe()['target']:
            raise ValueError('sample guard failed to withdraw complete production channels: '+observe()['error'])
        window.setProperty('motionChannels',json.dumps(channels));check()
        # Malformed dynamics retire all production channels, without changing Core contracts.
        window.setProperty('motionChannels','[]')
        wait(app,lambda:bool(observe()['error']),'strict motion rejection')
        if observe()['ready'] or observe()['snapshot'] or observe()['target']:raise ValueError('malformed motion retained production channels')
        window.close()
    except (OSError,ValueError,RuntimeError,ImportError) as error:
        print('Qt integrated Toggle motion failed: '+str(error),file=sys.stderr);return 1
    print('PASS integrated native Toggle motion: independent trajectories, current checked semantics, raw retargeting, stable target, CPU/native pixel oracle and immediate reduced motion')
    return 0

if __name__=='__main__':raise SystemExit(main())

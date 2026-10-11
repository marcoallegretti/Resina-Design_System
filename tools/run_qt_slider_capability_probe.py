"""Measure native toolkit Slider capabilities on an isolated AT-SPI bus.

This fixture deliberately uses Qt default delivery, with no Resina controller.
Observed gaps are diagnostics, never qualification of a production component.
"""
import argparse,json,subprocess,sys,tempfile,time
from pathlib import Path
parser=argparse.ArgumentParser(description='Measure Qt 6.11.2 native Slider mapping; this is a toolkit probe, not a Resina component.')
parser.add_argument('--orientation',choices=('horizontal','vertical'),default='horizontal')
parser.add_argument('--host',action='store_true',help=argparse.SUPPRESS)
args=parser.parse_args()
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
if args.host:
 from PySide6.QtCore import QUrl,qVersion
 from PySide6.QtGui import QGuiApplication
 from PySide6.QtQml import QQmlComponent,QQmlEngine
 from PySide6.QtQuick import QQuickWindow
 if qVersion()!='6.11.2':raise RuntimeError('requires Qt 6.11.2')
 app=QGuiApplication([]);engine=QQmlEngine();component=QQmlComponent(engine)
 component.setData(('''import QtQuick
Window { width:400; height:200; visible:true
 Item { id: control; x:40;y:40;width:240;height:80; focus:true; activeFocusOnTab:true
   property real value: .4
   property real minimumValue: 0
   property real maximumValue: 1
   property real stepSize: .05
   property string valueText: "40 percent"
   property int orientation: Qt.ORIENTATION
   property bool readOnly: false
   Accessible.role:Accessible.Slider
   Accessible.name:"Slider capability fixture"
   Accessible.description:"Native toolkit capability only."
   Accessible.focusable:true
   Accessible.readOnly:readOnly
   onValueChanged: { console.log("VALUE " + value + " READONLY " + readOnly); readOnly=true; }
 }
 Component.onCompleted: console.log("READY")
}'''.replace('ORIENTATION','Horizontal' if args.orientation=='horizontal' else 'Vertical')).encode(),QUrl())
 window=component.create()
 if window is None:raise RuntimeError(str(component.errors()))
 window.requestActivate();sys.exit(app.exec())
from check_slint_native_availability import ACCESSIBLE,PROPERTIES,call,references,string_value,integer_values,payload
address=string_value(call('org.a11y.Bus','/org/a11y/bus','org.a11y.Bus.GetAddress'))
for name in ('IsEnabled','ScreenReaderEnabled'):
 call('org.a11y.Bus','/org/a11y/bus',PROPERTIES+'.Set','string:org.a11y.Status','string:'+name,'variant:boolean:true')
with tempfile.TemporaryFile(mode='w+') as log:
 process=subprocess.Popen([sys.executable,__file__,'--host','--orientation',args.orientation],stdout=log,stderr=log)
 try:
  deadline=time.monotonic()+10;node=None
  while time.monotonic()<deadline:
   pending=references(call('org.a11y.atspi.Registry','/org/a11y/atspi/accessible/root',ACCESSIBLE+'.GetChildren',address=address))
   if len(pending)>1:raise ValueError('requires an isolated native AT-SPI bus')
   visited=set()
   while pending:
    item=pending.pop(0)
    if item in visited or len(visited)>=16:raise ValueError('cyclic or excessive native tree')
    visited.add(item)
    name=string_value(call(*item,PROPERTIES+'.Get','string:'+ACCESSIBLE,'string:Name',address=address))
    if name=='Slider capability fixture':node=item;break
    pending.extend(references(call(*item,ACCESSIBLE+'.GetChildren',address=address)))
   if node:break
   if process.poll() is not None:log.seek(0);raise RuntimeError(log.read())
   time.sleep(.02)
  if node is None:raise RuntimeError('No native Slider node')
  def get(name):return payload(call(*node,PROPERTIES+'.Get','string:org.a11y.atspi.Value','string:'+name,address=address))
  def state():
   words=integer_values(call(*node,ACCESSIBLE+'.GetState',address=address),array=True)
   return {'words':words,'horizontal':bool(words[0]&(1<<14)),'vertical':bool(words[0]&(1<<29)),'readOnly':bool(words[1]&(1<<(43-32)))}
  report={'scope':'toolkit capability probe; not a Resina component','hostVersion':'6.11.2','authoredOrientation':args.orientation,'role':integer_values(call(*node,ACCESSIBLE+'.GetRole',address=address))[0],
   'value':{name:get(name) for name in ('MinimumValue','MaximumValue','CurrentValue','MinimumIncrement')},'state':state(),
   'attributes':payload(call(*node,ACCESSIBLE+'.GetAttributes',address=address)),
   'actions':payload(call(*node,'org.a11y.atspi.Action.GetActions',address=address)),
   'introspection':payload(call(*node,'org.freedesktop.DBus.Introspectable.Introspect',address=address))}
  if report['role']!=51:raise ValueError('requires native Slider role 51')
  try:report['valueText']=get('Text')
  except (RuntimeError,ValueError) as e:report['valueTextError']=str(e)
  call(*node,PROPERTIES+'.Set','string:org.a11y.atspi.Value','string:CurrentValue','variant:double:0.65',address=address)
  time.sleep(.1);report['afterValidSet']={'value':get('CurrentValue'),'state':state()}
  call(*node,PROPERTIES+'.Set','string:org.a11y.atspi.Value','string:CurrentValue','variant:double:2',address=address)
  time.sleep(.1);report['afterReadOnlyOutOfRangeSet']={'value':get('CurrentValue'),'state':state()}
  log.seek(0);report['host']=log.read();print('RESINA_QT_SLIDER_CAPABILITY '+json.dumps(report,sort_keys=True))
 finally:process.terminate();process.wait(timeout=3)

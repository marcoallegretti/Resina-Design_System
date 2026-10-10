"""Complete static Toggle realization; injected Qt input is not physical-device evidence."""
import argparse
import copy
import json
import io
import math
import os
from pathlib import Path
import sys

from check_qt_command_snapshot_runtime import ROOT, request_for as command_request
from check_qt_command_control_runtime import wait
from check_schemas import validator_for
from check_quickshell_paint_runtime import compare_images, read_backend_output


def request_for(theme="light", text="Automatic updates", direction="ltr"):
    request = command_request(theme, text)
    definition = json.loads((ROOT / "definitions/components/toggle.json").read_text())
    content = request["content"]
    content.update(minimumSize={"width":140,"height":80},maximumSize={"width":180,"height":240},
                   padding={"start":8,"end":8,"top":8,"bottom":8})
    content["environment"]["layoutDirection"] = direction
    def part(name):
        item = definition[name]
        surface = copy.deepcopy(request["body"]["surface"])
        surface.update(materialRole=item["materialRole"],colorRole=item["colorRole"])
        surface["form"].update(shape=item["shape"],elevation=item["elevation"])
        return {"surface":surface,"appearance":request["body"]["appearance"],
                "checkedColorRole":item["checkedColorRole"],"interactionAppearance":request["body"]["commandAppearance"],
                "foregroundRole":item["contentRole"],"minimumContentContrast":4.5,"minimumEdgeContrast":3}
    result = {key:request[key] for key in ("content","activation","hovered","description","focusable","availableBounds","componentMinimum","occupiedRegions")}
    result.update(availableBounds={"x":-300,"y":-100,"width":1000,"height":1000},track=part("track"),thumb=part("thumb"),
        layout={"trackSize":{"width":128,"height":80},"thumbSize":{"width":24,"height":24},
                "insets":{"start":40,"end":40,"top":28,"bottom":28}},
        labelOrigin={"x":160 if direction == "ltr" else -200,"y":0},
        labelForegroundRole=definition["label"]["colorRole"],background=request["body"]["adjacentColor"],
        minimumLabelContrast=4.5,minimumThumbContrast=3,checked=False,
        targetBounds={"x":-20 if direction == "ltr" else -220,"y":-20,"width":380,"height":300})
    return result


def verify_paint(window, control, source, snapshot, path, raster):
    from PIL import Image, ImageDraw
    from PySide6.QtCore import QPointF
    frame=control.property("currentFrame")
    origin=frame.mapToScene(QPointF(0,0));dpr=window.devicePixelRatio()
    image=window.grabWindow()
    if image.isNull() or not image.save(str(path)):raise ValueError("native Toggle readback failed")
    background=tuple(round(value*255) for value in source["background"]["components"])+(255,)
    expected=Image.new("RGBA",(image.width(),image.height()),"white")
    bounds=snapshot["hitRegion"]["bounds"]
    ImageDraw.Draw(expected).rectangle((round((origin.x()+bounds["x"])*dpr),round((origin.y()+bounds["y"])*dpr),
        round((origin.x()+bounds["x"]+bounds["width"])*dpr)-1,round((origin.y()+bounds["y"]+bounds["height"])*dpr)-1),fill=background)
    for part in ("track","thumb"):
        config=source[part]
        backdrop=source["background"] if part=="track" else snapshot["track"]["paint"]["body"]["pigment"]["body"]
        body={"schemaVersion":"0.1.0","theme":{"schemaVersion":"0.1.0",**{key:source["content"][key]
            for key in ("themeSource","externalSources","environment")}},
            "surface":copy.deepcopy(config["surface"]),"size":source["layout"][part+"Size"],
            "appearance":config["appearance"],"foregroundRole":config["foregroundRole"],
            "postTreatmentBackdrop":backdrop,"adjacentColor":backdrop,
            "minimumContentContrast":config["minimumContentContrast"],"minimumEdgeContrast":config["minimumEdgeContrast"]}
        body["surface"]["states"]=snapshot[part]["paint"]["body"]["states"]
        request={"schemaVersion":"0.1.0","surface":{"schemaVersion":"0.1.0","body":body,"surroundingColor":source["background"] if part=="track" else None},
            "part":part,"checkedColorRole":config["checkedColorRole"],"interactionAppearance":config["interactionAppearance"]}
        if part=="thumb":request["surface"].pop("surroundingColor")
        placement={"x":0,"y":0} if part=="track" else snapshot["layout"]["thumbBounds"]
        png=read_backend_output([str(raster.resolve()),"-",str(image.width()),str(image.height()),
            str(-origin.x()-placement["x"]),str(-origin.y()-placement["y"]),str(dpr),"4"],json.dumps(request).encode())
        with Image.open(io.BytesIO(png)) as paint:expected.alpha_composite(paint.convert("RGBA"))
    with Image.open(path) as captured:
        actual=captured.convert("RGBA")
        label=snapshot["label"]["labelBounds"];offset=snapshot["labelOrigin"]
        rectangle=(math.floor((origin.x()+offset["x"]+label["x"])*dpr)-1,
            math.floor((origin.y()+offset["y"]+label["y"])*dpr)-1,
            math.ceil((origin.x()+offset["x"]+label["x"]+label["width"])*dpr)+1,
            math.ceil((origin.y()+offset["y"]+label["y"]+label["height"])*dpr)+1)
        ImageDraw.Draw(expected).rectangle(rectangle,fill="white");ImageDraw.Draw(actual).rectangle(rectangle,fill="white")
        compare_images(expected,actual)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module-path",required=True,type=Path)
    parser.add_argument("--raster",required=True,type=Path)
    parser.add_argument("--font",required=True,type=Path)
    parser.add_argument("--output-dir",required=True,type=Path)
    parser.add_argument("--parent-x11-display")
    parser.add_argument("--parent-x11-window",type=lambda value:int(value,0))
    args=parser.parse_args()
    if bool(args.parent_x11_display) != (args.parent_x11_window is not None):parser.error("supply both parent display and output window")
    os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
    os.environ.setdefault("QT_QUICK_BACKEND","software")
    try:
        from PySide6.QtCore import QObject,QPluginLoader,QPointF,Qt,QUrl,qVersion
        from PySide6.QtGui import QFontDatabase,QGuiApplication
        from PySide6.QtQml import QQmlComponent,QQmlEngine
        from PySide6.QtQuick import QQuickWindow
        from PySide6.QtTest import QTest
        if qVersion() != "6.11.2":raise ValueError("requires Qt 6.11.2")
        app=QGuiApplication([])
        plugin=QPluginLoader(str(args.module_path.resolve()/"Resina/Runtime/libResina_Runtime.so"))
        if plugin.instance() is None: raise ValueError(plugin.errorString())
        if QFontDatabase.applicationFontFamilies(QFontDatabase.addApplicationFont(str(args.font.resolve()))) != ["DejaVu Sans"]:
            raise ValueError("requires explicit DejaVu Sans")
        engine=QQmlEngine();engine.addImportPath(str(args.module_path.resolve()))
        component=QQmlComponent(engine,QUrl.fromLocalFile(str(ROOT/"conformance/quickshell/toggle-control-fixture.qml")))
        window=component.create()
        if window is None:raise ValueError(str(component.errors()))
        control=window.findChild(QObject,"toggleControl")
        if control.metaObject().property(control.metaObject().indexOfProperty("checked")).isWritable():
            raise ValueError("checked must be read-only; external replacement uses the Rust owner")
        args.output_dir.mkdir(parents=True,exist_ok=True)
        window.requestActivate()
        wait(app,lambda:window.isActive() and window.isExposed(),"exposed native Toggle window")
        def observe():return json.loads(window.observation())
        def check():
            wait(app,lambda:observe()["ready"] or bool(observe()["error"]),"complete Toggle publication")
            result=observe()
            if result["error"] or not result["ready"]:raise ValueError(result["error"])
            snapshot=json.loads(result["snapshot"])
            for member,schema in (("label","command-label-ir"),("layout","toggle-layout-ir"),("track","toggle-part-paint-ir"),
                                  ("thumb","toggle-part-paint-ir"),("hitRegion","hit-region-ir"),("accessibility","toggle-accessibility-ir")):
                validator_for(f"schemas/{schema}.schema.json").validate(snapshot[member])
            checked=result["checked"]
            if snapshot["layout"]["checked"] != checked or snapshot["accessibility"]["state"]["checked"] != checked:
                raise ValueError("layout/semantics disagree with checked owner")
            state=result["state"];hold=state["hold"]
            pressed=bool(hold and (hold["kind"]=="key" or hold["inside"]))
            states=[]
            if not pressed and not result["hovered"] and state["enabled"]:states.append("rest")
            if result["hovered"]:states.append("hover")
            if state["focused"]:states.append("focused")
            if pressed:states.append("pressed")
            if checked:states.append("checked")
            if not state["enabled"]:states.append("disabled")
            for part in ("track","thumb"):
                if snapshot[part]["part"] != part or snapshot[part]["checked"] != checked:
                    raise ValueError("part identity/selection differs from complete owner")
                if ("focus" in snapshot[part]["paint"]) != (part=="track" and state["focused"]):
                    raise ValueError("track-only navigation disagrees with actual focus")
                if snapshot[part]["paint"]["body"]["states"]["states"] != states:
                    raise ValueError("part feedback differs from activation/selection")
            if snapshot["accessibility"]["role"] != "switch" or json.loads(result["target"]) != snapshot["hitRegion"]:
                raise ValueError("complete target/role incoherence")
            if result["readinessViolations"]:raise ValueError("premature readiness")
            return result
        def prepare(request):
            window.setProperty("configuration",json.dumps(request))
            return check()
        window.availability(True)
        for theme in ("light","dark"):
            for direction in ("ltr","rtl"):
                source=request_for(theme,direction=direction)
                prepare(source)
                before=observe()["effects"]
                for checked in (False,True,False):
                    if not window.setChecked(checked):raise ValueError("checked replacement rejected")
                    result=check()
                    if result["checked"] != checked or result["effects"] != before:raise ValueError("external replacement produced activation")
                    verify_paint(window,control,source,json.loads(result["snapshot"]),
                        args.output_dir/f"{theme}-{direction}-{checked}.png",args.raster)
        prepare(request_for())
        window.focusCommand();wait(app,lambda:observe()["state"]["focused"],"native focus")
        before=observe()["effects"]
        QTest.keyPress(window,Qt.Key_Space)
        wait(app,lambda:observe()["state"]["hold"] is not None,"Space hold")
        window.setChecked(True);check()
        QTest.keyRelease(window,Qt.Key_Space)
        result=check()
        if result["checked"] or result["effects"] != before+1:raise ValueError("Space release did not use live replacement")
        QTest.keyPress(window,Qt.Key_Return)
        result=check()
        if not result["checked"] or result["effects"] != before+2 or result["state"]["hold"] != {"kind":"key","key":"enter"}:
            raise ValueError("Enter did not commit on down while retaining its native hold")
        QTest.keyRelease(window,Qt.Key_Return)
        result=check()
        if result["effects"] != before+2 or result["state"]["hold"]:raise ValueError("Enter release activated twice")
        # A real QQuickWindow event exercises the compiled pointer capture owner.
        QTest.mousePress(window,Qt.LeftButton,Qt.NoModifier,QPointF(100,100).toPoint())
        wait(app,lambda:observe()["state"]["hold"] is not None,"native pointer capture")
        window.setChecked(False)
        QTest.mouseRelease(window,Qt.LeftButton,Qt.NoModifier,QPointF(100,100).toPoint())
        result=check()
        if not result["checked"] or result["effects"] != before+3:raise ValueError("pointer release did not commit exactly once")
        window.setProperty("revoke",True)
        if not window.invoke():raise ValueError("live invoke rejected")
        result=check()
        if result["checked"] or result["state"]["enabled"] or result["focused"]:raise ValueError("product revocation lost coherent disabled state")
        effects=result["effects"]
        if window.invoke() or observe()["effects"] != effects:raise ValueError("stale invoke activated")
        invalid=request_for();invalid["minimumThumbContrast"]=21
        window.setProperty("configuration",json.dumps(invalid))
        wait(app,lambda:bool(observe()["error"]),"contrast rejection")
        if observe()["ready"] or observe()["snapshot"] or observe()["target"]:raise ValueError("failed preparation retained production channels")
        window.setProperty("revoke",False)
        window.availability(True)
        prepare(request_for())
        # Focus loss, scene transform and visibility retire live holds and stale channels.
        baseline=observe()["effects"]
        window.focusCommand();wait(app,lambda:observe()["state"]["focused"],"restored focus")
        QTest.keyPress(window,Qt.Key_Space)
        window.focusOther();wait(app,lambda:observe()["state"]["hold"] is None,"focus-loss cancellation")
        QTest.keyRelease(window,Qt.Key_Space)
        if observe()["effects"] != baseline:raise ValueError("lost focus activated")
        QTest.mousePress(window,Qt.LeftButton,Qt.NoModifier,QPointF(100,100).toPoint())
        wait(app,lambda:observe()["state"]["hold"] is not None,"held pointer before transform")
        window.rotateCarrier(5)
        wait(app,lambda:not observe()["target"] and not observe()["snapshot"],"affine guard withdrawal")
        QTest.mouseRelease(window,Qt.LeftButton,Qt.NoModifier,QPointF(100,100).toPoint())
        if observe()["effects"] != baseline:raise ValueError("transformed target activated")
        window.rotateCarrier(0);window.renew();check()
        window.showCommand(False)
        wait(app,lambda:not observe()["target"] and not observe()["snapshot"],"hidden channel withdrawal")
        window.showCommand(True);check()
        window.setProperty("revokeOnReady",True)
        window.setChecked(not observe()["checked"])
        result=check()
        if result["state"]["enabled"] or window.property("revokeOnReady") or result["effects"] != baseline:
            raise ValueError("snapshot callback lost availability revocation or produced an effect")
        if args.parent_x11_display:
            from qt_nested_input import exercise
            exercise(app,window,args.parent_x11_display,args.parent_x11_window,wait,observe,check)
        window.close()
    except (OSError,ValueError,RuntimeError,ImportError) as error:
        print("Qt complete Toggle check failed: "+str(error),file=sys.stderr);return 1
    print("PASS complete static Toggle: light/dark RTL/LTR, complete native capture, live checked replacement, injected key/pointer capture, exactly-once revocation, contrast rejection")
    return 0

if __name__ == "__main__":raise SystemExit(main())

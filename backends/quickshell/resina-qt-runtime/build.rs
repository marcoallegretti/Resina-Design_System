use cxx_qt_build::{CxxQtBuilder, PluginType, QmlModule};

fn main() {
    println!("cargo:rerun-if-changed=src/context.h");
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("Resina.Runtime")
            .version(0, 1)
            .plugin_type(PluginType::Dynamic),
    )
    .files([
        "src/lib.rs",
        "src/toggle.rs",
        "src/toggle_snapshot.rs",
        "src/label.rs",
        "src/snapshot.rs",
        "src/pointer.rs",
    ])
    .qt_module("Quick")
    .qt_module("Gui")
    .cpp_file("src/command.h")
    .cpp_file("src/command.cpp")
    .cpp_file("src/label.h")
    .cpp_file("src/label.cpp")
    .cpp_file("src/pointer.h")
    .cpp_file("src/pointer.cpp");
    // Rust cdylib export filtering hides C++ symbols, including Qt's plugin entry.
    // Rename the generated entry so the Rust export can forward to it.
    unsafe {
        builder.cc_builder(|cc| {
            cc.define("qt_plugin_instance", "resina_qt_plugin_instance");
        })
    }
    .build();
}

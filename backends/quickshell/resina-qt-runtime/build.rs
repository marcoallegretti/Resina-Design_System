use cxx_qt_build::{CxxQtBuilder, PluginType, QmlModule};

fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("Resina.Runtime")
            .version(0, 1)
            .plugin_type(PluginType::Dynamic),
    )
    .files(["src/lib.rs"]);
    // Rust cdylib export filtering hides C++ symbols, including Qt's plugin entry.
    // Rename the generated entry so the Rust export can forward to it.
    unsafe {
        builder.cc_builder(|cc| {
            cc.define("qt_plugin_instance", "resina_qt_plugin_instance");
        })
    }
    .build();
}

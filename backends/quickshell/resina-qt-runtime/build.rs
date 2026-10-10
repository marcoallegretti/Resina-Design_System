use cxx_qt_build::{CxxQtBuilder, PluginType, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("Resina.Runtime")
            .version(0, 1)
            .plugin_type(PluginType::Dynamic),
    )
    .files(["src/lib.rs"])
    .build();
}

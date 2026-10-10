use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use resina_model::{ActivationEvent, ActivationState};
use resina_resolver::{
    ActivationResult, CaptureChange, resolve_activation, resolve_activation_source,
};
use std::pin::Pin;

mod label;
mod snapshot;

const MAX_EVENT_BYTES: usize = 64 * 1024;

unsafe extern "C" {
    fn resina_qt_plugin_instance() -> *mut std::ffi::c_void;
}

#[unsafe(no_mangle)]
extern "C" fn qt_plugin_instance() -> *mut std::ffi::c_void {
    // Qt's generated factory owns the QObject; no Rust reference or ownership
    // crosses this boundary. The signature is Qt's QObject* plugin entry ABI.
    unsafe { resina_qt_plugin_instance() }
}

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, publication, READ, NOTIFY)]
        #[qproperty(QString, last_error, READ, cxx_name = "lastError")]
        type CommandController = super::CommandControllerRust;

        #[qinvokable]
        fn dispatch(self: Pin<&mut CommandController>, event: &QString) -> bool;

        #[qsignal]
        fn activated(self: Pin<&mut CommandController>);

        #[qsignal]
        #[cxx_name = "captureRequested"]
        fn capture_requested(self: Pin<&mut CommandController>, id: QString, acquire: bool);
    }
}

/// Owns committed activation state independently of toolkit publication.
#[derive(Debug)]
struct CommandOwner {
    state: ActivationState,
}

impl Default for CommandOwner {
    fn default() -> Self {
        Self {
            state: ActivationState::try_new(false, false, None).expect("valid inactive command"),
        }
    }
}

impl CommandOwner {
    fn state(&self) -> &ActivationState {
        &self.state
    }

    fn prepare(&self, event: &str) -> Result<ActivationResult, String> {
        if event.len() > MAX_EVENT_BYTES {
            return Err("Qt command event exceeds 64 KiB".to_owned());
        }
        let state = serde_json::to_string(&self.state).expect("validated activation state");
        // Preserve source members so the shared parser can reject duplicates.
        let request = format!(r#"{{"schemaVersion":"0.1.0","state":{state},"event":{event}}}"#);
        resolve_activation_source(&request).map_err(|error| error.to_string())
    }

    fn commit(&mut self, result: &ActivationResult) {
        self.state = result.state().clone();
    }
}

pub struct CommandControllerRust {
    publication: QString,
    last_error: QString,
    owner: CommandOwner,
    publishing: bool,
}

impl Default for CommandControllerRust {
    fn default() -> Self {
        let owner = CommandOwner::default();
        let initial = resolve_activation(owner.state(), &ActivationEvent::Cancel {})
            .expect("valid inactive command");
        Self {
            publication: QString::from(
                serde_json::to_string(&initial).expect("validated transition"),
            ),
            last_error: QString::default(),
            owner,
            publishing: false,
        }
    }
}

impl qobject::CommandController {
    fn dispatch(mut self: Pin<&mut Self>, event: &QString) -> bool {
        if self.rust().publishing {
            self.as_mut().rust_mut().last_error =
                QString::from("command publication is in progress");
            return false;
        }
        let result = match self.rust().owner.prepare(&event.to_string()) {
            Ok(result) => result,
            Err(error) => {
                self.as_mut().rust_mut().last_error = QString::from(error);
                return false;
            }
        };
        let publication =
            QString::from(serde_json::to_string(&result).expect("validated transition"));
        {
            let mut state = self.as_mut().rust_mut();
            state.publishing = true;
            state.owner.commit(&result);
            state.publication = publication;
            state.last_error = QString::default();
        }
        self.as_mut().publication_changed();
        self.as_mut().rust_mut().publishing = false;
        if let Some(capture) = result.capture() {
            let (id, acquire) = match capture {
                CaptureChange::Acquire { id } => (id, true),
                CaptureChange::Release { id } => (id, false),
            };
            self.as_mut()
                .capture_requested(QString::from(id.as_str()), acquire);
        }
        if result.activate() {
            self.as_mut().activated();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_and_stale_invocations_follow_live_committed_state() {
        let mut owner = CommandOwner::default();
        assert!(!owner.prepare(r#"{"kind":"invoke"}"#).unwrap().activate());
        let enabled = owner
            .prepare(r#"{"kind":"availability","enabled":true}"#)
            .unwrap();
        owner.commit(&enabled);
        let invoke = owner.prepare(r#"{"kind":"invoke"}"#).unwrap();
        assert!(invoke.activate());
        owner.commit(&invoke);
        let disabled = owner
            .prepare(r#"{"kind":"availability","enabled":false}"#)
            .unwrap();
        owner.commit(&disabled);
        assert!(!owner.prepare(r#"{"kind":"invoke"}"#).unwrap().activate());
    }

    #[test]
    fn rejected_source_never_changes_committed_state() {
        let owner = CommandOwner::default();
        let before = owner.state().clone();
        for event in [
            r#"{"kind":"invoke","kind":"invoke"}"#,
            r#"["invoke"]"#,
            r#"{"kind":"pointerDown","id":"","inside":true}"#,
            "null",
        ] {
            assert!(owner.prepare(event).is_err());
            assert_eq!(owner.state(), &before);
        }
        assert!(owner.prepare(&" ".repeat(MAX_EVENT_BYTES + 1)).is_err());
    }
}

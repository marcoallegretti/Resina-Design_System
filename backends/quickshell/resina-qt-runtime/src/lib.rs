use cxx_qt_lib::QString;
use resina_model::{ActivationEvent, ActivationState};
use resina_resolver::{ActivationResult, resolve_activation, resolve_activation_source};

mod appearance_source;
mod label;
mod layout;
mod pointer;
mod slider_accessibility;
mod snapshot;
mod surface;
mod toggle;
mod toggle_snapshot;

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

#[cxx::bridge(namespace = "resina::qt")]
mod command_ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        type CommandOwner;
        fn new_command_owner() -> Box<CommandOwner>;
        fn command_initial_publication(owner: &CommandOwner) -> QString;
        fn dispatch_command(owner: &mut CommandOwner, event: &QString) -> Result<QString>;
    }
}

/// Owns committed activation state independently of toolkit publication.
#[derive(Debug)]
pub struct CommandOwner {
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

fn new_command_owner() -> Box<CommandOwner> {
    Box::new(CommandOwner::default())
}

fn command_initial_publication(owner: &CommandOwner) -> QString {
    let initial = resolve_activation(owner.state(), &ActivationEvent::Cancel {})
        .expect("valid inactive command");
    QString::from(serde_json::to_string(&initial).expect("validated transition"))
}

fn dispatch_command(owner: &mut CommandOwner, event: &QString) -> Result<QString, String> {
    let result = owner.prepare(&event.to_string())?;
    let publication = QString::from(serde_json::to_string(&result).expect("validated transition"));
    owner.commit(&result);
    Ok(publication)
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

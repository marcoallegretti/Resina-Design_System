use crate::{CommandOwner, MAX_EVENT_BYTES};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use resina_model::ActivationEvent;
use resina_resolver::{
    CaptureChange, ToggleActivationResult, resolve_toggle_activation,
    resolve_toggle_activation_source,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use std::pin::Pin;

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
        type ToggleController = super::ToggleControllerRust;
        #[qinvokable]
        fn dispatch(self: Pin<&mut ToggleController>, event: &QString) -> bool;
        #[qinvokable]
        #[cxx_name = "setChecked"]
        fn set_checked(self: Pin<&mut ToggleController>, source: &QString) -> bool;
        #[qsignal]
        fn activated(self: Pin<&mut ToggleController>, checked: bool);
        #[qsignal]
        #[cxx_name = "captureRequested"]
        fn capture_requested(self: Pin<&mut ToggleController>, id: QString, acquire: bool);
    }
}

#[derive(Debug, Default)]
struct ToggleOwner {
    activation: CommandOwner,
    checked: bool,
}
impl ToggleOwner {
    fn prepare(&self, event: &str) -> Result<ToggleActivationResult, String> {
        if event.len() > MAX_EVENT_BYTES {
            return Err("Qt toggle event exceeds 64 KiB".to_owned());
        }
        let state = serde_json::to_string(self.activation.state()).expect("validated activation");
        let request = format!(
            r#"{{"schemaVersion":"0.1.0","state":{state},"checked":{},"event":{event}}}"#,
            self.checked
        );
        resolve_toggle_activation_source(&request).map_err(|error| error.to_string())
    }
    fn replace_checked(&self, source: &str) -> Result<ToggleActivationResult, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Update {
            checked: bool,
        }
        if source.len() > MAX_EVENT_BYTES {
            return Err("Qt checked update exceeds 64 KiB".to_owned());
        }
        let document = parse_token_document(source).map_err(|error| error.to_string())?;
        if !document.is_object() {
            return Err("Qt checked update must be an object".to_owned());
        }
        let update: Update = serde_json::from_value(document).map_err(|error| error.to_string())?;
        // A same-availability observation preserves the live gesture while the
        // external selection value replaces the next operation's checked input.
        resolve_toggle_activation(
            self.activation.state(),
            update.checked,
            &ActivationEvent::Availability {
                enabled: self.activation.state().enabled(),
            },
        )
        .map_err(|error| error.to_string())
    }
    fn commit(&mut self, result: &ToggleActivationResult) {
        self.activation.commit(result.activation());
        self.checked = result.checked();
    }
}

pub struct ToggleControllerRust {
    publication: QString,
    last_error: QString,
    owner: ToggleOwner,
    publishing: bool,
}
impl Default for ToggleControllerRust {
    fn default() -> Self {
        let owner = ToggleOwner::default();
        let initial = resolve_toggle_activation(
            owner.activation.state(),
            owner.checked,
            &ActivationEvent::Cancel {},
        )
        .expect("valid inactive toggle");
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
impl qobject::ToggleController {
    fn deliver(mut self: Pin<&mut Self>, source: &QString, replace_checked: bool) -> bool {
        if self.rust().publishing {
            self.as_mut().rust_mut().last_error =
                QString::from("toggle publication is in progress");
            return false;
        }
        let result = if replace_checked {
            self.rust().owner.replace_checked(&source.to_string())
        } else {
            self.rust().owner.prepare(&source.to_string())
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.as_mut().rust_mut().last_error = QString::from(error);
                return false;
            }
        };
        {
            let mut owner = self.as_mut().rust_mut();
            owner.publishing = true;
            owner.owner.commit(&result);
            owner.publication =
                QString::from(serde_json::to_string(&result).expect("validated transition"));
            owner.last_error = QString::default();
        }
        self.as_mut().publication_changed();
        self.as_mut().rust_mut().publishing = false;
        if let Some(capture) = result.activation().capture() {
            let (id, acquire) = match capture {
                CaptureChange::Acquire { id } => (id, true),
                CaptureChange::Release { id } => (id, false),
            };
            self.as_mut()
                .capture_requested(QString::from(id.as_str()), acquire);
        }
        if result.activation().activate() {
            self.as_mut().activated(result.checked());
        }
        true
    }
    fn dispatch(self: Pin<&mut Self>, source: &QString) -> bool {
        self.deliver(source, false)
    }
    fn set_checked(self: Pin<&mut Self>, source: &QString) -> bool {
        self.deliver(source, true)
    }
}

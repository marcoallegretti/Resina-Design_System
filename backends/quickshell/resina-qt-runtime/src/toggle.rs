use crate::{CommandOwner, MAX_EVENT_BYTES};
use cxx_qt_lib::QString;
use resina_model::ActivationEvent;
use resina_resolver::{
    ToggleActivationResult, resolve_toggle_activation, resolve_toggle_activation_source,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;

#[cxx::bridge(namespace = "resina::qt")]
mod toggle_ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        type ToggleOwner;
        fn new_toggle_owner() -> Box<ToggleOwner>;
        fn toggle_initial_publication(owner: &ToggleOwner) -> QString;
        fn dispatch_toggle(
            owner: &mut ToggleOwner,
            source: &QString,
            replace_checked: bool,
        ) -> Result<QString>;
    }
}

#[derive(Debug, Default)]
pub struct ToggleOwner {
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

fn new_toggle_owner() -> Box<ToggleOwner> {
    Box::new(ToggleOwner::default())
}

fn toggle_initial_publication(owner: &ToggleOwner) -> QString {
    let initial = resolve_toggle_activation(
        owner.activation.state(),
        owner.checked,
        &ActivationEvent::Cancel {},
    )
    .expect("valid inactive toggle");
    QString::from(serde_json::to_string(&initial).expect("validated transition"))
}

fn dispatch_toggle(
    owner: &mut ToggleOwner,
    source: &QString,
    replace_checked: bool,
) -> Result<QString, String> {
    let result = if replace_checked {
        owner.replace_checked(&source.to_string())
    } else {
        owner.prepare(&source.to_string())
    }?;
    let publication = QString::from(serde_json::to_string(&result).expect("validated transition"));
    owner.commit(&result);
    Ok(publication)
}

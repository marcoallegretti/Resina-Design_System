use crate::IconContent;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandContentForm {
    Label,
    IconAndLabel,
    IconOnly,
}

impl CommandContentForm {
    fn name(self) -> &'static str {
        match self {
            Self::Label => "label",
            Self::IconAndLabel => "iconAndLabel",
            Self::IconOnly => "iconOnly",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CommandContentInput", into = "CommandContentInput")]
pub struct CommandContent {
    form: CommandContentForm,
    icon: Option<IconContent>,
    text: String,
}

impl CommandContent {
    pub fn form(&self) -> CommandContentForm {
        self.form
    }

    pub fn icon(&self) -> Option<&IconContent> {
        self.icon.as_ref()
    }

    pub fn label(&self) -> Option<&str> {
        (self.form != CommandContentForm::IconOnly).then_some(self.text.as_str())
    }

    pub fn accessible_label(&self) -> Option<&str> {
        (self.form == CommandContentForm::IconOnly).then_some(self.text.as_str())
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandContentInput {
    schema_version: String,
    form: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    icon: Option<IconContent>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    label: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    accessible_label: Option<String>,
}

// An explicit null is not an absent member.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

impl TryFrom<CommandContentInput> for CommandContent {
    type Error = String;

    fn try_from(input: CommandContentInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("unsupported command content version".into());
        }
        let form = match input.form.as_str() {
            "label" => CommandContentForm::Label,
            "iconAndLabel" => CommandContentForm::IconAndLabel,
            "iconOnly" => CommandContentForm::IconOnly,
            other => return Err(format!("unknown command content form {other:?}")),
        };
        let name = form.name();
        let needs_icon = form != CommandContentForm::Label;
        if !needs_icon && input.icon.is_some() {
            return Err(format!("{name} content must not declare icon"));
        }
        if needs_icon && input.icon.is_none() {
            return Err(format!("{name} content requires icon"));
        }
        let (text, text_member, foreign, foreign_member) = if form == CommandContentForm::IconOnly {
            (
                input.accessible_label,
                "accessibleLabel",
                input.label,
                "label",
            )
        } else {
            (
                input.label,
                "label",
                input.accessible_label,
                "accessibleLabel",
            )
        };
        if foreign.is_some() {
            return Err(format!("{name} content must not declare {foreign_member}"));
        }
        let text = text.ok_or_else(|| format!("{name} content requires {text_member}"))?;
        if text.trim().is_empty() {
            return Err(format!("{text_member} must not be blank"));
        }
        Ok(Self {
            form,
            icon: input.icon,
            text,
        })
    }
}

impl From<CommandContent> for CommandContentInput {
    fn from(content: CommandContent) -> Self {
        let (label, accessible_label) = if content.form == CommandContentForm::IconOnly {
            (None, Some(content.text))
        } else {
            (Some(content.text), None)
        };
        Self {
            schema_version: "0.1.0".into(),
            form: content.form.name().into(),
            icon: content.icon,
            label,
            accessible_label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn content_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/content/command-content-vectors.json"
        ))
        .unwrap();
        let mut rejected = 0;
        for vector in vectors {
            let name = vector["name"].as_str().unwrap();
            for result in [
                serde_json::from_str::<CommandContent>(&vector["document"].to_string()),
                serde_json::from_value::<CommandContent>(vector["document"].clone()),
            ] {
                match vector.get("error") {
                    None => {
                        let content = result.unwrap();
                        let written = serde_json::to_value(&content).unwrap();
                        assert_eq!(written, vector["document"], "{name}");
                        assert_eq!(
                            serde_json::from_value::<CommandContent>(written).unwrap(),
                            content,
                            "{name}"
                        );
                    }
                    Some(expected) => {
                        let error = match result {
                            Ok(content) => panic!("{name}: accepted as {content:?}"),
                            Err(error) => error.to_string(),
                        };
                        assert!(
                            error.contains(expected.as_str().unwrap()),
                            "{name}: {error}"
                        );
                        rejected += 1;
                    }
                }
            }
        }
        assert!(rejected > 0);
    }

    #[test]
    fn missing_accessible_label_never_names_the_icon() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/content/command-content-vectors.json"
        ))
        .unwrap();
        for vector in vectors.iter().filter(|vector| {
            vector["document"]["form"] == "iconOnly"
                && vector.get("error").is_some()
                && vector["name"]
                    .as_str()
                    .unwrap()
                    .contains("accessible label")
        }) {
            let reference = vector["document"]["icon"]["reference"].as_str().unwrap();
            let error = match serde_json::from_value::<CommandContent>(vector["document"].clone()) {
                Ok(content) => panic!("{}: accepted as {content:?}", vector["name"]),
                Err(error) => error.to_string(),
            };
            assert!(!error.contains(reference), "{}: {error}", vector["name"]);
        }
    }

    #[test]
    fn forms_expose_only_their_own_text() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/content/command-content-vectors.json"
        ))
        .unwrap();
        let document = |name: &str| {
            serde_json::from_value::<CommandContent>(
                vectors
                    .iter()
                    .find(|vector| vector["name"] == name)
                    .unwrap()["document"]
                    .clone(),
            )
            .unwrap()
        };
        let label = document("label");
        assert_eq!(label.form(), CommandContentForm::Label);
        assert_eq!(label.label(), Some("Reconnect"));
        assert_eq!(label.accessible_label(), None);
        assert_eq!(label.text(), "Reconnect");
        assert!(label.icon().is_none());
        let pair = document("icon and label");
        assert_eq!(pair.form(), CommandContentForm::IconAndLabel);
        assert_eq!(pair.label(), Some("Back"));
        assert_eq!(pair.accessible_label(), None);
        assert_eq!(pair.text(), "Back");
        assert_eq!(
            pair.icon().unwrap().reference(),
            "resina.fixture:chevron-back"
        );
        let only = document("icon only label preserves whitespace");
        assert_eq!(only.form(), CommandContentForm::IconOnly);
        assert_eq!(only.label(), None);
        assert_eq!(only.accessible_label(), Some("  Mute\nmicrophone "));
        assert_eq!(only.text(), "  Mute\nmicrophone ");
        assert_eq!(
            only.icon().unwrap().reference(),
            "resina.fixture:microphone-off"
        );
    }

    #[test]
    fn duplicate_members_are_rejected() {
        let source = include_str!(
            "../../../../conformance/content/invalid-duplicate-command-content-member.json"
        );
        let unique = source.replacen(r#""accessibleLabel": "Mute microphone","#, "", 1);
        assert_ne!(unique, source);
        assert!(serde_json::from_str::<CommandContent>(&unique).is_ok());
        let error = serde_json::from_str::<CommandContent>(source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("duplicate field `accessibleLabel`"),
            "{error}"
        );
        let form = unique.replacen(
            r#""form": "iconOnly","#,
            r#""form": "iconOnly", "form": "label","#,
            1,
        );
        assert_ne!(form, unique);
        let error = serde_json::from_str::<CommandContent>(&form).unwrap_err();
        assert!(
            error.to_string().contains("duplicate field `form`"),
            "{error}"
        );
    }
}

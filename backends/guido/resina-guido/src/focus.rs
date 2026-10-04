use guido::{prelude::WidgetRef, tree::WidgetId};
use resina_resolver::FocusTraversalResult;
use std::{collections::HashSet, fmt};

pub struct FocusBinding<'a> {
    pub id: &'a str,
    pub widget: WidgetRef,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FocusTransferError {
    EmptyId(usize),
    DuplicateId(String),
    AliasedWidget(String),
    MissingBinding(String),
    UnboundTarget(String),
    StaleTarget(String),
}

impl fmt::Display for FocusTransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId(index) => write!(f, "focus binding {index} has an empty ID"),
            Self::DuplicateId(id) => write!(f, "duplicate focus binding ID {id:?}"),
            Self::AliasedWidget(id) => {
                write!(f, "focus binding {id:?} aliases another native widget")
            }
            Self::MissingBinding(id) => write!(f, "no native focus binding for {id:?}"),
            Self::UnboundTarget(id) => write!(f, "focus target {id:?} is not mounted"),
            Self::StaleTarget(id) => write!(f, "focus target {id:?} was removed or rebound"),
        }
    }
}

impl std::error::Error for FocusTransferError {}

pub struct RequestedFocus {
    target_id: String,
    widget: WidgetRef,
    mounted_id: WidgetId,
}

impl RequestedFocus {
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    pub fn is_focused(&self) -> Result<bool, FocusTransferError> {
        if self.widget.widget() != Some(self.mounted_id) {
            return Err(FocusTransferError::StaleTarget(self.target_id.clone()));
        }
        Ok(self.widget.is_focused())
    }
}

pub fn request_focus(
    result: &FocusTraversalResult,
    bindings: &[FocusBinding<'_>],
) -> Result<Option<RequestedFocus>, FocusTransferError> {
    let Some(target_id) = result.target_id() else {
        return Ok(None);
    };
    let mut ids = HashSet::new();
    let mut widgets = HashSet::new();
    let mut target = None;
    for (index, binding) in bindings.iter().enumerate() {
        if binding.id.is_empty() {
            return Err(FocusTransferError::EmptyId(index));
        }
        if !ids.insert(binding.id) {
            return Err(FocusTransferError::DuplicateId(binding.id.to_owned()));
        }
        if let Some(id) = binding.widget.widget()
            && !widgets.insert(id)
        {
            return Err(FocusTransferError::AliasedWidget(binding.id.to_owned()));
        }
        if binding.id == target_id {
            target = Some(binding.widget);
        }
    }
    let widget = target.ok_or_else(|| FocusTransferError::MissingBinding(target_id.to_owned()))?;
    let mounted_id = widget
        .widget()
        .ok_or_else(|| FocusTransferError::UnboundTarget(target_id.to_owned()))?;
    widget.focus();
    Ok(Some(RequestedFocus {
        target_id: target_id.to_owned(),
        widget,
        mounted_id,
    }))
}

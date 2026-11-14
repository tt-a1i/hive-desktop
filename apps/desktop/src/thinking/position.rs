use std::collections::HashMap;

pub(crate) struct ThinkingPosition {
    default_expanded: bool,
    states: HashMap<(String, usize), bool>,
}

impl ThinkingPosition {
    pub(crate) fn new(
        default_expanded: bool,
        states: impl IntoIterator<Item = ((String, usize), bool)>,
    ) -> Self {
        Self {
            default_expanded,
            states: states.into_iter().collect(),
        }
    }

    pub(crate) fn expanded(
        &self,
        default: bool,
        message: &str,
        index: usize,
        redacted: bool,
    ) -> bool {
        // Cache explicit choices for previously seen blocks. New blocks created
        // while this conversation was in the background still use the default.
        !redacted
            && if self.default_expanded == default {
                self.states
                    .get(&(message.to_owned(), index))
                    .copied()
                    .unwrap_or(default)
            } else {
                default
            }
    }
}

#[cfg(test)]
mod tests {
    use super::ThinkingPosition;

    #[test]
    fn returning_preserves_manual_choices_and_uses_default_for_new_background_blocks() {
        let position =
            ThinkingPosition::new(true, [(("a".into(), 0), false), (("a".into(), 1), true)]);
        assert!(!position.expanded(true, "a", 0, false));
        assert!(position.expanded(true, "a", 1, false));
        assert!(position.expanded(true, "a", 2, false));
        assert!(position.expanded(true, "new-message", 0, false));
        assert!(!position.expanded(true, "a", 1, true));
        assert!(!position.expanded(true, "new-message", 0, true));
    }

    #[test]
    fn changed_default_replaces_stale_choices_without_changing_new_manual_overrides() {
        let previous = ThinkingPosition::new(false, [(("a".into(), 0), false)]);
        assert!(previous.expanded(true, "a", 0, false));
        let next = ThinkingPosition::new(true, [(("a".into(), 0), false)]);
        assert!(!next.expanded(true, "a", 0, false));
        assert!(!next.expanded(false, "new-message", 0, false));
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
//! Where entered values live, and the read-only view the application gets.

use core::fmt;

use nowayos_bounded::BoundedStr;

use crate::form::FieldKind;

/// Storage for one field's value.
///
/// The variant is chosen from the field's kind when the flow is created and
/// never changes afterwards. Clearing therefore always happens in place:
/// text is emptied with [`BoundedStr::clear`], which zeroes its bytes,
/// instead of being dropped and left behind in memory. The tests here prove
/// the bytes are zero, not merely that a value is empty, with
/// [`BoundedStr::is_zeroed`].
#[derive(Default)]
pub(crate) enum Slot<const V: usize> {
    /// Spare capacity beyond the form's fields. Never holds a value.
    #[default]
    Unused,
    /// A text field's value.
    Text(BoundedStr<V>),
    /// A choice field's chosen option.
    Choice(Option<u8>),
    /// Whether a checkbox is checked.
    Checkbox(bool),
}

impl<const V: usize> Slot<V> {
    /// Empty storage for a field of `kind`.
    pub(crate) fn for_kind(kind: FieldKind) -> Self {
        match kind {
            FieldKind::Text(_) => Self::Text(BoundedStr::new()),
            FieldKind::Choice { .. } => Self::Choice(None),
            FieldKind::Checkbox => Self::Checkbox(false),
        }
    }

    /// Forgets the value, in place.
    pub(crate) fn clear(&mut self) {
        match self {
            Self::Unused => {}
            Self::Text(text) => text.clear(),
            Self::Choice(choice) => *choice = None,
            Self::Checkbox(checked) => *checked = false,
        }
    }

    /// Whether nothing has been entered. Text that is only spaces counts as
    /// entered: it is still something the person typed.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Unused => true,
            Self::Text(text) => text.is_empty(),
            Self::Choice(choice) => choice.is_none(),
            Self::Checkbox(checked) => !checked,
        }
    }

    /// The value, or `None` for spare capacity.
    pub(crate) fn view(&self) -> Option<Value<'_>> {
        match self {
            Self::Unused => None,
            Self::Text(text) => Some(Value::Text(text.as_str())),
            Self::Choice(choice) => Some(Value::Choice(*choice)),
            Self::Checkbox(checked) => Some(Value::Checkbox(*checked)),
        }
    }
}

/// A read-only view of one field's current value.
///
/// Its `Debug` output gives the length of text, never the text itself, so
/// logging a value with `{:?}` cannot leak what a person typed. Choice
/// numbers and checkbox states are shown: without the UI's labels they are
/// only numbers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// A text field's value; empty if nothing has been entered.
    Text(&'a str),
    /// A choice field's chosen option, if any.
    Choice(Option<u8>),
    /// Whether a checkbox is checked.
    Checkbox(bool),
}

impl<'a> Value<'a> {
    /// Whether nothing has been entered: empty text, no option chosen, or an
    /// unchecked box.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.is_empty(),
            Self::Choice(choice) => choice.is_none(),
            Self::Checkbox(checked) => !checked,
        }
    }

    /// The text, if this is a text field.
    #[must_use]
    pub fn as_text(&self) -> Option<&'a str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The chosen option, if this is a choice field with one chosen.
    #[must_use]
    pub fn as_choice(&self) -> Option<u8> {
        match self {
            Self::Choice(choice) => *choice,
            _ => None,
        }
    }

    /// Whether this is a checkbox, and it is checked.
    #[must_use]
    pub fn is_checked(&self) -> bool {
        matches!(self, Self::Checkbox(true))
    }
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "Text(<{} bytes>)", text.len()),
            Self::Choice(choice) => f.debug_tuple("Choice").field(choice).finish(),
            Self::Checkbox(checked) => f.debug_tuple("Checkbox").field(checked).finish(),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]
mod tests {
    use super::*;
    use crate::form::{Charset, TextRule};

    #[test]
    fn storage_matches_the_field_kind_and_starts_empty() {
        let text: Slot<8> = Slot::for_kind(FieldKind::Text(TextRule::new(Charset::Text, 0, 8)));
        assert_eq!(text.view(), Some(Value::Text("")));
        let choice: Slot<8> = Slot::for_kind(FieldKind::Choice { options: 2 });
        assert_eq!(choice.view(), Some(Value::Choice(None)));
        let tick: Slot<8> = Slot::for_kind(FieldKind::Checkbox);
        assert_eq!(tick.view(), Some(Value::Checkbox(false)));
        for slot in [&text, &choice, &tick] {
            assert!(slot.is_empty());
        }
        assert_eq!(Slot::<8>::default().view(), None);
    }

    #[test]
    fn clearing_empties_the_value_and_keeps_its_storage() {
        let mut slot: Slot<8> = Slot::Text(BoundedStr::try_from_str("secret").unwrap());
        assert!(!slot.is_empty());
        slot.clear();
        let Slot::Text(text) = &slot else {
            panic!("clearing changed the kind of storage");
        };
        assert!(text.is_zeroed(), "every byte zero, not merely empty");
        assert_eq!(text.capacity(), 8);

        let mut slot: Slot<8> = Slot::Choice(Some(1));
        slot.clear();
        assert_eq!(slot.view(), Some(Value::Choice(None)));
        let mut slot: Slot<8> = Slot::Checkbox(true);
        slot.clear();
        assert_eq!(slot.view(), Some(Value::Checkbox(false)));
    }

    #[test]
    fn whitespace_counts_as_entered() {
        let slot: Slot<8> = Slot::Text(BoundedStr::try_from_str("  ").unwrap());
        assert!(!slot.is_empty());
    }

    #[test]
    fn value_accessors() {
        assert_eq!(Value::Text("abc").as_text(), Some("abc"));
        assert_eq!(Value::Choice(Some(2)).as_text(), None);
        assert_eq!(Value::Choice(Some(2)).as_choice(), Some(2));
        assert_eq!(Value::Checkbox(true).as_choice(), None);
        assert!(Value::Checkbox(true).is_checked());
        assert!(!Value::Text("x").is_checked());
        assert!(Value::Text("").is_empty());
        assert!(!Value::Choice(Some(0)).is_empty());
    }

    #[test]
    fn debug_hides_text() {
        let shown = std::format!("{:?}", Value::Text("Sam Sample"));
        assert_eq!(shown, "Text(<10 bytes>)");
        assert_eq!(
            std::format!("{:?}", Value::Choice(Some(1))),
            "Choice(Some(1))"
        );
        assert_eq!(
            std::format!("{:?}", Value::Checkbox(true)),
            "Checkbox(true)"
        );
    }
}

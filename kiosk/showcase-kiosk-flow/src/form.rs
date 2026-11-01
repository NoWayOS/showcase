// SPDX-License-Identifier: MIT OR Apache-2.0
//! The form an application defines: its steps, their fields, and the rules
//! each field's value must follow.
//!
//! A form is plain static data. The flow stores only field ids and values;
//! labels, help text and layout belong to the UI, which keeps its own
//! mapping from these ids to what it draws.

use crate::error::{Issue, SetupError};

/// Identifies a field. Chosen by the application; unique within a [`Form`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId(pub u16);

/// Identifies a step. Chosen by the application; unique within a [`Form`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepId(pub u16);

/// The characters a text field accepts. Each is an allow-list: anything
/// not listed is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    /// One line of printable Latin-1 (ISO 8859-1) text, the character set
    /// the device's built-in fonts can draw today: the space, ASCII letters,
    /// digits and punctuation, and the accented letters and symbols from
    /// U+00A1 to U+00FF, such as é, ø, ß and £.
    ///
    /// Refused: control characters (tabs, line breaks, escape sequences),
    /// the no-break space and the soft hyphen (which pass for a space and
    /// for nothing), and every character outside Latin-1. That keeps out
    /// Unicode line and paragraph separators, direction overrides and
    /// isolates, zero-width and other invisible characters, and private-use
    /// characters, and so it keeps out a value that looks blank, splits a
    /// line downstream or shows differently from what was stored.
    ///
    /// Provisional: other scripts are added when the device can draw them
    /// and the supported languages are decided.
    Text,
    /// ASCII digits `0`–`9` only, for reference numbers and similar codes.
    Digits,
    /// ASCII letters and digits only, for booking codes and similar.
    Alphanumeric,
}

impl Charset {
    /// Whether `ch` may be entered.
    #[must_use]
    pub fn allows(self, ch: char) -> bool {
        match self {
            Self::Text => matches!(ch, ' '..='~' | '\u{a1}'..='\u{ac}' | '\u{ae}'..='\u{ff}'),
            Self::Digits => ch.is_ascii_digit(),
            Self::Alphanumeric => ch.is_ascii_alphanumeric(),
        }
    }
}

/// Length limits and allowed characters for a text field.
///
/// Lengths count **bytes** of UTF-8, as the storage does
/// ([`nowayos_bounded::BoundedStr`]). For ASCII a byte is a character; an
/// accented Latin letter takes two bytes, and characters from other
/// scripts take two to four.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRule {
    /// Which characters may be entered.
    pub charset: Charset,
    /// The shortest acceptable value, in bytes. Checked when the person moves
    /// on, not while they type. A blank optional field is always acceptable.
    pub min_len: u16,
    /// The longest acceptable value, in bytes. Longer input is refused as it
    /// is entered. At least 1, and no more than the flow's storage per value.
    pub max_len: u16,
}

impl TextRule {
    /// A rule accepting `charset`, from `min_len` to `max_len` bytes.
    #[must_use]
    pub const fn new(charset: Charset, min_len: u16, max_len: u16) -> Self {
        Self {
            charset,
            min_len,
            max_len,
        }
    }

    /// Why `text` cannot be entered, or `None` if it can.
    ///
    /// Over-long input is refused before any character is examined.
    #[must_use]
    pub fn entry_issue(&self, text: &str) -> Option<Issue> {
        if text.len() > usize::from(self.max_len) {
            return Some(Issue::TooLong);
        }
        if text.chars().any(|ch| !self.charset.allows(ch)) {
            return Some(Issue::BadCharacter);
        }
        None
    }

    /// Why a stored value does not complete its field, or `None` if it does.
    /// Blank means empty or only whitespace: a value with nothing visible
    /// in it (the charsets refuse every invisible character but the space).
    pub(crate) fn completion_issue(&self, value: &str, required: bool) -> Option<Issue> {
        if value.trim().is_empty() {
            return required.then_some(Issue::Missing);
        }
        if value.len() < usize::from(self.min_len) {
            return Some(Issue::TooShort);
        }
        None
    }
}

/// What kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// One line of text, checked against the rule as it is entered.
    Text(TextRule),
    /// One option out of several, numbered from 0. The flow stores only the
    /// number; the UI owns the labels.
    Choice {
        /// How many options there are; at least 1.
        options: u8,
    },
    /// A box that is checked or not. A required checkbox must be checked.
    Checkbox,
}

/// One field of a [`Step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The field's id, unique within the form.
    pub id: FieldId,
    /// What the field holds, and its rules.
    pub kind: FieldKind,
    /// Whether the person must complete it before moving on.
    pub required: bool,
}

impl Field {
    /// An optional text field.
    #[must_use]
    pub const fn text(id: FieldId, rule: TextRule) -> Self {
        Self {
            id,
            kind: FieldKind::Text(rule),
            required: false,
        }
    }

    /// An optional choice between `options` options.
    #[must_use]
    pub const fn choice(id: FieldId, options: u8) -> Self {
        Self {
            id,
            kind: FieldKind::Choice { options },
            required: false,
        }
    }

    /// An optional checkbox.
    #[must_use]
    pub const fn checkbox(id: FieldId) -> Self {
        Self {
            id,
            kind: FieldKind::Checkbox,
            required: false,
        }
    }

    /// The same field, made required.
    #[must_use]
    pub const fn required(self) -> Self {
        Self {
            required: true,
            ..self
        }
    }
}

/// One screen of the form: an id and the fields shown on it.
///
/// A step may have no fields at all: a notice the person reads before
/// continuing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step<'a> {
    /// The step's id, unique within the form.
    pub id: StepId,
    /// The fields on this step, in the order the UI shows them.
    pub fields: &'a [Field],
}

impl<'a> Step<'a> {
    /// A step with these fields.
    #[must_use]
    pub const fn new(id: StepId, fields: &'a [Field]) -> Self {
        Self { id, fields }
    }
}

/// A whole form: its steps, in the order the person meets them.
///
/// Usually a `static`. It is checked when a [`KioskFlow`](crate::KioskFlow)
/// is created, and can be checked in a unit test with [`Form::check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Form<'a> {
    /// The steps, first to last. There must be at least one.
    pub steps: &'a [Step<'a>],
}

impl<'a> Form<'a> {
    /// A form with these steps.
    #[must_use]
    pub const fn new(steps: &'a [Step<'a>]) -> Self {
        Self { steps }
    }

    /// Every field of every step, in order.
    pub fn fields(&self) -> impl Iterator<Item = &'a Field> {
        let steps: &'a [Step<'a>] = self.steps;
        steps.iter().flat_map(|step| step.fields.iter())
    }

    /// Every field with the index of its step, in order.
    pub(crate) fn fields_by_step(&self) -> impl Iterator<Item = (usize, &'a Field)> {
        let steps: &'a [Step<'a>] = self.steps;
        steps
            .iter()
            .enumerate()
            .flat_map(|(index, step)| step.fields.iter().map(move |field| (index, field)))
    }

    /// How many fields the whole form has.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields().count()
    }

    /// Checks that a flow holding up to `max_fields` values of up to
    /// `value_bytes` bytes each can run this form: at least one step, unique
    /// step and field ids, and field rules that allow some value and fit the
    /// storage.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn check(&self, max_fields: usize, value_bytes: usize) -> Result<(), SetupError> {
        if self.steps.is_empty() {
            return Err(SetupError::NoSteps);
        }
        for (index, step) in self.steps.iter().enumerate() {
            if self
                .steps
                .iter()
                .take(index)
                .any(|earlier| earlier.id == step.id)
            {
                return Err(SetupError::DuplicateStep(step.id));
            }
        }
        // Counting first bounds the duplicate search below to `max_fields`
        // fields, however long a form is passed in.
        for (index, field) in self.fields().enumerate() {
            if index >= max_fields {
                return Err(SetupError::TooManyFields {
                    capacity: max_fields,
                });
            }
            if self
                .fields()
                .take(index)
                .any(|earlier| earlier.id == field.id)
            {
                return Err(SetupError::DuplicateField(field.id));
            }
            match field.kind {
                FieldKind::Text(rule) => {
                    if rule.max_len == 0 || rule.min_len > rule.max_len {
                        return Err(SetupError::BadTextRule(field.id));
                    }
                    if usize::from(rule.max_len) > value_bytes {
                        return Err(SetupError::TextExceedsStorage {
                            field: field.id,
                            max_len: rule.max_len,
                            capacity: value_bytes,
                        });
                    }
                }
                FieldKind::Choice { options } => {
                    if options == 0 {
                        return Err(SetupError::NoOptions(field.id));
                    }
                }
                FieldKind::Checkbox => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]
mod tests {
    use super::*;

    const NAME: FieldId = FieldId(1);
    const CODE: FieldId = FieldId(2);
    const PICK: FieldId = FieldId(3);
    const TICK: FieldId = FieldId(4);

    #[test]
    fn charsets() {
        for ch in [
            'a', 'Z', '0', ' ', '~', '\'', '-', 'é', 'ø', 'ß', 'ÿ', '£', '¡',
        ] {
            assert!(Charset::Text.allows(ch), "{ch:?}");
        }
        for ch in [
            '\n',         // line feed
            '\t',         // tab
            '\r',         // carriage return
            '\0',         // null
            '\u{1b}',     // escape
            '\u{7f}',     // delete
            '\u{85}',     // C1 next line
            '\u{9f}',     // last C1 control
            '\u{a0}',     // no-break space
            '\u{ad}',     // soft hyphen
            '\u{2028}',   // line separator
            '\u{2029}',   // paragraph separator
            '\u{202a}',   // left-to-right embedding
            '\u{202e}',   // right-to-left override
            '\u{2066}',   // left-to-right isolate
            '\u{2069}',   // pop directional isolate
            '\u{200b}',   // zero-width space
            '\u{200f}',   // right-to-left mark
            '\u{2060}',   // word joiner
            '\u{feff}',   // byte order mark
            '\u{061c}',   // Arabic letter mark
            '\u{180e}',   // Mongolian vowel separator
            '\u{115f}',   // Hangul choseong filler
            '\u{3164}',   // Hangul filler
            '\u{ffa0}',   // halfwidth Hangul filler
            '\u{e000}',   // private use
            '\u{f8ff}',   // private use
            '\u{10fffd}', // private use, plane 16
            '\u{fffe}',   // noncharacter
            '\u{ffff}',   // noncharacter
            '\u{100}',    // Ā: outside Latin-1, for now
            '\u{20ac}',   // € is not in ISO 8859-1
        ] {
            assert!(!Charset::Text.allows(ch), "{ch:?}");
        }
        assert!(Charset::Digits.allows('7'));
        assert!(!Charset::Digits.allows('a'));
        assert!(!Charset::Digits.allows('٣')); // Arabic-Indic digit: not ASCII
        assert!(Charset::Alphanumeric.allows('Q'));
        assert!(Charset::Alphanumeric.allows('9'));
        assert!(!Charset::Alphanumeric.allows('-'));
        assert!(!Charset::Alphanumeric.allows('é'));
    }

    #[test]
    fn text_allows_exactly_printable_latin_1() {
        let allowed: std::vec::Vec<char> = (char::MIN..=char::MAX)
            .filter(|&ch| Charset::Text.allows(ch))
            .collect();
        // 95 printable ASCII characters, then U+00A1 to U+00FF less the
        // soft hyphen.
        assert_eq!(allowed.len(), 95 + 94);
        for ch in allowed {
            assert!(u32::from(ch) <= 0xff, "{ch:?}");
            assert!(!ch.is_control(), "{ch:?}");
            // The space is the only invisible character, and blank values
            // count as missing (`completion_issue`).
            assert!(ch == ' ' || !ch.is_whitespace(), "{ch:?}");
        }
    }

    #[test]
    fn entry_checks_length_before_characters() {
        let rule = TextRule::new(Charset::Digits, 2, 4);
        assert_eq!(rule.entry_issue("1234"), None);
        assert_eq!(rule.entry_issue(""), None);
        assert_eq!(rule.entry_issue("12a"), Some(Issue::BadCharacter));
        // Too long *and* bad characters: length is reported, as it is checked first.
        assert_eq!(rule.entry_issue("abcde"), Some(Issue::TooLong));
        // Length is in bytes: two 2-byte letters fill a 4-byte field.
        let text = TextRule::new(Charset::Text, 0, 4);
        assert_eq!(text.entry_issue("éé"), None);
        assert_eq!(text.entry_issue("ééa"), Some(Issue::TooLong));
    }

    #[test]
    fn completion_treats_blank_as_empty() {
        let rule = TextRule::new(Charset::Text, 3, 10);
        assert_eq!(rule.completion_issue("", true), Some(Issue::Missing));
        assert_eq!(rule.completion_issue("   ", true), Some(Issue::Missing));
        assert_eq!(rule.completion_issue("", false), None);
        assert_eq!(rule.completion_issue("  ", false), None);
        assert_eq!(rule.completion_issue("ab", false), Some(Issue::TooShort));
        assert_eq!(rule.completion_issue("ab", true), Some(Issue::TooShort));
        assert_eq!(rule.completion_issue("abc", true), None);
    }

    #[test]
    fn builders_set_kind_and_required() {
        let f = Field::choice(PICK, 3).required();
        assert_eq!(f.kind, FieldKind::Choice { options: 3 });
        assert!(f.required);
        assert!(!Field::checkbox(TICK).required);
    }

    #[test]
    fn a_good_form_passes_and_counts_its_fields() {
        let first = [
            Field::text(NAME, TextRule::new(Charset::Text, 1, 16)).required(),
            Field::text(CODE, TextRule::new(Charset::Alphanumeric, 6, 6)),
        ];
        let second = [Field::choice(PICK, 2), Field::checkbox(TICK).required()];
        let steps = [
            Step::new(StepId(1), &first),
            Step::new(StepId(2), &[]),
            Step::new(StepId(3), &second),
        ];
        let form = Form::new(&steps);
        assert_eq!(form.check(4, 16), Ok(()));
        assert_eq!(form.field_count(), 4);
        let ids: std::vec::Vec<u16> = form.fields().map(|f| f.id.0).collect();
        assert_eq!(ids, [1, 2, 3, 4]);
        let by_step: std::vec::Vec<usize> = form.fields_by_step().map(|(s, _)| s).collect();
        assert_eq!(by_step, [0, 0, 2, 2]);
        // One field too many for the capacity.
        assert_eq!(
            form.check(3, 16),
            Err(SetupError::TooManyFields { capacity: 3 })
        );
        // A text field longer than the storage.
        assert_eq!(
            form.check(4, 15),
            Err(SetupError::TextExceedsStorage {
                field: NAME,
                max_len: 16,
                capacity: 15
            })
        );
    }

    #[test]
    fn bad_forms_are_refused() {
        assert_eq!(Form::new(&[]).check(4, 16), Err(SetupError::NoSteps));

        let steps = [Step::new(StepId(1), &[]), Step::new(StepId(1), &[])];
        assert_eq!(
            Form::new(&steps).check(4, 16),
            Err(SetupError::DuplicateStep(StepId(1)))
        );

        let a = [Field::checkbox(TICK)];
        let b = [Field::choice(TICK, 2)];
        let steps = [Step::new(StepId(1), &a), Step::new(StepId(2), &b)];
        assert_eq!(
            Form::new(&steps).check(4, 16),
            Err(SetupError::DuplicateField(TICK))
        );

        for rule in [
            TextRule::new(Charset::Text, 0, 0),
            TextRule::new(Charset::Text, 5, 4),
        ] {
            let fields = [Field::text(NAME, rule)];
            let steps = [Step::new(StepId(1), &fields)];
            assert_eq!(
                Form::new(&steps).check(4, 16),
                Err(SetupError::BadTextRule(NAME))
            );
        }

        let fields = [Field::choice(PICK, 0)];
        let steps = [Step::new(StepId(1), &fields)];
        assert_eq!(
            Form::new(&steps).check(4, 16),
            Err(SetupError::NoOptions(PICK))
        );
    }

    #[test]
    fn checking_stops_at_capacity() {
        // However long the form, at most `max_fields` fields are examined:
        // identical fields are refused at the second one...
        let fields = [Field::checkbox(TICK); 1000];
        let steps = [Step::new(StepId(1), &fields)];
        assert_eq!(
            Form::new(&steps).check(8, 16),
            Err(SetupError::DuplicateField(TICK))
        );
        // ...and distinct ones on count, at the ninth.
        let distinct: std::vec::Vec<Field> =
            (0..1000u16).map(|i| Field::checkbox(FieldId(i))).collect();
        let steps = [Step::new(StepId(1), &distinct)];
        assert_eq!(
            Form::new(&steps).check(8, 16),
            Err(SetupError::TooManyFields { capacity: 8 })
        );
    }
}

use ruff_formatter::FormatContext;
use ruff_python_trivia::{SimpleToken, SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::TextRange;

use crate::MagicTrailingComma;
use crate::prelude::*;

/// Returns `true` if the range ends with a magic trailing comma (and the magic trailing comma
/// should be respected).
pub(crate) fn has_magic_trailing_comma(range: TextRange, context: &PyFormatContext) -> bool {
    match context.options().magic_trailing_comma() {
        MagicTrailingComma::Respect => has_trailing_comma(range, context),
        MagicTrailingComma::Ignore => false,
    }
}

/// Returns `true` if the range contains a line break and line joining is disabled.
/// This is used to preserve multi-line formatting in the source when the user
/// has opted out of line joining.
pub(crate) fn has_skip_line_joining_line_break(range: TextRange, context: &PyFormatContext) -> bool {
    if context.options().line_joining().is_enabled() {
        return false;
    }
    has_line_break(range, context)
}

/// Returns `true` if the range contains a line break (newline character).
pub(crate) fn has_line_break(range: TextRange, context: &PyFormatContext) -> bool {
    let source = &context.source()[range];
    source.contains(['\n', '\r'])
}

/// Returns `true` if the range ends with a trailing comma.
pub(crate) fn has_trailing_comma(range: TextRange, context: &PyFormatContext) -> bool {
    let first_token = SimpleTokenizer::new(context.source(), range)
        .skip_trivia()
        // Skip over any closing parentheses belonging to the expression
        .find(|token| token.kind() != SimpleTokenKind::RParen);

    matches!(
        first_token,
        Some(SimpleToken {
            kind: SimpleTokenKind::Comma,
            ..
        })
    )
}

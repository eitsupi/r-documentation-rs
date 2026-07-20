use super::Parser;
use crate::{diagnostic::ParseError, lexer, lexer::Token};
use std::ops::Range;

/// Minimum v1 budget for cumulative work done by literal-percent splices:
/// bytes re-lexed plus tokens shifted by each `Vec::splice`. The effective
/// budget is `max(input.len() * 8, MIN_RELEX_BUDGET)`.
const MIN_RELEX_BUDGET: usize = 64 * 1024;

impl<'a> Parser<'a> {
    pub(super) fn relex_comment_tail(&mut self, range: Range<usize>) -> bool {
        let tail_start = range.start + 1;
        let tail_input = &self.input[tail_start..range.end];
        let budget = self.input.len().saturating_mul(8).max(MIN_RELEX_BUDGET);
        // Always charge bytes re-lexed. Charge suffix shifts
        // only when the replacement length differs from one:
        // an equal-length replacement moves nothing.
        self.relex_work = self.relex_work.saturating_add(tail_input.len());
        if self.relex_work > budget {
            self.fatal_error = Some(ParseError::InputTooLarge);
            return false;
        }
        let tail: Vec<Token> = lexer::lex_with_line_start(
            tail_input,
            tail_start == 0 || self.input[tail_start - 1] == b'\n',
        )
        .into_iter()
        .map(|token| crate::lexer::Token {
            kind: token.kind,
            range: token.range.start + tail_start..token.range.end + tail_start,
        })
        .collect();
        if tail.len() != 1 {
            let shifted = self.tokens.len() - self.index - 1;
            self.relex_work = self.relex_work.saturating_add(shifted);
            if self.relex_work > budget {
                self.fatal_error = Some(ParseError::InputTooLarge);
                return false;
            }
        }
        if tail.len() == 1 {
            self.tokens[self.index] = tail.into_iter().next().unwrap();
        } else {
            self.tokens.splice(self.index..=self.index, tail);
        }
        true
    }
}

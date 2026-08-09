use super::frame::Mode;
use crate::unstable_rlike::{State, Unit};

#[derive(Debug, Clone, Default)]
pub(super) struct RLikeState {
    transition: State,
}

impl RLikeState {
    pub(super) fn is_normal(&self) -> bool {
        self.transition.is_normal()
    }

    pub(super) fn is_transient_opener(&self) -> bool {
        self.transition.is_transient_opener()
    }

    pub(super) fn is_ordinary_quote(&self) -> bool {
        self.transition.is_ordinary_quote()
    }

    pub(super) fn is_raw_string(&self) -> bool {
        self.transition.is_raw_string()
    }

    pub(super) fn is_comment(&self) -> bool {
        self.transition.is_comment()
    }

    pub(super) fn is_raw_delimiter(&self) -> bool {
        self.transition.is_raw_delimiter()
    }

    pub(super) fn is_active(&self) -> bool {
        !self.is_normal()
    }

    pub(super) fn clear_transient_opener(&mut self) {
        self.transition.clear_transient_opener();
    }

    pub(super) fn clear_raw_prefix(&mut self) {
        self.transition.clear_raw_prefix();
    }

    pub(super) fn append_to(&mut self, buf: &mut String, value: &str, mode: Mode) {
        if mode != Mode::RLike {
            buf.push_str(value);
            return;
        }
        buf.push_str(value);
        for byte in value.bytes() {
            self.transition.step(if byte.is_ascii() {
                Unit::Ascii(byte)
            } else {
                Unit::Other
            });
        }
    }
}

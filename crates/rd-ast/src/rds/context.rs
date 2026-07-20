use super::*;

pub(super) struct NodeContext<'a> {
    pub(super) tag: Option<&'a str>,
    pub(super) value: &'a RValue,
    pub(super) attributes: &'a Attributes,
}

pub(super) struct LowerContext {
    path: Vec<RdPathSegment>,
}

impl LowerContext {
    pub(super) fn new() -> Self {
        Self { path: Vec::new() }
    }

    pub(super) fn scoped<T>(
        &mut self,
        segment: RdPathSegment,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.path.push(segment);
        let result = f(self);
        self.path.pop();
        result
    }

    pub(super) fn location(&self, tag: Option<&str>, attribute: Option<&str>) -> LowerLocation {
        LowerLocation {
            path: RdPath::new(self.path.clone()),
            tag: tag.map(str::to_owned),
            attribute: attribute.map(str::to_owned),
        }
    }

    pub(super) fn invalid_string(
        &self,
        tag: Option<&str>,
        attribute: Option<&str>,
        value: &RStr,
    ) -> LowerError {
        LowerError::InvalidStringEncoding {
            location: self.location(tag, attribute),
            encoding: match value.encoding() {
                Some(rd_rds::REncoding::Native) => StringEncoding::Native,
                Some(rd_rds::REncoding::Utf8) => StringEncoding::Utf8,
                Some(rd_rds::REncoding::Latin1) => StringEncoding::Latin1,
                Some(rd_rds::REncoding::Bytes) => StringEncoding::Bytes,
                Some(_) => StringEncoding::Bytes,
                None => StringEncoding::Native,
            },
        }
    }
}

use crate::{ByteCursor, Error};

/// The serialized RDS stream header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub format_version: u32,
    pub writer_version: RVersion,
    pub minimum_reader_version: RVersion,
    pub native_encoding: Option<String>,
}

impl Header {
    pub fn parse(cursor: &mut ByteCursor<'_>) -> Result<Self, Error> {
        let marker_offset = cursor.position();
        let marker = cursor.read_exact(2)?;
        if marker != b"X\n" {
            return Err(Error::UnsupportedMarker {
                marker: [marker[0], marker[1]],
                offset: marker_offset,
            });
        }

        let version_offset = cursor.position();
        let format_version = cursor.read_be_u32()?;
        if format_version != 2 && format_version != 3 {
            return Err(Error::UnsupportedVersion {
                version: format_version,
                offset: version_offset,
            });
        }

        let writer_version = RVersion::from_raw(cursor.read_be_u32()?);
        let minimum_reader_version = RVersion::from_raw(cursor.read_be_u32()?);

        let native_encoding = if format_version == 3 {
            let byte_len = cursor.read_be_u32()? as usize;
            let encoding_offset = cursor.position();
            let encoding = cursor.read_exact(byte_len)?;
            Some(
                String::from_utf8(encoding.to_vec()).map_err(|_| Error::InvalidUtf8 {
                    offset: encoding_offset,
                })?,
            )
        } else {
            None
        };

        Ok(Self {
            format_version,
            writer_version,
            minimum_reader_version,
            native_encoding,
        })
    }
}

/// An R version encoded in the serializer's packed `major * 65536 + minor * 256 + patch` form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RVersion {
    raw: u32,
}

impl RVersion {
    pub fn from_raw(raw: u32) -> Self {
        Self { raw }
    }

    pub fn raw(self) -> u32 {
        self.raw
    }

    pub fn components(self) -> (u32, u32, u32) {
        (self.raw >> 16, (self.raw >> 8) & 0xff, self.raw & 0xff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_input_returns_offset() {
        let mut cursor = ByteCursor::new(b"X\n");
        let err = Header::parse(&mut cursor).unwrap_err();
        assert!(matches!(
            err,
            Error::UnexpectedEof {
                offset: 2,
                needed: 4,
                remaining: 0
            }
        ));
    }

    #[test]
    fn wrong_marker_is_reported() {
        let mut cursor = ByteCursor::new(b"A\n\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00");
        let err = Header::parse(&mut cursor).unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedMarker {
                marker: [b'A', b'\n'],
                offset: 0
            }
        ));
    }

    #[test]
    fn unsupported_version_is_reported() {
        let mut cursor = ByteCursor::new(b"X\n\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x00");
        let err = Header::parse(&mut cursor).unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedVersion {
                version: 4,
                offset: 2
            }
        ));
    }
}

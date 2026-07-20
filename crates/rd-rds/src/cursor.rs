use crate::Error;

/// A read-only cursor over an in-memory byte slice.
#[derive(Debug, Clone, Copy)]
pub struct ByteCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> ByteCursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub fn read_u8(&mut self) -> Result<u8, Error> {
        Ok(self.read_exact(1)?[0])
    }

    pub fn read_be_u32(&mut self) -> Result<u32, Error> {
        let offset = self.position;
        let bytes = self.read_exact(4)?;
        let mut raw = [0u8; 4];
        raw.copy_from_slice(bytes);
        debug_assert_eq!(self.position, offset + 4);
        Ok(u32::from_be_bytes(raw))
    }

    pub fn read_be_u64(&mut self) -> Result<u64, Error> {
        let offset = self.position;
        let bytes = self.read_exact(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        debug_assert_eq!(self.position, offset + 8);
        Ok(u64::from_be_bytes(raw))
    }

    pub fn read_be_i32(&mut self) -> Result<i32, Error> {
        let offset = self.position;
        let bytes = self.read_exact(4)?;
        let mut raw = [0u8; 4];
        raw.copy_from_slice(bytes);
        debug_assert_eq!(self.position, offset + 4);
        Ok(i32::from_be_bytes(raw))
    }

    pub fn read_exact(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let offset = self.position;
        let end = offset.checked_add(len).ok_or(Error::UnexpectedEof {
            offset,
            needed: len,
            remaining: self.remaining(),
        })?;
        if end > self.bytes.len() {
            return Err(Error::UnexpectedEof {
                offset,
                needed: len,
                remaining: self.remaining(),
            });
        }
        self.position = end;
        Ok(&self.bytes[offset..end])
    }
}

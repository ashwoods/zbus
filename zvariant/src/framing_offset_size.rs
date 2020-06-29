use crate::{Error, Result};
use byteorder::{ByteOrder, WriteBytesExt, LE};
use std::collections::VecDeque;

// Used internally for GVariant encoding and decoding.
//
// GVariant containers keeps framing offsets at the end and size of these offsets is dependent on
// the size of the container (which includes offsets themselves.

pub(crate) struct FramingOffsets(VecDeque<usize>);

impl FramingOffsets {
    pub fn new() -> Self {
        // FIXME: Set some good default capacity
        Self(VecDeque::new())
    }

    pub fn from_encoded_array(container: &[u8]) -> Self {
        let offset_size = FramingOffsetSize::for_encoded_container(container.len());

        // The last offset tells us the start of offsets.
        let mut i = offset_size.read_last_offset_from_buffer(container);
        let slice_len = offset_size as usize;
        let mut offsets = Self::new();
        while i < container.len() {
            let end = i + slice_len;
            let offset = offset_size.read_last_offset_from_buffer(&container[i..end]);
            offsets.push(offset);

            i += slice_len;
        }

        offsets
    }

    pub fn push(&mut self, offset: usize) {
        self.0.push_back(offset);
    }

    pub fn write_all<W>(self, writer: &mut W, container_len: usize) -> Result<()>
    where
        W: std::io::Write,
    {
        let offset_size = FramingOffsetSize::for_bare_container(container_len, self.0.len());

        for offset in self.0 {
            offset_size.write_offset(writer, offset)?;
        }

        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn pop(&mut self) -> Option<usize> {
        self.0.pop_front()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(usize)]
pub(crate) enum FramingOffsetSize {
    U8 = 1,
    U16 = 2,
    U32 = 4,
    U64 = 8,
    U128 = 16,
}

impl FramingOffsetSize {
    pub(crate) fn for_bare_container(container_len: usize, num_offsets: usize) -> Self {
        let mut offset_size = FramingOffsetSize::U8;

        loop {
            if container_len + num_offsets * (offset_size as usize) <= offset_size.max() {
                return offset_size;
            }

            offset_size = offset_size
                .bump_up()
                .expect("Can't handle container too large for a 128-bit pointer");
        }
    }

    pub(crate) fn for_encoded_container(container_len: usize) -> Self {
        Self::for_bare_container(container_len, 0)
    }

    pub(crate) fn write_offset<W>(self, writer: &mut W, offset: usize) -> Result<()>
    where
        W: std::io::Write,
    {
        match self {
            FramingOffsetSize::U8 => writer.write_u8(offset as u8),
            FramingOffsetSize::U16 => writer.write_u16::<LE>(offset as u16),
            FramingOffsetSize::U32 => writer.write_u32::<LE>(offset as u32),
            FramingOffsetSize::U64 => writer.write_u64::<LE>(offset as u64),
            FramingOffsetSize::U128 => writer.write_u128::<LE>(offset as u128),
        }
        .map_err(Error::Io)
    }

    pub fn read_last_offset_from_buffer(self, buffer: &[u8]) -> usize {
        let end = buffer.len();
        match self {
            FramingOffsetSize::U8 => buffer[end - 1] as usize,
            FramingOffsetSize::U16 => LE::read_u16(&buffer[end - 2..end]) as usize,
            FramingOffsetSize::U32 => LE::read_u32(&buffer[end - 4..end]) as usize,
            FramingOffsetSize::U64 => LE::read_u64(&buffer[end - 8..end]) as usize,
            FramingOffsetSize::U128 => LE::read_u128(&buffer[end - 16..end]) as usize,
        }
    }

    fn max(self) -> usize {
        match self {
            FramingOffsetSize::U8 => u8::MAX as usize,
            FramingOffsetSize::U16 => u16::MAX as usize,
            FramingOffsetSize::U32 => u32::MAX as usize,
            FramingOffsetSize::U64 => u64::MAX as usize,
            FramingOffsetSize::U128 => u128::MAX as usize,
        }
    }

    fn bump_up(self) -> Option<Self> {
        match self {
            FramingOffsetSize::U8 => Some(FramingOffsetSize::U16),
            FramingOffsetSize::U16 => Some(FramingOffsetSize::U32),
            FramingOffsetSize::U32 => Some(FramingOffsetSize::U64),
            FramingOffsetSize::U64 => Some(FramingOffsetSize::U128),
            FramingOffsetSize::U128 => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::framing_offset_size::FramingOffsetSize;

    #[test]
    fn framing_offset_size_bump() {
        assert_eq!(
            FramingOffsetSize::for_bare_container(u8::MAX as usize - 3, 3),
            FramingOffsetSize::U8
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(u8::MAX as usize - 1, 2),
            FramingOffsetSize::U16
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(u16::MAX as usize - 4, 2),
            FramingOffsetSize::U16
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(u16::MAX as usize - 3, 2),
            FramingOffsetSize::U32
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(u32::MAX as usize - 12, 3),
            FramingOffsetSize::U32
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(u32::MAX as usize - 11, 3),
            FramingOffsetSize::U64
        );
    }
}

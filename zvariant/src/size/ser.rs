use serde::{ser, ser::SerializeSeq, Serialize};
use std::os::unix::io::RawFd;
use std::{marker::PhantomData, str};

use crate::error::{Error, Result};
use crate::r#type::Type;
use crate::signature_parser::SignatureParser;
use crate::utils::*;
use crate::{basic::Basic, encoding_context::EncodingContext};
use crate::{object_path::ObjectPath, signature::Signature};

/// Calculate the serialized size of `T`.
///
/// # Panics
///
/// This function will panic if the value to serialize contains file descriptors. Use
/// [`serialized_size_fds`] if `T` (potentially) contains FDs.
///
/// # Examples
///
/// ```
/// use zvariant::{EncodingContext, serialized_size};
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let len = serialized_size(ctxt, "hello world").unwrap();
/// assert_eq!(len, 16);
///
/// let len = serialized_size(ctxt, &("hello world!", 42_u64)).unwrap();
/// assert_eq!(len, 32);
/// ```
///
/// [`serialized_size_fds`]: fn.serialized_size_fds.html
pub fn serialized_size<B, T: ?Sized>(ctxt: EncodingContext<B>, value: &T) -> Result<usize>
where
    B: byteorder::ByteOrder,
    T: Serialize + Type,
{
    let (len, num_fds) = serialized_size_fds(ctxt, value)?;
    if num_fds != 0 {
        panic!("can't serialize with FDs")
    }

    Ok(len)
}

/// Calculate the serialized size of `T` that (potentially) contains FDs.
///
/// Returns the serialized size of `T` and the number of FDs.
pub fn serialized_size_fds<B, T: ?Sized>(
    ctxt: EncodingContext<B>,
    value: &T,
) -> Result<(usize, usize)>
where
    B: byteorder::ByteOrder,
    T: Serialize + Type,
{
    let signature = T::signature();

    let mut fds = vec![];
    let mut serializer = Serializer::<B>::new(&signature, &mut fds, ctxt);
    value.serialize(&mut serializer)?;

    Ok((serializer.size, fds.len()))
}

/// Our size serializer implementation.
struct Serializer<'ser, 'sig, B> {
    pub(self) ctxt: EncodingContext<B>,
    pub(self) size: usize,
    pub(self) fds: &'ser mut Vec<RawFd>,

    pub(self) sign_parser: SignatureParser<'sig>,

    pub(self) value_sign: Option<Signature<'static>>,

    b: PhantomData<B>,
}

impl<'ser, 'sig, B> Serializer<'ser, 'sig, B>
where
    B: byteorder::ByteOrder,
{
    /// Create a Serializer struct instance.
    pub(self) fn new<'f: 'ser>(
        signature: &Signature<'sig>,
        fds: &'f mut Vec<RawFd>,
        ctxt: EncodingContext<B>,
    ) -> Self {
        let sign_parser = SignatureParser::new(signature.clone());

        Self {
            ctxt,
            sign_parser,
            fds,
            size: 0,
            value_sign: None,
            b: PhantomData,
        }
    }

    fn add_fd(&mut self, fd: RawFd) -> u32 {
        if let Some(idx) = self.fds.iter().position(|&x| x == fd) {
            return idx as u32;
        }
        let idx = self.fds.len();
        self.fds.push(fd);

        idx as u32
    }

    fn add_padding(&mut self, alignment: usize) -> usize {
        let padding = padding_for_n_bytes(self.abs_pos(), alignment);
        self.size += padding;

        padding
    }

    fn serialize_basic<T>(&mut self) -> Result<()>
    where
        T: Basic,
    {
        self.sign_parser.parse_char(Some(T::SIGNATURE_CHAR))?;
        self.add_padding(T::ALIGNMENT);
        self.size += T::ALIGNMENT;

        Ok(())
    }

    fn prep_serialize_enum_variant(&mut self, variant_index: u32) -> Result<()> {
        // Encode enum variants as a struct with first field as variant index
        self.serialize_basic::<u32>()?;

        Ok(())
    }

    fn abs_pos(&self) -> usize {
        self.ctxt.position() + self.size
    }
}

impl<'ser, 'sig, 'b, B> ser::Serializer for &'b mut Serializer<'ser, 'sig, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'ser, 'sig, 'b, B>;
    type SerializeTuple = StructSerializer<'ser, 'sig, 'b, B>;
    type SerializeTupleStruct = StructSerializer<'ser, 'sig, 'b, B>;
    type SerializeTupleVariant = StructSerializer<'ser, 'sig, 'b, B>;
    type SerializeMap = SeqSerializer<'ser, 'sig, 'b, B>;
    type SerializeStruct = StructSerializer<'ser, 'sig, 'b, B>;
    type SerializeStructVariant = StructSerializer<'ser, 'sig, 'b, B>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_basic::<bool>()
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        // No i8 type in D-Bus/GVariant, let's pretend it's i16
        self.serialize_basic::<i16>()
    }

    // TODO: Use macro to avoid code-duplication here
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_basic::<i16>()
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        match self.sign_parser.next_char()? {
            'h' => {
                self.sign_parser.parse_char(None)?;
                self.add_padding(u32::ALIGNMENT);
                let v = self.add_fd(v);
                self.size += u32::ALIGNMENT;

                Ok(())
            }
            _ => self.serialize_basic::<i32>(),
        }
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.serialize_basic::<i64>()
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_basic::<u8>()
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_basic::<u16>()
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_basic::<u32>()
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.serialize_basic::<u64>()
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        // No f32 type in D-Bus/GVariant, let's pretend it's f64
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.serialize_basic::<f64>()
    }

    fn serialize_char(self, v: char) -> Result<()> {
        // No char type in D-Bus/GVariant, let's pretend it's a string
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        let c = self.sign_parser.next_char()?;

        match c {
            ObjectPath::SIGNATURE_CHAR | <&str>::SIGNATURE_CHAR => {
                self.add_padding(<&str>::ALIGNMENT);
                self.size += u32::ALIGNMENT;
            }
            Signature::SIGNATURE_CHAR | VARIANT_SIGNATURE_CHAR => {
                self.size += 1;

                if c == VARIANT_SIGNATURE_CHAR {
                    self.value_sign = Some(signature_string!(v));
                }
            }
            _ => {
                let expected = format!(
                    "`{}`, `{}`, `{}` or `{}`",
                    <&str>::SIGNATURE_STR,
                    Signature::SIGNATURE_STR,
                    ObjectPath::SIGNATURE_STR,
                    VARIANT_SIGNATURE_CHAR,
                );
                return Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Char(c),
                    &expected.as_str(),
                ));
            }
        }
        self.sign_parser.parse_char(None)?;
        // String and null byte
        self.size += v.len() + 1;

        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for byte in v {
            seq.serialize_element(byte)?;
        }

        seq.end()
    }

    fn serialize_none(self) -> Result<()> {
        // FIXME: Corresponds to GVariant's `Maybe` type, which is empty (no bytes) for None.
        todo!();
    }

    fn serialize_some<T>(self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // FIXME: Corresponds to GVariant's `Maybe` type.
        todo!();
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        variant_index.serialize(self)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)?;

        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.prep_serialize_enum_variant(variant_index)?;

        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.sign_parser.parse_char(Some(ARRAY_SIGNATURE_CHAR))?;
        self.add_padding(ARRAY_ALIGNMENT);
        self.size += u32::ALIGNMENT;

        let next_signature_char = self.sign_parser.next_char()?;
        let alignment = alignment_for_signature_char(next_signature_char, self.ctxt.format());
        // D-Bus expects us to add padding for the first element even when there is no first
        // element (i-e empty array) so we add padding already.
        let start = self.size + self.add_padding(alignment);
        let element_signature_pos = self.sign_parser.pos();
        let rest_of_signature =
            Signature::from_str_unchecked(&self.sign_parser.signature()[element_signature_pos..]);
        let element_signature = slice_signature(&rest_of_signature)?;
        let element_signature_len = element_signature.len();

        Ok(SeqSerializer {
            serializer: self,
            start,
            element_signature_len,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_struct("", len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.prep_serialize_enum_variant(variant_index)?;

        self.serialize_struct(name, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_seq(len)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        let c = self.sign_parser.next_char()?;
        let end_parens;
        if c == VARIANT_SIGNATURE_CHAR {
            end_parens = None;
        } else {
            self.sign_parser.parse_char(Some(c))?;
            self.add_padding(STRUCT_ALIGNMENT);

            if c == STRUCT_SIG_START_CHAR {
                end_parens = Some(STRUCT_SIG_END_CHAR);
            } else if c == DICT_ENTRY_SIG_START_CHAR {
                end_parens = Some(DICT_ENTRY_SIG_END_CHAR);
            } else {
                let expected = format!(
                    "`{}`, `{}`, `{}` or `{}`",
                    <&str>::SIGNATURE_STR,
                    Signature::SIGNATURE_STR,
                    ObjectPath::SIGNATURE_STR,
                    VARIANT_SIGNATURE_CHAR,
                );
                return Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Char(c),
                    &expected.as_str(),
                ));
            }
        }

        Ok(StructSerializer {
            serializer: self,
            end_parens,
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.prep_serialize_enum_variant(variant_index)?;

        self.serialize_struct(name, len)
    }
}

struct SeqSerializer<'ser, 'sig, 'b, B> {
    serializer: &'b mut Serializer<'ser, 'sig, B>,
    start: usize,
    // where value signature starts
    element_signature_len: usize,
}

impl<'ser, 'sig, 'b, B> SeqSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    pub(self) fn end_seq(self) -> Result<()> {
        if self.start == self.serializer.size {
            // Empty sequence so we need to parse the element signature.
            self.serializer
                .sign_parser
                .skip_chars(self.element_signature_len)?;
        }

        Ok(())
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeSeq for SeqSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.start != self.serializer.size {
            // The signature needs to be rewinded before encoding each element.
            self.serializer
                .sign_parser
                .rewind_chars(self.element_signature_len);
        }
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<()> {
        self.end_seq()
    }
}

struct StructSerializer<'ser, 'sig, 'b, B> {
    serializer: &'b mut Serializer<'ser, 'sig, B>,
    end_parens: Option<char>,
}

impl<'ser, 'sig, 'b, B> StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    fn serialize_struct_element<T>(&mut self, name: Option<&'static str>, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match name {
            Some("zvariant::Value::Value") => {
                // Serializing the value of a Value, which means signature was serialized
                // already, and also put aside for us to be picked here.
                let signature = self
                    .serializer
                    .value_sign
                    .take()
                    .expect("Incorrect Value encoding");

                let sign_parser = SignatureParser::new(signature);
                let mut serializer = Serializer::<B> {
                    ctxt: self.serializer.ctxt,
                    sign_parser,
                    fds: self.serializer.fds,
                    size: self.serializer.size,
                    value_sign: None,
                    b: PhantomData,
                };
                value.serialize(&mut serializer)?;
                self.serializer.size = serializer.size;

                Ok(())
            }
            _ => value.serialize(&mut *self.serializer),
        }
    }

    fn end_struct(self) -> Result<()> {
        if let Some(c) = self.end_parens {
            self.serializer.sign_parser.parse_char(Some(c))?;
        }

        Ok(())
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeTuple for StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_struct_element(None, value)
    }

    fn end(self) -> Result<()> {
        self.end_struct()
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeTupleStruct for StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_struct_element(None, value)
    }

    fn end(self) -> Result<()> {
        self.end_struct()
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeTupleVariant for StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_struct_element(None, value)
    }

    fn end(self) -> Result<()> {
        self.end_struct()
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeMap for SeqSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    // TODO: The Serde data model allows map keys to be any serializable type. We can only support keys of
    // basic types so the implementation below will produce invalid encoding if the key serializes
    // is something other than a basic type.
    //
    // We need to validate that map keys are of basic type. We do this by using a different Serializer
    // to serialize the key (instead of `&mut **self`) and having that other serializer only implement
    // `serialize_*` for basic types and return an error on any other data type.
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.start == self.serializer.size {
            // First key
            self.serializer
                .sign_parser
                .parse_char(Some(DICT_ENTRY_SIG_START_CHAR))?;
        } else {
            // The signature needs to be rewinded before encoding each element.
            self.serializer
                .sign_parser
                .rewind_chars(self.element_signature_len - 2);
        }
        self.serializer.add_padding(DICT_ENTRY_ALIGNMENT);

        key.serialize(&mut *self.serializer)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<()> {
        if self.start != self.serializer.size {
            // Non-empty map, take }
            self.serializer
                .sign_parser
                .parse_char(Some(DICT_ENTRY_SIG_END_CHAR))?;
        }
        self.end_seq()
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeStruct for StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_struct_element(Some(key), value)
    }

    fn end(self) -> Result<()> {
        self.end_struct()
    }
}

impl<'ser, 'sig, 'b, B> ser::SerializeStructVariant for StructSerializer<'ser, 'sig, 'b, B>
where
    B: byteorder::ByteOrder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_struct_element(Some(key), value)
    }

    fn end(self) -> Result<()> {
        self.end_struct()
    }
}

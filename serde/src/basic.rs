use crate::{Signature, VariantValue};

pub trait Basic: Clone {
    const SIGNATURE_CHAR: char;
    const SIGNATURE_STR: &'static str;
    const ALIGNMENT: usize;
}

impl<B: ?Sized> VariantValue for B
where
    B: Basic,
{
    fn signature() -> Signature<'static> {
        B::SIGNATURE_STR.into()
    }
}

impl Basic for u8 {
    const SIGNATURE_CHAR: char = 'y';
    const SIGNATURE_STR: &'static str = "y";
    const ALIGNMENT: usize = 1;
}

// No i8 type in D-Bus/GVariant, let's pretend it's i16
impl Basic for i8 {
    const SIGNATURE_CHAR: char = i16::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = i16::SIGNATURE_STR;
    const ALIGNMENT: usize = i16::ALIGNMENT;
}

impl Basic for bool {
    const SIGNATURE_CHAR: char = 'b';
    const SIGNATURE_STR: &'static str = "b";
    const ALIGNMENT: usize = 4;
}

impl Basic for i16 {
    const SIGNATURE_CHAR: char = 'n';
    const SIGNATURE_STR: &'static str = "n";
    const ALIGNMENT: usize = 2;
}

impl Basic for u16 {
    const SIGNATURE_CHAR: char = 'q';
    const SIGNATURE_STR: &'static str = "q";
    const ALIGNMENT: usize = 2;
}

impl Basic for i32 {
    const SIGNATURE_CHAR: char = 'i';
    const SIGNATURE_STR: &'static str = "i";
    const ALIGNMENT: usize = 4;
}

impl Basic for u32 {
    const SIGNATURE_CHAR: char = 'u';
    const SIGNATURE_STR: &'static str = "u";
    const ALIGNMENT: usize = 4;
}

impl Basic for i64 {
    const SIGNATURE_CHAR: char = 'x';
    const SIGNATURE_STR: &'static str = "x";
    const ALIGNMENT: usize = 8;
}

impl Basic for u64 {
    const SIGNATURE_CHAR: char = 't';
    const SIGNATURE_STR: &'static str = "t";
    const ALIGNMENT: usize = 8;
}

// No f32 type in D-Bus/GVariant, let's pretend it's f64
impl Basic for f32 {
    const SIGNATURE_CHAR: char = f64::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = f64::SIGNATURE_STR;
    const ALIGNMENT: usize = f64::ALIGNMENT;
}

impl Basic for f64 {
    const SIGNATURE_CHAR: char = 'd';
    const SIGNATURE_STR: &'static str = "d";
    const ALIGNMENT: usize = 8;
}

impl Basic for &str {
    const SIGNATURE_CHAR: char = 's';
    const SIGNATURE_STR: &'static str = "s";
    const ALIGNMENT: usize = 4;
}

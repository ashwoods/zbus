//! This crate provides API for serialization/deserialization of data to/from [D-Bus] wire format.
//! This binary wire format is simple and very efficient and hence useful outside of D-Bus context
//! as well. A slightly modified form of this format, [GVariant] is also very common and will be
//! supported by a future version of this crate.
//!
//! Since version 2.0, the API is [serde]-based and hence you'll find it very intuitive if you're
//! already familiar with serde. If you're not familiar with serde, you may want to first read its
//! [tutorial] before learning further about this crate.
//!
//! Serialization and deserialization is achieved through the [toplevel functions]:
//!
//! ```rust
//! use std::collections::HashMap;
//! use byteorder::LE;
//! use zvariant::{from_slice, to_bytes};
//! use zvariant::EncodingContext as Context;
//!
//! // All serialization and deserialization API, needs a context.
//! let ctxt = Context::<LE>::new_dbus(0);
//!
//! // i16
//! let encoded = to_bytes(ctxt, &42i16).unwrap();
//! let decoded: i16 = from_slice(&encoded, ctxt).unwrap();
//! assert_eq!(decoded, 42);
//!
//! // strings
//! let encoded = to_bytes(ctxt, &"hello").unwrap();
//! let decoded: &str = from_slice(&encoded, ctxt).unwrap();
//! assert_eq!(decoded, "hello");
//!
//! // tuples
//! let t = ("hello", 42i32, true);
//! let encoded = to_bytes(ctxt, &t).unwrap();
//! let decoded: (&str, i32, bool) = from_slice(&encoded, ctxt).unwrap();
//! assert_eq!(decoded, t);
//!
//! // Vec
//! let v = vec!["hello", "world!"];
//! let encoded = to_bytes(ctxt, &v).unwrap();
//! let decoded: Vec<&str> = from_slice(&encoded, ctxt).unwrap();
//! assert_eq!(decoded, v);
//!
//! // Dictionary
//! let mut map: HashMap<i64, &str> = HashMap::new();
//! map.insert(1, "123");
//! map.insert(2, "456");
//! let encoded = to_bytes(ctxt, &map).unwrap();
//! let decoded: HashMap<i64, &str> = from_slice(&encoded, ctxt).unwrap();
//! assert_eq!(decoded[&1], "123");
//! assert_eq!(decoded[&2], "456");
//! ```
//!
//! Apart from the obvious requirement of [`EncodingContext`] instance by the main serialization and
//! deserialization API, the type being serialized or deserialized must also implement `Type`
//! trait in addition to [`Serialize`] or [`Deserialize`], respectively. Please refer to [`Type`
//! module documentation] for more details.
//!
//! Most of the [basic types] of D-Bus match 1-1 wit all the primitive Rust types. The only two
//! exceptions being, [`Signature`] and [`ObjectPath`], which are really just strings. These types
//! are covered by the [`Basic`] trait.
//!
//! Similarly, most of the [container types] also map nicely to the usual Rust types and
//! collections (as can be seen in the example code above). The only note worthy exception being
//! ARRAY type. As arrays in Rust are fixed-sized, serde treats them as tuples and so does this
//! crate. This means they are encoded as STRUCT type of D-Bus. If you need to serialize to, or
//! deserialize from a D-Bus array, you'll need to use a [slice] (array can easily be converted to a
//! slice), a [`Vec`] or an [`arrayvec::ArrayVec`].
//!
//! The generic D-Bus type, `VARIANT` is represented by `Value`, an enum that holds exactly one
//! value of any of the other types. Please refer to [`Value` module documentation] for examples.
//!
//! # no-std
//!
//! While `std` is currently a hard requirement, optional `no-std` support is planned in the future.
//! On the other hand, `noalloc` support is not planned as it will be an extremely difficult to
//! accomplish, if at all possible.
//!
//! # Optional features
//!
//! | Feature | Description |
//! | ---     | ----------- |
//! | arrayvec | Implement `Type` for [`arrayvec::ArrayVec`] and [`arrayvec::ArrayString`] |
//! | enumflags2 | Implement `Type` for [`enumflags2::BitFlags<F>`] |
//!
//! # Portability
//!
//! zvariant is currently Unix-only and will fail to build on non-unix. This is hopefully a
//! temporary limitation.
//!
//! [D-Bus]: https://dbus.freedesktop.org/doc/dbus-specification.html
//! [GVariant]: https://developer.gnome.org/glib/stable/glib-GVariant.html
//! [serde]: https://crates.io/crates/serde
//! [tutorial]: https://serde.rs/
//! [toplevel functions]: #functions
//! [`EncodingContext`]: struct.EncodingContext.html
//! [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
//! [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
//! [`Type` module documentation]: trait.Type.html
//! [basic types]: https://dbus.freedesktop.org/doc/dbus-specification.html#basic-types
//! [`Signature`]: struct.Signature.html
//! [`ObjectPath`]: struct.ObjectPath.html
//! [`Basic`]: trait.Basic.html
//! [container types]: https://dbus.freedesktop.org/doc/dbus-specification.html#container-types
//! [slice]: https://doc.rust-lang.org/std/primitive.slice.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`arrayvec::ArrayVec`]: https://docs.rs/arrayvec/0.5.1/arrayvec/struct.ArrayVec.html
//! [`Value` module documentation]: enum.Value.html
#![feature(test)]
extern crate test;

#[macro_use]
mod utils;
pub use utils::*;

mod array;
pub use array::*;

mod basic;
pub use basic::*;

mod dict;
pub use dict::*;

mod encoding_context;
pub use encoding_context::*;

mod fd;
pub use fd::*;

mod object_path;
pub use crate::object_path::*;

mod ser;
pub use ser::*;

mod de;
pub use de::*;

mod signature;
pub use crate::signature::*;

mod str;
pub use crate::str::*;

mod structure;
pub use crate::structure::*;

mod value;
pub use value::*;

mod error;
pub use error::*;

mod r#type;
pub use r#type::*;

mod from_value;
pub use from_value::*;

mod into_value;
pub use into_value::*;

mod owned_value;
pub use owned_value::*;

mod signature_parser;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::convert::{TryFrom, TryInto};

    #[cfg(feature = "arrayvec")]
    use arrayvec::{ArrayString, ArrayVec};
    use byteorder::{self, ByteOrder, BE, LE};
    #[cfg(feature = "arrayvec")]
    use std::str::FromStr;

    use zvariant_derive::Type;

    use crate::{from_slice, from_slice_fds, from_slice_for_signature};
    use crate::{to_bytes, to_bytes_fds, to_bytes_for_signature};

    use crate::{Array, Dict, EncodingContext as Context};
    use crate::{Fd, ObjectPath, Signature, Str, Structure};
    use crate::{Type, Value};

    // Test through both generic and specific API (wrt byte order)
    macro_rules! basic_type_test {
        ($trait:ty, $test_value:expr, $expected_len:expr, $expected_ty:ty) => {{
            let ctxt = Context::<$trait>::new_dbus(0);
            let (encoded, fds) = to_bytes_fds(ctxt, &$test_value).unwrap();
            assert_eq!(
                encoded.len(),
                $expected_len,
                "invalid encoding using `to_bytes`"
            );
            let decoded: $expected_ty = from_slice_fds(&encoded, Some(&fds), ctxt).unwrap();
            assert!(
                decoded == $test_value,
                "invalid decoding using `from_slice`"
            );

            encoded
        }};
    }

    #[test]
    fn u8_value() {
        basic_type_test!(LE, 77_u8, 1, u8);

        // As Value
        let v: Value = 77_u8.into();
        assert_eq!(v.value_signature(), "y");
        assert_eq!(v, Value::U8(77));
        basic_type_test!(LE, v, 4, Value);

        let v: u8 = v.try_into().unwrap();
        assert_eq!(v, 77_u8);
    }

    #[test]
    fn fd_value() {
        basic_type_test!(LE, Fd::from(42), 4, Fd);

        // As Value
        let v: Value = Fd::from(42).into();
        assert_eq!(v.value_signature(), "h");
        assert_eq!(v, Value::Fd(Fd::from(42)));
        basic_type_test!(LE, v, 8, Value);
    }

    #[test]
    fn u16_value() {
        basic_type_test!(BE, 0xABBA_u16, 2, u16);

        // As Value
        let v: Value = 0xFEFE_u16.into();
        assert_eq!(v.value_signature(), "q");
        assert_eq!(v, Value::U16(0xFEFE));
        basic_type_test!(LE, v, 6, Value);

        let v: u16 = v.try_into().unwrap();
        assert_eq!(v, 0xFEFE_u16);
    }

    #[test]
    fn i16_value() {
        let encoded = basic_type_test!(BE, -0xAB0_i16, 2, i16);
        assert_eq!(LE::read_i16(&encoded), 0x50F5_i16);

        // As Value
        let v: Value = 0xAB_i16.into();
        assert_eq!(v.value_signature(), "n");
        assert_eq!(v, Value::I16(0xAB));
        basic_type_test!(LE, v, 6, Value);

        let v: i16 = v.try_into().unwrap();
        assert_eq!(v, 0xAB_i16);
    }

    #[test]
    fn u32_value() {
        basic_type_test!(BE, 0xABBA_ABBA_u32, 4, u32);

        // As Value
        let v: Value = 0xABBA_ABBA_u32.into();
        assert_eq!(v.value_signature(), "u");
        assert_eq!(v, Value::U32(0xABBA_ABBA));
        basic_type_test!(LE, v, 8, Value);

        let v: u32 = v.try_into().unwrap();
        assert_eq!(v, 0xABBA_ABBA_u32);
    }

    #[test]
    fn i32_value() {
        let encoded = basic_type_test!(BE, -0xABBA_AB0_i32, 4, i32);
        assert_eq!(LE::read_i32(&encoded), 0x5055_44F5_i32);

        // As Value
        let v: Value = 0xABBA_AB0_i32.into();
        assert_eq!(v.value_signature(), "i");
        assert_eq!(v, Value::I32(0xABBA_AB0));
        basic_type_test!(LE, v, 8, Value);

        let v: i32 = v.try_into().unwrap();
        assert_eq!(v, 0xABBA_AB0_i32);
    }

    // u64 is covered by `value_value` test below

    #[test]
    fn i64_value() {
        let encoded = basic_type_test!(BE, -0xABBA_ABBA_ABBA_AB0_i64, 8, i64);
        assert_eq!(LE::read_i64(&encoded), 0x5055_4455_4455_44F5_i64);

        // As Value
        let v: Value = 0xABBA_AB0i64.into();
        assert_eq!(v.value_signature(), "x");
        assert_eq!(v, Value::I64(0xABBA_AB0));
        basic_type_test!(LE, v, 16, Value);

        let v: i64 = v.try_into().unwrap();
        assert_eq!(v, 0xABBA_AB0_i64);
    }

    #[test]
    fn f64_value() {
        let encoded = basic_type_test!(BE, 99999.99999_f64, 8, f64);
        assert_eq!(LE::read_f64(&encoded), -5759340900185448e-143);

        // As Value
        let v: Value = 99999.99999_f64.into();
        assert_eq!(v.value_signature(), "d");
        assert_eq!(v, Value::F64(99999.99999));
        basic_type_test!(LE, v, 16, Value);

        let v: f64 = v.try_into().unwrap();
        assert_eq!(v, 99999.99999_f64);
    }

    #[test]
    fn str_value() {
        let string = String::from("hello world");
        basic_type_test!(LE, string, 16, String);
        basic_type_test!(LE, string, 16, &str);

        let string = "hello world";
        basic_type_test!(LE, string, 16, &str);
        basic_type_test!(LE, string, 16, String);

        // As Value
        let v: Value = string.into();
        assert_eq!(v.value_signature(), "s");
        assert_eq!(v, Value::new("hello world"));
        basic_type_test!(LE, v, 20, Value);

        let v: String = v.try_into().unwrap();
        assert_eq!(v, "hello world");
        let v: String = v.try_into().unwrap();
        assert_eq!(v, "hello world");

        // Characters are treated as strings
        basic_type_test!(LE, 'c', 6, char);

        // As Value
        let v: Value = "c".into();
        assert_eq!(v.value_signature(), "s");
        let ctxt = Context::new_dbus(0);
        let encoded = to_bytes::<LE, _>(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        assert_eq!(v, Value::new("c"));
    }

    #[cfg(feature = "arrayvec")]
    #[test]
    fn array_string_value() {
        let s = ArrayString::<[_; 32]>::from_str("hello world!").unwrap();
        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 17);
        let decoded: ArrayString<[_; 32]> = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(&decoded, "hello world!");
    }

    #[test]
    fn signature_value() {
        let sig = Signature::try_from("yys").unwrap();
        basic_type_test!(LE, sig, 5, Signature);

        // As Value
        let v: Value = sig.into();
        assert_eq!(v.value_signature(), "g");
        let encoded = basic_type_test!(LE, v, 8, Value);
        let ctxt = Context::new_dbus(0);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        assert_eq!(v, Value::Signature(Signature::try_from("yys").unwrap()));
    }

    #[test]
    fn object_path_value() {
        let o = ObjectPath::try_from("/hello/world").unwrap();
        basic_type_test!(LE, o, 17, ObjectPath);

        // As Value
        let v: Value = o.into();
        assert_eq!(v.value_signature(), "o");
        let encoded = basic_type_test!(LE, v, 21, Value);
        let ctxt = Context::new_dbus(0);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        assert_eq!(
            v,
            Value::ObjectPath(ObjectPath::try_from("/hello/world").unwrap())
        );
    }

    #[test]
    fn unit() {
        basic_type_test!(BE, (), 0, ());
    }

    #[test]
    fn array_value() {
        // Let's use D-Bus/GVariant terms

        //
        // Array of u8
        //
        // First a normal Rust array that is actually serialized as a struct (thank you Serde!)
        let ay = [77u8, 88];
        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 2);
        let decoded: [u8; 2] = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(&decoded, &[77u8, 88]);

        // Then rest of the tests just use ArrayVec or Vec
        #[cfg(feature = "arrayvec")]
        let ay = ArrayVec::from([77u8, 88]);
        #[cfg(not(feature = "arrayvec"))]
        let ay = vec![77u8, 88];
        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &ay).unwrap();
        assert_eq!(encoded.len(), 6);
        #[cfg(feature = "arrayvec")]
        let decoded: ArrayVec<[u8; 2]> = from_slice(&encoded, ctxt).unwrap();
        #[cfg(not(feature = "arrayvec"))]
        let decoded: Vec<u8> = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(&decoded.as_slice(), &[77u8, 88]);

        // As Value
        let v: Value = ay[..].into();
        assert_eq!(v.value_signature(), "ay");
        let encoded = to_bytes::<LE, _>(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "y");
            assert_eq!(array.len(), 2);
            assert_eq!(array.get()[0], Value::U8(77));
            assert_eq!(array.get()[1], Value::U8(88));
        } else {
            panic!();
        }

        // Now try as Vec
        let vec = ay.to_vec();
        let encoded = to_bytes::<LE, _>(ctxt, &vec).unwrap();
        assert_eq!(encoded.len(), 6);

        // Vec as Value
        let v: Value = Array::from(&vec).into();
        assert_eq!(v.value_signature(), "ay");
        let encoded = to_bytes::<LE, _>(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 10);

        // Emtpy array
        let at: Vec<u64> = vec![];
        let encoded = to_bytes::<LE, _>(ctxt, &at).unwrap();
        assert_eq!(encoded.len(), 8);

        // As Value
        let v: Value = at[..].into();
        assert_eq!(v.value_signature(), "at");
        let encoded = to_bytes::<LE, _>(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 8);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "t");
            assert_eq!(array.len(), 0);
        } else {
            panic!();
        }

        //
        // Array of strings
        //
        // Can't use 'as' as it's a keyword
        let as_ = vec!["Hello", "World", "Now", "Bye!"];
        let encoded = to_bytes::<LE, _>(ctxt, &as_).unwrap();
        assert_eq!(encoded.len(), 45);
        let decoded = from_slice::<LE, Vec<&str>>(&encoded, ctxt).unwrap();
        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0], "Hello");
        assert_eq!(decoded[1], "World");

        let decoded = from_slice::<LE, Vec<String>>(&encoded, ctxt).unwrap();
        assert_eq!(decoded.as_slice(), as_.as_slice());

        // Decode just the second string
        let ctxt = Context::<LE>::new_dbus(14);
        let decoded: &str = from_slice(&encoded[14..], ctxt).unwrap();
        assert_eq!(decoded, "World");
        let ctxt = Context::<LE>::new_dbus(0);

        // As Value
        let v: Value = as_[..].into();
        assert_eq!(v.value_signature(), "as");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 49);
        let v = from_slice(&encoded, ctxt).unwrap();
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "s");
            assert_eq!(array.len(), 4);
            assert_eq!(array.get()[0], Value::new("Hello"));
            assert_eq!(array.get()[1], Value::new("World"));
        } else {
            panic!();
        }

        let v: Value = as_[..].into();
        let a: Array = v.try_into().unwrap();
        let _ve: Vec<String> = a.try_into().unwrap();

        // Array of Struct, which in turn containin an Array (We gotta go deeper!)
        // Signature: "a(yu(xbxas)s)");
        let ar = vec![(
            // top-most simple fields
            u8::max_value(),
            u32::max_value(),
            (
                // 2nd level simple fields
                i64::max_value(),
                true,
                i64::max_value(),
                // 2nd level array field
                &["Hello", "World"][..],
            ),
            // one more top-most simple field
            "hello",
        )];
        let encoded = to_bytes(ctxt, &ar).unwrap();
        assert_eq!(encoded.len(), 78);
        let decoded =
            from_slice::<LE, Vec<(u8, u32, (i64, bool, i64, Vec<&str>), &str)>>(&encoded, ctxt)
                .unwrap();
        assert_eq!(decoded.len(), 1);
        let r = &decoded[0];
        assert_eq!(r.0, u8::max_value());
        assert_eq!(r.1, u32::max_value());
        let inner_r = &r.2;
        assert_eq!(inner_r.0, i64::max_value());
        assert_eq!(inner_r.1, true);
        assert_eq!(inner_r.2, i64::max_value());
        let as_ = &inner_r.3;
        assert_eq!(as_.len(), 2);
        assert_eq!(as_[0], "Hello");
        assert_eq!(as_[1], "World");
        assert_eq!(r.3, "hello");

        // As Value
        let v: Value = ar[..].into();
        assert_eq!(v.value_signature(), "a(yu(xbxas)s)");
        let encoded = to_bytes::<LE, _>(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 94);
        let v = from_slice::<LE, Value>(&encoded, ctxt).unwrap();
        if let Value::Array(array) = v {
            assert_eq!(*array.element_signature(), "(yu(xbxas)s)");
            assert_eq!(array.len(), 1);
            let r = &array.get()[0];
            if let Value::Structure(r) = r {
                let fields = r.fields();
                assert_eq!(fields[0], Value::U8(u8::max_value()));
                assert_eq!(fields[1], Value::U32(u32::max_value()));
                if let Value::Structure(r) = &fields[2] {
                    let fields = r.fields();
                    assert_eq!(fields[0], Value::I64(i64::max_value()));
                    assert_eq!(fields[1], Value::Bool(true));
                    assert_eq!(fields[2], Value::I64(i64::max_value()));
                    if let Value::Array(as_) = &fields[3] {
                        assert_eq!(as_.len(), 2);
                        assert_eq!(as_.get()[0], Value::new("Hello"));
                        assert_eq!(as_.get()[1], Value::new("World"));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
                assert_eq!(fields[3], Value::new("hello"));
            } else {
                panic!();
            }
        } else {
            panic!();
        }

        // Test conversion of Array of Value to Vec<Value>
        let v = Value::new(vec![Value::new(43), Value::new("bonjour")]);
        let av = <Array>::try_from(v).unwrap();
        let av = <Vec<Value>>::try_from(av).unwrap();
        assert_eq!(av[0], Value::new(43));
        assert_eq!(av[1], Value::new("bonjour"));
    }

    #[test]
    fn struct_value() {
        // Struct->Value
        let s: Value = ("a", "b", (1, 2)).into();

        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(dbg!(encoded.len()), 40);
        let decoded: Value = from_slice(&encoded, ctxt).unwrap();
        let s = <Structure>::try_from(decoded).unwrap();
        let outer = <(Str, Str, Structure)>::try_from(s).unwrap();
        assert_eq!(outer.0, "a");
        assert_eq!(outer.1, "b");

        let inner = <(i32, i32)>::try_from(outer.2).unwrap();
        assert_eq!(inner.0, 1);
        assert_eq!(inner.1, 2);
    }

    #[test]
    fn dict_value() {
        let mut map: HashMap<i64, &str> = HashMap::new();
        map.insert(1, "123");
        map.insert(2, "456");
        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &map).unwrap();
        assert_eq!(dbg!(encoded.len()), 40);
        let decoded: HashMap<i64, &str> = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded[&1], "123");
        assert_eq!(decoded[&2], "456");

        // As Value
        let v: Value = Dict::from(map).into();
        assert_eq!(v.value_signature(), "a{xs}");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 48);
        // Convert it back
        let dict: Dict = v.try_into().unwrap();
        let map: HashMap<i64, String> = dict.try_into().unwrap();
        assert_eq!(map[&1], "123");
        assert_eq!(map[&2], "456");
        // Also decode it back
        let v = from_slice(&encoded, ctxt).unwrap();
        if let Value::Dict(dict) = v {
            assert_eq!(dict.get::<i64, str>(&1).unwrap().unwrap(), "123");
            assert_eq!(dict.get::<i64, str>(&2).unwrap().unwrap(), "456");
        } else {
            panic!();
        }

        // Now a hand-crafted Dict Value but with a Value as value
        let mut dict = Dict::new(<&str>::signature(), Value::signature());
        dict.add("hello", Value::new("there")).unwrap();
        dict.add("bye", Value::new("now")).unwrap();
        let v: Value = dict.into();
        assert_eq!(v.value_signature(), "a{sv}");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(dbg!(encoded.len()), 68);
        let v: Value = from_slice(&encoded, ctxt).unwrap();
        if let Value::Dict(dict) = v {
            assert_eq!(
                *dict.get::<_, Value>("hello").unwrap().unwrap(),
                Value::new("there")
            );
            assert_eq!(
                *dict.get::<_, Value>("bye").unwrap().unwrap(),
                Value::new("now")
            );

            // Try converting to a HashMap
            let map = <HashMap<String, Value>>::try_from(dict).unwrap();
            assert_eq!(map["hello"], Value::new("there"));
            assert_eq!(map["bye"], Value::new("now"));
        } else {
            panic!();
        }
    }

    #[test]
    fn value_value() {
        let ctxt = Context::<BE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &0xABBA_ABBA_ABBA_ABBA_u64).unwrap();
        assert_eq!(encoded.len(), 8);
        assert_eq!(LE::read_u64(&encoded), 0xBAAB_BAAB_BAAB_BAAB_u64);
        let decoded: u64 = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, 0xABBA_ABBA_ABBA_ABBA);

        // Lie about there being bytes before
        let ctxt = Context::<LE>::new_dbus(2);
        let encoded = to_bytes(ctxt, &0xABBA_ABBA_ABBA_ABBA_u64).unwrap();
        assert_eq!(encoded.len(), 14);
        let decoded: u64 = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, 0xABBA_ABBA_ABBA_ABBA_u64);
        let ctxt = Context::<LE>::new_dbus(0);

        // As Value
        let v: Value = 0xFEFE_u64.into();
        assert_eq!(v.value_signature(), "t");
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 16);
        let v = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(v, Value::U64(0xFEFE));

        // And now as Value in a Value
        let v = Value::Value(Box::new(v));
        let encoded = to_bytes(ctxt, &v).unwrap();
        assert_eq!(encoded.len(), 16);
        let v = from_slice(&encoded, ctxt).unwrap();
        if let Value::Value(v) = v {
            assert_eq!(v.value_signature(), "t");
            assert_eq!(*v, Value::U64(0xFEFE));
        } else {
            panic!();
        }

        // Ensure Value works with other Serializer & Deserializer
        let v: Value = 0xFEFE_u64.into();
        let encoded = serde_json::to_string(&v).unwrap();
        let v = serde_json::from_str::<Value>(&encoded).unwrap();
        assert_eq!(v, Value::U64(0xFEFE));
    }

    #[test]
    fn enums() {
        // TODO: Document enum handling.
        //
        // 1. `Value`.
        // 2. custom (de)serialize impl.
        // 3. to/from_*_for_signature()
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        enum Test {
            Unit,
            NewType(u8),
            Tuple(u8, u64),
            Struct { y: u8, t: u64 },
        }

        let ctxt = Context::<BE>::new_dbus(0);
        let signature = "u".try_into().unwrap();
        let encoded = to_bytes_for_signature(ctxt, &signature, &Test::Unit).unwrap();
        assert_eq!(encoded.len(), 4);
        let decoded: Test = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
        assert_eq!(decoded, Test::Unit);

        let signature = "y".try_into().unwrap();
        let encoded = to_bytes_for_signature(ctxt, &signature, &Test::NewType(42)).unwrap();
        assert_eq!(encoded.len(), 5);
        let decoded: Test = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
        assert_eq!(decoded, Test::NewType(42));

        // TODO: Provide convenience API to create complex signatures
        let signature = "(yt)".try_into().unwrap();
        let encoded = to_bytes_for_signature(ctxt, &signature, &Test::Tuple(42, 42)).unwrap();
        assert_eq!(encoded.len(), 24);
        let decoded: Test = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
        assert_eq!(decoded, Test::Tuple(42, 42));

        let s = Test::Struct { y: 42, t: 42 };
        let encoded = to_bytes_for_signature(ctxt, &signature, &s).unwrap();
        assert_eq!(encoded.len(), 24);
        let decoded: Test = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
        assert_eq!(decoded, Test::Struct { y: 42, t: 42 });
    }

    #[test]
    fn derive() {
        use crate as zvariant;

        use serde::{Deserialize, Serialize};
        use serde_repr::{Deserialize_repr, Serialize_repr};

        #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
        struct Struct<'s> {
            field1: u16,
            field2: i64,
            field3: &'s str,
        }

        assert_eq!(Struct::signature(), "(qxs)");
        let s = Struct {
            field1: 0xFF_FF,
            field2: 0xFF_FF_FF_FF_FF_FF,
            field3: "hello",
        };
        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &s).unwrap();
        assert_eq!(encoded.len(), 26);
        let decoded: Struct = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, s);

        #[derive(Deserialize, Serialize, Type)]
        struct UnitStruct;

        assert_eq!(UnitStruct::signature(), <()>::signature());
        let encoded = to_bytes(ctxt, &UnitStruct).unwrap();
        assert_eq!(encoded.len(), 0);
        let _: UnitStruct = from_slice(&encoded, ctxt).unwrap();

        #[repr(u8)]
        #[derive(Deserialize_repr, Serialize_repr, Type, Debug, PartialEq)]
        enum Enum {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(Enum::signature(), u8::signature());
        let encoded = to_bytes(ctxt, &Enum::Variant3).unwrap();
        assert_eq!(encoded.len(), 1);
        let decoded: Enum = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, Enum::Variant3);

        #[repr(i64)]
        #[derive(Deserialize_repr, Serialize_repr, Type, Debug, PartialEq)]
        enum Enum2 {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(Enum2::signature(), i64::signature());
        let encoded = to_bytes(ctxt, &Enum2::Variant2).unwrap();
        assert_eq!(encoded.len(), 8);
        let decoded: Enum2 = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, Enum2::Variant2);

        #[derive(Deserialize, Serialize, Type, Debug, PartialEq)]
        enum NoReprEnum {
            Variant1,
            Variant2,
            Variant3,
        }

        assert_eq!(NoReprEnum::signature(), u32::signature());
        let encoded = to_bytes(ctxt, &NoReprEnum::Variant2).unwrap();
        assert_eq!(encoded.len(), 4);
        let decoded: NoReprEnum = from_slice(&encoded, ctxt).unwrap();
        assert_eq!(decoded, NoReprEnum::Variant2);
    }

    #[bench]
    fn serialized_size(b: &mut test::Bencher) {
        let ctxt = Context::<BE>::new_dbus(0);
        let vec: Vec<u64> = (0..10_000_000).collect();
        b.iter(|| {
            let l = crate::serialized_size(ctxt, &()).unwrap();
            assert_eq!(l, 0);

            let stdout = std::io::stdout();
            let l = crate::serialized_size_fds(ctxt, &Fd::from(&stdout)).unwrap();
            assert_eq!(l, (4, 1));

            let l = crate::serialized_size(ctxt, &('a', "abc", &(1_u32, 2))).unwrap();
            assert_eq!(l, 24);

            let l = crate::serialized_size(ctxt, &('a', "abc", &vec)).unwrap();
            assert_eq!(l, 80000024);
        });
    }

    #[bench]
    fn serialized_blah(b: &mut test::Bencher) {
        let ctxt = Context::<BE>::new_dbus(0);
        let vec: Vec<u64> = (0..10_000_000).collect();
        b.iter(|| {
            let b = crate::to_bytes(ctxt, &()).unwrap();
            assert_eq!(b.len(), 0);

            let stdout = std::io::stdout();
            let (b, fds) = crate::to_bytes_fds(ctxt, &Fd::from(&stdout)).unwrap();
            assert_eq!(b.len(), 4);

            let b = crate::to_bytes(ctxt, &('a', "abc", &(1_u32, 2))).unwrap();
            assert_eq!(b.len(), 24);

            let b = crate::to_bytes(ctxt, &('a', "abc", &vec)).unwrap();
            assert_eq!(b.len(), 80000024);
        });
    }

    #[test]
    fn struct_with_hashmap() {
        use crate as zvariant;
        use serde::{Deserialize, Serialize};

        let mut hmap = HashMap::new();
        hmap.insert("key".into(), "value".into());

        #[derive(Type, Deserialize, Serialize, PartialEq, Debug)]
        struct Foo {
            hmap: HashMap<String, String>,
        }

        let foo = Foo { hmap };
        assert_eq!(Foo::signature(), "(a{ss})");

        let ctxt = Context::<LE>::new_dbus(0);
        let encoded = to_bytes(ctxt, &(&foo, 1)).unwrap();
        let f: Foo = from_slice_fds(&encoded, None, ctxt).unwrap();
        assert_eq!(f, foo);
    }
}

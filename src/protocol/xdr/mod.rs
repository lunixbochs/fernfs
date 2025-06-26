//! XDR is a standard for the description and encoding of data.
//! It is useful for transferring data between different computer
//! architectures, and it has been used to communicate data between such
//! diverse machines as the SUN WORKSTATION*, VAX*, IBM-PC*, and Cray*
//!
//! <https://datatracker.ietf.org/doc/html/rfc450>
//!
//! Its Rust-specific implementation is presented below.
//! Where appropriate, the standard types of the XDR language have
//! been replaced by similar types of the Rust language. For example,
//! the 32-bit `Integer` type was replaced by the `i32`` type, and the
//! `opaque<>` type was replaced by `[u8]`. All the places where such
//! a replacement has been carried out have relevant comments.
//!
//! Despite the replacement of names, all guarantees for the corresponding
//! types must be respected.

use std::io::{Read, Write};

use byteorder::BigEndian;
use byteorder::{ReadBytesExt, WriteBytesExt};
use num_traits::{FromPrimitive, ToPrimitive};

pub mod mount;
pub mod nfs3;
pub mod portmap;
pub mod rpc;
mod utils;

/// XDR assumes big endian encoding.
pub type XDREndian = BigEndian;

pub trait Serialize {
    /// Serializes the implementing type to the provided writer.
    ///
    /// ## Parameters
    /// * `dest` - Where will the value be serialized to.
    ///
    /// ## Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if serialization fails.
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()>;
}

pub trait Deserialize {
    /// Deserializes data from the provided reader into the implementing type.
    ///
    /// ## Parameters
    /// * `src` - From where the value will be deserialized.
    ///
    /// ## Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if deserialization fails.
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()>;
}

/// Deserialization based on the [Default] trait of the type T.
///
/// # Parameters
/// * src - From where the value will be deserialized
///
/// # Returns
/// * `std::io::Result<()>` - Ok(()) on success, or an error if deserialization fails.
pub fn deserialize<T>(src: &mut impl Read) -> std::io::Result<T>
where
    T: Deserialize + Default,
{
    let mut val = T::default();
    val.deserialize(src)?;

    Ok(val)
}

/// Marker trait for XDR `enum` type serialization.
pub trait SerializeEnum: ToPrimitive {}

/// Enumerations have the same representation as signed integers.
impl<T: SerializeEnum> Serialize for T {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        if let Some(val) = self.to_i32() {
            return dest.write_i32::<XDREndian>(val);
        }
        Err(utils::invalid_data("Invalid enum value"))
    }
}

/// Marker trait for XDR `enum` type deserialization.
pub trait DeserializeEnum: FromPrimitive {}

/// Enumerations have the same representation as signed integers.
impl<T: DeserializeEnum> Deserialize for T {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let val = src.read_i32::<XDREndian>()?;
        if let Some(val) = FromPrimitive::from_i32(val) {
            *self = val;
            return Ok(());
        }

        Err(utils::invalid_data("Invalid enum value"))
    }
}

/// XDR `bool` type serialization implementation.
///
/// ```
/// bool identifier;
/// ```
///
/// This is equivalent to:
///
/// ```
///  enum { FALSE = 0, TRUE = 1 } identifier;
/// ```
///
/// Thus, the `bool` type is serialized as an `enum`, i.e. in `i32`.
impl Serialize for bool {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i32::<XDREndian>(if *self { 1 } else { 0 })
    }
}

/// XDR `bool` type deserialization implementation.
///
/// ```
/// bool identifier;
/// ```
///
/// This is equivalent to:
///
/// ```
///  enum { FALSE = 0, TRUE = 1 } identifier;
/// ```
///
/// Thus, the `bool` type is deserialized as an enum, i.e. in `i32`.
impl Deserialize for bool {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        match src.read_i32::<XDREndian>()? {
            0 => *self = false,
            1 => *self = true,
            _ => return Err(utils::invalid_data("Invalid value for bool enum")),
        }
        Ok(())
    }
}

/// XDR `int` type serialization implementation.
impl Serialize for i32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i32::<XDREndian>(*self)
    }
}

/// XDR `int` type deserialization implementation.
impl Deserialize for i32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR `hyper` type serialization implementation.
impl Serialize for i64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i64::<XDREndian>(*self)
    }
}

/// XDR `hyper` type deserialization implementation.
impl Deserialize for i64 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR `unsigned int` type serialization implementation.
impl Serialize for u32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u32::<XDREndian>(*self)
    }
}

/// XDR `unsigned int` type deserialization implementation.
impl Deserialize for u32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR `unsigned hyper` type serialization implementation.
impl Serialize for u64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u64::<XDREndian>(*self)
    }
}

/// XDR `unsigned hyper` type deserialization implementation.
impl Deserialize for u64 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR `float` type serialization implementation.
impl Serialize for f32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_f32::<XDREndian>(*self)
    }
}

/// XDR `float` type deserialization implementation.
impl Deserialize for f32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_f32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR `double` type serialization implementation.
impl Serialize for f64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_f64::<XDREndian>(*self)
    }
}

/// XDR `double` type deserialization implementation.
impl Deserialize for f64 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_f64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR Fixed-Length Opaque Data serialization implementation.
///
/// ```
/// opaque identifier[n];
/// ```
impl<const N: usize> Serialize for [u8; N] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_all(self)?;
        utils::write_padding(N, dest)?;

        Ok(())
    }
}

/// XDR Fixed-Length Opaque Data deserialization implementation.
///
/// ```
/// opaque identifier[n];
/// ```
impl<const N: usize> Deserialize for [u8; N] {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        src.read_exact(self)?;
        utils::read_padding(N, src)?;

        Ok(())
    }
}

/// Object lengths in XDR are always serialized as [u32]. This wrapper
/// type provides a way to serialize the [usize] type common to Rust as [u32].
#[derive(Default)]
struct UsizeAsU32(usize);

/// Try to convert [usize] to [u32] and serialize.
impl Serialize for UsizeAsU32 {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        let Some(val) = self.0.to_u32() else {
            return Err(utils::invalid_data("cannot cast `usize` to `u32`"));
        };

        val.serialize(dest)
    }
}

/// Try to deserialize [u32] and convert to [usize].
impl Deserialize for UsizeAsU32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let Some(val) = deserialize::<u32>(src)?.to_usize() else {
            return Err(utils::invalid_data("cannot cast `u32` to `usize`"));
        };

        self.0 = val;
        Ok(())
    }
}

/// XDR Variable-Length Opaque Data serialization implementation.
impl Serialize for [u8] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        UsizeAsU32(self.len()).serialize(dest)?;
        dest.write_all(self)?;
        utils::write_padding(self.len(), dest)?;

        Ok(())
    }
}

/// XDR Variable-Length Opaque Data deserialization implementation.
impl Deserialize for Vec<u8> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let length = deserialize::<UsizeAsU32>(src)?.0;
        self.resize(length, 0);

        src.read_exact(self)?;
        utils::read_padding(length, src)?;

        Ok(())
    }
}

/// XDR String serialization implementation.
impl Serialize for str {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        self.as_bytes().serialize(dest)
    }
}

/// XDR String deserialization implementation.
impl Deserialize for String {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        // SAFETY: we clear buffer on every step until verification
        unsafe {
            if let err @ Err(_) = self.as_mut_vec().deserialize(src) {
                self.clear();
                return err;
            }

            // XDR String is always ascii
            if !self.as_mut_vec().is_ascii() {
                self.clear();
                return Err(utils::invalid_data("Not ASCII string"));
            }
        };

        Ok(())
    }
}

/// XDR Fixed-Length Array serialization implementation.
///
/// ```
/// opaque identifier[n];
/// ```
impl<const N: usize, T: Serialize> Serialize for [T; N] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        for i in self {
            i.serialize(dest)?;
        }

        Ok(())
    }
}

/// XDR Fixed-Length Array deserialization implementation.
///
/// ```
/// opaque identifier[n];
/// ```
impl<const N: usize, T: Deserialize> Deserialize for [T; N] {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        for i in self {
            i.deserialize(src)?
        }

        Ok(())
    }
}

/// XDR implementation for vectors of 32-bit unsigned integers.
///
/// Serialized as a 4-byte length prefix followed by that many 4-byte integers.
impl<T: Serialize> Serialize for [T] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        UsizeAsU32(self.len()).serialize(dest)?;
        for i in self {
            i.serialize(dest)?;
        }

        Ok(())
    }
}

impl<T: Deserialize + Clone + Default> Deserialize for Vec<T> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let length = deserialize::<UsizeAsU32>(src)?.0;
        self.resize(length, T::default()); // TODO(test)
        for i in self {
            i.deserialize(src)?;
        }
        Ok(())
    }
}

/// Macro for implementing XDR serialization and deserialization for structs.
///
/// This macro simplifies implementation of the XDR trait for struct types
/// by serializing or deserializing each field in sequence.
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! SerializeStruct {
    (
        $t:ident,
        $($element:ident),*
    ) => {
        impl Serialize for $t {
            fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
                $(self.$element.serialize(dest)?;)*
                Ok(())
            }
        }
    };
}

#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! DeserializeStruct {
    (
        $t:ident,
        $($element:ident),*
    ) => {
        impl Deserialize for $t {
            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                $(self.$element.deserialize(src)?;)*
                Ok(())
            }
        }
    };
}

// XDR Optional-Data serialization implementation.
impl<T: Serialize> Serialize for Option<T> {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        match self {
            Some(data) => {
                true.serialize(dest)?;
                data.serialize(dest)?;

                Ok(())
            }
            None => false.serialize(dest),
        }
    }
}

// XDR Optional-Data deserialization implementation.
impl<T: Deserialize + Default> Deserialize for Option<T> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        if deserialize::<bool>(src)? {
            *self = Some(deserialize::<T>(src)?);
        } else {
            *self = None;
        }

        Ok(())
    }
}

// Re-export public types for use in other modules
pub use crate::DeserializeStruct;
pub use crate::SerializeStruct;

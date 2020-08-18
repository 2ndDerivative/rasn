#![no_std]

extern crate alloc;

pub mod ber;
mod error;
mod identifier;
mod parser;
mod types;
mod tag;

pub type Result<T, E = error::Error> = core::result::Result<T, E>;

pub trait Decode {
    fn decode<T: Decoder>(decoder: T, slice: &[u8]) -> Result<Self>
    where
        Self: core::marker::Sized;
}

pub trait Encode {
    fn encode<W>(&self, writer: &mut W) -> Result<()>;
}

pub trait Decoder {
    fn decode_bool(&self, slice: &[u8]) -> Result<bool>;
    fn decode_integer(&self, slice: &[u8]) -> Result<num_bigint::BigInt>;
    fn decode_octet_string(&self, slice: &[u8]) -> Result<bytes::Bytes>;
}

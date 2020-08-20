mod identifier;
mod parser;
mod error;

use snafu::OptionExt;

use crate::{Decode, Decoder, tag::Tag, types};

pub use self::error::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub fn decode<T: Decode>(slice: &[u8]) -> Result<T> {
    T::decode(Ber, slice)
}

// pub fn encode<T, W>(writer: &mut W, value: &T) -> Result<T> {
//     todo!()
// }

struct Ber;

impl Decoder for Ber {
    type Error = Error;

    fn decode_bool(&self, slice: &[u8]) -> Result<bool> {
        let (_, (identifier, contents)) = self::parser::parse_value(slice)
            .ok()
            .context(error::Parser)?;
        error::assert_tag(Tag::BOOL, identifier.tag)?;
        error::assert_length(1, contents.len())?;
        Ok(contents[0] != 0)
    }

    fn decode_integer(&self, slice: &[u8]) -> Result<types::Integer> {
        let (_, (identifier, contents)) = self::parser::parse_value(slice)
            .ok()
            .context(error::Parser)?;
        error::assert_tag(Tag::INTEGER, identifier.tag)?;
        Ok(types::Integer::from_signed_bytes_be(contents))
    }

    fn decode_octet_string(&self, slice: &[u8]) -> Result<types::OctetString> {
        let (_, (identifier, contents)) = self::parser::parse_value(slice)
            .ok()
            .context(error::Parser)?;
        error::assert_tag(Tag::OCTET_STRING, identifier.tag)?;

        Ok(types::OctetString::copy_from_slice(contents))
    }

    fn decode_null(&self, slice: &[u8]) -> Result<()> {
        let (_, (identifier, contents)) = self::parser::parse_value(slice)
            .ok()
            .context(error::Parser)?;
        error::assert_tag(Tag::NULL, identifier.tag)?;
        error::assert_length(0, contents.len())
    }

    fn decode_object_identifier(&self, slice: &[u8]) -> Result<crate::types::ObjectIdentifier> {
        use num_traits::ToPrimitive;
        let (_, (identifier, contents)) = self::parser::parse_value(slice)
            .ok()
            .context(error::Parser)?;
        error::assert_tag(Tag::OBJECT_IDENTIFIER, identifier.tag)?;
        let (input, root_octets) = parser::parse_encoded_number(contents).ok().context(error::Parser)?;
        let second = (&root_octets % 40u8)
            .to_u32()
            .expect("Second root component greater than `u32`");
        let first = ((root_octets - second) / 40u8)
            .to_u32()
            .expect("first root component greater than `u32`");
        let mut buffer = alloc::vec![first, second];

        let mut input = input;
        while !input.is_empty() {
            let (new_input, number) = parser::parse_encoded_number(input).ok().context(error::Parser)?;
            input = new_input;
            buffer.push(number.to_u32().expect("sub component greater than `u32`"));
        }

        Ok(crate::types::ObjectIdentifier::new(buffer))
    }

    fn decode_bit_string(&self, slice: &[u8]) -> Result<types::BitString> {
        parser::parse_bit_string(slice).map(|(_, bs)| bs)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean() {
        assert_eq!(true, decode(&[0x01, 0x01, 0xff]).unwrap());
        assert_eq!(false, decode(&[0x01, 0x01, 0x00]).unwrap());
    }

    #[test]
    fn integer() {
        assert_eq!(32768, decode(&[0x02, 0x03, 0x00, 0x80, 0x00,]).unwrap());
        assert_eq!(32767, decode(&[0x02, 0x02, 0x7f, 0xff]).unwrap());
        assert_eq!(256, decode(&[0x02, 0x02, 0x01, 0x00]).unwrap());
        assert_eq!(255, decode(&[0x02, 0x02, 0x00, 0xff]).unwrap());
        assert_eq!(128, decode(&[0x02, 0x02, 0x00, 0x80]).unwrap());
        assert_eq!(127, decode(&[0x02, 0x01, 0x7f]).unwrap());
        assert_eq!(1, decode(&[0x02, 0x01, 0x01]).unwrap());
        assert_eq!(0, decode(&[0x02, 0x01, 0x00]).unwrap());
        assert_eq!(-1, decode(&[0x02, 0x01, 0xff]).unwrap());
        assert_eq!(-128, decode(&[0x02, 0x01, 0x80]).unwrap());
        assert_eq!(-129, decode(&[0x02, 0x02, 0xff, 0x7f]).unwrap());
        assert_eq!(-256, decode(&[0x02, 0x02, 0xff, 0x00]).unwrap());
        assert_eq!(-32768, decode(&[0x02, 0x02, 0x80, 0x00]).unwrap());
        assert_eq!(-32769, decode(&[0x02, 0x03, 0xff, 0x7f, 0xff]).unwrap());

        let mut data = [0u8; 261];
        data[0] = 0x02;
        data[1] = 0x82;
        data[2] = 0x01;
        data[3] = 0x01;
        data[4] = 0x01;
        let mut bigint = types::Integer::from(1);
        bigint <<= 2048;
        assert_eq!(bigint, decode(&data).unwrap());
    }

    #[test]
    fn oid_from_bytes() {
        let oid = types::ObjectIdentifier::new(alloc::vec![1, 2, 840, 113549]);
        let from_raw =
            decode(&[0x6, 0x6, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d][..]).unwrap();

        assert_eq!(oid, from_raw);
    }

    #[test]
    fn bit_string() {
        let bitstring = {
            let mut b = types::BitString::from_vec(alloc::vec![0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0]);

            for _ in 0..4 {
                b.pop();
            }

            b
        };

        let primitive_encoded: types::BitString =
            decode(&[0x03, 0x07, 0x04, 0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0][..]).unwrap();

        let constructed_encoded: types::BitString =
            decode(&[
                // TAG + LENGTH
                0x23, 0x80,
                    // Part 1
                    0x03, 0x03, 0x00, 0x0A, 0x3B,
                    // Part 2
                    0x3, 0x5, 0x04, 0x5F, 0x29, 0x1C, 0xD0,
                // EOC
                0x0, 0x0][..]).map_err(|e| panic!("{}", e)).unwrap();

        assert_eq!(bitstring, primitive_encoded);
        assert_eq!(bitstring, constructed_encoded);
    }
}

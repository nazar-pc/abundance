use crate::pieces::Piece;
use serde::de::Visitor;
use serde::de::value::{BytesDeserializer, Error, StrDeserializer};
use serde::{Deserialize, Deserializer};

/// Same as [`BytesDeserializer`], but not human-readable like most binary formats
struct BinaryDeserializer<'de>(BytesDeserializer<'de, Error>);

impl<'de> Deserializer<'de> for BinaryDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0.deserialize_any(visitor)
    }

    fn is_human_readable(&self) -> bool {
        false
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option
        unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

#[test]
fn piece_deserialize_binary_checks_length() {
    let bytes = vec![0; Piece::SIZE + 1];

    for length in [0, 5, Piece::SIZE - 1, Piece::SIZE + 1] {
        let deserializer = BinaryDeserializer(BytesDeserializer::new(&bytes[..length]));
        Piece::deserialize(deserializer).unwrap_err();
    }

    let deserializer = BinaryDeserializer(BytesDeserializer::new(&bytes[..Piece::SIZE]));
    let piece = Piece::deserialize(deserializer).unwrap();
    assert_eq!(piece.as_ref().len(), Piece::SIZE);
}

#[test]
// Hex-decoding a full-size piece takes too long under Miri, and there is no `unsafe` code involved
#[cfg_attr(miri, ignore)]
fn piece_deserialize_human_readable_checks_length() {
    let deserializer = StrDeserializer::<Error>::new("0011");
    Piece::deserialize(deserializer).unwrap_err();

    let hex = "00".repeat(Piece::SIZE);
    let deserializer = StrDeserializer::<Error>::new(&hex);
    let piece = Piece::deserialize(deserializer).unwrap();
    assert_eq!(piece.as_ref().len(), Piece::SIZE);
}

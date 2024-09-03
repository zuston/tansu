// Copyright ⓒ 2024 Peter Morgan <peter.james.morgan@gmail.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

pub mod batch;
pub(crate) mod codec;
pub mod compression;
pub mod header;

use crate::{
    primitive::{
        varint::{LongVarInt, VarInt},
        ByteSize,
    },
    Result,
};
pub use batch::{Batch, Frame};
use bytes::Bytes;
use codec::{Octets, VarIntSequence};
pub use header::Header;
use serde::{
    ser::{self, SerializeSeq},
    Deserialize, Serialize, Serializer,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Record {
    #[serde(serialize_with = "VarInt::serialize")]
    #[serde(deserialize_with = "VarInt::deserialize")]
    pub length: i32,

    pub attributes: u8,

    #[serde(serialize_with = "LongVarInt::serialize")]
    #[serde(deserialize_with = "LongVarInt::deserialize")]
    pub timestamp_delta: i64,

    #[serde(serialize_with = "VarInt::serialize")]
    #[serde(deserialize_with = "VarInt::deserialize")]
    pub offset_delta: i32,

    #[serde(serialize_with = "Octets::serialize")]
    #[serde(deserialize_with = "Octets::deserialize")]
    pub key: Option<Bytes>,

    #[serde(serialize_with = "Octets::serialize")]
    #[serde(deserialize_with = "Octets::deserialize")]
    pub value: Option<Bytes>,

    #[serde(serialize_with = "VarIntSequence::<Header>::serialize")]
    #[serde(deserialize_with = "VarIntSequence::<Header>::deserialize")]
    pub headers: Vec<Header>,
}

impl Record {
    #[must_use]
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn key(&self) -> Option<Bytes> {
        self.key.clone()
    }

    pub fn value(&self) -> Option<Bytes> {
        self.value.clone()
    }

    pub fn is_tombstone(&self) -> bool {
        self.key.is_some() && self.value.is_none()
    }
}

impl TryFrom<Builder> for Record {
    type Error = crate::Error;

    fn try_from(builder: Builder) -> Result<Self, Self::Error> {
        Ok(Self {
            length: builder
                .size_in_bytes()
                .and_then(|sz| i32::try_from(sz).map_err(Into::into))?,
            attributes: builder.attributes,
            timestamp_delta: *builder.timestamp_delta,
            offset_delta: *builder.offset_delta,
            key: builder.key.into(),
            value: builder.value.into(),
            headers: builder.headers.0.into_iter().map(Into::into).collect(),
        })
    }
}

impl From<Record> for Builder {
    fn from(value: Record) -> Self {
        Self {
            attributes: value.attributes,
            timestamp_delta: value.timestamp_delta.into(),
            offset_delta: value.offset_delta.into(),
            key: value.key.into(),
            value: value.value.into(),
            headers: value.headers.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Builder {
    attributes: u8,
    timestamp_delta: LongVarInt,
    offset_delta: VarInt,
    key: Octets,
    value: Octets,
    headers: VarIntSequence<header::Builder>,
}

impl Serialize for Builder {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_seq(None).and_then(|mut s| {
            self.size_in_bytes()
                .map_err(|e| ser::Error::custom(e.to_string()))
                .map(|size| {
                    VarInt::try_from(size)
                        .map_err(|e| ser::Error::custom(e.to_string()))
                        .and_then(|v| s.serialize_element(&v))
                })
                .and(s.serialize_element(&self.attributes))
                .and(s.serialize_element(&self.timestamp_delta))
                .and(s.serialize_element(&self.offset_delta))
                .and(s.serialize_element(&self.key))
                .and(s.serialize_element(&self.value))
                .and(s.serialize_element(&self.headers))
                .and(s.end())
        })
    }
}

impl ByteSize for Builder {
    fn size_in_bytes(&self) -> Result<usize> {
        Ok(size_of::<u8>()
            + self.timestamp_delta.size_in_bytes()?
            + self.offset_delta.size_in_bytes()?
            + self.key.size_in_bytes()?
            + self.value.size_in_bytes()?
            + self.headers.size_in_bytes()?)
    }
}

impl Builder {
    #[must_use]
    pub fn attributes(mut self, attributes: u8) -> Self {
        self.attributes = attributes;
        self
    }

    #[must_use]
    pub fn timestamp_delta(mut self, timestamp_delta: i64) -> Self {
        self.timestamp_delta = timestamp_delta.into();
        self
    }

    #[must_use]
    pub fn offset_delta(mut self, offset_delta: i32) -> Self {
        self.offset_delta = offset_delta.into();
        self
    }

    #[must_use]
    pub fn key(mut self, key: Octets) -> Self {
        self.key = key;
        self
    }

    #[must_use]
    pub fn value(mut self, value: Octets) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn header(mut self, header: header::Builder) -> Self {
        self.headers.0.extend(vec![header]);
        self
    }

    pub fn build(self) -> Result<Record> {
        Record::try_from(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{de::Decoder, ser::Encoder, Result};
    use codec::Sequence;
    use std::io::Cursor;

    #[test]
    fn bytes_size() -> Result<()> {
        assert_eq!(4, Octets::from(Some(vec![100, 101, 102])).size_in_bytes()?);
        Ok(())
    }

    #[test]
    fn record_size() -> Result<()> {
        assert_eq!(
            9,
            Record::builder()
                .value(vec![100, 101, 102].into())
                .size_in_bytes()?
        );
        Ok(())
    }

    #[test]
    fn serialize_record_builder() -> Result<()> {
        let rb = Record::builder().value(vec![100, 101, 102].into());

        let mut c = Cursor::new(vec![]);
        let mut e = Encoder::new(&mut c);
        rb.serialize(&mut e)?;

        assert_eq!(vec![18, 0, 0, 0, 1, 6, 100, 101, 102, 0], c.into_inner());
        Ok(())
    }

    #[test]
    fn try_from_record_builder() -> Result<()> {
        let record = Record::try_from(Record::builder().value(vec![100, 101, 102].into()))?;
        assert_eq!(9, record.length);
        Ok(())
    }

    #[test]
    fn sequence_of_record_builder() -> Result<()> {
        let rb = Record::builder().value(vec![100, 101, 102].into());
        let records = Sequence::from(vec![rb.clone()]);
        assert_eq!(14, records.size_in_bytes()?);

        let mut c = Cursor::new(vec![]);
        let mut e = Encoder::new(&mut c);
        records.serialize(&mut e)?;

        assert_eq!(
            vec![0, 0, 0, 1, 18, 0, 0, 0, 1, 6, 100, 101, 102, 0],
            c.into_inner()
        );

        Ok(())
    }

    #[test]
    fn batch() -> Result<()> {
        let decoded = Batch::builder()
            .base_offset(0)
            .partition_leader_epoch(-1)
            .magic(2)
            .attributes(0)
            .last_offset_delta(0)
            .base_timestamp(1_707_058_170_165)
            .max_timestamp(1_707_058_170_165)
            .producer_id(1)
            .producer_epoch(0)
            .base_sequence(1)
            .record(Record::builder().value(vec![100, 101, 102].into()))
            .build()?;

        assert_eq!(decoded.batch_length, 59);
        assert_eq!(decoded.crc, 1_126_819_645);

        Ok(())
    }

    #[test]
    fn crc_check() {
        use crc::Crc;
        use crc::CRC_32_ISCSI;

        let b = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 141, 116, 152, 137, 53, 0, 0, 1, 141, 116, 152, 137, 53, 0,
            0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 18, 0, 0, 0, 1, 6, 100, 101, 102, 0,
        ];

        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
        let mut digester = crc.digest();
        digester.update(&b);
        assert_eq!(1_126_819_645, digester.finalize());
    }

    #[test]
    fn batch_decode() -> Result<()> {
        let mut encoded = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 255, 2, 67, 41, 231, 61, 0, 0, 0,
            0, 0, 0, 0, 0, 1, 141, 116, 152, 137, 53, 0, 0, 1, 141, 116, 152, 137, 53, 0, 0, 0, 0,
            0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 18, 0, 0, 0, 1, 6, 100, 101, 102, 0,
        ];

        let decoded = Batch::builder()
            .base_offset(0)
            .partition_leader_epoch(-1)
            .magic(2)
            .attributes(0)
            .last_offset_delta(0)
            .base_timestamp(1_707_058_170_165)
            .max_timestamp(1_707_058_170_165)
            .producer_id(1)
            .producer_epoch(0)
            .base_sequence(1)
            .record(Record::builder().value(vec![100, 101, 102].into()))
            .build()?;

        let mut c = Cursor::new(&mut encoded);
        let mut decoder = Decoder::new(&mut c);
        let actual = Batch::deserialize(&mut decoder)?;

        assert_eq!(decoded, actual);

        Ok(())
    }

    #[test]
    fn batch_encode() -> Result<()> {
        let mut encoded = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 255, 2, 67, 41, 231, 61, 0, 0, 0,
            0, 0, 0, 0, 0, 1, 141, 116, 152, 137, 53, 0, 0, 1, 141, 116, 152, 137, 53, 0, 0, 0, 0,
            0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 18, 0, 0, 0, 1, 6, 100, 101, 102, 0,
        ];

        let decoded = Batch::builder()
            .base_offset(0)
            .partition_leader_epoch(-1)
            .magic(2)
            .attributes(0)
            .last_offset_delta(0)
            .base_timestamp(1_707_058_170_165)
            .max_timestamp(1_707_058_170_165)
            .producer_id(1)
            .producer_epoch(0)
            .base_sequence(1)
            .record(Record::builder().value(vec![100, 101, 102].into()))
            .build()?;

        let mut c = Cursor::new(&mut encoded);
        let mut decoder = Decoder::new(&mut c);
        let actual = Batch::deserialize(&mut decoder)?;

        assert_eq!(decoded, actual);

        Ok(())
    }

    #[test]
    fn build_batch_records() -> Result<()> {
        let keys: Vec<String> = (0..=6).map(|i| format!("k{i}")).collect();
        let values: Vec<String> = (0..=11).map(|i| format!("v{i}")).collect();

        let mut builder = Batch::builder();
        let indexes = [
            (1, 1),
            (2, 2),
            (1, 3),
            (1, 4),
            (3, 5),
            (2, 6),
            (4, 7),
            (5, 8),
            (5, 9),
            (2, 10),
            (6, 11),
        ];

        for (offset_delta, (key_index, value_index)) in indexes.into_iter().enumerate() {
            builder = builder.record(
                Record::builder()
                    .offset_delta(i32::try_from(offset_delta)?)
                    .key(keys[key_index].as_bytes().into())
                    .value(values[value_index].as_bytes().into()),
            );
        }

        let batch = builder.build()?;
        assert_eq!(indexes.len(), batch.records.len());

        Ok(())
    }
}

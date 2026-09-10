//! TLV records inside a layer payload: `[type u8][len u32 BE][bytes]`.

use bytes::{BufMut, Bytes, BytesMut};

/// Record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    /// fMP4 initialisation segment (`ftyp` + `moov`).
    Init = 1,
    /// One fMP4 media fragment (`moof` + `mdat`).
    Media = 2,
    /// UTF-8 JSON metadata.
    Meta = 3,
}

impl RecordType {
    /// Parse a type byte.
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Init),
            2 => Some(Self::Media),
            3 => Some(Self::Meta),
            _ => None,
        }
    }
}

/// One record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Type.
    pub kind: RecordType,
    /// Payload.
    pub data: Bytes,
}

impl Record {
    /// Convenience constructor.
    pub fn new(kind: RecordType, data: impl Into<Bytes>) -> Self {
        Self {
            kind,
            data: data.into(),
        }
    }
}

/// Encode records back to back. An empty slice encodes to an empty payload.
pub fn encode(records: &[Record]) -> Bytes {
    let n: usize = records.iter().map(|r| 5 + r.data.len()).sum();
    let mut out = BytesMut::with_capacity(n);
    for r in records {
        out.put_u8(r.kind as u8);
        out.put_u32(r.data.len() as u32);
        out.put_slice(&r.data);
    }
    out.freeze()
}

/// Decode a payload. Stops at the first malformed record; unknown types are
/// skipped. Opaque non-TLV bytes therefore decode to an empty list.
pub fn decode(payload: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= payload.len() {
        let t = payload[i];
        let len = u32::from_be_bytes([
            payload[i + 1],
            payload[i + 2],
            payload[i + 3],
            payload[i + 4],
        ]) as usize;
        i += 5;
        if i + len > payload.len() {
            break;
        }
        if let Some(kind) = RecordType::from_u8(t) {
            out.push(Record {
                kind,
                data: Bytes::copy_from_slice(&payload[i..i + len]),
            });
        } else {
            break;
        }
        i += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let recs = vec![
            Record::new(RecordType::Init, vec![1u8, 2, 3]),
            Record::new(RecordType::Media, vec![9u8; 1000]),
            Record::new(RecordType::Meta, b"{}".to_vec()),
        ];
        let enc = encode(&recs);
        assert_eq!(enc.len(), 5 * 3 + 3 + 1000 + 2);
        assert_eq!(decode(&enc), recs);
    }

    #[test]
    fn empty_and_opaque() {
        assert!(decode(&[]).is_empty());
        assert!(decode(&encode(&[])).is_empty());
        // Opaque bytes (e.g. an mp4 streamed raw) yield nothing or stop early.
        let junk = vec![0x00u8, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p', 1, 2, 3];
        assert!(decode(&junk).is_empty());
        // Truncated record is dropped.
        let mut t = encode(&[Record::new(RecordType::Media, vec![1u8; 10])]).to_vec();
        t.truncate(8);
        assert!(decode(&t).is_empty());
    }
}

//! Minimal ISO BMFF box splitter for ffmpeg's fragmented output.

use bytes::{Buf, Bytes, BytesMut};

/// A top-level box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Box4 {
    /// FourCC.
    pub kind: [u8; 4],
    /// Whole box bytes, header included.
    pub bytes: Bytes,
}

/// Incremental splitter: feed bytes, pull complete boxes.
#[derive(Debug, Default)]
pub struct BoxSplitter {
    buf: BytesMut,
}

impl BoxSplitter {
    /// New splitter.
    pub fn new() -> Self {
        Self::default()
    }
    /// Append input.
    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
    /// Next complete box, if any.
    pub fn next_box(&mut self) -> Option<Box4> {
        if self.buf.len() < 8 {
            return None;
        }
        let size32 =
            u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        let kind = [self.buf[4], self.buf[5], self.buf[6], self.buf[7]];
        let size = match size32 {
            1 => {
                if self.buf.len() < 16 {
                    return None;
                }
                u64::from_be_bytes([
                    self.buf[8],
                    self.buf[9],
                    self.buf[10],
                    self.buf[11],
                    self.buf[12],
                    self.buf[13],
                    self.buf[14],
                    self.buf[15],
                ]) as usize
            }
            0 => return None, // "to end of file": not produced by fragmented output
            n => n,
        };
        if size < 8 || self.buf.len() < size {
            return None;
        }
        let bytes = self.buf.split_to(size).freeze();
        let _ = self.buf.remaining();
        Some(Box4 { kind, bytes })
    }
}

/// Groups boxes into INIT (`ftyp`+`moov`) and MEDIA (`moof`+`mdat`) units.
#[derive(Debug, Default)]
pub struct FragmentAssembler {
    splitter: BoxSplitter,
    pending: Vec<Bytes>,
    in_media: bool,
}

/// An assembled unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    /// Initialisation segment.
    Init(Bytes),
    /// One media fragment.
    Media(Bytes),
}

impl FragmentAssembler {
    /// New assembler.
    pub fn new() -> Self {
        Self::default()
    }
    /// Feed bytes and collect completed units.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Unit> {
        self.splitter.feed(data);
        let mut out = Vec::new();
        while let Some(b) = self.splitter.next_box() {
            match &b.kind {
                b"ftyp" | b"styp" | b"free" | b"skip" | b"sidx" => self.pending.push(b.bytes),
                b"moov" => {
                    self.pending.push(b.bytes);
                    out.push(Unit::Init(concat(std::mem::take(&mut self.pending))));
                    self.in_media = false;
                }
                b"moof" => {
                    self.pending.push(b.bytes);
                    self.in_media = true;
                }
                b"mdat" => {
                    self.pending.push(b.bytes);
                    if self.in_media {
                        out.push(Unit::Media(concat(std::mem::take(&mut self.pending))));
                        self.in_media = false;
                    }
                }
                _ => self.pending.push(b.bytes),
            }
        }
        out
    }
}

fn concat(parts: Vec<Bytes>) -> Bytes {
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap();
    }
    let n: usize = parts.iter().map(|p| p.len()).sum();
    let mut v = BytesMut::with_capacity(n);
    for p in parts {
        v.extend_from_slice(&p);
    }
    v.freeze()
}

/// Find the `avcC` box inside an init segment and return the RFC 6381 codec
/// string (`avc1.PPCCLL`), if present.
pub fn avc1_codec_string(init: &[u8]) -> Option<String> {
    let pos = init.windows(4).position(|w| w == b"avcC")?;
    // avcC payload: configurationVersion(1) profile(1) compat(1) level(1)
    let p = pos + 4;
    if p + 4 > init.len() {
        return None;
    }
    Some(format!(
        "avc1.{:02x}{:02x}{:02x}",
        init[p + 1],
        init[p + 2],
        init[p + 3]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn splits_and_groups() {
        let mut a = FragmentAssembler::new();
        let mut stream = Vec::new();
        stream.extend(bx(b"ftyp", b"isom"));
        stream.extend(bx(b"moov", &[0u8; 40]));
        stream.extend(bx(b"moof", &[1u8; 20]));
        stream.extend(bx(b"mdat", &[2u8; 100]));
        stream.extend(bx(b"moof", &[3u8; 20]));
        stream.extend(bx(b"mdat", &[4u8; 50]));
        // Feed in odd-sized pieces.
        let mut units = Vec::new();
        for c in stream.chunks(7) {
            units.extend(a.feed(c));
        }
        assert_eq!(units.len(), 3);
        assert!(matches!(units[0], Unit::Init(ref b) if b.len() == 12 + 48));
        assert!(matches!(units[1], Unit::Media(ref b) if b.len() == 28 + 108));
        assert!(matches!(units[2], Unit::Media(ref b) if b.len() == 28 + 58));
    }

    #[test]
    fn codec_string() {
        let mut init = bx(b"ftyp", b"isom");
        let mut avcc = bx(b"avcC", &[1, 0x64, 0x00, 0x1f, 0xff]);
        init.append(&mut avcc);
        assert_eq!(avc1_codec_string(&init).as_deref(), Some("avc1.64001f"));
        assert_eq!(avc1_codec_string(b"nothing"), None);
    }
}

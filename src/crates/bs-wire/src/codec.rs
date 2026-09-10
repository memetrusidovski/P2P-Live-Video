//! Encode/decode traits and cursor helpers shared by every layout in this crate.

use bytes::{Buf, BufMut};

use crate::error::{Result, WireError};

/// A structure with a canonical byte encoding.
pub trait Encode {
    /// Number of bytes [`Encode::encode`] writes.
    fn encoded_len(&self) -> usize;
    /// Append the encoding to `buf`.
    fn encode<B: BufMut>(&self, buf: &mut B);
    /// Encode into a fresh vector.
    fn to_vec(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len());
        self.encode(&mut v);
        v
    }
}

/// A structure parsed from its canonical byte encoding.
pub trait Decode: Sized {
    /// Parse from the front of `buf`, advancing past the structure.
    fn decode<B: Buf>(buf: &mut B) -> Result<Self>;
    /// Parse a complete slice, rejecting trailing bytes.
    fn from_slice_exact(bytes: &[u8]) -> Result<Self> {
        let mut cur = bytes;
        let v = Self::decode(&mut cur)?;
        if !cur.is_empty() {
            return Err(WireError::TrailingBytes(cur.len()));
        }
        Ok(v)
    }
}

/// Ensure `buf` has at least `n` readable bytes.
#[inline]
pub fn need<B: Buf>(buf: &B, n: usize, context: &'static str) -> Result<()> {
    if buf.remaining() < n {
        Err(WireError::Truncated {
            needed: n - buf.remaining(),
            context,
        })
    } else {
        Ok(())
    }
}
/// Read a fixed-size array.
#[inline]
pub fn get_array<const N: usize, B: Buf>(buf: &mut B, context: &'static str) -> Result<[u8; N]> {
    need(buf, N, context)?;
    let mut out = [0u8; N];
    buf.copy_to_slice(&mut out);
    Ok(out)
}
/// Read `n` bytes into a vector.
#[inline]
pub fn get_vec<B: Buf>(buf: &mut B, n: usize, context: &'static str) -> Result<Vec<u8>> {
    need(buf, n, context)?;
    let mut out = vec![0u8; n];
    buf.copy_to_slice(&mut out);
    Ok(out)
}
macro_rules! getter {
    ($name:ident, $ty:ty, $n:expr, $m:ident) => {
        /// Read a big-endian integer.
        #[inline]
        pub fn $name<B: Buf>(buf: &mut B, context: &'static str) -> Result<$ty> {
            need(buf, $n, context)?;
            Ok(buf.$m())
        }
    };
}
getter!(get_u8, u8, 1, get_u8);
getter!(get_u16, u16, 2, get_u16);
getter!(get_u32, u32, 4, get_u32);
getter!(get_u64, u64, 8, get_u64);

/// Skip `n` reserved bytes. Senders write zero; receivers ignore the value.
#[inline]
pub fn skip_reserved<B: Buf>(buf: &mut B, n: usize, context: &'static str) -> Result<()> {
    need(buf, n, context)?;
    buf.advance(n);
    Ok(())
}
/// Write `n` zero bytes.
#[inline]
pub fn put_reserved<B: BufMut>(buf: &mut B, n: usize) {
    for _ in 0..n {
        buf.put_u8(0);
    }
}

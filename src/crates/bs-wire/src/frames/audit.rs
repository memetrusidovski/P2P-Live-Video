//! REPUTATION_AUDIT_GOSSIP (0x22): signed, evidence-carrying accusations with the
//! accuser's own RANK_PROOF (Ch5 §5.3.3, App D §D.4.18).

use bytes::{Buf, BufMut};

use crate::codec::{
    get_u16, get_u64, get_u8, get_vec, put_reserved, skip_reserved, Decode, Encode,
};
use crate::error::{Result, WireError};
use crate::frames::{ProofOfUpload, RankProof};
use crate::types::{NodeId, PublicKeyBytes, SignatureBytes};

/// What an accusation proves (Ch5 §5.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EvidenceType {
    /// Two receipts, one in each direction, same tree, same window, similar bytes.
    SymmetricTreePair = 0x01,
    /// Two statements signed by the suspect that cannot both be true.
    Equivocation = 0x02,
}

impl EvidenceType {
    fn from_code(c: u8) -> Result<Self> {
        match c {
            0x01 => Ok(Self::SymmetricTreePair),
            0x02 => Ok(Self::Equivocation),
            v => Err(WireError::InvalidField {
                field: "EvidenceType",
                value: v as u64,
            }),
        }
    }
}

/// One accusation. The evidence is two PROOF_OF_UPLOAD bodies, each length-prefixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accusation {
    /// The accused.
    pub suspect: NodeId,
    /// Evidence kind.
    pub evidence_type: EvidenceType,
    /// First receipt.
    pub first: ProofOfUpload,
    /// Second receipt.
    pub second: ProofOfUpload,
    /// Signer keys of the two receipts as `(public key, static nonce)`, so a receiver
    /// that never met the signers can check both signatures (ISSUE-065).
    pub signers: Vec<(PublicKeyBytes, u64)>,
    /// µs when the accuser signed.
    pub timestamp_us: u64,
    /// Accuser signature over every preceding byte of the accusation.
    pub signature: SignatureBytes,
}

impl Accusation {
    fn evidence_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + self.first.encoded_len() + self.second.encoded_len());
        v.extend_from_slice(&(self.first.encoded_len() as u16).to_be_bytes());
        self.first.encode(&mut v);
        v.extend_from_slice(&(self.second.encoded_len() as u16).to_be_bytes());
        self.second.encode(&mut v);
        v.push(self.signers.len() as u8);
        for (pk, nonce) in &self.signers {
            pk.encode(&mut v);
            v.extend_from_slice(&nonce.to_be_bytes());
        }
        v
    }

    /// Bytes the accuser signs.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let ev = self.evidence_bytes();
        let mut v = Vec::with_capacity(32 + 4 + ev.len() + 8);
        v.extend_from_slice(self.suspect.as_bytes());
        v.push(self.evidence_type as u8);
        v.push(0);
        v.extend_from_slice(&(ev.len() as u16).to_be_bytes());
        v.extend_from_slice(&ev);
        v.extend_from_slice(&self.timestamp_us.to_be_bytes());
        v
    }
}

impl Encode for Accusation {
    fn encoded_len(&self) -> usize {
        32 + 1
            + 1
            + 2
            + 4
            + self.first.encoded_len()
            + self.second.encoded_len()
            + 1
            + 40 * self.signers.len()
            + 8
            + 64
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_slice(&self.signable_bytes());
        self.signature.encode(buf);
    }
}

impl Decode for Accusation {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let suspect = NodeId::decode(buf)?;
        let evidence_type = EvidenceType::from_code(get_u8(buf, "Accusation.type")?)?;
        skip_reserved(buf, 1, "Accusation.reserved")?;
        let elen = get_u16(buf, "Accusation.evidence_len")? as usize;
        let ev = get_vec(buf, elen, "Accusation.evidence")?;
        let mut cur = &ev[..];
        let l1 = get_u16(&mut cur, "Accusation.first_len")? as usize;
        let b1 = get_vec(&mut cur, l1, "Accusation.first")?;
        let first = ProofOfUpload::from_slice_exact(&b1)?;
        let l2 = get_u16(&mut cur, "Accusation.second_len")? as usize;
        let b2 = get_vec(&mut cur, l2, "Accusation.second")?;
        let second = ProofOfUpload::from_slice_exact(&b2)?;
        let ns = get_u8(&mut cur, "Accusation.signers")? as usize;
        let mut signers = Vec::with_capacity(ns);
        for _ in 0..ns {
            let pk = PublicKeyBytes::decode(&mut cur)?;
            let nonce = get_u64(&mut cur, "Accusation.signer_nonce")?;
            signers.push((pk, nonce));
        }
        if !cur.is_empty() {
            return Err(WireError::InvalidField {
                field: "Accusation.evidence",
                value: cur.len() as u64,
            });
        }
        let timestamp_us = get_u64(buf, "Accusation.timestamp")?;
        let signature = SignatureBytes::decode(buf)?;
        Ok(Self {
            suspect,
            evidence_type,
            first,
            second,
            signers,
            timestamp_us,
            signature,
        })
    }
}

/// REPUTATION_AUDIT_GOSSIP: up to `MAX_ACCUSATIONS` accusations by one accuser, with
/// the accuser's RANK_PROOF unless the receiver ranks the accuser already.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditGossip {
    /// Accuser (the prover of `rank_proof`).
    pub accuser: NodeId,
    /// Accuser's `(public key, static nonce)`, so a receiver two hops away can check
    /// the accusation signatures (ISSUE-065).
    pub accuser_key: (PublicKeyBytes, u64),
    /// Accuser standing, omitted toward a current tree parent or child.
    pub rank_proof: Option<Box<RankProof>>,
    /// Accusations.
    pub accusations: Vec<Accusation>,
}

impl Encode for AuditGossip {
    fn encoded_len(&self) -> usize {
        32 + 40
            + 1
            + 1
            + 2
            + self
                .rank_proof
                .as_ref()
                .map(|p| p.encoded_len())
                .unwrap_or(0)
            + self
                .accusations
                .iter()
                .map(|a| a.encoded_len())
                .sum::<usize>()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.accuser.encode(buf);
        self.accuser_key.0.encode(buf);
        buf.put_u64(self.accuser_key.1);
        buf.put_u8(self.accusations.len() as u8);
        put_reserved(buf, 1);
        match &self.rank_proof {
            Some(p) => {
                buf.put_u16(p.encoded_len() as u16);
                p.encode(buf);
            }
            None => buf.put_u16(0),
        }
        for a in &self.accusations {
            a.encode(buf);
        }
    }
}

impl Decode for AuditGossip {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let accuser = NodeId::decode(buf)?;
        let accuser_key = (
            PublicKeyBytes::decode(buf)?,
            get_u64(buf, "AuditGossip.accuser_nonce")?,
        );
        let count = get_u8(buf, "AuditGossip.count")?;
        if count as usize > crate::consts::MAX_ACCUSATIONS as usize {
            return Err(WireError::InvalidField {
                field: "AuditGossip.count",
                value: count as u64,
            });
        }
        skip_reserved(buf, 1, "AuditGossip.reserved")?;
        let plen = get_u16(buf, "AuditGossip.proof_len")? as usize;
        let rank_proof = if plen == 0 {
            None
        } else {
            let pb = get_vec(buf, plen, "AuditGossip.proof")?;
            Some(Box::new(RankProof::from_slice_exact(&pb)?))
        };
        let mut accusations = Vec::with_capacity(count as usize);
        for _ in 0..count {
            accusations.push(Accusation::decode(buf)?);
        }
        Ok(Self {
            accuser,
            accuser_key,
            rank_proof,
            accusations,
        })
    }
}

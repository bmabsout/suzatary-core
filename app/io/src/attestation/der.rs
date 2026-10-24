//! The DER this module reads off Intel's certificates and CRLs: a walk over
//! tag-length-values, never a general ASN.1 parser, since rustls-webpki has already
//! checked every signature over these bytes and only a few fields are wanted.

/// A DER encoding that could not be walked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("malformed DER: {0}")]
pub struct DerError(pub &'static str);

/// The tags this module meets.
pub(super) mod tag {
    pub const INTEGER: u8 = 0x02;
    pub const OCTET_STRING: u8 = 0x04;
    pub const OID: u8 = 0x06;
    pub const UTC_TIME: u8 = 0x17;
    pub const GENERALIZED_TIME: u8 = 0x18;
    pub const SEQUENCE: u8 = 0x30;
    pub const SET: u8 = 0x31;
    pub const EXPLICIT_0: u8 = 0xa0;
    pub const EXPLICIT_3: u8 = 0xa3;
}

/// One tag-length-value, borrowed from its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Tlv<'a> {
    pub tag: u8,
    pub value: &'a [u8],
}

/// Reads TLVs off the front of a slice, in order.
#[derive(Debug, Clone)]
pub(super) struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The next TLV; definite lengths of at most four bytes only, as DER requires
    /// of anything this size.
    pub fn next(&mut self) -> Result<Tlv<'a>, DerError> {
        let [tag, first, rest @ ..] = self.0 else {
            return Err(DerError("truncated header"));
        };
        let (length, rest) = if first & 0x80 == 0 {
            (usize::from(*first), rest)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > 4 || rest.len() < count {
                return Err(DerError("unsupported length"));
            }
            let (digits, rest) = rest.split_at(count);
            let length = digits
                .iter()
                .fold(0usize, |sum, digit| (sum << 8) | usize::from(*digit));
            (length, rest)
        };
        if rest.len() < length {
            return Err(DerError("truncated value"));
        }
        let (value, rest) = rest.split_at(length);
        self.0 = rest;
        Ok(Tlv { tag: *tag, value })
    }

    /// The next TLV's value, which must carry `tag`.
    pub fn expect(&mut self, tag: u8) -> Result<&'a [u8], DerError> {
        let tlv = self.next()?;
        if tlv.tag == tag {
            Ok(tlv.value)
        } else {
            Err(DerError("unexpected tag"))
        }
    }

    /// The next TLV's value when it carries `tag`; nothing is consumed otherwise.
    pub fn optional(&mut self, tag: u8) -> Result<Option<&'a [u8]>, DerError> {
        match self.0.first() {
            Some(found) if *found == tag => self.expect(tag).map(Some),
            _ => Ok(None),
        }
    }

    /// Every remaining TLV.
    pub fn all(mut self) -> Result<Vec<Tlv<'a>>, DerError> {
        let mut all = Vec::new();
        while !self.is_empty() {
            all.push(self.next()?);
        }
        Ok(all)
    }
}

/// A non-negative INTEGER's value, at most `u16`; DER prefixes a zero byte when the
/// top bit is set, so `00 ff` is 255.
pub(super) fn small_unsigned(value: &[u8]) -> Result<u16, DerError> {
    let digits = match value {
        [0, rest @ ..] if !rest.is_empty() => rest,
        [top, ..] if top & 0x80 != 0 => return Err(DerError("negative integer")),
        digits => digits,
    };
    if digits.is_empty() || digits.len() > 2 {
        return Err(DerError("integer out of range"));
    }
    Ok(digits
        .iter()
        .fold(0u16, |sum, digit| (sum << 8) | u16::from(*digit)))
}

/// A `UTCTime` or `GeneralizedTime`, as Unix seconds; only the `Z` forms RFC 5280 allows.
pub(super) fn time(tlv: Tlv<'_>) -> Result<i64, DerError> {
    let text = std::str::from_utf8(tlv.value).map_err(|_| DerError("time is not ASCII"))?;
    let full = match tlv.tag {
        tag::UTC_TIME => {
            let year: u32 = text
                .get(..2)
                .and_then(|year| year.parse().ok())
                .ok_or(DerError("UTCTime year"))?;
            format!("{}{text}", if year < 50 { "20" } else { "19" })
        }
        tag::GENERALIZED_TIME => text.to_owned(),
        _ => return Err(DerError("not a time")),
    };
    chrono::NaiveDateTime::parse_from_str(&full, "%Y%m%d%H%M%SZ")
        .map(|at| at.and_utc().timestamp())
        .map_err(|_| DerError("time is not YYYYMMDDHHMMSSZ"))
}

/// The ECDSA `SEQUENCE { r INTEGER, s INTEGER }` a raw `r ‖ s` signature encodes as,
/// which is the form rustls-webpki verifies.
pub(super) fn ecdsa_signature(raw: &[u8; 64]) -> Vec<u8> {
    fn integer(half: &[u8]) -> Vec<u8> {
        let start = half
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(half.len() - 1);
        let digits = &half[start..];
        let pad = digits[0] & 0x80 != 0;
        let mut out = vec![tag::INTEGER, u8::from(pad) + digit_count(digits)];
        if pad {
            out.push(0);
        }
        out.extend_from_slice(digits);
        out
    }
    fn digit_count(digits: &[u8]) -> u8 {
        u8::try_from(digits.len()).expect("half of 64 bytes")
    }
    let body = [integer(&raw[..32]), integer(&raw[32..])].concat();
    let mut out = vec![
        tag::SEQUENCE,
        u8::try_from(body.len()).expect("two integers of at most 33 bytes"),
    ];
    out.extend(body);
    out
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn truncation_and_indefinite_lengths_are_refused() {
        for bad in [
            &[tag::SEQUENCE][..],
            &[tag::SEQUENCE, 3, 0],
            &[tag::SEQUENCE, 0x80],
        ] {
            assert!(Reader::new(bad).next().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn both_time_forms_read() {
        let utc = Tlv {
            tag: tag::UTC_TIME,
            value: b"261102042624Z",
        };
        let generalized = Tlv {
            tag: tag::GENERALIZED_TIME,
            value: b"20261102042624Z",
        };
        assert_eq!(time(utc), Ok(1_793_593_584));
        assert_eq!(time(generalized), time(utc));
    }

    proptest! {
        /// The encoding is DER's: minimal, sign-padded, and read back by this reader as
        /// the two halves it was given.
        #[test]
        fn a_raw_signature_encodes_as_two_minimal_integers(raw in prop::array::uniform32(any::<u8>()), s in prop::array::uniform32(any::<u8>())) {
            let mut both = [0u8; 64];
            both[..32].copy_from_slice(&raw);
            both[32..].copy_from_slice(&s);
            let der = ecdsa_signature(&both);
            let mut outer = Reader::new(&der);
            let mut inner = Reader::new(outer.expect(tag::SEQUENCE).expect("a sequence"));
            prop_assert!(outer.is_empty());
            for half in [&both[..32], &both[32..]] {
                let digits = inner.expect(tag::INTEGER).expect("an integer");
                prop_assert!(digits[0] & 0x80 == 0, "never negative");
                prop_assert!(digits.len() == 1 || digits[0] != 0 || digits[1] & 0x80 != 0, "minimal");
                let value = digits.iter().skip_while(|byte| **byte == 0).copied().collect::<Vec<_>>();
                let expected = half.iter().skip_while(|byte| **byte == 0).copied().collect::<Vec<_>>();
                prop_assert_eq!(value, expected);
            }
            prop_assert!(inner.is_empty());
        }
    }
}

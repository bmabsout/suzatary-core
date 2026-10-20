//! A TDX quote, version 4, as Intel's DCAP quoting enclave writes it.
//!
//! Parsing checks only the layout: a parsed quote is a claim until its signatures are
//! checked against Intel's collateral.

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;

use super::der::DerError;
use super::pck::Pck;

/// The one version this module reads; version 5 carries a different report body.
const VERSION: u16 = 4;

/// ECDSA-256 over P-256.
const ATTESTATION_KEY_TYPE: u16 = 2;

/// The header's `tee_type` for TDX.
const TEE_TDX: u32 = 0x81;

/// Intel's quoting enclave.
const INTEL_QE_VENDOR: [u8; 16] = [
    0x93, 0x9a, 0x72, 0x33, 0xf7, 0x9c, 0x4c, 0xa9, 0x94, 0x0a, 0x0d, 0xb3, 0x95, 0x7f, 0x06, 0x07,
];

/// The certification data that carries the QE report (6), and the PEM PCK chain inside
/// it (5).
const QE_REPORT_CERTIFICATION: u16 = 6;
const PCK_CHAIN_CERTIFICATION: u16 = 5;

const HEADER: usize = 48;
const TD_REPORT: usize = 584;
const QE_REPORT: usize = 384;

/// The TD's own report: its measurements and the data it bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdReport {
    /// The TDX module's security versions; byte 1 names the module's major version.
    pub tee_tcb_svn: [u8; 16],
    /// The TDX module's signer.
    pub mr_signer_seam: [u8; 48],
    /// The TDX module's attributes.
    pub seam_attributes: [u8; 8],
    /// The TD's attributes; bit 0 is debug.
    pub td_attributes: [u8; 8],
    /// The TD's build-time measurement.
    pub mr_td: [u8; 48],
    /// The four run-time measurement registers.
    pub rtmr: [[u8; 48]; 4],
    /// What the TD bound into the quote.
    pub report_data: [u8; 64],
}

/// The quoting enclave's SGX report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QeReport {
    /// The report's bytes, which the PCK key signed.
    pub bytes: [u8; QE_REPORT],
    /// The enclave's `MISCSELECT`.
    pub misc_select: u32,
    /// The enclave's attributes; bit 1 of byte 0 is debug.
    pub attributes: [u8; 16],
    /// The enclave's signer.
    pub mr_signer: [u8; 32],
    /// The enclave's product id.
    pub isv_prod_id: u16,
    /// The enclave's security version.
    pub isv_svn: u16,
    /// What the enclave bound: the attestation key's hash, then zeros.
    pub report_data: [u8; 64],
}

/// A TDX quote, its layout checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// The header and the TD report, the bytes the attestation key signed.
    pub signed: Vec<u8>,
    /// The TD's report.
    pub td: TdReport,
    /// The attestation key's signature over `signed`, `r ‖ s`.
    pub signature: [u8; 64],
    /// The attestation key, an uncompressed P-256 point without its `04`.
    pub attestation_key: [u8; 64],
    /// The quoting enclave's report.
    pub qe: QeReport,
    /// The PCK key's signature over the QE report, `r ‖ s`.
    pub qe_signature: [u8; 64],
    /// The QE's authentication data, hashed with the attestation key into its report.
    pub qe_auth_data: Vec<u8>,
    /// The PCK certificate chain, leaf first.
    pub pck_chain: Vec<CertificateDer<'static>>,
    /// The leaf's SGX extension.
    pub pck: Pck,
}

/// Why bytes are not a quote this module reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuoteLayoutError {
    /// Shorter than its own lengths say.
    #[error("the quote is truncated at {0}")]
    Truncated(&'static str),
    /// Non-zero bytes follow the quote's own length.
    #[error("non-zero bytes follow the quote")]
    Trailing,
    /// A header field names something other than an Intel TDX v4 ECDSA quote.
    #[error("the quote's {field} is {found}, not {wanted}")]
    Unsupported {
        /// The header field.
        field: &'static str,
        /// What it held.
        found: String,
        /// What this module reads.
        wanted: String,
    },
    /// The certification data is not a QE report carrying a PEM PCK chain.
    #[error("the quote's certification data is type {0}, not a QE report with a PCK chain")]
    Certification(u16),
    /// The PCK chain was not PEM certificates.
    #[error("the quote's PCK chain is not PEM certificates: {0}")]
    Chain(String),
    /// The PCK leaf's SGX extension could not be read.
    #[error("the PCK certificate's SGX extension: {0}")]
    Pck(#[from] DerError),
}

impl Quote {
    /// Read a quote's layout.
    ///
    /// # Errors
    /// Truncation, a header other than Intel TDX v4 ECDSA-P256, or certification data
    /// other than a QE report carrying a PEM PCK chain.
    pub fn parse(bytes: &[u8]) -> Result<Self, QuoteLayoutError> {
        let mut cursor = Cursor(bytes);
        let header = cursor.take::<HEADER>("the header")?;
        let td = cursor.take::<TD_REPORT>("the TD report")?;
        let signed = [&header[..], &td[..]].concat();
        expect("version", &le16(&header[0..2]), &VERSION)?;
        expect(
            "attestation key type",
            &le16(&header[2..4]),
            &ATTESTATION_KEY_TYPE,
        )?;
        expect("TEE type", &le32(&header[4..8]), &TEE_TDX)?;
        expect(
            "QE vendor",
            &hex::encode(&header[12..28]),
            &hex::encode(INTEL_QE_VENDOR),
        )?;

        let signature_length = cursor.u32("the signature length")?;
        let mut signature_data = Cursor(cursor.slice(signature_length, "the signature data")?);
        // The gateway pads its quote with zeros; anything else after it is not this quote.
        if cursor.0.iter().any(|byte| *byte != 0) {
            return Err(QuoteLayoutError::Trailing);
        }
        let signature = signature_data.take::<64>("the quote signature")?;
        let attestation_key = signature_data.take::<64>("the attestation key")?;
        let kind = signature_data.u16("the certification type")?;
        let length = signature_data.u32("the certification length")?;
        if kind != QE_REPORT_CERTIFICATION {
            return Err(QuoteLayoutError::Certification(kind));
        }
        let mut certification = Cursor(signature_data.slice(length, "the certification data")?);
        let qe = certification.take::<QE_REPORT>("the QE report")?;
        let qe_signature = certification.take::<64>("the QE report signature")?;
        let auth_length = certification.u16("the QE auth data length")?;
        let qe_auth_data = certification
            .slice(u32::from(auth_length), "the QE auth data")?
            .to_vec();
        let inner = certification.u16("the PCK chain type")?;
        let inner_length = certification.u32("the PCK chain length")?;
        if inner != PCK_CHAIN_CERTIFICATION {
            return Err(QuoteLayoutError::Certification(inner));
        }
        let pem = certification.slice(inner_length, "the PCK chain")?;
        let pck_chain = CertificateDer::pem_slice_iter(pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|failed| QuoteLayoutError::Chain(failed.to_string()))?;
        let leaf = pck_chain
            .first()
            .ok_or_else(|| QuoteLayoutError::Chain("no certificate".to_owned()))?;
        let pck = Pck::read(leaf)?;

        Ok(Self {
            signed,
            td: TdReport::read(&td),
            signature,
            attestation_key,
            qe: QeReport::read(&qe),
            qe_signature,
            qe_auth_data,
            pck_chain,
            pck,
        })
    }
}

impl TdReport {
    fn read(bytes: &[u8; TD_REPORT]) -> Self {
        let at = |start: usize| -> [u8; 48] { array(&bytes[start..start + 48]) };
        Self {
            tee_tcb_svn: array(&bytes[0..16]),
            mr_signer_seam: at(64),
            seam_attributes: array(&bytes[112..120]),
            td_attributes: array(&bytes[120..128]),
            mr_td: at(136),
            rtmr: [at(328), at(376), at(424), at(472)],
            report_data: array(&bytes[520..584]),
        }
    }
}

impl QeReport {
    fn read(bytes: &[u8; QE_REPORT]) -> Self {
        Self {
            misc_select: le32(&bytes[16..20]),
            attributes: array(&bytes[48..64]),
            mr_signer: array(&bytes[128..160]),
            isv_prod_id: le16(&bytes[256..258]),
            isv_svn: le16(&bytes[258..260]),
            report_data: array(&bytes[320..384]),
            bytes: *bytes,
        }
    }
}

fn expect<T: PartialEq + std::fmt::Display>(
    field: &'static str,
    found: &T,
    wanted: &T,
) -> Result<(), QuoteLayoutError> {
    if found == wanted {
        Ok(())
    } else {
        Err(QuoteLayoutError::Unsupported {
            field,
            found: found.to_string(),
            wanted: wanted.to_string(),
        })
    }
}

fn array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    <[u8; N]>::try_from(bytes).expect("every caller slices exactly N bytes")
}

fn le16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(array(bytes))
}

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(array(bytes))
}

/// Reads fixed and length-prefixed fields off the front of a slice.
struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn slice(&mut self, length: u32, what: &'static str) -> Result<&'a [u8], QuoteLayoutError> {
        let length = usize::try_from(length).map_err(|_| QuoteLayoutError::Truncated(what))?;
        if self.0.len() < length {
            return Err(QuoteLayoutError::Truncated(what));
        }
        let (head, rest) = self.0.split_at(length);
        self.0 = rest;
        Ok(head)
    }

    fn take<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N], QuoteLayoutError> {
        let length = u32::try_from(N).expect("a field is far shorter than 4 GB");
        self.slice(length, what).map(array)
    }

    fn u16(&mut self, what: &'static str) -> Result<u16, QuoteLayoutError> {
        self.take::<2>(what).map(u16::from_le_bytes)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, QuoteLayoutError> {
        self.take::<4>(what).map(u32::from_le_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::super::attested::fixture::quote_bytes as live;
    use super::*;
    use crate::attestation::pck::{Ca, Platform};

    #[test]
    fn the_live_quote_reads() {
        let quote = Quote::parse(&live()).expect("the live quote reads");
        assert_eq!(quote.signed.len(), HEADER + TD_REPORT);
        assert_eq!(quote.pck_chain.len(), 3, "leaf, platform CA, root");
        assert_eq!(
            quote.pck.platform,
            Platform {
                fmspc: [0x20, 0xa0, 0x6f, 0x00, 0x00, 0x00],
                ca: Ca::Platform,
            }
        );
        assert_eq!(quote.pck.pce_id, [0, 0]);
        assert_eq!(quote.pck.pce_svn, 13);
        assert_eq!(quote.td.tee_tcb_svn[..2], [0x0b, 0x01]);
        assert_eq!(quote.qe_auth_data.len(), 32);
        assert_eq!(
            hex::encode(&quote.td.report_data[..32]),
            "cd7ba554c759bd85aa72f78653be61190eb905bd247f2e1103552101038c150d"
        );
    }

    /// The live quote's own length, before the zeros the gateway pads it with.
    const LENGTH: usize = 4936;

    #[test]
    fn every_truncation_is_named_never_a_panic() {
        let bytes = live();
        assert!(bytes[LENGTH..].iter().all(|byte| *byte == 0));
        for cut in (0..LENGTH).step_by(37) {
            assert!(Quote::parse(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        Quote::parse(&bytes[..LENGTH]).expect("the quote without its padding reads");
    }

    #[test]
    fn a_non_zero_byte_after_the_quote_is_refused() {
        let mut bytes = live();
        bytes[LENGTH + 3] = 1;
        assert_eq!(Quote::parse(&bytes), Err(QuoteLayoutError::Trailing));
    }

    #[test]
    fn another_version_or_tee_is_unsupported() {
        let mut v5 = live();
        v5[0] = 5;
        assert!(matches!(
            Quote::parse(&v5),
            Err(QuoteLayoutError::Unsupported {
                field: "version",
                ..
            })
        ));
        let mut sgx = live();
        sgx[4] = 0;
        assert!(matches!(
            Quote::parse(&sgx),
            Err(QuoteLayoutError::Unsupported {
                field: "TEE type",
                ..
            })
        ));
    }
}

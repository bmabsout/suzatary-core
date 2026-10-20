//! The SGX extension of a PCK certificate: which platform signed a quote, at what TCB.

use super::der::{self, DerError, Reader, tag};

/// `1.2.840.113741.1.13.1`, Intel's SGX extension.
const SGX_EXTENSION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf8, 0x4d, 0x01, 0x0d, 0x01];

/// `2.5.4.3`, a name's common name.
const COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];

/// Which Intel CA issued a PCK certificate, and so which PCK CRL can revoke it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ca {
    /// `Intel SGX PCK Platform CA`.
    Platform,
    /// `Intel SGX PCK Processor CA`.
    Processor,
}

impl Ca {
    /// The `ca` parameter Intel's PCS names it by.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Processor => "processor",
        }
    }
}

/// A platform as Intel's collateral is keyed: its family-model-stepping-platform code
/// and the CA that issued its PCK certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Platform {
    /// The FMSPC, which names the platform's TCB info.
    pub fmspc: [u8; 6],
    /// The issuing CA.
    pub ca: Ca,
}

/// What a PCK certificate attests of its platform; read from a certificate whose chain
/// is checked separately, so holding one says nothing about who signed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pck {
    /// The platform, the collateral's key.
    pub platform: Platform,
    /// The PCE's id, which the TCB info must name too.
    pub pce_id: [u8; 2],
    /// The sixteen SGX TCB components.
    pub cpu_svn: [u8; 16],
    /// The PCE's security version.
    pub pce_svn: u16,
}

impl Pck {
    /// Read the issuer and the SGX extension of a DER certificate.
    ///
    /// # Errors
    /// A malformed certificate, an issuer that is neither PCK CA, or an extension
    /// missing a field.
    pub fn read(certificate: &[u8]) -> Result<Self, DerError> {
        let mut outer = Reader::new(certificate);
        let mut tbs = Reader::new(Reader::new(outer.expect(tag::SEQUENCE)?).expect(tag::SEQUENCE)?);
        tbs.optional(tag::EXPLICIT_0)?;
        tbs.expect(tag::INTEGER)?;
        tbs.expect(tag::SEQUENCE)?;
        let ca = issuer_ca(tbs.expect(tag::SEQUENCE)?)?;
        let extensions = tbs
            .all()?
            .into_iter()
            .find(|tlv| tlv.tag == tag::EXPLICIT_3)
            .ok_or(DerError("no extensions"))?;
        let sgx = Reader::new(Reader::new(extensions.value).expect(tag::SEQUENCE)?)
            .all()?
            .into_iter()
            .find_map(|extension| {
                let mut fields = Reader::new(extension.value);
                (fields.expect(tag::OID).ok()? == SGX_EXTENSION)
                    .then(|| fields.expect(tag::OCTET_STRING).ok())
                    .flatten()
            })
            .ok_or(DerError("no SGX extension"))?;

        let entries = Reader::new(Reader::new(sgx).expect(tag::SEQUENCE)?).all()?;
        let tcb = Reader::new(named(&entries, &[0x02])?.value).all()?;
        let pce_svn = named(&tcb, &[0x02, 0x11])?;
        let cpu_svn = named(&tcb, &[0x02, 0x12])?;
        if pce_svn.tag != tag::INTEGER || cpu_svn.tag != tag::OCTET_STRING {
            return Err(DerError("a TCB field has the wrong type"));
        }
        Ok(Self {
            platform: Platform {
                fmspc: fixed(named(&entries, &[0x04])?)?,
                ca,
            },
            pce_id: fixed(named(&entries, &[0x03])?)?,
            cpu_svn: fixed(cpu_svn)?,
            pce_svn: der::small_unsigned(pce_svn.value)?,
        })
    }
}

/// The value of the `SEQUENCE { OID, value }` among `entries` whose OID is the SGX
/// extension's followed by `suffix`.
fn named<'a>(entries: &[der::Tlv<'a>], suffix: &[u8]) -> Result<der::Tlv<'a>, DerError> {
    entries
        .iter()
        .find_map(|entry| {
            let mut pair = Reader::new(entry.value);
            let oid = pair.expect(tag::OID).ok()?;
            if oid.strip_prefix(SGX_EXTENSION) == Some(suffix) {
                pair.next().ok()
            } else {
                None
            }
        })
        .ok_or(DerError("an SGX extension field is missing"))
}

fn fixed<const N: usize>(tlv: der::Tlv<'_>) -> Result<[u8; N], DerError> {
    if tlv.tag != tag::OCTET_STRING {
        return Err(DerError("expected an octet string"));
    }
    <[u8; N]>::try_from(tlv.value).map_err(|_| DerError("an octet string has the wrong length"))
}

/// The CA a PCK certificate's issuer names, by its common name.
fn issuer_ca(name: &[u8]) -> Result<Ca, DerError> {
    let common_name = Reader::new(name)
        .all()?
        .into_iter()
        .filter(|set| set.tag == tag::SET)
        .find_map(|set| {
            let mut attribute = Reader::new(Reader::new(set.value).expect(tag::SEQUENCE).ok()?);
            (attribute.expect(tag::OID).ok()? == COMMON_NAME)
                .then(|| attribute.next().ok())
                .flatten()
        })
        .ok_or(DerError("the issuer has no common name"))?;
    match common_name.value {
        b"Intel SGX PCK Platform CA" => Ok(Ca::Platform),
        b"Intel SGX PCK Processor CA" => Ok(Ca::Processor),
        _ => Err(DerError("the issuer is not an Intel PCK CA")),
    }
}

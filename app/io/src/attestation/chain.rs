//! Intel's SGX root, the one trust anchor, and the check every collateral and PCK
//! signature rests on: a chain to it, under its CRLs, at an instant.

use std::time::Duration;

use rustls_pki_types::{CertificateDer, UnixTime};
use webpki::{
    CertRevocationList, EndEntityCert, ExpirationPolicy, KeyPurposeIdIter, OwnedCertRevocationList,
    RevocationCheckDepth, RevocationOptionsBuilder, UnknownStatusPolicy,
};

use super::der;

/// `Intel SGX Root CA`, from `certificates.trustedservices.intel.com`; its SHA-256 is
/// pinned by a test, so a changed byte is a failing build rather than a new root.
const INTEL_SGX_ROOT_CA: &[u8] = include_bytes!("intel-sgx-root-ca.der");

/// Intel's two CRLs, parsed: the root's, which can revoke an intermediate, and the PCK
/// CA's, which can revoke a platform's certificate.
#[derive(Debug, Clone)]
pub(super) struct Crls {
    root: OwnedCertRevocationList,
    pck: OwnedCertRevocationList,
}

impl Crls {
    /// Parse both; their signatures are checked by every chain they are used for.
    pub(super) fn parse(root: &[u8], pck: &[u8]) -> Result<Self, webpki::Error> {
        let owned = |der: &[u8]| OwnedCertRevocationList::from_der(der);
        Ok(Self {
            root: owned(root)?,
            pck: owned(pck)?,
        })
    }
}

/// A CRL's `thisUpdate` and `nextUpdate`, in Unix seconds, which rustls-webpki checks
/// and does not expose; a CRL with no `nextUpdate` is refused, since Intel's all have one.
pub(super) fn crl_window(crl: &[u8]) -> Result<(i64, i64), der::DerError> {
    let mut list = der::Reader::new(crl);
    let mut tbs = der::Reader::new(
        der::Reader::new(list.expect(der::tag::SEQUENCE)?).expect(der::tag::SEQUENCE)?,
    );
    tbs.optional(der::tag::INTEGER)?;
    tbs.expect(der::tag::SEQUENCE)?;
    tbs.expect(der::tag::SEQUENCE)?;
    let this_update = der::time(tbs.next()?)?;
    let next_update = der::time(tbs.next()?)?;
    Ok((this_update, next_update))
}

/// Intel's DCAP certificates name no extended key usage, so any is accepted.
struct AnyPurpose;

impl webpki::ExtendedKeyUsageValidator for AnyPurpose {
    fn validate(&self, _: KeyPurposeIdIter<'_, '_>) -> Result<(), webpki::Error> {
        Ok(())
    }
}

/// `chain`'s leaf, once its path to Intel's root verifies at `now` and no certificate on
/// it is revoked by an unexpired CRL; a certificate no CRL speaks for is refused.
pub(super) fn verified_leaf<'c>(
    chain: &'c [CertificateDer<'c>],
    crls: &Crls,
    now: i64,
) -> Result<EndEntityCert<'c>, webpki::Error> {
    let root = CertificateDer::from(INTEL_SGX_ROOT_CA);
    let anchor = webpki::anchor_from_trusted_cert(&root)?;
    let [leaf, intermediates @ ..] = chain else {
        return Err(webpki::Error::BadDer);
    };
    let leaf = EndEntityCert::try_from(leaf)?;
    let lists = [&crls.root, &crls.pck].map(|crl| CertRevocationList::from(crl.clone()));
    let lists = [&lists[0], &lists[1]];
    let revocation = RevocationOptionsBuilder::new(&lists)
        .expect("two CRLs are given")
        .with_depth(RevocationCheckDepth::Chain)
        .with_status_policy(UnknownStatusPolicy::Deny)
        .with_expiration_policy(ExpirationPolicy::Enforce)
        .build();
    let at = UnixTime::since_unix_epoch(Duration::from_secs(
        u64::try_from(now).map_err(|_| webpki::Error::BadDerTime)?,
    ));
    leaf.verify_for_usage(
        &[webpki::ring::ECDSA_P256_SHA256],
        &[anchor],
        intermediates,
        at,
        AnyPurpose,
        Some(revocation),
        None,
    )?;
    Ok(leaf)
}

/// Verify a raw `r ‖ s` ECDSA P-256 signature over `message` under `leaf`'s key.
pub(super) fn signed_by(
    leaf: &EndEntityCert<'_>,
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), webpki::Error> {
    leaf.verify_signature(
        webpki::ring::ECDSA_P256_SHA256,
        message,
        &der::ecdsa_signature(signature),
    )
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    /// The fingerprint Intel publishes for its SGX root, which `dcap-qvl` embeds too.
    #[test]
    fn the_root_is_intels() {
        assert_eq!(
            hex::encode(Sha256::digest(INTEL_SGX_ROOT_CA)),
            "44a0196b2b99f889b8e149e95b807a350e7424964399e885a7cbb8ccfab674d3"
        );
    }
}

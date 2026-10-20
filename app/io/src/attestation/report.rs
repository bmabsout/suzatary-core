//! The gateway's attestation report, read: its keyset checked against the digest it
//! names, and its quote's bytes. Nothing in it is believed until an
//! [`Identity`](super::Identity) is verified from it.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::quote::Quote;
use super::{AttestationError, canonical, prefix};

/// A report whose keyset hashes to the digest it names, and whose header agrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// `sha256:` over the keyset's JCS form, recomputed and equal to the report's.
    pub workload_keyset_digest: String,
    /// The keyset whose digest checked out.
    pub keyset: Value,
    /// The gateway's `image_digest`, stated beside the quote and bound by nothing.
    pub image_digest: Option<String>,
    /// The gateway's `repo_commit`, likewise.
    pub repo_commit: Option<String>,
    /// The gateway's `repo_url`, likewise.
    pub repo_url: Option<String>,
    quote: Vec<u8>,
}

impl Report {
    /// Read a report body (ACI §4.1) and the `x-aci-keyset-digest` header it came
    /// with.
    ///
    /// # Errors
    /// Another version or TEE, a body not of the shape §4.1 gives, a keyset whose
    /// digest is not the one named, or a header that disagrees.
    pub fn parse(text: &str, header: Option<&str>) -> Result<Self, AttestationError> {
        let malformed = || AttestationError::Malformed(prefix(text));
        let body: Value = serde_json::from_str(text).map_err(|_| malformed())?;
        let field =
            |value: &Value, name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
        if field(&body, "api_version").as_deref() != Some("aci/1") {
            return Err(AttestationError::ReportVersion {
                served: field(&body, "api_version"),
            });
        }
        let attestation = body.get("attestation").ok_or_else(malformed)?;
        if field(attestation, "tee_type").as_deref() != Some("tdx") {
            return Err(AttestationError::NotTdx {
                served: field(attestation, "tee_type"),
            });
        }
        let workload_keyset_digest =
            field(&body, "workload_keyset_digest").ok_or_else(malformed)?;
        if let Some(header) = header
            && header != workload_keyset_digest
        {
            return Err(AttestationError::HeaderBodyMismatch {
                header: header.to_owned(),
                body: workload_keyset_digest,
            });
        }
        let keyset = attestation.get("workload_keyset").ok_or_else(malformed)?;
        let recomputed = format!("sha256:{}", hex::encode(Sha256::digest(canonical(keyset))));
        if recomputed != workload_keyset_digest {
            return Err(AttestationError::KeysetDigestMismatch {
                claimed: workload_keyset_digest,
                recomputed,
            });
        }
        let quote = attestation
            .get("evidence")
            .and_then(|evidence| field(evidence, "quote"))
            .and_then(|quote| hex::decode(quote).ok())
            .ok_or_else(malformed)?;
        let provenance = attestation.get("source_provenance").unwrap_or(&Value::Null);
        Ok(Self {
            workload_keyset_digest,
            keyset: keyset.clone(),
            image_digest: field(provenance, "image_digest"),
            repo_commit: field(provenance, "repo_commit"),
            repo_url: field(provenance, "repo_url"),
            quote,
        })
    }

    /// The quote's layout, read.
    ///
    /// # Errors
    /// Bytes that are not a TDX v4 quote.
    pub fn quote(&self) -> Result<Quote, AttestationError> {
        Quote::parse(&self.quote).map_err(AttestationError::QuoteLayout)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testkit::{Signer, digest, keyset, report};
    use super::*;

    fn world() -> (Value, String) {
        let keyset = keyset(
            &[Signer::generate("receipt-test").key_entry()],
            2_000_000_000,
        );
        let text = report(&keyset, &"ab".repeat(20));
        (keyset, text)
    }

    #[test]
    fn a_consistent_report_reads_its_keyset_and_provenance() {
        let (keyset, text) = world();
        let read = Report::parse(&text, Some(&digest(&keyset))).expect("it reads");
        assert_eq!(read.workload_keyset_digest, digest(&keyset));
        assert_eq!(read.keyset, keyset);
        assert_eq!(read.repo_commit, Some("ab".repeat(20)));
        assert!(read.image_digest.is_none(), "null is absent");
    }

    #[test]
    fn a_header_body_disagreement_is_refused() {
        let (keyset, text) = world();
        match Report::parse(&text, Some("sha256:wrong")) {
            Err(AttestationError::HeaderBodyMismatch { header, body }) => {
                assert_eq!(header, "sha256:wrong");
                assert_eq!(body, digest(&keyset));
            }
            other => panic!("expected HeaderBodyMismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_tampered_keyset_digest_is_refused() {
        let (_, text) = world();
        let tampered = text.replacen(
            "\"workload_keyset_digest\":\"sha256:",
            "\"workload_keyset_digest\":\"sha256:ff",
            1,
        );
        assert!(matches!(
            Report::parse(&tampered, None),
            Err(AttestationError::KeysetDigestMismatch { .. })
        ));
    }

    /// The wire's member order is free (ACI §3.1): the digest is over the JCS form.
    #[test]
    fn reordering_the_keyset_members_still_reads() {
        let (keyset, text) = world();
        let entries = serde_json::to_string(&keyset["receipt_signing_keys"]).expect("serializes");
        let reordered = text.replacen(
            &format!(r#""subject":null,"not_after":2000000000,"receipt_signing_keys":{entries}"#),
            &format!(r#""receipt_signing_keys":{entries},"not_after":2000000000,"subject":null"#),
            1,
        );
        assert_ne!(
            reordered, text,
            "the replacement must actually have matched"
        );
        assert_eq!(
            Report::parse(&reordered, None)
                .map(|read| read.workload_keyset_digest)
                .ok(),
            Some(digest(&keyset))
        );
    }

    #[test]
    fn another_version_tee_or_shape_is_named() {
        let (_, text) = world();
        assert!(matches!(
            Report::parse(&text.replacen("aci/1", "aci/2", 1), None),
            Err(AttestationError::ReportVersion { served: Some(v) }) if v == "aci/2"
        ));
        assert!(matches!(
            Report::parse(&text.replacen("\"tdx\"", "\"sev_snp\"", 1), None),
            Err(AttestationError::NotTdx { served: Some(v) }) if v == "sev_snp"
        ));
        for shapeless in ["{}", "not json", r#"{"api_version":"aci/1"}"#] {
            assert!(Report::parse(shapeless, None).is_err(), "{shapeless}");
        }
    }
}

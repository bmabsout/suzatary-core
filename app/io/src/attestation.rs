//! Resolves a confidential-inference receipt against the gateway's attested identity
//! (ACI §9): the receipt's Ed25519 signature, under a keyset whose digest a genuine
//! TDX TD bound with this process's nonce into a quote that verifies to Intel's root.
//!
//! Not checked, and said so: the report's source provenance is stated beside the
//! quote and compared with no measurement (the event log's RTMR3, the compose hash),
//! and the receipt's event log is not compared with the bytes this process sent and
//! received.

pub mod attested;
mod chain;
pub mod collateral;
mod der;
mod identities;
mod identity;
pub mod pck;
pub mod quote;
mod report;
pub mod tcb;

use std::time::Duration;

use serde_json::Value;

use suzatary_core::capability::{ReceiptId, Secret};

use crate::http::{Http, Req};

pub use identities::Identities;
pub use identity::{Identity, Nonce, TCB_CEILING};
pub use report::Report;

/// A receipt is a few kilobytes, not a stream.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The keyset digest every gateway response names (ACI §5.2): a report's must agree
/// with its body and a reply's with its receipt, so a proxy rewriting one is caught.
pub const KEYSET_DIGEST_HEADER: &str = "x-aci-keyset-digest";

/// The public repository the live gateway reports as its `repo_url`, printed by
/// `doctor` beside the commit the owner is asked to confirm.
pub const KNOWN_REPO_URL: &str = "https://github.com/Dstack-TEE/private-ai-gateway.git";

/// The gateway's expected source commit, recorded by the owner and never learned
/// from the provider; a commit rather than an image digest because the live gateway
/// reports `image_digest` as `null`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedSource {
    repo_commit: String,
}

impl ExpectedSource {
    /// Parse exactly 40 lowercase hex characters, so a misconfiguration shows at
    /// startup rather than as every comparison silently failing.
    ///
    /// # Errors
    /// Any other shape.
    pub fn parse(text: &str) -> Result<Self, InvalidExpectedSource> {
        if text.len() == 40 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self {
                repo_commit: text.to_owned(),
            })
        } else {
            Err(InvalidExpectedSource(text.to_owned()))
        }
    }

    /// The commit id, verbatim.
    #[must_use]
    pub fn repo_commit(&self) -> &str {
        &self.repo_commit
    }
}

/// The core's `SourceCommit` is already this shape, so it needs no second parse.
impl From<&suzatary_core::settings::SourceCommit> for ExpectedSource {
    fn from(commit: &suzatary_core::settings::SourceCommit) -> Self {
        Self {
            repo_commit: commit.as_str().to_owned(),
        }
    }
}

/// `expect_commit` was not 40 lowercase hex characters.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("expect_commit {0:?} is not 40 lowercase hex characters")]
pub struct InvalidExpectedSource(pub String);

/// The key that signed a receipt; [`check_signature`] is the only constructor, so
/// holding one means the Ed25519 verification succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedBy {
    key_id: String,
}

impl SignedBy {
    /// The `key_id` of the `receipt_signing_keys` entry that verified.
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
}

/// Why a receipt's signature did not verify.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    /// The receipt names no `key_id`, so there is no attested entry to verify it under.
    #[error("the receipt names no key_id")]
    MissingKeyId,
    /// The receipt carries no `signature` string.
    #[error("the receipt carries no signature")]
    MissingSignature,
    /// `key_id` is not in the attested keyset's `receipt_signing_keys`.
    #[error("the receipt's key_id {key_id:?} is not in the attested keyset (it lists {known:?})")]
    UnknownKeyId {
        /// The id the receipt named.
        key_id: String,
        /// Every `key_id` the attested keyset does list.
        known: Vec<String>,
    },
    /// The named entry's `algo` is not `ed25519`, the one this module
    /// verifies.
    #[error("key {key_id:?} is {algo:?}; only ed25519 receipts are verified")]
    UnsupportedAlgorithm {
        /// The entry's `key_id`.
        key_id: String,
        /// The entry's `algo`, verbatim (empty when absent).
        algo: String,
    },
    /// The named entry's `public_key` is not 32 bytes of hex.
    #[error("key {key_id:?}'s public_key is not 32 bytes of hex: {reason}")]
    PublicKeyEncoding {
        /// The entry's `key_id`.
        key_id: String,
        /// What was wrong with it.
        reason: String,
    },
    /// `signature` is not 64 bytes of hex.
    #[error("the receipt's signature is not 64 bytes of hex: {0}")]
    SignatureEncoding(String),
    /// Well-formed, and wrong: Ed25519 rejected it over these bytes.
    #[error("the receipt's signature does not verify under key {key_id:?}")]
    BadSignature {
        /// The key it was checked under.
        key_id: String,
    },
}

/// What [`Verified::resolve`] established about one receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// The receipt id this attestation was resolved for.
    pub receipt_id: ReceiptId,
    /// The receipt's signature, verified under the identity's keyset.
    pub signature: SignedBy,
    /// The identity whose keyset the receipt names.
    pub identity: Identity,
}

/// What [`Verified::resolve`] refuses on.
#[derive(Debug, thiserror::Error)]
pub enum AttestationError {
    /// The transport itself could not reach the endpoint.
    #[error("the attestation endpoint could not be reached: {0}")]
    Unreachable(String),
    /// A non-2xx status.
    #[error("the attestation endpoint answered {status}: {message}")]
    Failed {
        /// The HTTP status.
        status: u16,
        /// The endpoint's own body, or its prefix.
        message: String,
    },
    /// A 2xx body that was not the expected shape.
    #[error("the attestation body was not the expected shape: {0}")]
    Malformed(String),
    /// The report's `api_version` is not `aci/1`.
    #[error("the attestation report's api_version is {served:?}, not \"aci/1\"")]
    ReportVersion {
        /// The version the report named, if any.
        served: Option<String>,
    },
    /// The report's evidence is not a TDX quote, the one TEE this module verifies.
    #[error("the attestation report's tee_type is {served:?}, not \"tdx\"")]
    NotTdx {
        /// The TEE the report named, if any.
        served: Option<String>,
    },
    /// The `x-aci-keyset-digest` header and the body's `workload_keyset_digest`
    /// disagree.
    #[error(
        "the attestation response's own header and body disagree on the keyset digest: \
         header {header:?}, body {body:?}"
    )]
    HeaderBodyMismatch {
        /// The `x-aci-keyset-digest` header's value.
        header: String,
        /// The body's `workload_keyset_digest` field.
        body: String,
    },
    /// `workload_keyset_digest` does not equal the digest this module
    /// recomputed over the response's own `workload_keyset` bytes.
    #[error(
        "workload_keyset_digest {claimed:?} does not match the digest recomputed over this \
         response's own workload_keyset ({recomputed:?})"
    )]
    KeysetDigestMismatch {
        /// The digest the response claimed.
        claimed: String,
        /// The digest this module recomputed.
        recomputed: String,
    },
    /// `not_after` is not in the future.
    #[error("the attested keyset expired at {not_after} (now {now})")]
    Expired {
        /// The keyset's own expiry, Unix seconds.
        not_after: i64,
        /// The clock reading this module checked it against, Unix seconds.
        now: i64,
    },
    /// The quote's bytes are not a TDX v4 quote.
    #[error("the attestation report's quote: {0}")]
    QuoteLayout(quote::QuoteLayoutError),
    /// The quote does not bind this keyset and this process's nonce, so the keys
    /// beside it are not the TD's (§3.2).
    #[error("the quote does not bind keyset {keyset_digest:?} with this request's nonce")]
    Unbound {
        /// The keyset the report put beside the quote.
        keyset_digest: String,
    },
    /// Intel's collateral for the quote's platform was refused.
    #[error("Intel's collateral was refused: {0}")]
    Collateral(collateral::CollateralError),
    /// The quote does not verify against Intel's collateral.
    #[error("the quote does not verify: {0}")]
    Quote(attested::QuoteError),
    /// The quote verifies, and its TCB is worse than [`TCB_CEILING`].
    #[error("the quote's TCB is {status:?} ({advisories:?}), worse than {TCB_CEILING:?}")]
    Tcb {
        /// Intel's verdict.
        status: tcb::TcbStatus,
        /// The advisories it names.
        advisories: Vec<String>,
    },
    /// The receipts endpoint could not be reached.
    #[error("the receipts endpoint could not be reached: {0}")]
    ReceiptUnreachable(String),
    /// The receipts endpoint answered a non-2xx status; a receipt is bound to the key
    /// that made the request, so a wrong key is a 401 or 404.
    #[error("the receipts endpoint answered {status}: {message}")]
    ReceiptFailed {
        /// The HTTP status.
        status: u16,
        /// The endpoint's own body, or its prefix.
        message: String,
    },
    /// A 2xx receipt body that was not a JSON object.
    #[error("the receipt was not the expected shape: {0}")]
    ReceiptMalformed(String),
    /// The receipt's `api_version` is not `aci/1`.
    #[error("the receipt's api_version is {served:?}, not \"aci/1\"")]
    ReceiptVersion {
        /// The version the receipt named, if any.
        served: Option<String>,
    },
    /// The receipt served is for a different id than the one asked for.
    #[error("asked for receipt {requested:?}, the endpoint served {served:?}")]
    ReceiptIdMismatch {
        /// The id this module asked for.
        requested: String,
        /// The receipt's own `receipt_id`, if any.
        served: Option<String>,
    },
    /// The receipt binds a different keyset than the identity established for it.
    #[error("the receipt names keyset {receipt:?}, the attestation report {report:?}")]
    ReceiptKeysetMismatch {
        /// The receipt's own `workload_keyset_digest`, if any.
        receipt: Option<String>,
        /// The established identity's.
        report: String,
    },
    /// The reply was served under another keyset than its receipt names.
    #[error("the reply was served under keyset {served:?}, its receipt names {receipt:?}")]
    ServedKeysetMismatch {
        /// The reply's `x-aci-keyset-digest`.
        served: String,
        /// The receipt's `workload_keyset_digest`.
        receipt: String,
    },
    /// The receipt's signature did not verify.
    #[error("the receipt's signature was refused: {0}")]
    Signature(#[from] SignatureError),
}

impl Verified {
    /// Resolve and check one receipt, the only way to build a [`Verified`]: the receipt
    /// under `key`, the provider key that made the request, naming the keyset the reply
    /// was `served` under when it named one; then the identity `identity_of` answers for
    /// that keyset, then the signature under that identity's keyset.
    ///
    /// # Errors
    /// Any check failing, as an [`AttestationError`].
    pub fn resolve(
        http: &dyn Http,
        base_url: &str,
        receipt_id: &ReceiptId,
        key: &Secret,
        served: Option<&str>,
        identity_of: impl FnOnce(&str) -> Result<Identity, AttestationError>,
    ) -> Result<Self, AttestationError> {
        let url = format!(
            "{}/aci/receipts/{}",
            base_url.trim_end_matches('/'),
            receipt_id.as_str()
        );
        let resp = http
            .send(
                Req::get(url)
                    .header("Authorization", format!("Bearer {}", key.expose()))
                    .timeout(DEFAULT_TIMEOUT),
            )
            .map_err(|failed| AttestationError::ReceiptUnreachable(failed.to_string()))?;
        let (status, text) = (resp.status, resp.text());
        if !(200..300).contains(&status) {
            return Err(AttestationError::ReceiptFailed {
                status,
                message: prefix(&text),
            });
        }
        let receipt: Value = serde_json::from_str(&text)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| AttestationError::ReceiptMalformed(prefix(&text)))?;
        let field = |name: &str| receipt.get(name).and_then(Value::as_str).map(str::to_owned);

        if field("api_version").as_deref() != Some("aci/1") {
            return Err(AttestationError::ReceiptVersion {
                served: field("api_version"),
            });
        }
        if field("receipt_id").as_deref() != Some(receipt_id.as_str()) {
            return Err(AttestationError::ReceiptIdMismatch {
                requested: receipt_id.as_str().to_owned(),
                served: field("receipt_id"),
            });
        }
        let named = field("workload_keyset_digest").ok_or_else(|| {
            AttestationError::ReceiptMalformed("the receipt names no keyset".to_owned())
        })?;
        if let Some(served) = served
            && served != named
        {
            return Err(AttestationError::ServedKeysetMismatch {
                served: served.to_owned(),
                receipt: named,
            });
        }
        let identity = identity_of(&named)?;
        if named != identity.workload_keyset_digest {
            return Err(AttestationError::ReceiptKeysetMismatch {
                receipt: Some(named),
                report: identity.workload_keyset_digest,
            });
        }
        let signature = check_signature(identity.keyset(), &receipt)?;
        Ok(Self {
            receipt_id: receipt_id.clone(),
            signature,
            identity,
        })
    }
}

/// Verify a receipt's Ed25519 `signature` over `JCS(receipt − "signature")`, under
/// the `receipt_signing_keys` entry its `key_id` names; `keyset` must already have
/// had its digest checked.
///
/// # Errors
/// The [`SignatureError`] naming which fact failed.
pub fn check_signature(keyset: &Value, receipt: &Value) -> Result<SignedBy, SignatureError> {
    let key_id = receipt
        .get("key_id")
        .and_then(Value::as_str)
        .ok_or(SignatureError::MissingKeyId)?;
    let signature = receipt
        .get("signature")
        .and_then(Value::as_str)
        .ok_or(SignatureError::MissingSignature)?;

    let entries = keyset
        .get("receipt_signing_keys")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let id_of = |entry: &Value| {
        entry
            .get("key_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let entry = entries
        .iter()
        .find(|entry| id_of(entry).as_deref() == Some(key_id))
        .ok_or_else(|| SignatureError::UnknownKeyId {
            key_id: key_id.to_owned(),
            known: entries.iter().filter_map(id_of).collect(),
        })?;

    let algo = entry
        .get("algo")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if algo != "ed25519" {
        return Err(SignatureError::UnsupportedAlgorithm {
            key_id: key_id.to_owned(),
            algo: algo.to_owned(),
        });
    }
    let public_key = entry
        .get("public_key")
        .and_then(Value::as_str)
        .ok_or_else(|| "absent".to_owned())
        .and_then(fixed_hex::<32>)
        .map_err(|reason| SignatureError::PublicKeyEncoding {
            key_id: key_id.to_owned(),
            reason,
        })?;
    let signature = fixed_hex::<64>(signature).map_err(SignatureError::SignatureEncoding)?;

    let mut unsigned = receipt.clone();
    if let Some(members) = unsigned.as_object_mut() {
        members.remove("signature");
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(&canonical(&unsigned), &signature)
        .map_err(|_| SignatureError::BadSignature {
            key_id: key_id.to_owned(),
        })?;
    Ok(SignedBy {
        key_id: key_id.to_owned(),
    })
}

/// Exactly `N` bytes of lowercase hex; uppercase is refused because the spec names
/// only lowercase.
fn fixed_hex<const N: usize>(text: &str) -> Result<[u8; N], String> {
    if text.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(format!("{text:?} is not lowercase hex"));
    }
    let bytes = hex::decode(text).map_err(|failed| format!("{failed}"))?;
    <[u8; N]>::try_from(bytes).map_err(|bytes| format!("{} bytes, not {N}", bytes.len()))
}

/// RFC 8785 JCS, the bytes digested and signed, not the bytes as they arrived.
///
/// Only sorted member names are implemented, which `serde_json` gives because the
/// workspace has no `preserve_order`; ACI documents hold only ASCII names and integers,
/// and a document that broke that would be falsely refused, never falsely passed.
fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_string(value)
        .expect("a Value parsed from a response body serializes back to JSON")
        .into_bytes()
}

fn prefix(text: &str) -> String {
    text.chars().take(200).collect()
}

/// Receipts signed the way the gateway signs them, shared with `openai.rs`'s tests.
#[cfg(test)]
pub(crate) mod testkit {
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::{Identity, canonical};

    /// One receipt-signing key and the `key_id` a keyset lists it under.
    #[derive(Debug)]
    pub(crate) struct Signer {
        pair: Ed25519KeyPair,
        pub(crate) key_id: String,
    }

    impl Signer {
        /// A fresh keypair from the system's randomness.
        pub(crate) fn generate(key_id: &str) -> Self {
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .expect("the system RNG yields a key");
            let pair =
                Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).expect("ring reads its own PKCS#8");
            Self {
                pair,
                key_id: key_id.to_owned(),
            }
        }

        /// The keypair RFC 8032 derives from a 32-byte seed — how the spec's
        /// own test vectors name their keys.
        pub(crate) fn from_seed(seed: &[u8; 32], key_id: &str) -> Self {
            Self {
                pair: Ed25519KeyPair::from_seed_unchecked(seed).expect("any 32 bytes are a seed"),
                key_id: key_id.to_owned(),
            }
        }

        /// This key's `receipt_signing_keys` entry.
        pub(crate) fn key_entry(&self) -> Value {
            json!({
                "key_id": self.key_id,
                "algo": "ed25519",
                "public_key": hex::encode(self.pair.public_key().as_ref()),
            })
        }

        /// `receipt` with `signature` set to this key's signature over its
        /// JCS form without `signature`.
        pub(crate) fn sign(&self, receipt: &Value) -> Value {
            let mut signed = receipt.clone();
            let members = signed.as_object_mut().expect("a receipt is an object");
            members.remove("signature");
            let signature = self.pair.sign(&canonical(&Value::Object(members.clone())));
            members.insert(
                "signature".to_owned(),
                Value::String(hex::encode(signature.as_ref())),
            );
            signed
        }
    }

    /// A keyset listing `entries`, expiring at `not_after`.
    pub(crate) fn keyset(entries: &[Value], not_after: i64) -> Value {
        json!({ "subject": null, "not_after": not_after, "receipt_signing_keys": entries })
    }

    /// `sha256:` over the keyset's JCS form.
    pub(crate) fn digest(keyset: &Value) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(canonical(keyset))))
    }

    /// The identity a verified report would give `keyset`: the live quote's attested
    /// TD, so a receipt test exercises the signature and never a quote it cannot sign.
    pub(crate) fn identity(keyset: &Value, repo_commit: Option<&str>) -> Identity {
        use super::collateral::{Collateral, fixture};
        let collateral = Collateral::verify(&fixture::fetched(), fixture::NOW)
            .expect("the saved collateral verifies");
        let not_after = keyset["not_after"]
            .as_i64()
            .expect("a keyset has an expiry");
        Identity {
            workload_keyset_digest: digest(keyset),
            keyset: keyset.clone(),
            not_after,
            image_digest: None,
            repo_commit: repo_commit.map(str::to_owned),
            repo_url: Some(super::KNOWN_REPO_URL.to_owned()),
            attested: super::attested::Attested::verify(
                &super::attested::fixture::quote(),
                &collateral,
                fixture::NOW,
            )
            .expect("the live quote verifies"),
            // The saved collateral's window closes in November 2026; a test reading at
            // the wall clock must not stop passing then.
            valid_until: not_after,
        }
    }

    /// A report naming `keyset` beside a quote that is not its own, its members
    /// deliberately unsorted on the wire so digesting the raw bytes would refuse it.
    pub(crate) fn report(keyset: &Value, repo_commit: &str) -> String {
        let entries = serde_json::to_string(&keyset["receipt_signing_keys"]).expect("serializes");
        format!(
            r#"{{"api_version":"aci/1","workload_keyset_digest":{digest:?},"attestation":{{"tee_type":"tdx","workload_keyset":{{"subject":null,"not_after":{not_after},"receipt_signing_keys":{entries}}},"source_provenance":{{"repo_url":"https://github.com/Dstack-TEE/private-ai-gateway.git","repo_commit":{repo_commit:?},"image_digest":null}},"evidence":{{"quote":"00"}}}}}}"#,
            digest = digest(keyset),
            not_after = keyset["not_after"],
        )
    }

    /// An unsigned receipt document for `receipt_id`, binding the keyset
    /// `keyset_digest` and naming `key_id`.
    pub(crate) fn receipt(receipt_id: &str, keyset_digest: &str, key_id: &str) -> Value {
        json!({
            "api_version": "aci/1",
            "receipt_id": receipt_id,
            "chat_id": "chatcmpl-test",
            "model": "demo-model",
            "workload_keyset_digest": keyset_digest,
            "endpoint": "/v1/chat/completions",
            "method": "POST",
            "served_at": 1_750_000_000,
            "event_log": [
                { "type": "request.received", "body_hash": format!("sha256:{}", "11".repeat(32)) },
                { "type": "response.returned", "body_hash": format!("sha256:{}", "22".repeat(32)) },
            ],
            "key_id": key_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::testkit::{Signer, digest, identity, keyset, receipt};
    use super::*;
    use crate::http::{Resp, Scripted};

    const BASE: &str = "https://inference.example/v1";

    fn id() -> ReceiptId {
        ReceiptId::new("rcpt-test")
    }

    fn key() -> Secret {
        Secret::new("sk-test")
    }

    /// A keyset and a receipt signed under it; every refusal test changes one thing.
    struct World {
        signer: Signer,
        keyset: Value,
        digest: String,
        receipt: Value,
    }

    impl World {
        fn new() -> Self {
            let signer = Signer::generate("dstack-kms-receipt-ed25519-v1");
            let keyset = keyset(&[signer.key_entry()], 2_000_000_000);
            let digest = digest(&keyset);
            let receipt = signer.sign(&receipt(id().as_str(), &digest, &signer.key_id));
            Self {
                signer,
                keyset,
                digest,
                receipt,
            }
        }

        fn serving(receipt: &Value) -> Scripted {
            Scripted::always(Ok(Resp::new(200, receipt.to_string())))
        }

        /// Resolve `receipt`, answering every keyset with this world's identity.
        fn resolve(&self, receipt: &Value) -> Result<Verified, AttestationError> {
            Verified::resolve(&Self::serving(receipt), BASE, &id(), &key(), None, |_| {
                Ok(identity(&self.keyset, None))
            })
        }
    }

    /// The gateway spec's own test vector (seed 32 × `0x02`, key `receipt-1`); Ed25519
    /// is deterministic, so matching its signature proves the signing input byte for byte.
    #[test]
    fn the_specs_own_receipt_vector_verifies() {
        const SERVED: &str = r#"{"api_version":"aci/1","chat_id":"chatcmpl-123","endpoint":"/v1/chat/completions","event_log":[{"body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b","type":"request.received"},{"body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b","type":"request.forwarded"},{"model_id":"demo-model","required":true,"result":"verified","session_id":"95ad1cb4dd25445808c2e9d116caf420b05703730b506395e8fc1ca6faeae28f","type":"upstream.verified"},{"body_hash":"sha256:dedfffe5b14d031b8e2c01996d021a15293cb7c63b56be7e4be9e89b6f0a5f61","type":"response.returned"}],"key_id":"receipt-1","method":"POST","model":"demo-model","receipt_id":"rcpt-0001","served_at":1750000000,"signature":"d5b005e093bde3b577faf270b7184b09e169cacb0ecb206b103bd2581f997db03da616175454b063323a23ac1dc68f1ce506c2a6eba8aa0561d5e724f0b80c03","workload_keyset_digest":"sha256:53a5cd44b30dcc51999754c719f2628a041f174ecbf9662a6f8e898a10cd9371"}"#;
        const PUBLIC_KEY: &str = "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394";

        let signer = Signer::from_seed(&[0x02; 32], "receipt-1");
        assert_eq!(signer.key_entry()["public_key"], PUBLIC_KEY);

        let served: Value = serde_json::from_str(SERVED).expect("the vector is JSON");
        assert_eq!(
            signer.sign(&served),
            served,
            "the same signing input, byte for byte"
        );

        let keyset = keyset(&[signer.key_entry()], 1_800_000_000);
        let verified = check_signature(&keyset, &served).expect("the spec's receipt verifies");
        assert_eq!(verified.key_id(), "receipt-1");
    }

    #[test]
    fn a_receipt_signed_by_a_listed_key_verifies_in_any_encoding() {
        let world = World::new();
        let signed = check_signature(&world.keyset, &world.receipt).expect("it verifies");
        assert_eq!(signed.key_id(), "dstack-kms-receipt-ed25519-v1");

        let pretty = serde_json::to_string_pretty(&world.receipt).expect("serializes");
        let reparsed: Value = serde_json::from_str(&pretty).expect("parses");
        check_signature(&world.keyset, &reparsed).expect("another encoding still verifies");
    }

    #[test]
    fn one_tampered_byte_is_a_bad_signature() {
        let world = World::new();
        let text = world.receipt.to_string();
        let tampered_log = text.replacen(&"22".repeat(32), &format!("23{}", "22".repeat(31)), 1);
        assert_ne!(
            tampered_log, text,
            "the replacement must actually have matched"
        );

        let signature = world.receipt["signature"]
            .as_str()
            .expect("signed")
            .to_owned();
        let flipped = format!(
            "{}{}",
            if signature.starts_with('0') { "1" } else { "0" },
            &signature[1..]
        );
        let tampered_signature = text.replacen(&signature, &flipped, 1);

        for tampered in [tampered_log, tampered_signature] {
            let tampered: Value = serde_json::from_str(&tampered).expect("still JSON");
            assert_eq!(
                check_signature(&world.keyset, &tampered),
                Err(SignatureError::BadSignature {
                    key_id: "dstack-kms-receipt-ed25519-v1".to_owned()
                })
            );
        }
    }

    #[test]
    fn an_unknown_key_id_is_refused_by_name() {
        let world = World::new();
        let stranger = Signer::generate("someone-else");
        let forged = stranger.sign(&receipt(id().as_str(), &world.digest, "someone-else"));
        assert_eq!(
            check_signature(&world.keyset, &forged),
            Err(SignatureError::UnknownKeyId {
                key_id: "someone-else".to_owned(),
                known: vec!["dstack-kms-receipt-ed25519-v1".to_owned()],
            })
        );
    }

    #[test]
    fn a_strangers_signature_under_a_listed_id_is_bad() {
        let world = World::new();
        let stranger = Signer::generate("unused");
        let forged = stranger.sign(&receipt(id().as_str(), &world.digest, &world.signer.key_id));
        assert!(matches!(
            check_signature(&world.keyset, &forged),
            Err(SignatureError::BadSignature { .. })
        ));
    }

    #[test]
    fn encoding_defects_are_named() {
        let world = World::new();
        let mut entry = world.signer.key_entry();

        let mut missing_id = world.receipt.clone();
        missing_id.as_object_mut().expect("object").remove("key_id");
        assert_eq!(
            check_signature(&world.keyset, &missing_id),
            Err(SignatureError::MissingKeyId)
        );

        let mut missing_signature = world.receipt.clone();
        missing_signature
            .as_object_mut()
            .expect("object")
            .remove("signature");
        assert_eq!(
            check_signature(&world.keyset, &missing_signature),
            Err(SignatureError::MissingSignature)
        );

        entry["algo"] = json!("ecdsa");
        assert!(matches!(
            check_signature(&keyset(&[entry.clone()], 2_000_000_000), &world.receipt),
            Err(SignatureError::UnsupportedAlgorithm { algo, .. }) if algo == "ecdsa"
        ));

        let public_key = world.signer.key_entry()["public_key"]
            .as_str()
            .expect("hex")
            .to_owned();
        for bad in [public_key[2..].to_owned(), public_key.to_uppercase()] {
            entry = world.signer.key_entry();
            entry["public_key"] = json!(bad);
            assert!(matches!(
                check_signature(&keyset(&[entry], 2_000_000_000), &world.receipt),
                Err(SignatureError::PublicKeyEncoding { .. })
            ));
        }

        let mut odd = world.receipt.clone();
        odd["signature"] = json!("abc");
        assert!(matches!(
            check_signature(&world.keyset, &odd),
            Err(SignatureError::SignatureEncoding(_))
        ));
    }

    /// The receipt is fetched with the key, and the identity is asked for by the keyset
    /// the receipt names.
    #[test]
    fn a_consistent_receipt_verifies() {
        let world = World::new();
        let transport = World::serving(&world.receipt);
        let mut asked = Vec::new();
        let attested = Verified::resolve(
            &transport,
            BASE,
            &id(),
            &key(),
            Some(&world.digest),
            |named| {
                asked.push(named.to_owned());
                Ok(identity(&world.keyset, None))
            },
        )
        .expect("a consistent receipt verifies");
        assert_eq!(asked, vec![world.digest.clone()]);
        assert_eq!(attested.identity.workload_keyset_digest, world.digest);
        assert_eq!(attested.signature.key_id(), "dstack-kms-receipt-ed25519-v1");
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].url, format!("{BASE}/aci/receipts/rcpt-test"));
        assert_eq!(
            sent[0].header_value("authorization"),
            Some("Bearer sk-test")
        );
    }

    #[test]
    fn a_tampered_receipt_refuses_resolution() {
        let world = World::new();
        let mut tampered = world.receipt.clone();
        tampered["served_at"] = json!(1_750_000_001);
        assert!(matches!(
            world.resolve(&tampered),
            Err(AttestationError::Signature(
                SignatureError::BadSignature { .. }
            ))
        ));
    }

    #[test]
    fn a_receipt_for_another_id_is_refused() {
        let world = World::new();
        let other = world
            .signer
            .sign(&receipt("rcpt-other", &world.digest, &world.signer.key_id));
        match world.resolve(&other).expect_err("another id refuses") {
            AttestationError::ReceiptIdMismatch { requested, served } => {
                assert_eq!(requested, "rcpt-test");
                assert_eq!(served.as_deref(), Some("rcpt-other"));
            }
            other => panic!("expected ReceiptIdMismatch, got {other:?}"),
        }
    }

    /// The identity answered for a receipt's keyset is another keyset's: the gateway
    /// rotated between the reply and the report, or lied in one of them.
    #[test]
    fn a_receipt_binding_another_keyset_is_refused() {
        let world = World::new();
        let elsewhere = format!("sha256:{}", "00".repeat(32));
        let other = world
            .signer
            .sign(&receipt(id().as_str(), &elsewhere, &world.signer.key_id));
        match world.resolve(&other).expect_err("another keyset refuses") {
            AttestationError::ReceiptKeysetMismatch { receipt, report } => {
                assert_eq!(receipt.as_deref(), Some(elsewhere.as_str()));
                assert_eq!(report, world.digest);
            }
            other => panic!("expected ReceiptKeysetMismatch, got {other:?}"),
        }
    }

    /// The reply's header and its receipt are two channels for one keyset.
    #[test]
    fn a_reply_served_under_another_keyset_than_its_receipt_is_refused() {
        let world = World::new();
        let failed = Verified::resolve(
            &World::serving(&world.receipt),
            BASE,
            &id(),
            &key(),
            Some("sha256:another"),
            |_| panic!("no identity is asked for"),
        );
        assert!(matches!(
            failed,
            Err(AttestationError::ServedKeysetMismatch { served, receipt })
                if served == "sha256:another" && receipt == world.digest
        ));
    }

    #[test]
    fn another_api_version_is_refused() {
        let world = World::new();
        let mut unsigned = receipt(id().as_str(), &world.digest, &world.signer.key_id);
        unsigned["api_version"] = json!("aci/2");
        assert!(matches!(
            world.resolve(&world.signer.sign(&unsigned)),
            Err(AttestationError::ReceiptVersion { served: Some(v) }) if v == "aci/2"
        ));
    }
}

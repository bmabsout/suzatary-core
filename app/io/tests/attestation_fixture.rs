//! The gateway's identity established from a saved report and Intel's saved collateral,
//! through the public API alone.
//!
//! Real answers, not hand-built ones, are the evidence: that this crate's JCS agrees
//! with the gateway's digest, that the quote binds the keyset with the nonce it was
//! asked with, and that it verifies to Intel's root. No receipt signed by the live key
//! is held, so the signature check is exercised only as far as refusing one the key did
//! not make.

use serde_json::Value;
use sha2::{Digest, Sha256};

use suzatary_io::attestation::collateral::{Collateral, INTEL_PCS, INTEL_ROOT_CRL};
use suzatary_io::attestation::tcb::TcbStatus;
use suzatary_io::attestation::{
    AttestationError, Identity, Nonce, SignatureError, check_signature,
};
use suzatary_io::http::{Resp, Scripted};

const FIXTURE: &str = "tests/fixtures/tdx-2026-10-03";

/// The digest the gateway reported, the same on 2026-09-21 and 2026-10-03.
const DIGEST: &str = "sha256:ef8a03c0c5e349310394c0d5865f766f8a1258f1acdb5fcb6ed2ebcd757e5f5a";

/// 05:30 UTC on 2026-10-03, inside every saved document's window.
const NOW: i64 = 1_791_005_400;

fn saved(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{FIXTURE}/{name}", env!("CARGO_MANIFEST_DIR")))
        .expect("the fixture reads")
}

fn text(name: &str) -> String {
    String::from_utf8(saved(name)).expect("the fixture is text")
}

/// The nonce the report was asked with: a sentence's SHA-256.
fn nonce() -> Nonce {
    Nonce(Sha256::digest(b"suzatary receiptsignature fixture 2026-10-03").into())
}

/// The gateway and Intel, answering as they did that morning.
fn world() -> Scripted {
    Scripted::new(|req| {
        let url = req.url.as_str();
        Ok(
            if url.starts_with("https://inference.example/v1/aci/attestation?nonce=") {
                Resp::new(200, saved("report.json")).with_header("x-aci-keyset-digest", DIGEST)
            } else if url == format!("{INTEL_PCS}/tdx/certification/v4/tcb?fmspc=20a06f000000") {
                Resp::new(200, saved("tcb-info.json"))
                    .with_header("TCB-Info-Issuer-Chain", text("tcb-info.issuer-chain"))
            } else if url == format!("{INTEL_PCS}/tdx/certification/v4/qe/identity") {
                Resp::new(200, saved("qe-identity.json")).with_header(
                    "SGX-Enclave-Identity-Issuer-Chain",
                    text("qe-identity.issuer-chain"),
                )
            } else if url
                == format!("{INTEL_PCS}/sgx/certification/v4/pckcrl?ca=platform&encoding=der")
            {
                Resp::new(200, saved("pck-crl.der"))
            } else if url == INTEL_ROOT_CRL {
                Resp::new(200, saved("root-ca-crl.der"))
            } else {
                Resp::new(404, "")
            },
        )
    })
}

fn establish(nonce: &Nonce) -> Result<Identity, AttestationError> {
    let world = world();
    Identity::establish(
        &world,
        "https://inference.example/v1",
        nonce,
        |platform| Collateral::fetch(&world, platform, NOW).map_err(AttestationError::Collateral),
        NOW,
    )
}

/// A guard on the fixture, not the verifier: it names the file when a byte moved.
#[test]
fn the_fixture_keyset_still_hashes_to_the_recorded_digest() {
    let body: Value = serde_json::from_slice(&saved("report.json")).expect("JSON");
    let keyset = serde_json::to_string(&body["attestation"]["workload_keyset"]).expect("JSON");
    assert_eq!(
        format!("sha256:{}", hex::encode(Sha256::digest(keyset.as_bytes()))),
        DIGEST
    );
}

#[test]
fn the_live_report_establishes_the_gateways_identity() {
    let identity = establish(&nonce()).expect("the gateway's identity is established");
    assert_eq!(identity.workload_keyset_digest, DIGEST);
    assert_eq!(identity.attested.tcb().status, TcbStatus::UpToDate);
    assert_eq!(
        identity.repo_commit.as_deref(),
        Some("8d0a666a2418898a8c823a9af49a634edd122a64")
    );
    assert_eq!(
        identity.repo_url.as_deref(),
        Some("https://github.com/Dstack-TEE/private-ai-gateway.git")
    );
    assert!(
        identity.image_digest.is_none(),
        "the gateway reports no image digest"
    );
    assert!(identity.valid_until > NOW);
}

/// The saved quote answers one nonce; any other asker's is a replay to it.
#[test]
fn the_live_report_is_unbound_under_any_other_nonce() {
    assert!(matches!(
        establish(&Nonce([7; 32])),
        Err(AttestationError::Unbound { keyset_digest }) if keyset_digest == DIGEST
    ));
}

/// `BadSignature` rather than an encoding refusal is what proves the live key's shape
/// and this module's reading of it agree.
#[test]
fn the_live_keyset_reaches_ed25519_and_refuses_a_signature_it_did_not_make() {
    let identity = establish(&nonce()).expect("established");
    let receipt = |key_id: &str| {
        serde_json::json!({
            "api_version": "aci/1",
            "receipt_id": "rcpt-fixture",
            "workload_keyset_digest": DIGEST,
            "key_id": key_id,
            "signature": "00".repeat(64),
        })
    };
    assert_eq!(
        check_signature(identity.keyset(), &receipt("dstack-kms-receipt-ed25519-v1")),
        Err(SignatureError::BadSignature {
            key_id: "dstack-kms-receipt-ed25519-v1".to_owned()
        })
    );
    assert_eq!(
        check_signature(identity.keyset(), &receipt("receipt-1")),
        Err(SignatureError::UnknownKeyId {
            key_id: "receipt-1".to_owned(),
            known: vec!["dstack-kms-receipt-ed25519-v1".to_owned()],
        })
    );
}

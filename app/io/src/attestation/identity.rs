//! A workload's identity, established (ACI §9.1): a keyset whose digest a genuine TD
//! bound, with this process's nonce, into a quote that verifies to Intel's root.

use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::attested::Attested;
use super::collateral::Collateral;
use super::pck::Platform;
use super::report::Report;
use super::tcb::TcbStatus;
use super::{AttestationError, KEYSET_DIGEST_HEADER, prefix};
use crate::http::{Http, Req};

/// The worst TCB status a quote may carry and still identify a workload: software
/// hardening is the TD's own to apply; a configuration change or an update is the
/// platform's, and until it is made the quote's protection is not the one claimed.
pub const TCB_CEILING: TcbStatus = TcbStatus::SWHardeningNeeded;

/// The report is some fifty kilobytes, not a stream.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A fresh 32-byte challenge, so the quote that binds it postdates the asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nonce(pub [u8; 32]);

impl Nonce {
    /// Thirty-two bytes from the system's randomness.
    ///
    /// # Panics
    /// When the system has no randomness to give, which no supported system lacks.
    #[must_use]
    pub fn fresh() -> Self {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("the system's randomness answers");
        Self(bytes)
    }

    /// The §3.2 statement for `keyset_digest`, whose SHA-256 the quote must bind.
    fn statement(&self, keyset_digest: &str) -> String {
        format!(
            r#"{{"keyset_digest":"{keyset_digest}","nonce":"{}","purpose":"aci.report_data.v1"}}"#,
            hex::encode(self.0)
        )
    }
}

/// An established identity; [`Identity::verify`] is the only constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The keyset's digest, which a receipt names.
    pub workload_keyset_digest: String,
    pub(super) keyset: Value,
    /// The keyset's own expiry, Unix seconds.
    pub not_after: i64,
    /// The gateway's `image_digest`, stated beside the quote; no measurement in it is
    /// compared with it.
    pub image_digest: Option<String>,
    /// The gateway's `repo_commit`, likewise.
    pub repo_commit: Option<String>,
    /// The gateway's `repo_url`, likewise.
    pub repo_url: Option<String>,
    /// The TD and its TCB, as Intel's root attests them.
    pub attested: Attested,
    /// The earlier of the keyset's expiry and the collateral's next update: past it,
    /// the identity is established again.
    pub valid_until: i64,
}

impl Identity {
    /// Believe `report`: its keyset unexpired, its digest bound with `nonce` into
    /// `quote`, the quote verified against `collateral`, and its TCB at most
    /// [`TCB_CEILING`].
    ///
    /// # Errors
    /// The first of those that fails.
    pub fn verify(
        report: Report,
        quote: &super::quote::Quote,
        nonce: &Nonce,
        collateral: &Collateral,
        now: i64,
    ) -> Result<Self, AttestationError> {
        let not_after = report
            .keyset
            .get("not_after")
            .and_then(Value::as_i64)
            .ok_or_else(|| AttestationError::Malformed(prefix(&report.keyset.to_string())))?;
        if not_after <= now {
            return Err(AttestationError::Expired { not_after, now });
        }
        let statement = Sha256::digest(nonce.statement(&report.workload_keyset_digest));
        let bound = &quote.td.report_data;
        if bound[..32] != statement[..] || bound[32..] != [0; 32] {
            return Err(AttestationError::Unbound {
                keyset_digest: report.workload_keyset_digest,
            });
        }
        let attested = Attested::verify(quote, collateral, now).map_err(AttestationError::Quote)?;
        let tcb = attested.tcb();
        if !tcb.status.at_most(TCB_CEILING) {
            return Err(AttestationError::Tcb {
                status: tcb.status,
                advisories: tcb.advisories.iter().cloned().collect(),
            });
        }
        Ok(Self {
            workload_keyset_digest: report.workload_keyset_digest,
            keyset: report.keyset,
            not_after,
            image_digest: report.image_digest,
            repo_commit: report.repo_commit,
            repo_url: report.repo_url,
            attested,
            valid_until: not_after.min(collateral.valid_until),
        })
    }

    /// Fetch `GET {base_url}/aci/attestation?nonce=` and believe it, with the
    /// collateral `collateral_of` answers for the quote's platform.
    ///
    /// # Errors
    /// The endpoint failing, or any check of [`Report::parse`] and [`Identity::verify`].
    pub fn establish(
        http: &dyn Http,
        base_url: &str,
        nonce: &Nonce,
        collateral_of: impl FnOnce(Platform) -> Result<Collateral, AttestationError>,
        now: i64,
    ) -> Result<Self, AttestationError> {
        let url = format!(
            "{}/aci/attestation?nonce={}",
            base_url.trim_end_matches('/'),
            hex::encode(nonce.0)
        );
        let resp = http
            .send(Req::get(url).timeout(TIMEOUT))
            .map_err(|failed| AttestationError::Unreachable(failed.to_string()))?;
        let text = resp.text();
        if !resp.ok() {
            return Err(AttestationError::Failed {
                status: resp.status,
                message: prefix(&text),
            });
        }
        let report = Report::parse(&text, resp.header(KEYSET_DIGEST_HEADER))?;
        let quote = report.quote()?;
        let collateral = collateral_of(quote.pck.platform)?;
        Self::verify(report, &quote, nonce, &collateral, now)
    }

    /// The keyset whose digest a genuine TD bound.
    #[must_use]
    pub fn keyset(&self) -> &Value {
        &self.keyset
    }
}

#[cfg(test)]
mod tests {
    use super::super::attested::fixture::{REPORT, quote};
    use super::super::collateral::{Collateral, fixture};
    use super::super::testkit::{Signer, keyset, report};
    use super::*;
    use crate::http::{Resp, Scripted};

    /// The nonce the fixture report was asked with: a sentence's SHA-256, so it is
    /// visibly nobody's secret.
    fn nonce() -> Nonce {
        Nonce(Sha256::digest(b"suzatary receiptsignature fixture 2026-10-03").into())
    }

    fn collateral() -> Collateral {
        Collateral::verify(&fixture::fetched(), fixture::NOW)
            .expect("the saved collateral verifies")
    }

    fn live() -> Report {
        Report::parse(REPORT, None).expect("the live report reads")
    }

    #[test]
    fn the_live_report_identifies_the_gateway() {
        let identity = Identity::verify(live(), &quote(), &nonce(), &collateral(), fixture::NOW)
            .expect("the live report verifies");
        assert_eq!(
            identity.workload_keyset_digest,
            "sha256:ef8a03c0c5e349310394c0d5865f766f8a1258f1acdb5fcb6ed2ebcd757e5f5a"
        );
        assert_eq!(
            identity.repo_commit.as_deref(),
            Some("8d0a666a2418898a8c823a9af49a634edd122a64")
        );
        assert_eq!(identity.attested.tcb().status, TcbStatus::UpToDate);
        assert_eq!(identity.not_after, 1_792_250_755);
        assert_eq!(
            identity.valid_until, identity.not_after,
            "the keyset expires before Intel's next update"
        );
        assert!(identity.not_after < collateral().valid_until);
    }

    /// Another nonce is another statement: a replayed quote, or one for another asker.
    #[test]
    fn another_nonce_is_unbound() {
        assert!(matches!(
            Identity::verify(
                live(),
                &quote(),
                &Nonce([0; 32]),
                &collateral(),
                fixture::NOW
            ),
            Err(AttestationError::Unbound { .. })
        ));
    }

    /// Keys beside a quote that did not bind them are not the TD's (§3.2), however
    /// consistent the report is with itself.
    #[test]
    fn a_self_consistent_keyset_the_quote_did_not_bind_is_unbound() {
        let forged = keyset(
            &[Signer::generate("dstack-kms-receipt-ed25519-v1").key_entry()],
            1_792_250_755,
        );
        let report = Report::parse(
            &report(&forged, "8d0a666a2418898a8c823a9af49a634edd122a64"),
            None,
        )
        .expect("a self-consistent report reads");
        assert!(matches!(
            Identity::verify(report, &quote(), &nonce(), &collateral(), fixture::NOW),
            Err(AttestationError::Unbound { .. })
        ));
    }

    #[test]
    fn an_expired_keyset_is_refused_before_anything_else() {
        let expired = keyset(&[], 500);
        let report = Report::parse(&report(&expired, &"ab".repeat(20)), None).expect("reads");
        assert!(matches!(
            Identity::verify(report, &quote(), &nonce(), &collateral(), fixture::NOW),
            Err(AttestationError::Expired { not_after: 500, .. })
        ));
    }

    #[test]
    fn a_tcb_above_the_ceiling_is_refused_with_its_advisories() {
        let mut behind = collateral();
        for level in &mut behind.tcb_info.tcb_levels[..1] {
            level.tcb.pcesvn = 14;
        }
        match Identity::verify(live(), &quote(), &nonce(), &behind, fixture::NOW) {
            Err(AttestationError::Tcb { status, advisories }) => {
                assert_eq!(status, TcbStatus::OutOfDate);
                assert!(advisories.contains(&"INTEL-SA-01192".to_owned()));
            }
            other => panic!("expected Tcb, got {other:?}"),
        }
    }

    /// The whole path over a scripted gateway and Intel: the report asked with the nonce,
    /// the collateral of the platform the quote names.
    #[test]
    fn an_identity_is_established_from_the_gateway_and_intel() {
        let intel = fixture::intel();
        let transport = Scripted::new(move |req| {
            if req
                .url
                .starts_with("https://inference.example/v1/aci/attestation")
            {
                Ok(Resp::new(200, REPORT).with_header(
                    KEYSET_DIGEST_HEADER,
                    "sha256:ef8a03c0c5e349310394c0d5865f766f8a1258f1acdb5fcb6ed2ebcd757e5f5a",
                ))
            } else {
                intel(req)
            }
        });
        let identity = Identity::establish(
            &transport,
            "https://inference.example/v1/",
            &nonce(),
            |platform| {
                Collateral::fetch(&transport, platform, fixture::NOW)
                    .map_err(AttestationError::Collateral)
            },
            fixture::NOW,
        )
        .expect("the gateway's identity is established");
        assert_eq!(identity.attested.tcb().status, TcbStatus::UpToDate);
        let urls: Vec<String> = transport.sent().into_iter().map(|req| req.url).collect();
        assert_eq!(
            urls[0],
            format!(
                "https://inference.example/v1/aci/attestation?nonce={}",
                hex::encode(nonce().0)
            )
        );
        assert_eq!(urls.len(), 5, "the report and Intel's four documents");
    }
}

//! What Intel's root says of a TD: a quote whose every signature chains to the root
//! through current collateral, and the TCB verdict its platform, TDX module and
//! quoting enclave reach.

use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
use sha2::{Digest, Sha256};

use super::chain;
use super::collateral::{Collateral, Hex, IsvLevel, ModuleIdentity, TcbLevel};
use super::quote::{Quote, TdReport};
use super::tcb::Tcb;

/// A TD's report and its TCB verdict; [`Attested::verify`] is the only constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attested {
    td: TdReport,
    tcb: Tcb,
}

/// Why a quote did not verify.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuoteError {
    /// The collateral is not current at the instant asked.
    #[error("the collateral is current until {valid_until}, not at {now}")]
    StaleCollateral {
        /// The collateral's earliest next update.
        valid_until: i64,
        /// The instant asked.
        now: i64,
    },
    /// The collateral is for another platform.
    #[error("the collateral's {0} is not the PCK certificate's")]
    OtherPlatform(&'static str),
    /// The PCK chain does not verify to Intel's root, or is revoked.
    #[error("the PCK certificate does not verify to Intel's root: {0:?}")]
    PckChain(webpki::Error),
    /// The PCK key did not sign the QE report.
    #[error("the QE report's signature does not verify under the PCK certificate")]
    QeReportSignature,
    /// The QE report does not bind the attestation key.
    #[error("the QE report does not bind the attestation key")]
    AttestationKeyUnbound,
    /// The QE is not the one Intel's QE identity names.
    #[error("the quoting enclave's {0} is not the one Intel's QE identity names")]
    QeIdentity(&'static str),
    /// The attestation key did not sign the TD report.
    #[error("the quote's signature does not verify under its attestation key")]
    QuoteSignature,
    /// The TDX module is not one Intel's TCB info names.
    #[error("the TDX module's {0} is not one Intel's TCB info names")]
    Module(&'static str),
    /// A component's TCB is below every level Intel lists.
    #[error("the {0}'s TCB is below every level Intel lists")]
    BelowEveryLevel(&'static str),
    /// The TD runs with debug on, or without `SEPT_VE_DISABLE`, so its host can read or
    /// steer it.
    #[error("the TD's attributes {0} allow its host in (debug on, or SEPT_VE_DISABLE off)")]
    TdAttributes(String),
}

/// `TDATTRIBUTES` bit 0.
const DEBUG: u64 = 1;
/// `TDATTRIBUTES` bit 28.
const SEPT_VE_DISABLE: u64 = 1 << 28;

impl Attested {
    /// The TD's report, signed by a genuine Intel platform.
    #[must_use]
    pub fn td(&self) -> &TdReport {
        &self.td
    }

    /// The verdict its platform, TDX module and quoting enclave reach.
    #[must_use]
    pub fn tcb(&self) -> &Tcb {
        &self.tcb
    }

    /// Verify `quote` against `collateral` at `now`, Intel's DCAP appraisal: the PCK
    /// chain, the QE report and its identity, the quote's signature, then the TCB
    /// levels each component reaches, joined.
    ///
    /// # Errors
    /// The [`QuoteError`] naming the first check that failed.
    pub fn verify(quote: &Quote, collateral: &Collateral, now: i64) -> Result<Self, QuoteError> {
        if now >= collateral.valid_until {
            return Err(QuoteError::StaleCollateral {
                valid_until: collateral.valid_until,
                now,
            });
        }
        let tcb_info = &collateral.tcb_info;
        if tcb_info.fmspc.0 != quote.pck.platform.fmspc {
            return Err(QuoteError::OtherPlatform("FMSPC"));
        }
        if tcb_info.pce_id.0 != quote.pck.pce_id {
            return Err(QuoteError::OtherPlatform("PCE id"));
        }

        let pck = chain::verified_leaf(&quote.pck_chain, &collateral.crls, now)
            .map_err(QuoteError::PckChain)?;
        chain::signed_by(&pck, &quote.qe.bytes, &quote.qe_signature)
            .map_err(|_| QuoteError::QeReportSignature)?;
        let bound = Sha256::digest([&quote.attestation_key[..], &quote.qe_auth_data].concat());
        if quote.qe.report_data[..32] != bound[..] || quote.qe.report_data[32..] != [0; 32] {
            return Err(QuoteError::AttestationKeyUnbound);
        }
        let qe = qe_verdict(quote, collateral)?;
        let key = [&[0x04][..], &quote.attestation_key].concat();
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, key)
            .verify(&quote.signed, &quote.signature)
            .map_err(|_| QuoteError::QuoteSignature)?;

        let td = &quote.td;
        let attributes = u64::from_le_bytes(td.td_attributes);
        if attributes & DEBUG != 0 || attributes & SEPT_VE_DISABLE == 0 {
            return Err(QuoteError::TdAttributes(hex::encode(td.td_attributes)));
        }
        let platform = reached(
            &tcb_info.tcb_levels,
            |level: &TcbLevel, other: &TcbLevel| {
                let (a, b) = (&level.tcb, &other.tcb);
                at_least(&b.sgxtcbcomponents.0, &a.sgxtcbcomponents.0)
                    && b.pcesvn >= a.pcesvn
                    && at_least(&b.tdxtcbcomponents.0, &a.tdxtcbcomponents.0)
            },
            |level| {
                at_least(&quote.pck.cpu_svn, &level.tcb.sgxtcbcomponents.0)
                    && quote.pck.pce_svn >= level.tcb.pcesvn
                    && at_least(&td.tee_tcb_svn, &level.tcb.tdxtcbcomponents.0)
            },
            |level| level.verdict.tcb(),
        )
        .ok_or(QuoteError::BelowEveryLevel("platform"))?;

        Ok(Self {
            td: td.clone(),
            tcb: [platform, module_verdict(td, collateral)?, qe]
                .into_iter()
                .collect(),
        })
    }
}

/// The quoting enclave against Intel's QE identity: its signer, product, masked
/// `MISCSELECT` and attributes, then the level its security version reaches.
fn qe_verdict(quote: &Quote, collateral: &Collateral) -> Result<Tcb, QuoteError> {
    let identity = &collateral.qe_identity;
    let qe = &quote.qe;
    if qe.mr_signer != identity.mrsigner.0 {
        return Err(QuoteError::QeIdentity("signer"));
    }
    if qe.isv_prod_id != identity.isvprodid {
        return Err(QuoteError::QeIdentity("product id"));
    }
    let mask = u32::from_le_bytes(identity.miscselect_mask.0);
    if qe.misc_select & mask != u32::from_le_bytes(identity.miscselect.0) & mask {
        return Err(QuoteError::QeIdentity("MISCSELECT"));
    }
    if !masked_equal(
        &qe.attributes,
        &identity.attributes,
        &identity.attributes_mask,
    ) {
        return Err(QuoteError::QeIdentity("attributes"));
    }
    by_security_version(&identity.tcb_levels, qe.isv_svn)
        .ok_or(QuoteError::BelowEveryLevel("quoting enclave"))
}

/// The TDX module against the TCB info: the identity its major version names (or the
/// base module, for version 0), and the level its minor version reaches; the base
/// module carries no levels, so a match is its whole verdict.
fn module_verdict(td: &TdReport, collateral: &Collateral) -> Result<Tcb, QuoteError> {
    let tcb_info = &collateral.tcb_info;
    let [minor, major, ..] = td.tee_tcb_svn;
    let named = if major == 0 {
        None
    } else {
        let id = format!("TDX_{major:02X}");
        Some(
            tcb_info
                .tdx_module_identities
                .iter()
                .find(|identity| identity.id.eq_ignore_ascii_case(&id))
                .ok_or(QuoteError::Module("version"))?,
        )
    };
    let identity: &ModuleIdentity = named.map_or(&tcb_info.tdx_module, |named| &named.identity);
    if td.mr_signer_seam != identity.mrsigner.0 {
        return Err(QuoteError::Module("signer"));
    }
    if !masked_equal(
        &td.seam_attributes,
        &identity.attributes,
        &identity.attributes_mask,
    ) {
        return Err(QuoteError::Module("attributes"));
    }
    match named {
        None => Ok(Tcb::bottom()),
        Some(named) => by_security_version(&named.tcb_levels, u16::from(minor))
            .ok_or(QuoteError::BelowEveryLevel("TDX module")),
    }
}

/// The verdict of the levels a component reaches: a level is reached when the
/// component is at least it, and only the greatest reached levels speak, so the
/// reading is a function of the set of levels and never of the order Intel lists them
/// in. Several greatest levels, incomparable, join their verdicts; none is `None`.
fn reached<L>(
    levels: &[L],
    below: impl Fn(&L, &L) -> bool,
    is_reached: impl Fn(&L) -> bool,
    verdict: impl Fn(&L) -> Tcb,
) -> Option<Tcb> {
    let reached: Vec<&L> = levels.iter().filter(|level| is_reached(level)).collect();
    let greatest = reached.iter().filter(|level| {
        !reached
            .iter()
            .any(|other| below(level, other) && !below(other, level))
    });
    let mut verdicts = greatest.map(|level| verdict(level)).peekable();
    verdicts.peek()?;
    Some(verdicts.collect())
}

/// [`reached`] over levels of one security version each.
fn by_security_version(levels: &[IsvLevel], svn: u16) -> Option<Tcb> {
    reached(
        levels,
        |level, other| level.tcb.isvsvn <= other.tcb.isvsvn,
        |level| svn >= level.tcb.isvsvn,
        |level| level.verdict.tcb(),
    )
}

/// Component by component, `found` is at least `wanted`.
fn at_least(found: &[u8; 16], wanted: &[u8; 16]) -> bool {
    found
        .iter()
        .zip(wanted)
        .all(|(found, wanted)| found >= wanted)
}

fn masked_equal<const N: usize>(found: &[u8; N], wanted: &Hex<N>, mask: &Hex<N>) -> bool {
    found
        .iter()
        .zip(wanted.0.iter().zip(mask.0.iter()))
        .all(|(found, (wanted, mask))| found & mask == wanted & mask)
}

#[cfg(test)]
pub(crate) mod fixture {
    use serde_json::Value;

    use super::Quote;

    /// The gateway's report as served on 2026-10-03.
    pub(crate) const REPORT: &str = include_str!("../../tests/fixtures/tdx-2026-10-03/report.json");

    /// The quote's bytes.
    pub(crate) fn quote_bytes() -> Vec<u8> {
        let report: Value = serde_json::from_str(REPORT).expect("the fixture is JSON");
        hex::decode(
            report["attestation"]["evidence"]["quote"]
                .as_str()
                .expect("the fixture carries a quote"),
        )
        .expect("the quote is hex")
    }

    /// The quote, read.
    pub(crate) fn quote() -> Quote {
        Quote::parse(&quote_bytes()).expect("the live quote reads")
    }
}

#[cfg(test)]
mod tests {
    use super::super::collateral::fixture::{NOW, fetched};
    use super::super::tcb::TcbStatus;
    use super::fixture::{quote, quote_bytes};
    use super::*;

    fn collateral() -> Collateral {
        Collateral::verify(&fetched(), NOW).expect("the saved collateral verifies")
    }

    #[test]
    fn the_live_quote_verifies_up_to_date() {
        let attested =
            Attested::verify(&quote(), &collateral(), NOW).expect("the live quote verifies");
        assert_eq!(attested.tcb, Tcb::bottom(), "UpToDate, nothing advised");
        assert_eq!(attested.tcb.status, TcbStatus::UpToDate);
        assert_eq!(attested.td, quote().td);
    }

    /// Each signed region, one byte changed, is refused by the check that covers it.
    #[test]
    fn a_changed_byte_is_refused_by_the_check_that_covers_it() {
        let collateral = collateral();
        let changed = |offset: usize| {
            let mut bytes = quote_bytes();
            bytes[offset] ^= 1;
            Quote::parse(&bytes).expect("still a quote's layout")
        };
        // The TD report's `REPORTDATA`, which the attestation key signed.
        assert_eq!(
            Attested::verify(&changed(48 + 520), &collateral, NOW),
            Err(QuoteError::QuoteSignature)
        );
        // The QE report's signer, which the PCK key signed.
        assert_eq!(
            Attested::verify(&changed(636 + 134 + 128), &collateral, NOW),
            Err(QuoteError::QeReportSignature)
        );
        // The attestation key, which the QE report binds.
        assert_eq!(
            Attested::verify(&changed(636 + 64), &collateral, NOW),
            Err(QuoteError::AttestationKeyUnbound)
        );
    }

    #[test]
    fn a_quote_needs_current_collateral_for_its_own_platform() {
        let collateral = collateral();
        assert_eq!(
            Attested::verify(&quote(), &collateral, collateral.valid_until),
            Err(QuoteError::StaleCollateral {
                valid_until: collateral.valid_until,
                now: collateral.valid_until
            })
        );
        let mut elsewhere = quote();
        elsewhere.pck.platform.fmspc[5] = 1;
        assert_eq!(
            Attested::verify(&elsewhere, &collateral, NOW),
            Err(QuoteError::OtherPlatform("FMSPC"))
        );
    }

    /// The attributes are changed after the signature is read, so only the attribute
    /// check can refuse it.
    #[test]
    fn a_debug_td_is_refused() {
        let mut debug = quote();
        debug.td.td_attributes[0] |= 1;
        assert!(matches!(
            Attested::verify(&debug, &collateral(), NOW),
            Err(QuoteError::TdAttributes(_))
        ));
    }

    /// The verdict reads the set of levels: Intel's order is not consulted, and a
    /// platform below an up-to-date level reads the out-of-date one it does reach.
    #[test]
    fn the_verdict_is_a_function_of_the_set_of_levels() {
        let mut reversed = collateral();
        reversed.tcb_info.tcb_levels.reverse();
        assert_eq!(
            Attested::verify(&quote(), &reversed, NOW),
            Attested::verify(&quote(), &collateral(), NOW)
        );

        let mut behind = collateral();
        behind.tcb_info.tcb_levels[0].tcb.pcesvn = 14;
        let attested = Attested::verify(&quote(), &behind, NOW).expect("still verifies");
        assert_eq!(attested.tcb.status, TcbStatus::OutOfDate);
        assert!(attested.tcb.advisories.contains("INTEL-SA-01192"));

        let mut nowhere = collateral();
        for level in &mut nowhere.tcb_info.tcb_levels {
            level.tcb.pcesvn = 14;
        }
        assert_eq!(
            Attested::verify(&quote(), &nowhere, NOW),
            Err(QuoteError::BelowEveryLevel("platform"))
        );
    }

    #[test]
    fn incomparable_greatest_levels_join() {
        let level = |svn: u8, status: &str| -> TcbLevel {
            serde_json::from_value(serde_json::json!({
                "tcb": {
                    "sgxtcbcomponents": (0..16).map(|i| serde_json::json!({ "svn": u8::from(i == 0) * svn })).collect::<Vec<_>>(),
                    "pcesvn": u16::from(svn),
                    "tdxtcbcomponents": (0..16).map(|_| serde_json::json!({ "svn": 0 })).collect::<Vec<_>>(),
                },
                "tcbStatus": status,
            }))
            .expect("a level")
        };
        let mut a = level(2, "SWHardeningNeeded");
        a.tcb.pcesvn = 1;
        let mut b = level(1, "ConfigurationNeeded");
        b.tcb.pcesvn = 2;
        let levels = [level(0, "OutOfDate"), a, b];
        let verdict = reached(
            &levels,
            |level, other| {
                level.tcb.sgxtcbcomponents.0[0] <= other.tcb.sgxtcbcomponents.0[0]
                    && level.tcb.pcesvn <= other.tcb.pcesvn
            },
            |_| true,
            |level| level.verdict.tcb(),
        );
        assert_eq!(
            verdict.map(|tcb| tcb.status),
            Some(TcbStatus::ConfigurationAndSWHardeningNeeded)
        );
    }
}

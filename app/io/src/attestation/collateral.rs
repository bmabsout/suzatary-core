//! Intel's collateral for one platform: the signed TCB info and QE identity, and the
//! CRLs, verified to Intel's root and current for a stated window.

use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use serde::Deserialize;
use serde::de::Error as _;
use serde_json::value::RawValue;

use super::chain::{self, Crls};
use super::pck::Platform;
use super::tcb::{Tcb, TcbStatus};
use crate::http::{Http, Req, Resp};

/// Intel's Provisioning Certification Service, which serves the TCB info, the QE
/// identity and the PCK CRLs.
pub const INTEL_PCS: &str = "https://api.trustedservices.intel.com";

/// Where Intel publishes its SGX root's CRL.
pub const INTEL_ROOT_CRL: &str =
    "https://certificates.trustedservices.intel.com/IntelSGXRootCA.der";

/// Each document is a few kilobytes.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Intel's collateral as fetched, before any of it is believed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The platform it was fetched for.
    pub platform: Platform,
    /// The TCB info body, `{"tcbInfo": …, "signature": …}`, as served.
    pub tcb_info: String,
    /// The `TCB-Info-Issuer-Chain` header's certificates, leaf first.
    pub tcb_info_chain: Vec<CertificateDer<'static>>,
    /// The QE identity body, `{"enclaveIdentity": …, "signature": …}`, as served.
    pub qe_identity: String,
    /// The `SGX-Enclave-Identity-Issuer-Chain` header's certificates, leaf first.
    pub qe_identity_chain: Vec<CertificateDer<'static>>,
    /// The PCK CA's CRL, DER.
    pub pck_crl: Vec<u8>,
    /// The root CA's CRL, DER.
    pub root_crl: Vec<u8>,
}

impl Fetched {
    /// Fetch `platform`'s collateral from Intel's PCS and the root CRL from Intel's
    /// certificate site; nothing is believed until [`Collateral::verify`].
    ///
    /// # Errors
    /// A document that could not be fetched, or an issuer chain that is not PEM.
    pub fn fetch(http: &dyn Http, platform: Platform) -> Result<Self, CollateralError> {
        let tcb_info = get(
            http,
            &format!(
                "{INTEL_PCS}/tdx/certification/v4/tcb?fmspc={}",
                hex::encode(platform.fmspc)
            ),
        )?;
        let qe_identity = get(
            http,
            &format!("{INTEL_PCS}/tdx/certification/v4/qe/identity"),
        )?;
        let pck_crl = get(
            http,
            &format!(
                "{INTEL_PCS}/sgx/certification/v4/pckcrl?ca={}&encoding=der",
                platform.ca.as_str()
            ),
        )?;
        let root_crl = get(http, INTEL_ROOT_CRL)?;
        let chain = |resp: &Resp, header: &'static str| {
            resp.header(header)
                .ok_or(CollateralError::Chain(format!("no {header} header")))
                .and_then(Self::chain)
        };
        Ok(Self {
            platform,
            tcb_info_chain: chain(&tcb_info, "TCB-Info-Issuer-Chain")?,
            tcb_info: tcb_info.text(),
            qe_identity_chain: chain(&qe_identity, "SGX-Enclave-Identity-Issuer-Chain")?,
            qe_identity: qe_identity.text(),
            pck_crl: pck_crl.body,
            root_crl: root_crl.body,
        })
    }

    /// An issuer-chain header's certificates: PEM, percent-encoded, as Intel's PCS
    /// sends it.
    ///
    /// # Errors
    /// A value that does not decode to PEM certificates.
    pub fn chain(header: &str) -> Result<Vec<CertificateDer<'static>>, CollateralError> {
        let pem = percent_encoding::percent_decode_str(header)
            .decode_utf8()
            .map_err(|failed| CollateralError::Chain(failed.to_string()))?;
        let certificates = CertificateDer::pem_slice_iter(pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|failed| CollateralError::Chain(failed.to_string()))?;
        if certificates.is_empty() {
            return Err(CollateralError::Chain("no certificate".to_owned()));
        }
        Ok(certificates)
    }
}

/// Collateral every part of which verified to Intel's root, current from `issued` to
/// `valid_until`; [`Collateral::verify`] is the only constructor.
#[derive(Debug, Clone)]
pub struct Collateral {
    /// The platform it speaks for.
    pub platform: Platform,
    pub(super) tcb_info: TcbInfo,
    pub(super) qe_identity: QeIdentity,
    pub(super) crls: Crls,
    /// The latest of its parts' issue instants, Unix seconds.
    pub issued: i64,
    /// The earliest of its parts' next updates, Unix seconds: past it, Intel may say
    /// something else.
    pub valid_until: i64,
}

fn get(http: &dyn Http, url: &str) -> Result<Resp, CollateralError> {
    let resp = http
        .send(Req::get(url).timeout(TIMEOUT))
        .map_err(|failed| CollateralError::Unreachable {
            url: url.to_owned(),
            reason: failed.to_string(),
        })?;
    if resp.ok() {
        Ok(resp)
    } else {
        Err(CollateralError::Failed {
            url: url.to_owned(),
            status: resp.status,
        })
    }
}

/// Why collateral was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollateralError {
    /// A document could not be fetched.
    #[error("{url} could not be reached: {reason}")]
    Unreachable {
        /// What was asked.
        url: String,
        /// The transport's reason.
        reason: String,
    },
    /// A document's endpoint answered other than 2xx.
    #[error("{url} answered {status}")]
    Failed {
        /// What was asked.
        url: String,
        /// The status.
        status: u16,
    },
    /// An issuer chain that is not PEM certificates.
    #[error("an issuer chain is not PEM certificates: {0}")]
    Chain(String),
    /// A body that is not Intel's signed JSON of the expected shape.
    #[error("the {what} is not the expected shape: {reason}")]
    Malformed {
        /// Which document.
        what: &'static str,
        /// What was wrong.
        reason: String,
    },
    /// A CRL that could not be read.
    #[error("a CRL could not be read: {0}")]
    Crl(String),
    /// A signing chain that does not verify to Intel's root, or is revoked.
    #[error("the {what} signing chain does not verify to Intel's root: {reason:?}")]
    Untrusted {
        /// Which document.
        what: &'static str,
        /// rustls-webpki's refusal.
        reason: webpki::Error,
    },
    /// A signature that does not verify under its chain's leaf.
    #[error("the {0}'s signature does not verify")]
    BadSignature(&'static str),
    /// TCB info or a QE identity for something other than TDX.
    #[error("the {what} is {found}, not {wanted}")]
    NotTdx {
        /// Which document.
        what: &'static str,
        /// Its id and version.
        found: String,
        /// What a TDX quote needs.
        wanted: String,
    },
    /// The TCB info is for another FMSPC than the platform asked for.
    #[error("the TCB info is for another platform than the one asked for")]
    OtherPlatform,
    /// Some part is not yet issued, or already past its next update.
    #[error("the collateral is current from {issued} to {valid_until}, not at {now}")]
    NotCurrent {
        /// The latest issue instant.
        issued: i64,
        /// The earliest next update.
        valid_until: i64,
        /// The instant it was checked at.
        now: i64,
    },
}

impl Collateral {
    /// Fetch `platform`'s collateral and verify it at `now`.
    ///
    /// # Errors
    /// As [`Fetched::fetch`] and [`Collateral::verify`].
    pub fn fetch(http: &dyn Http, platform: Platform, now: i64) -> Result<Self, CollateralError> {
        Self::verify(&Fetched::fetch(http, platform)?, now)
    }

    /// Verify every signature to Intel's root at `now`, and that every part is current.
    ///
    /// # Errors
    /// The [`CollateralError`] naming the first part that failed.
    pub fn verify(fetched: &Fetched, now: i64) -> Result<Self, CollateralError> {
        let crls = Crls::parse(&fetched.root_crl, &fetched.pck_crl)
            .map_err(|failed| CollateralError::Crl(format!("{failed:?}")))?;
        let tcb_info: TcbInfo = signed(
            "TCB info",
            &fetched.tcb_info,
            &fetched.tcb_info_chain,
            &crls,
            now,
        )?;
        let qe_identity: QeIdentity = signed(
            "QE identity",
            &fetched.qe_identity,
            &fetched.qe_identity_chain,
            &crls,
            now,
        )?;
        tdx("TCB info", &tcb_info.id, tcb_info.version, "TDX", 3)?;
        if tcb_info.fmspc.0 != fetched.platform.fmspc {
            return Err(CollateralError::OtherPlatform);
        }
        tdx(
            "QE identity",
            &qe_identity.id,
            qe_identity.version,
            "TD_QE",
            2,
        )?;

        let crl_windows = [&fetched.root_crl, &fetched.pck_crl]
            .into_iter()
            .map(|der| {
                chain::crl_window(der).map_err(|failed| CollateralError::Crl(failed.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let windows = crl_windows.into_iter().chain([
            (
                tcb_info.issue_date.timestamp(),
                tcb_info.next_update.timestamp(),
            ),
            (
                qe_identity.issue_date.timestamp(),
                qe_identity.next_update.timestamp(),
            ),
        ]);
        let (issued, valid_until) = windows
            .fold((i64::MIN, i64::MAX), |(from, to), (issue, next)| {
                (from.max(issue), to.min(next))
            });
        if !(issued <= now && now < valid_until) {
            return Err(CollateralError::NotCurrent {
                issued,
                valid_until,
                now,
            });
        }
        Ok(Self {
            platform: fetched.platform,
            tcb_info,
            qe_identity,
            crls,
            issued,
            valid_until,
        })
    }
}

/// The body of an Intel signed document, once its signature verifies under the leaf of
/// a chain that verifies to the root; the signed bytes are the body as served, so the
/// field is read raw.
fn signed<T: serde::de::DeserializeOwned>(
    what: &'static str,
    document: &str,
    chain: &[CertificateDer<'static>],
    crls: &Crls,
    now: i64,
) -> Result<T, CollateralError> {
    #[derive(Deserialize)]
    struct Signed<'a> {
        #[serde(borrow, alias = "tcbInfo", alias = "enclaveIdentity")]
        body: &'a RawValue,
        signature: Hex<64>,
    }
    let malformed = |reason: String| CollateralError::Malformed { what, reason };
    let signed: Signed<'_> =
        serde_json::from_str(document).map_err(|failed| malformed(failed.to_string()))?;
    let leaf = chain::verified_leaf(chain, crls, now)
        .map_err(|reason| CollateralError::Untrusted { what, reason })?;
    chain::signed_by(&leaf, signed.body.get().as_bytes(), &signed.signature.0)
        .map_err(|_| CollateralError::BadSignature(what))?;
    serde_json::from_str(signed.body.get()).map_err(|failed| malformed(failed.to_string()))
}

fn tdx(
    what: &'static str,
    id: &str,
    version: u32,
    wanted_id: &str,
    wanted_version: u32,
) -> Result<(), CollateralError> {
    if id == wanted_id && version == wanted_version {
        Ok(())
    } else {
        Err(CollateralError::NotTdx {
            what,
            found: format!("{id} version {version}"),
            wanted: format!("{wanted_id} version {wanted_version}"),
        })
    }
}

/// Exactly `N` bytes of hex, either case, as Intel writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hex<const N: usize>(pub [u8; N]);

impl<'de, const N: usize> Deserialize<'de> for Hex<N> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        let bytes = hex::decode(text).map_err(D::Error::custom)?;
        <[u8; N]>::try_from(bytes)
            .map(Self)
            .map_err(|bytes| D::Error::custom(format!("{} bytes, not {N}", bytes.len())))
    }
}

/// Sixteen TCB components' security versions, from Intel's `[{"svn": …}, …]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Svns(pub [u8; 16]);

impl<'de> Deserialize<'de> for Svns {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Component {
            svn: u8,
        }
        let components = Vec::<Component>::deserialize(deserializer)?;
        let svns: Vec<u8> = components.iter().map(|component| component.svn).collect();
        <[u8; 16]>::try_from(svns)
            .map(Self)
            .map_err(|svns| D::Error::custom(format!("{} components, not 16", svns.len())))
    }
}

/// A status and its advisories, as each TCB level states them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Verdict {
    tcb_status: TcbStatus,
    #[serde(rename = "advisoryIDs", default)]
    advisory_ids: BTreeSet<String>,
}

impl Verdict {
    pub(super) fn tcb(&self) -> Tcb {
        Tcb {
            status: self.tcb_status,
            advisories: self.advisory_ids.clone(),
        }
    }
}

/// Intel's TDX TCB info, version 3.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TcbInfo {
    id: String,
    version: u32,
    issue_date: DateTime<Utc>,
    next_update: DateTime<Utc>,
    pub fmspc: Hex<6>,
    pub pce_id: Hex<2>,
    pub tcb_levels: Vec<TcbLevel>,
    pub tdx_module: ModuleIdentity,
    #[serde(default)]
    pub tdx_module_identities: Vec<NamedModuleIdentity>,
}

/// One platform TCB level.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct TcbLevel {
    pub tcb: LevelTcb,
    #[serde(flatten)]
    pub verdict: Verdict,
}

/// What a platform must be at least to reach a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(super) struct LevelTcb {
    pub sgxtcbcomponents: Svns,
    pub pcesvn: u16,
    pub tdxtcbcomponents: Svns,
}

/// The TDX module a platform may run: its signer and attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModuleIdentity {
    pub mrsigner: Hex<48>,
    pub attributes: Hex<8>,
    pub attributes_mask: Hex<8>,
}

/// One TDX module version's identity and its own TCB levels.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NamedModuleIdentity {
    pub id: String,
    #[serde(flatten)]
    pub identity: ModuleIdentity,
    pub tcb_levels: Vec<IsvLevel>,
}

/// A level by one security version, as a QE or a TDX module has.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct IsvLevel {
    pub tcb: Isv,
    #[serde(flatten)]
    pub verdict: Verdict,
}

/// One security version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(super) struct Isv {
    pub isvsvn: u16,
}

/// Intel's identity of the TD quoting enclave, version 2.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QeIdentity {
    id: String,
    version: u32,
    issue_date: DateTime<Utc>,
    next_update: DateTime<Utc>,
    pub miscselect: Hex<4>,
    pub miscselect_mask: Hex<4>,
    pub attributes: Hex<16>,
    pub attributes_mask: Hex<16>,
    pub mrsigner: Hex<32>,
    pub isvprodid: u16,
    pub tcb_levels: Vec<IsvLevel>,
}

/// The collateral Intel's PCS served on 2026-10-03 for the gateway's platform, read from
/// `tests/fixtures/tdx-2026-10-03/`.
#[cfg(test)]
pub(crate) mod fixture {
    use super::super::pck::{Ca, Platform};
    use super::{Fetched, INTEL_PCS, INTEL_ROOT_CRL};
    use crate::http::{HttpError, Req, Resp};

    /// An instant inside every part's window: 05:30 UTC that day.
    pub(crate) const NOW: i64 = 1_791_005_400;

    pub(crate) const TCB_INFO: &str =
        include_str!("../../tests/fixtures/tdx-2026-10-03/tcb-info.json");
    pub(crate) const TCB_INFO_CHAIN: &str =
        include_str!("../../tests/fixtures/tdx-2026-10-03/tcb-info.issuer-chain");
    pub(crate) const QE_IDENTITY: &str =
        include_str!("../../tests/fixtures/tdx-2026-10-03/qe-identity.json");
    pub(crate) const QE_IDENTITY_CHAIN: &str =
        include_str!("../../tests/fixtures/tdx-2026-10-03/qe-identity.issuer-chain");
    pub(crate) const PCK_CRL: &[u8] =
        include_bytes!("../../tests/fixtures/tdx-2026-10-03/pck-crl.der");
    pub(crate) const ROOT_CRL: &[u8] =
        include_bytes!("../../tests/fixtures/tdx-2026-10-03/root-ca-crl.der");

    /// Intel's endpoints as they answered that day; anything else is a 404.
    pub(crate) fn intel() -> impl Fn(&Req) -> Result<Resp, HttpError> + Send + Sync + 'static {
        |req| {
            let url = req.url.as_str();
            Ok(
                if url.starts_with(&format!(
                    "{INTEL_PCS}/tdx/certification/v4/tcb?fmspc=20a06f000000"
                )) {
                    Resp::new(200, TCB_INFO).with_header("TCB-Info-Issuer-Chain", TCB_INFO_CHAIN)
                } else if url == format!("{INTEL_PCS}/tdx/certification/v4/qe/identity") {
                    Resp::new(200, QE_IDENTITY)
                        .with_header("SGX-Enclave-Identity-Issuer-Chain", QE_IDENTITY_CHAIN)
                } else if url
                    == format!("{INTEL_PCS}/sgx/certification/v4/pckcrl?ca=platform&encoding=der")
                {
                    Resp::new(200, PCK_CRL)
                } else if url == INTEL_ROOT_CRL {
                    Resp::new(200, ROOT_CRL)
                } else {
                    Resp::new(404, "")
                },
            )
        }
    }

    /// The gateway's platform.
    pub(crate) const PLATFORM: Platform = Platform {
        fmspc: [0x20, 0xa0, 0x6f, 0, 0, 0],
        ca: Ca::Platform,
    };

    pub(crate) fn fetched() -> Fetched {
        Fetched {
            platform: PLATFORM,
            tcb_info: TCB_INFO.to_owned(),
            tcb_info_chain: Fetched::chain(TCB_INFO_CHAIN).expect("the saved chain decodes"),
            qe_identity: QE_IDENTITY.to_owned(),
            qe_identity_chain: Fetched::chain(QE_IDENTITY_CHAIN).expect("the saved chain decodes"),
            pck_crl: PCK_CRL.to_vec(),
            root_crl: ROOT_CRL.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{NOW, fetched};
    use super::*;

    #[test]
    fn the_saved_collateral_verifies_and_states_its_window() {
        let collateral = Collateral::verify(&fetched(), NOW).expect("Intel's collateral verifies");
        assert_eq!(collateral.tcb_info.fmspc, Hex([0x20, 0xa0, 0x6f, 0, 0, 0]));
        assert_eq!(collateral.tcb_info.tcb_levels.len(), 4);
        assert_eq!(collateral.qe_identity.isvprodid, 2);
        assert_eq!(
            collateral.issued,
            DateTime::parse_from_rfc3339("2026-10-03T04:26:24Z")
                .expect("an instant")
                .timestamp(),
            "the PCK CRL is the latest issued"
        );
        assert_eq!(
            collateral.valid_until,
            DateTime::parse_from_rfc3339("2026-11-02T04:16:03Z")
                .expect("an instant")
                .timestamp(),
            "the QE identity is the first to need updating"
        );
    }

    #[test]
    fn the_platforms_collateral_is_fetched_from_intel() {
        let intel = crate::http::Scripted::new(super::fixture::intel());
        let platform = super::fixture::PLATFORM;
        assert_eq!(Fetched::fetch(&intel, platform), Ok(fetched()));
        assert_eq!(intel.sent().len(), 4);

        let elsewhere = Platform {
            fmspc: [0; 6],
            ..platform
        };
        assert!(matches!(
            Fetched::fetch(&intel, elsewhere),
            Err(CollateralError::Failed { status: 404, .. })
        ));
    }

    #[test]
    fn outside_its_window_it_is_not_current() {
        let collateral = Collateral::verify(&fetched(), NOW).expect("verifies");
        for at in [collateral.issued - 1, collateral.valid_until] {
            assert!(matches!(
                Collateral::verify(&fetched(), at),
                Err(CollateralError::NotCurrent { .. } | CollateralError::Untrusted { .. })
            ));
        }
    }

    #[test]
    fn one_changed_byte_of_a_signed_body_is_a_bad_signature() {
        let mut tampered = fetched();
        tampered.tcb_info = tampered
            .tcb_info
            .replacen("\"UpToDate\"", "\"UpToDate\" ", 1);
        assert_eq!(
            Collateral::verify(&tampered, NOW).map(|_| ()),
            Err(CollateralError::BadSignature("TCB info"))
        );
        let mut tampered = fetched();
        tampered.qe_identity =
            tampered
                .qe_identity
                .replacen("\"isvprodid\":2", "\"isvprodid\":3", 1);
        assert_ne!(tampered.qe_identity, super::fixture::QE_IDENTITY);
        assert_eq!(
            Collateral::verify(&tampered, NOW).map(|_| ()),
            Err(CollateralError::BadSignature("QE identity"))
        );
    }

    #[test]
    fn a_chain_not_rooted_at_intel_is_untrusted() {
        let mut stranger = fetched();
        stranger.tcb_info_chain = Fetched::chain(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/other-cert.pem"
            ))
            .expect("the fixture reads"),
        )
        .expect("a PEM certificate");
        assert!(matches!(
            Collateral::verify(&stranger, NOW),
            Err(CollateralError::Untrusted {
                what: "TCB info",
                ..
            })
        ));
    }

    #[test]
    fn collateral_for_another_platform_is_refused() {
        let mut elsewhere = fetched();
        elsewhere.platform.fmspc = [0; 6];
        assert_eq!(
            Collateral::verify(&elsewhere, NOW).map(|_| ()),
            Err(CollateralError::OtherPlatform)
        );
    }

    #[test]
    fn a_tcb_info_signed_as_a_qe_identity_is_not_tdx() {
        let mut swapped = fetched();
        swapped.tcb_info = super::fixture::QE_IDENTITY.to_owned();
        assert!(matches!(
            Collateral::verify(&swapped, NOW),
            Err(CollateralError::Malformed {
                what: "TCB info",
                ..
            })
        ));
    }
}

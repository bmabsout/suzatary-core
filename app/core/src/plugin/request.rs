//! A request as a plugin wrote it, and the fence it crosses to become one the host sends.

use crate::capability::Secret;

use super::{CredentialName, HeaderName, Manifest, Outgoing, Placement, PluginUrl, Refusal, Said};

/// A request as the plugin wrote it: untrusted, unparsed, and only ever turned into an
/// [`Outgoing`] by [`Request::admit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The verb, with its body.
    pub verb: Verb,
    /// Where it goes.
    pub target: Target,
    /// Headers, in order.
    pub headers: Vec<(String, String)>,
    /// A credential to put in a header, by name.
    pub credential: Option<String>,
}

/// The verb, with the body only where a body exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    /// No body.
    Get,
    /// With this body.
    Post(Vec<u8>),
}

/// Where a request goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A URL the plugin wrote.
    Url(String),
    /// A credential whose value is the URL, by name.
    SecretUrl(String),
}

/// A header credential the manifest declared, and where its value goes.
struct Injected<'m> {
    name: &'m CredentialName,
    header: HeaderName,
    prefix: &'static str,
    origin: &'m super::Origin,
}

impl Request {
    /// Check this request against `manifest` and put the credentials it names into it.
    ///
    /// Every declaration is checked before `resolve` is asked anything, so a name the manifest
    /// does not declare never reaches the secret store; `resolve` is the secret effect, passed
    /// in, and its values go only into the [`Outgoing`], never back to the plugin. A store that
    /// could not answer is [`Refusal::CredentialUnavailable`], never "unset".
    pub fn admit<E>(
        self,
        manifest: &Manifest,
        mut resolve: impl FnMut(&CredentialName) -> Result<Option<Secret>, E>,
    ) -> Result<Outgoing, Refusal> {
        let headers = self
            .headers
            .iter()
            .map(|(name, value)| header(name, value))
            .collect::<Result<Vec<_>, _>>()?;
        let injected = self
            .credential
            .as_deref()
            .map(|name| injected(manifest, name))
            .transpose()?;
        if let Some(taken) = injected
            .as_ref()
            .and_then(|at| headers.iter().find(|(name, _)| *name == at.header))
        {
            return Err(Refusal::BadHeader(Said::clip(taken.0.to_string())));
        }
        let mut secrets = Vec::new();
        let url = match &self.target {
            Target::Url(text) => {
                PluginUrl::parse(text).ok_or_else(|| Refusal::BadUrl(Said::clip(text)))?
            }
            Target::SecretUrl(name) => {
                let Some((declared, Placement::Url)) = manifest.placement(name) else {
                    return Err(Refusal::CredentialNotDeclared(Said::clip(name)));
                };
                let value = value(declared, &mut resolve)?;
                let url = PluginUrl::parse(value.expose())
                    .ok_or_else(|| Refusal::CredentialNotAUrl(Said::clip(declared.to_string())))?;
                if !url.origin().is_confidential() {
                    return Err(Refusal::InTheClear(Said::clip(declared.to_string())));
                }
                secrets.push((declared.clone(), value));
                url
            }
        };
        // A secret URL's origin is known only once its value is read; either way the origin is
        // checked before a header credential's value is.
        if !manifest.allows(url.origin()) {
            return Err(Refusal::OriginNotAllowed(Said::clip(
                url.origin().to_string(),
            )));
        }
        let mut headers = headers
            .into_iter()
            .map(|(name, value)| (name, Secret::new(value)))
            .collect::<Vec<_>>();
        if let Some(at) = injected {
            bound(&at, &url)?;
            let value = value(at.name, &mut resolve)?;
            headers.push((
                at.header,
                Secret::new(format!("{}{}", at.prefix, value.expose())),
            ));
            secrets.push((at.name.clone(), value));
        }
        Ok(Outgoing::new(self.verb, url, headers, secrets))
    }
}

/// A header the plugin wrote: a token the host does not own, a visible-ASCII value.
fn header(name: &str, value: &str) -> Result<(HeaderName, String), Refusal> {
    let visible = value
        .bytes()
        .all(|b| b == b'\t' || (b' '..=b'~').contains(&b));
    match HeaderName::parse(name) {
        Some(parsed) if visible => Ok((parsed, value.to_owned())),
        _ => Err(Refusal::BadHeader(Said::clip(name))),
    }
}

/// The manifest's header credential by this name, or the refusal of a name it does not declare
/// for a header.
fn injected<'m>(manifest: &'m Manifest, name: &str) -> Result<Injected<'m>, Refusal> {
    let undeclared = || Refusal::CredentialNotDeclared(Said::clip(name));
    let (name, placement) = manifest.placement(name).ok_or_else(undeclared)?;
    let (header, prefix, origin) = match placement {
        Placement::Url => return Err(undeclared()),
        Placement::Bearer { origin } => (
            HeaderName::parse("authorization").ok_or_else(undeclared)?,
            "Bearer ",
            origin,
        ),
        Placement::Header { header, origin } => (header.clone(), "", origin),
    };
    Ok(Injected {
        name,
        header,
        prefix,
        origin,
    })
}

/// A header credential goes only to the one origin it is bound to.
fn bound(at: &Injected<'_>, url: &PluginUrl) -> Result<(), Refusal> {
    if url.origin() == at.origin {
        Ok(())
    } else {
        Err(Refusal::CredentialNotDeclared(Said::clip(
            at.name.to_string(),
        )))
    }
}

/// A declared credential's value; an empty one is unset, since it would match every answer.
fn value<E>(
    name: &CredentialName,
    resolve: &mut impl FnMut(&CredentialName) -> Result<Option<Secret>, E>,
) -> Result<Secret, Refusal> {
    resolve(name)
        .map_err(|_| Refusal::CredentialUnavailable(Said::clip(name.to_string())))?
        .filter(|value| !value.expose().is_empty())
        .ok_or_else(|| Refusal::CredentialUnset(Said::clip(name.to_string())))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::*;
    use crate::plugin::Response;

    const MANIFEST: &str = r#"
        name = "fence"
        component = "fence.wasm"
        schema = "fence.schema"
        world = "source@0.3.0"
        origins = [
            "http://127.0.0.1:9000",
            "https://calendar.example",
            "http://calendar.example",
            "https://api.example",
        ]
        credentials = [
            { name = "PLUGIN_FENCE_ICS", into = "url" },
            { name = "PLUGIN_FENCE_TOKEN", into = "bearer", origin = "https://api.example" },
            { name = "PLUGIN_FENCE_KEY", into = "header", header = "x-key", origin = "http://127.0.0.1:9000" },
            { name = "PLUGIN_FENCE_UNSET", into = "bearer", origin = "https://api.example" },
            { name = "PLUGIN_FENCE_ELSEWHERE", into = "url" },
            { name = "PLUGIN_FENCE_PLAIN", into = "url" },
        ]
    "#;

    fn manifest() -> Manifest {
        toml::from_str(MANIFEST).expect("the fixture manifest")
    }

    /// A secret store that records every name it was asked for.
    #[derive(Default)]
    struct Store {
        asked: Vec<String>,
    }

    impl Store {
        fn values() -> BTreeMap<&'static str, &'static str> {
            BTreeMap::from([
                (
                    "PLUGIN_FENCE_ICS",
                    "https://calendar.example/private-s3cret/basic.ics",
                ),
                ("PLUGIN_FENCE_TOKEN", "t0ken"),
                ("PLUGIN_FENCE_KEY", "k3y"),
                (
                    "PLUGIN_FENCE_ELSEWHERE",
                    "https://evil.example/private-s3cret",
                ),
                (
                    "PLUGIN_FENCE_PLAIN",
                    "http://calendar.example/private-s3cret",
                ),
                ("ANTHROPIC_API_KEY", "the-app-s-own-key"),
            ])
        }

        fn admit(&mut self, request: Request) -> Result<Outgoing, Refusal> {
            request.admit(&manifest(), |name| {
                self.asked.push(name.to_string());
                Ok::<_, ()>(
                    Self::values()
                        .get(name.as_str())
                        .map(|value| Secret::new(*value)),
                )
            })
        }
    }

    fn get(target: Target) -> Request {
        Request {
            verb: Verb::Get,
            target,
            headers: Vec::new(),
            credential: None,
        }
    }

    fn url(text: &str) -> Target {
        Target::Url(text.to_owned())
    }

    #[test]
    fn an_allowed_url_is_sent_as_reprinted() {
        let mut store = Store::default();
        let sent = store
            .admit(get(url("http://127.0.0.1:9000?since=1")))
            .expect("an allowed origin");
        assert_eq!(sent.url().to_string(), "http://127.0.0.1:9000/?since=1");
        assert!(store.asked.is_empty(), "no credential was named");
    }

    #[test]
    fn an_origin_not_allowed_is_refused_by_name() {
        let mut store = Store::default();
        for (text, origin) in [
            ("https://evil.example/", "https://evil.example"),
            ("http://127.0.0.1:9001/", "http://127.0.0.1:9001"),
            ("http://api.example/", "http://api.example"),
        ] {
            assert_eq!(
                store.admit(get(url(text))).map(|_| ()),
                Err(Refusal::OriginNotAllowed(Said::clip(origin)))
            );
        }
        assert_eq!(
            store
                .admit(get(url("https://api.example@evil.example/")))
                .map(|_| ()),
            Err(Refusal::BadUrl(Said::clip(
                "https://api.example@evil.example/".to_owned()
            )))
        );
    }

    #[test]
    fn a_name_not_declared_never_reaches_the_secret_store() {
        let mut store = Store::default();
        for name in ["ANTHROPIC_API_KEY", "PLUGIN_FENCE_NOPE", "PLUGIN_FENCE_ICS"] {
            let mut request = get(url("https://api.example/"));
            request.credential = Some(name.to_owned());
            assert_eq!(
                store.admit(request).map(|_| ()),
                Err(Refusal::CredentialNotDeclared(Said::clip(name))),
                "{name} as a header credential"
            );
        }
        for name in ["ANTHROPIC_API_KEY", "PLUGIN_FENCE_TOKEN"] {
            assert_eq!(
                store
                    .admit(get(Target::SecretUrl(name.to_owned())))
                    .map(|_| ()),
                Err(Refusal::CredentialNotDeclared(Said::clip(name))),
                "{name} as a secret URL"
            );
        }
        assert!(store.asked.is_empty(), "asked for {:?}", store.asked);
    }

    #[test]
    fn a_header_credential_goes_only_to_its_own_origin() {
        let mut store = Store::default();
        let mut request = get(url("http://127.0.0.1:9000/"));
        request.credential = Some("PLUGIN_FENCE_TOKEN".to_owned());
        assert_eq!(
            store.admit(request).map(|_| ()),
            Err(Refusal::CredentialNotDeclared(Said::clip(
                "PLUGIN_FENCE_TOKEN".to_owned()
            )))
        );
        assert!(store.asked.is_empty());

        let mut request = get(url("https://api.example/v1"));
        request.credential = Some("PLUGIN_FENCE_TOKEN".to_owned());
        let sent = store.admit(request).expect("its own origin");
        let headers: Vec<_> = sent
            .headers()
            .iter()
            .map(|(name, value)| (name.to_string(), value.expose().to_owned()))
            .collect();
        assert_eq!(
            headers,
            [("authorization".to_owned(), "Bearer t0ken".to_owned())]
        );
        assert_eq!(store.asked, ["PLUGIN_FENCE_TOKEN"]);
    }

    #[test]
    fn a_secret_url_is_resolved_then_held_to_the_same_allowlist() {
        let mut store = Store::default();
        let sent = store
            .admit(get(Target::SecretUrl("PLUGIN_FENCE_ICS".to_owned())))
            .expect("an allowed secret URL");
        assert_eq!(
            sent.url().to_string(),
            "https://calendar.example/private-s3cret/basic.ics"
        );
        assert_eq!(
            store
                .admit(get(Target::SecretUrl("PLUGIN_FENCE_ELSEWHERE".to_owned())))
                .map(|_| ()),
            Err(Refusal::OriginNotAllowed(Said::clip(
                "https://evil.example"
            )))
        );
        assert_eq!(
            store
                .admit(get(Target::SecretUrl("PLUGIN_FENCE_PLAIN".to_owned())))
                .map(|_| ()),
            Err(Refusal::InTheClear(Said::clip("PLUGIN_FENCE_PLAIN"))),
            "an allowed origin, and still never in the clear"
        );
    }

    #[test]
    fn an_unset_credential_is_refused_by_name() {
        let mut store = Store::default();
        let mut request = get(url("https://api.example/"));
        request.credential = Some("PLUGIN_FENCE_UNSET".to_owned());
        assert_eq!(
            store.admit(request).map(|_| ()),
            Err(Refusal::CredentialUnset(Said::clip("PLUGIN_FENCE_UNSET")))
        );
    }

    /// A store that cannot answer is never read as an unset credential.
    #[test]
    fn a_locked_store_is_unavailable_not_unset() {
        let mut request = get(url("https://api.example/"));
        request.credential = Some("PLUGIN_FENCE_TOKEN".to_owned());
        assert_eq!(
            request
                .admit(&manifest(), |_| Err::<Option<Secret>, _>("locked"))
                .map(|_| ()),
            Err(Refusal::CredentialUnavailable(Said::clip(
                "PLUGIN_FENCE_TOKEN"
            )))
        );
    }

    #[test]
    fn a_plugin_writes_no_header_the_host_owns_or_the_credential_takes() {
        let mut store = Store::default();
        for (name, value) in [
            ("Host", "evil.example"),
            ("x-ok", "a\r\nhost: evil"),
            ("bad name", "v"),
        ] {
            let mut request = get(url("https://api.example/"));
            request.headers = vec![(name.to_owned(), value.to_owned())];
            assert_eq!(
                store.admit(request).map(|_| ()),
                Err(Refusal::BadHeader(Said::clip(name)))
            );
        }
        let mut request = get(url("https://api.example/"));
        request.headers = vec![("Authorization".to_owned(), "Bearer mine".to_owned())];
        request.credential = Some("PLUGIN_FENCE_TOKEN".to_owned());
        assert_eq!(
            store.admit(request).map(|_| ()),
            Err(Refusal::BadHeader(Said::clip("authorization")))
        );
        assert!(store.asked.is_empty());
    }

    #[test]
    fn an_answer_that_echoes_a_credential_is_withheld() {
        let mut store = Store::default();
        let mut request = get(url("http://127.0.0.1:9000/echo"));
        request.credential = Some("PLUGIN_FENCE_KEY".to_owned());
        let sent = store.admit(request).expect("its own origin");
        let echoed = Response {
            status: 200,
            headers: Vec::new(),
            body: b"you sent x-key: k3y".to_vec(),
        };
        assert_eq!(
            sent.answer(echoed),
            Err(Refusal::Echoed(Said::clip("PLUGIN_FENCE_KEY")))
        );
        let in_a_header = Response {
            status: 302,
            headers: vec![("location".to_owned(), "/k3y".to_owned())],
            body: Vec::new(),
        };
        assert_eq!(
            sent.answer(in_a_header),
            Err(Refusal::Echoed(Said::clip("PLUGIN_FENCE_KEY")))
        );
        let clean = Response {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/plain".to_owned())],
            body: b"items".to_vec(),
        };
        assert_eq!(sent.answer(clean.clone()), Ok(clean));
    }

    #[test]
    fn nothing_admitted_prints_a_value() {
        let mut store = Store::default();
        let mut request = get(Target::SecretUrl("PLUGIN_FENCE_ICS".to_owned()));
        request.verb = Verb::Post(b"body".to_vec());
        let sent = store.admit(request).expect("an allowed secret URL");
        let shown = format!("{sent:?}");
        assert!(!shown.contains("s3cret"), "{shown}");
    }

    proptest! {
        /// The fence's law: a name the manifest does not declare for a use is refused by name,
        /// whatever it is and wherever it is named, and the secret store is never asked.
        #[test]
        fn an_undeclared_name_never_reaches_the_secret_store(
            name in "(PLUGIN_FENCE_|ANTHROPIC_|)[A-Z_]{0,12}",
            as_url in any::<bool>(),
        ) {
            let declared = manifest()
                .placement(&name)
                .is_some_and(|(_, placement)| matches!(placement, Placement::Url) == as_url);
            prop_assume!(!declared);
            let mut request = get(url("https://api.example/"));
            if as_url {
                request.target = Target::SecretUrl(name.clone());
            } else {
                request.credential = Some(name.clone());
            }
            let mut store = Store::default();
            prop_assert_eq!(
                store.admit(request).map(|_| ()),
                Err(Refusal::CredentialNotDeclared(Said::clip(name)))
            );
            prop_assert!(store.asked.is_empty());
        }
    }
}

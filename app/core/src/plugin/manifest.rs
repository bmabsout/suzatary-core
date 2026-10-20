//! A plugin's manifest: the component, its world and every authority it asks for.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::{
    BundleRoot, ComponentPath, CredentialName, Digest, HeaderName, Origin, Placement, PluginName,
    SchemaPath, World, world::PACKAGE_VERSION,
};

/// One `[[plugins]]` table, parsed: every origin it may reach and every credential it may name.
///
/// Deserialising is the one way to build one, through `Raw`'s `TryFrom`, so a value of this
/// type cannot hold an origin or a credential the file did not declare, nor a credential bound
/// to an origin it does not allow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Raw")]
pub struct Manifest {
    name: PluginName,
    from: BundleRoot,
    component: ComponentPath,
    schema: SchemaPath,
    world: World,
    origins: BTreeSet<Origin>,
    credentials: BTreeMap<CredentialName, Placement>,
    reads: PluginName,
}

/// The table as the file spells it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    #[serde(default)]
    from: RawFrom,
    component: String,
    component_sha256: Option<String>,
    schema: String,
    schema_sha256: Option<String>,
    world: String,
    #[serde(default)]
    origins: Vec<String>,
    #[serde(default)]
    credentials: Vec<RawCredential>,
    reads: Option<String>,
}

/// Which repository the file names, before its pins are read.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawFrom {
    #[default]
    App,
    User,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "into", rename_all = "snake_case", deny_unknown_fields)]
enum RawCredential {
    Url {
        name: String,
    },
    Bearer {
        name: String,
        origin: String,
    },
    Header {
        name: String,
        header: String,
        origin: String,
    },
}

/// Why a `[[plugins]]` table was refused; it names the offending text, and a manifest holds no
/// secret to leak.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// Not `[a-z][a-z0-9]{0,31}`.
    #[error("{0:?} is not a plugin name (a lowercase letter, then lowercase letters or digits)")]
    BadName(String),
    /// Not a relative `.wasm` path of plain segments.
    #[error("{0:?} is not a component path (relative, plain segments, ending in .wasm)")]
    BadComponent(String),
    /// Not a relative `.schema` path of plain segments.
    #[error("{0:?} is not a schema path (relative, plain segments, ending in .schema)")]
    BadSchema(String),
    /// A world this host does not have.
    #[error(
        "{0:?} is not a world this host has (source@{PACKAGE_VERSION} or actor@{PACKAGE_VERSION})"
    )]
    UnknownWorld(String),
    /// An origin or a credential for a world that fetches nothing: an authority its plugin could
    /// never use, which a manifest does not get to ask for.
    #[error("{named} is named for a plugin of the {world} world, which fetches nothing")]
    NoFetch {
        /// The world.
        world: String,
        /// The first origin or credential it named.
        named: String,
    },
    /// A store named for a plugin that is no actor: a source writes its own store and reads
    /// none.
    #[error("{0} names a store to read, and only an actor reads one")]
    ReadsForSource(String),
    /// Not 64 lowercase hex digits.
    #[error("{0:?} is not a sha256 (64 lowercase hex digits)")]
    BadDigest(String),
    /// A bundle from the user repository without the sha256 of its component and its schema:
    /// the owner approves files, not paths.
    #[error("a bundle from the user repository names its {0}_sha256, the file the owner approved")]
    Unpinned(&'static str),
    /// A pin on an app bundle, which the flake builds from this repository and nothing pins.
    #[error("only a bundle from the user repository is pinned by sha256")]
    PinnedApp,
    /// A world at a version this host was not built against.
    #[error("the world is at version {asked}, and this host has {have}")]
    WorldVersion {
        /// The version the manifest named.
        asked: String,
        /// The version this host was built against.
        have: &'static str,
    },
    /// Not `http(s)://host[:port]`.
    #[error("{0:?} is not an origin (http(s)://host[:port], nothing after it)")]
    BadOrigin(String),
    /// An origin named twice.
    #[error("{0} is allowed twice")]
    DuplicateOrigin(String),
    /// A credential outside `PLUGIN_<NAME>_`.
    #[error("{name:?} is not a credential this plugin may name: it must start with {prefix}")]
    CredentialOutsideNamespace {
        /// The name the manifest gave.
        name: String,
        /// The namespace it had to be in.
        prefix: String,
    },
    /// A credential declared twice.
    #[error("{0} is declared twice")]
    DuplicateCredential(String),
    /// A header that is not a token, or one the host owns.
    #[error("{0:?} is not a header a credential may go in")]
    BadHeader(String),
    /// A credential bound to an origin it would travel to in the clear.
    #[error("{name} is bound to {origin}, which is neither https nor loopback")]
    CredentialInTheClear {
        /// The credential.
        name: String,
        /// The origin it named.
        origin: String,
    },
    /// A credential bound to an origin the manifest does not allow.
    #[error("{name} is bound to {origin}, which is not one of this plugin's origins")]
    CredentialOriginNotAllowed {
        /// The credential.
        name: String,
        /// The origin it named.
        origin: String,
    },
}

impl TryFrom<Raw> for Manifest {
    type Error = ManifestError;

    fn try_from(raw: Raw) -> Result<Self, Self::Error> {
        let name = PluginName::parse(&raw.name)?;
        let mut origins = BTreeSet::new();
        for text in &raw.origins {
            let origin = Origin::parse(text)?;
            if !origins.insert(origin.clone()) {
                return Err(ManifestError::DuplicateOrigin(origin.to_string()));
            }
        }
        let mut credentials = BTreeMap::new();
        for raw in raw.credentials {
            let (credential, placement) = Self::credential(&name, &origins, raw)?;
            if credentials.contains_key(&credential) {
                return Err(ManifestError::DuplicateCredential(credential.to_string()));
            }
            credentials.insert(credential, placement);
        }
        let world = World::parse(&raw.world)?;
        if !world.fetches() {
            let named = origins
                .iter()
                .map(ToString::to_string)
                .chain(credentials.keys().map(ToString::to_string))
                .next();
            if let Some(named) = named {
                return Err(ManifestError::NoFetch {
                    world: world.to_string(),
                    named,
                });
            }
        }
        let reads = match raw.reads {
            None => name.clone(),
            Some(_) if world.fetches() => return Err(ManifestError::ReadsForSource(raw.name)),
            Some(other) => PluginName::parse(&other)?,
        };
        let from = match raw.from {
            RawFrom::App if raw.component_sha256.is_some() || raw.schema_sha256.is_some() => {
                return Err(ManifestError::PinnedApp);
            }
            RawFrom::App => BundleRoot::App,
            RawFrom::User => BundleRoot::User {
                component: Digest::parse(
                    raw.component_sha256
                        .as_deref()
                        .ok_or(ManifestError::Unpinned("component"))?,
                )?,
                schema: Digest::parse(
                    raw.schema_sha256
                        .as_deref()
                        .ok_or(ManifestError::Unpinned("schema"))?,
                )?,
            },
        };
        Ok(Self {
            from,
            component: ComponentPath::parse(&raw.component)?,
            schema: SchemaPath::parse(&raw.schema)?,
            world,
            name,
            origins,
            credentials,
            reads,
        })
    }
}

impl Manifest {
    /// The manifest of an app bundle with no credential, through the one door the file's
    /// tables go through: what a template declares when the instance has none of its plugin.
    ///
    /// # Errors
    /// The [`ManifestError`] the file's table would be refused with.
    pub fn declare(
        name: &str,
        component: &str,
        schema: &str,
        world: &str,
        origins: &[&str],
    ) -> Result<Self, ManifestError> {
        Self::try_from(Raw {
            name: name.to_owned(),
            from: RawFrom::App,
            component: component.to_owned(),
            component_sha256: None,
            schema: schema.to_owned(),
            schema_sha256: None,
            world: world.to_owned(),
            origins: origins.iter().map(|origin| (*origin).to_owned()).collect(),
            credentials: Vec::new(),
            reads: None,
        })
    }

    /// This manifest with one more `url` credential, refused as the file would refuse it.
    ///
    /// # Errors
    /// A name outside its namespace, or one it already declares.
    pub fn with_link(&self, name: &str) -> Result<Self, ManifestError> {
        let credential = CredentialName::parse(&self.name, name)?;
        if self.credentials.contains_key(&credential) {
            return Err(ManifestError::DuplicateCredential(credential.to_string()));
        }
        let mut credentials = self.credentials.clone();
        credentials.insert(credential, Placement::Url);
        Ok(Self {
            credentials,
            ..self.clone()
        })
    }

    /// This manifest without the credential `name`, the same manifest when it declares none.
    #[must_use]
    pub fn without_credential(&self, name: &str) -> Self {
        Self {
            credentials: self
                .credentials
                .iter()
                .filter(|(declared, _)| declared.as_str() != name)
                .map(|(declared, placement)| (declared.clone(), placement.clone()))
                .collect(),
            ..self.clone()
        }
    }

    /// This manifest allowed `origin` too; allowing twice is allowing once.
    ///
    /// # Errors
    /// [`ManifestError::NoFetch`] for a world that fetches nothing.
    pub fn with_origin(&self, origin: Origin) -> Result<Self, ManifestError> {
        if !self.world.fetches() {
            return Err(ManifestError::NoFetch {
                world: self.world.to_string(),
                named: origin.to_string(),
            });
        }
        let mut origins = self.origins.clone();
        origins.insert(origin);
        Ok(Self {
            origins,
            ..self.clone()
        })
    }

    fn credential(
        plugin: &PluginName,
        origins: &BTreeSet<Origin>,
        raw: RawCredential,
    ) -> Result<(CredentialName, Placement), ManifestError> {
        let bound = |name: &str, origin: &str| {
            let origin = Origin::parse(origin)?;
            if !origins.contains(&origin) {
                Err(ManifestError::CredentialOriginNotAllowed {
                    name: name.to_owned(),
                    origin: origin.to_string(),
                })
            } else if !origin.is_confidential() {
                Err(ManifestError::CredentialInTheClear {
                    name: name.to_owned(),
                    origin: origin.to_string(),
                })
            } else {
                Ok(origin)
            }
        };
        match raw {
            RawCredential::Url { name } => {
                Ok((CredentialName::parse(plugin, &name)?, Placement::Url))
            }
            RawCredential::Bearer { name, origin } => Ok((
                CredentialName::parse(plugin, &name)?,
                Placement::Bearer {
                    origin: bound(&name, &origin)?,
                },
            )),
            RawCredential::Header {
                name,
                header,
                origin,
            } => Ok((
                CredentialName::parse(plugin, &name)?,
                Placement::Header {
                    header: HeaderName::parse(&header).ok_or(ManifestError::BadHeader(header))?,
                    origin: bound(&name, &origin)?,
                },
            )),
        }
    }

    /// The plugin's name, and its credentials' namespace.
    #[must_use]
    pub fn name(&self) -> &PluginName {
        &self.name
    }

    /// The repository its component and schema are read from.
    #[must_use]
    pub fn root(&self) -> &BundleRoot {
        &self.from
    }

    /// Where its component lives, relative to the root the binary resolves it against.
    #[must_use]
    pub fn component(&self) -> &ComponentPath {
        &self.component
    }

    /// Where its declared schema lives, beside its component.
    #[must_use]
    pub fn schema(&self) -> &SchemaPath {
        &self.schema
    }

    /// The world its component must target.
    #[must_use]
    pub fn world(&self) -> World {
        self.world
    }

    /// The one plugin whose store it reads: its own, or for an actor the one its manifest names
    /// (`reads`), which the owner grants it by writing it there.
    #[must_use]
    pub fn reads(&self) -> &PluginName {
        &self.reads
    }

    /// Whether a request to `origin` may be sent at all.
    #[must_use]
    pub fn allows(&self, origin: &Origin) -> bool {
        self.origins.contains(origin)
    }

    /// Every origin it may reach.
    pub fn origins(&self) -> impl Iterator<Item = &Origin> {
        self.origins.iter()
    }

    /// Where a declared credential goes, looked up by the name a plugin wrote; `None` for any
    /// name it did not declare, including every name outside its namespace.
    #[must_use]
    pub fn placement(&self, name: &str) -> Option<(&CredentialName, &Placement)> {
        self.credentials
            .iter()
            .find(|(declared, _)| declared.as_str() == name)
    }

    /// Every credential it declares, each a name the box files a card for while nothing stores it.
    pub fn credentials(&self) -> impl Iterator<Item = (&CredentialName, &Placement)> {
        self.credentials.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Instance {
        plugins: Vec<Manifest>,
    }

    fn parse(toml: &str) -> Result<Manifest, String> {
        toml::from_str::<Instance>(toml)
            .map(|mut instance| instance.plugins.remove(0))
            .map_err(|failed| failed.message().to_owned())
    }

    const CALENDAR: &str = r#"
        [[plugins]]
        name = "calendar"
        component = "plugins/ics.wasm"
        schema = "plugins/ics.schema"
        world = "source@0.3.0"
        origins = ["https://calendar.google.com", "https://api.example.com"]

        [[plugins.credentials]]
        name = "PLUGIN_CALENDAR_ICS_URL"
        into = "url"

        [[plugins.credentials]]
        name = "PLUGIN_CALENDAR_TOKEN"
        into = "bearer"
        origin = "https://api.example.com"

        [[plugins.credentials]]
        name = "PLUGIN_CALENDAR_KEY"
        into = "header"
        header = "X-Api-Key"
        origin = "https://api.example.com"
    "#;

    #[test]
    fn a_manifest_parses_once_into_its_authorities() {
        let manifest = parse(CALENDAR).expect("the calendar manifest");
        assert_eq!(manifest.name().as_str(), "calendar");
        assert_eq!(manifest.component().as_str(), "plugins/ics.wasm");
        assert_eq!(manifest.schema().as_str(), "plugins/ics.schema");
        assert_eq!(manifest.world(), World::Source);
        let api = Origin::parse("https://api.example.com").expect("an origin");
        assert!(manifest.allows(&api));
        assert!(!manifest.allows(&Origin::parse("https://example.com").expect("an origin")));
        assert_eq!(
            manifest.placement("PLUGIN_CALENDAR_KEY").map(|(_, at)| at),
            Some(&Placement::Header {
                header: HeaderName::parse("x-api-key").expect("a header"),
                origin: api,
            })
        );
        assert_eq!(
            manifest
                .placement("PLUGIN_CALENDAR_ICS_URL")
                .map(|(_, at)| at),
            Some(&Placement::Url)
        );
        assert_eq!(manifest.placement("ANTHROPIC_API_KEY"), None);
        assert_eq!(manifest.credentials().count(), 3);
    }

    #[test]
    fn a_credential_is_never_bound_to_an_origin_in_the_clear() {
        let with = |origin: &str| {
            parse(&format!(
                r#"
                [[plugins]]
                name = "plain"
                component = "plain.wasm"
                schema = "plain.schema"
                world = "source@0.3.0"
                origins = ["{origin}"]
                credentials = [{{ name = "PLUGIN_PLAIN_TOKEN", into = "bearer", origin = "{origin}" }}]
                "#
            ))
        };
        let said = with("http://api.example.com").expect_err("a token in the clear");
        assert!(said.contains("neither https nor loopback"), "{said}");
        assert!(with("https://api.example.com").is_ok());
        assert!(
            with("http://127.0.0.1:9000").is_ok(),
            "loopback never leaves the box"
        );
    }

    #[test]
    fn a_manifest_with_no_credentials_or_origins_is_one_that_asks_for_nothing() {
        let manifest = parse(
            r#"
            [[plugins]]
            name = "quiet"
            component = "quiet.wasm"
            schema = "quiet.schema"
            world = "source@0.3.0"
            "#,
        )
        .expect("a manifest");
        assert_eq!(manifest.origins().count(), 0);
        assert_eq!(manifest.credentials().count(), 0);
    }

    /// Each authority the file could overreach with is refused at the boundary, by name.
    #[test]
    fn every_overreach_is_refused_when_the_file_is_read() {
        let refused = |from: &str, to: &str| {
            let toml = CALENDAR.replacen(from, to, 1);
            assert_ne!(toml, CALENDAR, "{from:?} is not in the fixture");
            parse(&toml).expect_err(&format!("{from:?} -> {to:?} should be refused"))
        };
        let said = refused(
            r#"name = "PLUGIN_CALENDAR_TOKEN""#,
            r#"name = "ANTHROPIC_API_KEY""#,
        );
        assert!(said.contains("must start with PLUGIN_CALENDAR_"), "{said}");
        let said = refused(
            "origin = \"https://api.example.com\"\n",
            "origin = \"https://evil.example\"\n",
        );
        assert!(said.contains("not one of this plugin's origins"), "{said}");
        let said = refused(r#"header = "X-Api-Key""#, r#"header = "Host""#);
        assert!(said.contains("not a header"), "{said}");
        let said = refused(r#"world = "source@0.3.0""#, r#"world = "source@0.1.0""#);
        assert!(said.contains("this host has 0.3.0"), "{said}");
        let said = refused(
            r#"component = "plugins/ics.wasm""#,
            r#"component = "../ics.wasm""#,
        );
        assert!(said.contains("not a component path"), "{said}");
        let said = refused(
            r#"schema = "plugins/ics.schema""#,
            r#"schema = "../ics.schema""#,
        );
        assert!(said.contains("not a schema path"), "{said}");
        let said = refused(
            r#""https://calendar.google.com", "https://api.example.com""#,
            r#""https://calendar.google.com", "https://calendar.google.com:443""#,
        );
        assert!(said.contains("allowed twice"), "{said}");
        let said = refused(
            r#"name = "PLUGIN_CALENDAR_KEY""#,
            r#"name = "PLUGIN_CALENDAR_TOKEN""#,
        );
        assert!(said.contains("declared twice"), "{said}");
        let said = refused(r#"into = "url""#, r#"into = "env""#);
        assert!(said.contains("unknown variant"), "{said}");
        let said = refused(
            r#"into = "url""#,
            "into = \"url\"\norigin = \"https://api.example.com\"",
        );
        assert!(said.contains("unknown field"), "{said}");
        let said = refused(
            r#"name = "calendar""#,
            "name = \"calendar\"\nsockets = true",
        );
        assert!(said.contains("unknown field"), "{said}");
        let said = refused(r#"world = "source@0.3.0""#, r#"world = "actor@0.3.0""#);
        assert!(said.contains("which fetches nothing"), "{said}");
    }

    /// A bundle is the app's unless the manifest says it is the user's; no third place exists.
    #[test]
    fn a_bundle_is_the_app_s_or_the_user_s() {
        let from = |line: &str| {
            parse(&CALENDAR.replacen(
                "[[plugins]]\n",
                &format!("[[plugins]]\n        {line}\n"),
                1,
            ))
            .map(|manifest| manifest.root().clone())
        };
        let pin = |byte: char| byte.to_string().repeat(64);
        assert_eq!(from(""), Ok(BundleRoot::App));
        assert_eq!(from(r#"from = "app""#), Ok(BundleRoot::App));
        assert_eq!(
            from(&format!(
                "from = \"user\"\n        component_sha256 = \"{}\"\n        schema_sha256 = \"{}\"",
                pin('a'),
                pin('b')
            )),
            Ok(BundleRoot::User {
                component: Digest::parse(&pin('a')).expect("a pin"),
                schema: Digest::parse(&pin('b')).expect("a pin"),
            })
        );
        let said = from(r#"from = "user""#).expect_err("a user bundle is pinned");
        assert!(said.contains("component_sha256"), "{said}");
        let said = from(&format!(
            "from = \"user\"\n        component_sha256 = \"{}\"",
            pin('a')
        ))
        .expect_err("both files are pinned");
        assert!(said.contains("schema_sha256"), "{said}");
        let said = from(&format!("component_sha256 = \"{}\"", pin('a')))
            .expect_err("an app bundle is not pinned");
        assert!(
            said.contains("only a bundle from the user repository"),
            "{said}"
        );
        let said = from(r#"from = "web""#).expect_err("no third root");
        assert!(said.contains("unknown variant"), "{said}");
    }

    /// An actor asks for no origin and no credential, and has neither.
    #[test]
    fn an_actor_asks_for_nothing_it_could_fetch_with() {
        let manifest = parse(
            r#"
            [[plugins]]
            name = "invites"
            component = "invites.wasm"
            schema = "invites.schema"
            world = "actor@0.3.0"
            "#,
        )
        .expect("a manifest");
        assert_eq!(manifest.world(), World::Actor);
        assert_eq!(manifest.origins().count(), 0);
        assert_eq!(manifest.credentials().count(), 0);
    }

    /// An actor reads its own store, or the one store its manifest names; a source names none.
    #[test]
    fn an_actor_reads_its_own_store_or_the_one_it_is_granted() {
        let manifest = |world: &str, reads: &str| {
            parse(&format!(
                r#"
                [[plugins]]
                name = "due"
                component = "due.wasm"
                schema = "ics.schema"
                world = "{world}@0.3.0"
                {reads}
                "#
            ))
        };
        assert_eq!(
            manifest("actor", "").expect("a manifest").reads().as_str(),
            "due"
        );
        assert_eq!(
            manifest("actor", r#"reads = "ics""#)
                .expect("a manifest")
                .reads()
                .as_str(),
            "ics"
        );
        assert!(manifest("actor", r#"reads = "Ics""#).is_err());
        let said = manifest("source", r#"reads = "ics""#).expect_err("a source reads no store");
        assert!(said.contains("only an actor reads one"), "{said}");
    }
}

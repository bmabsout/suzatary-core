//! A credential a plugin may name, and where the host puts its value.

use std::fmt;

use super::{HeaderName, ManifestError, Origin, PluginName};

/// The longest credential name.
const MAX: usize = 96;

/// A secret's name in the plugin's own namespace, `PLUGIN_<NAME>_[A-Z0-9_]+`.
///
/// The namespace is the fence a manifest cannot reach around: the app's own keys, the release
/// secrets and another plugin's credentials are outside it, so naming one does not parse.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialName {
    name: String,
    /// Where the part after the plugin's namespace begins.
    label: usize,
}

impl CredentialName {
    /// Parse one in `plugin`'s namespace.
    pub fn parse(plugin: &PluginName, text: &str) -> Result<Self, ManifestError> {
        let prefix = plugin.credential_prefix();
        let rest = text.strip_prefix(&prefix).unwrap_or_default();
        let shaped = !rest.is_empty()
            && text.len() <= MAX
            && rest
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
        if shaped {
            Ok(Self {
                name: text.to_owned(),
                label: prefix.len(),
            })
        } else {
            Err(ManifestError::CredentialOutsideNamespace {
                name: text.to_owned(),
                prefix,
            })
        }
    }

    /// The name, as secretspec spells it; the box's alone, never drawn.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// The part after the plugin's namespace, `WORK` of `PLUGIN_ICS_WORK`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.name[self.label..]
    }
}

impl fmt::Display for CredentialName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Where a credential's value goes, which is also the one use a plugin may name it for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The value IS the URL (a calendar's secret iCal link); its origin must still be allowed.
    Url,
    /// `authorization: Bearer <value>`, on requests to `origin` only.
    Bearer {
        /// The one origin this credential is sent to.
        origin: Origin,
    },
    /// `<header>: <value>`, on requests to `origin` only.
    Header {
        /// The header the value goes in.
        header: HeaderName,
        /// The one origin this credential is sent to.
        origin: Origin,
    },
}

impl Placement {
    /// The manifest's word for it, `into = "url"`.
    #[must_use]
    pub const fn into(&self) -> &'static str {
        match self {
            Placement::Url => "url",
            Placement::Bearer { .. } => "bearer",
            Placement::Header { .. } => "header",
        }
    }

    /// The origin a header credential is bound to; `None` for a URL, whose value names its own.
    #[must_use]
    pub fn origin(&self) -> Option<&Origin> {
        match self {
            Placement::Url => None,
            Placement::Bearer { origin } | Placement::Header { origin, .. } => Some(origin),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_lives_in_its_plugin_namespace() {
        let calendar = PluginName::parse("calendar").expect("a name");
        assert!(CredentialName::parse(&calendar, "PLUGIN_CALENDAR_ICS_URL").is_ok());
        for bad in [
            "ANTHROPIC_API_KEY",
            "SUZATARY_ROUTINE_TOKEN",
            "PLUGIN_BANK_TOKEN",
            "PLUGIN_CALENDAR_",
            "PLUGIN_CALENDAR_ics",
            "plugin_calendar_ICS",
            "PLUGIN_CALENDAR_ICS-URL",
        ] {
            assert!(
                matches!(
                    CredentialName::parse(&calendar, bad),
                    Err(ManifestError::CredentialOutsideNamespace { .. })
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn no_name_is_in_two_namespaces() {
        let ics = PluginName::parse("ics").expect("a name");
        let icsfeed = PluginName::parse("icsfeed").expect("a name");
        let theirs = "PLUGIN_ICSFEED_TOKEN";
        assert!(CredentialName::parse(&icsfeed, theirs).is_ok());
        assert!(CredentialName::parse(&ics, theirs).is_err());
    }
}

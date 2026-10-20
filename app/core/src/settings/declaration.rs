//! What declares a secret this deployment reads: the table of the instance file that names it.

use crate::plugin::PluginName;
use crate::settings::Preset;
use crate::settings::remote::Pushed;

/// One table naming a secret; a name two tables declare is declared by both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Declaration {
    /// A built-in backend, which every deployment declares whether or not a job orders it.
    Backend(Preset),
    /// A `[[providers]]` entry's key, by the provider's name.
    Provider(String),
    /// A `[notify]` channel's secret.
    Channel(crate::connect::Channel),
    /// `[coding]`'s routine, whose fire token it is.
    Coding,
    /// The data repository, which every deployment pushes its stores to.
    DataPush,
    /// `[[secretary.repos]]`, the repositories the secretary reads and proposes patches to.
    Repos,
    /// The box's one Google Cloud client, which every Google account signs in through.
    GoogleClient,
    /// A Google account's sign-in with Google, which the box keeps, by the account.
    GoogleGrant(String),
    /// A `[[plugins]]` manifest's credential, and the use it is put to (`url`, `bearer`,
    /// `header`).
    Plugin {
        /// The plugin.
        plugin: PluginName,
        /// Where its value goes.
        into: &'static str,
    },
}

impl Declaration {
    /// Whether a table of the instance file declares it; a built-in backend's key is declared
    /// by no table, so its absence is no fault of the file's.
    #[must_use]
    pub const fn by_a_table(&self) -> bool {
        !matches!(self, Self::Backend(_))
    }

    /// Whether its name is asked for by a card of its own rather than a Secret card: the Google
    /// client, by the card that takes Google's downloaded file whole and checks its redirect,
    /// and a Google account's grant, which only its sign-in with Google stores.
    #[must_use]
    pub const fn own_card(&self) -> bool {
        matches!(self, Self::GoogleClient | Self::GoogleGrant(_))
    }

    /// Whether what it declares is in use, so its secret is asked for: a provider while it is
    /// not `idle`, the data repository's push while `pushed` needs its token, every other table
    /// always.
    #[must_use]
    pub fn in_use(&self, idle: impl Fn(&str) -> bool, pushed: &Pushed) -> bool {
        match self {
            Self::Provider(name) => !idle(name),
            Self::DataPush => pushed.needs_push_token(),
            Self::Backend(_)
            | Self::Channel(_)
            | Self::Coding
            | Self::Repos
            | Self::Plugin { .. }
            | Self::GoogleClient
            | Self::GoogleGrant(_) => true,
        }
    }

    /// What declares it, as a sentence names it: `the phala provider's key`.
    #[must_use]
    pub fn sentence(&self) -> String {
        match self {
            Self::Backend(preset) => format!("the {} backend's key", preset.backend()),
            Self::Provider(name) => format!("the {name} provider's key"),
            Self::Channel(name) => format!("the {name} notification channel"),
            Self::Coding => "the coding routine's fire token".to_owned(),
            Self::DataPush => "the push of the data repository".to_owned(),
            Self::Repos => "the secretary's repositories".to_owned(),
            Self::Plugin { plugin, into } => format!("the {plugin} plugin's {into} credential"),
            Self::GoogleClient => "every Google account's sign-in".to_owned(),
            Self::GoogleGrant(account) => format!("{account}'s mail and calendars"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_declaration_says_what_declares_it() {
        let ics = PluginName::parse("ics").expect("a name");
        assert_eq!(
            Declaration::Plugin {
                plugin: ics,
                into: "url"
            }
            .sentence(),
            "the ics plugin's url credential"
        );
        assert_eq!(
            Declaration::Provider("phala".to_owned()).sentence(),
            "the phala provider's key"
        );
        assert!(!Declaration::Backend(Preset::AnthropicApi).by_a_table());
        assert!(Declaration::Coding.by_a_table());
    }

    /// A provider's key is in use while it is not idle, the push token while the data repository
    /// pushes to GitHub, and every other table's secret while the table is there.
    #[test]
    fn a_provider_s_key_and_the_push_token_fall_out_of_use() {
        let nowhere = Pushed::Nowhere;
        let idle = |_: &str| true;
        let busy = |_: &str| false;
        let phala = Declaration::Provider("phala".to_owned());
        assert!(!phala.in_use(idle, &nowhere));
        assert!(phala.in_use(busy, &nowhere));
        assert!(!Declaration::DataPush.in_use(busy, &nowhere));
        assert!(Declaration::DataPush.in_use(
            busy,
            &Pushed::of_git_config(
                "[remote \"origin\"]\n\turl = https://github.com/ada/data.git\n"
            )
        ));
        for declaration in [
            Declaration::Backend(Preset::AnthropicApi),
            Declaration::Channel(crate::connect::Channel::Ntfy),
            Declaration::Coding,
            Declaration::Repos,
            Declaration::Plugin {
                plugin: PluginName::parse("ics").expect("a name"),
                into: "url",
            },
        ] {
            assert!(declaration.in_use(idle, &nowhere), "{declaration:?}");
        }
    }
}

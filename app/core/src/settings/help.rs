//! What the owner is told when the box asks for a secret: the template's words and, where one is
//! known, the page of the service that issues it. Data, keyed by the name, since every name a
//! deployment may set is derived from the template that declares it.

use crate::coding::RoutineId;
use crate::settings::Preset;

/// How to find one secret's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Help {
    /// One sentence: where the value is found.
    pub text: &'static str,
    /// The page that issues it; always https, and only ever this table's or the declared
    /// routine's own.
    pub link: Option<String>,
}

/// Where a secret's value is issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// No page is known.
    Unknown,
    /// One fixed page.
    At(&'static str),
    /// The page of the routine `[coding]` declares, which only a deployment knows.
    Routine,
}

/// A secret's words as data: what a template's step says of it, and where its value is issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Words {
    /// What the owner calls it; in a plugin's link, `{label}` stands for the link's own label.
    pub label: &'static str,
    /// One sentence: where the value is found.
    pub text: &'static str,
    /// Where it is issued.
    pub page: Page,
}

impl Help {
    /// The help for the secret named `name` on a deployment whose `[coding]` declares `routine`;
    /// `None` for a name no template here knows, and for one issued by a routine none declares.
    #[must_use]
    pub fn of(name: &str, routine: Option<&RoutineId>) -> Option<Self> {
        let Words { text, page, .. } = Words::of(name)?;
        let link = match page {
            Page::Unknown => None,
            Page::At(href) => Some(href.to_owned()),
            Page::Routine => Some(routine?.page()),
        };
        Some(Self { text, link })
    }

    /// What the owner calls the secret named `name`: the table's label, a plugin's link named by
    /// its own label, or for a name no template here knows, the plugin it belongs to, or that it
    /// is a key the deployment declares.
    #[must_use]
    pub fn label(name: &str) -> String {
        let credential = name
            .strip_prefix("PLUGIN_")
            .and_then(|rest| rest.split_once('_'));
        match (Words::of(name), credential) {
            (Some(words), Some((_, suffix))) => words.label.replace(
                "{label}",
                &crate::connect::Label::of_suffix(suffix)
                    .map_or_else(|_| suffix.to_owned(), |label| label.shown()),
            ),
            (Some(words), None) => words.label.to_owned(),
            (None, Some((plugin, _))) => {
                format!("a credential of the {} plugin", plugin.to_lowercase())
            }
            (None, None) => "a key this deployment declares".to_owned(),
        }
    }
}

impl Words {
    /// The words for the secret named `name`: its template's step's, or for a name no default
    /// template asks for, this table's.
    fn of(name: &str) -> Option<Self> {
        crate::connect::secret_words(name).or_else(|| {
            Preset::ALL
                .into_iter()
                .find(|preset| preset.key_name() == name)
                .and_then(Self::of_preset)
                .or(match name {
                    crate::coding::ROUTINE_TOKEN => Some(Self {
                        label: "the coding routine's token",
                        text: "On the routine's page, generate its API trigger's token.",
                        page: Page::Routine,
                    }),
                    // Set at a terminal alone, as a mail transport's secret is; named here
                    // so a page says what it is without its name.
                    "PROTON_BRIDGE_PASSWORD" => Some(Self {
                        label: "the Proton Bridge password",
                        text: "In `protonmail-bridge --cli`, `info` prints it.",
                        page: Page::Unknown,
                    }),
                    crate::google::CLIENT_ID | crate::google::CLIENT_SECRET => Some(Self {
                        label: "the box's Google client",
                        text: "Make it once in the Google Cloud console, from the card that asks \
                               for it, and paste its downloaded file there.",
                        page: Page::At(crate::google::CONSOLE),
                    }),
                    grant if grant.starts_with("GOOGLE_GRANT_") => Some(Self {
                        label: "a Google account's sign-in with Google",
                        text: "Sign the account in with Google from its card; the box keeps \
                               what Google grants.",
                        page: Page::Unknown,
                    }),
                    crate::settings::DATA_PUSH_TOKEN => Some(Self {
                        label: "the data repository's push token",
                        text: "On GitHub, create a fine-grained token for the data repository \
                               alone, with Contents read and write.",
                        page: Page::At("https://github.com/settings/personal-access-tokens/new"),
                    }),
                    crate::repos::TOKEN => Some(Self {
                        label: "the repositories' token",
                        text: "On GitHub, create a fine-grained token for the repositories the \
                               secretary reaches alone, with Contents and Pull requests read \
                               and write.",
                        page: Page::At("https://github.com/settings/personal-access-tokens/new"),
                    }),
                    _ => None,
                })
        })
    }

    /// A declared provider's key, which no default template asks for, and the subscription's
    /// token, which its template's sign-in stores; the Messages API's key is its template's.
    const fn of_preset(preset: Preset) -> Option<Self> {
        Some(match preset {
            Preset::ClaudeSubscription => Self {
                label: "the Claude sign-in",
                text: "Sign in with your Claude subscription from the Connect a model card, or \
                       run `claude setup-token` in a terminal.",
                page: Page::Unknown,
            },
            Preset::AnthropicApi => return None,
            Preset::Databricks => Self {
                label: "the Databricks token",
                text: "In your Databricks workspace, User settings, Developer: create an access \
                       token.",
                page: Page::Unknown,
            },
            Preset::Phala => Self {
                label: "the Phala key",
                text: "In Phala's dashboard, create an API key.",
                page: Page::Unknown,
            },
            Preset::OpenRouter => Self {
                label: "the OpenRouter key",
                text: "In OpenRouter's settings, create an API key.",
                page: Page::At("https://openrouter.ai/settings/keys"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every link is https, so a card never draws another scheme as a button.
    #[test]
    fn every_link_is_https() {
        let names = Preset::ALL
            .iter()
            .map(|preset| preset.key_name())
            .chain(["NTFY_TOPIC", "FCM_SERVICE_ACCOUNT_B64", "FCM_SENDER_ID"])
            .chain([
                crate::coding::ROUTINE_TOKEN,
                crate::settings::DATA_PUSH_TOKEN,
                crate::repos::TOKEN,
                "PLUGIN_ICS_WORK",
            ]);
        let routine = routine();
        for name in names {
            let help = Help::of(name, Some(&routine)).unwrap_or_else(|| panic!("{name} has help"));
            assert!(
                help.link.is_none_or(|link| link.starts_with("https://")),
                "{name}"
            );
        }
    }

    fn routine() -> RoutineId {
        "trig_01Example".parse().expect("a routine id")
    }

    /// The token's card opens the declared routine's own page, where its trigger's token is
    /// generated; with no routine declared there is nothing to open, so no help.
    #[test]
    fn the_routine_token_opens_the_declared_routine() {
        let help = Help::of(crate::coding::ROUTINE_TOKEN, Some(&routine())).expect("help");
        assert_eq!(
            help.link.as_deref(),
            Some("https://claude.ai/code/routines/trig_01Example")
        );
        assert_eq!(Help::of(crate::coding::ROUTINE_TOKEN, None), None);
    }

    /// A page that is a table's does not depend on the routine.
    #[test]
    fn a_fixed_page_ignores_the_routine() {
        assert_eq!(
            Help::of("NTFY_TOPIC", None),
            Help::of("NTFY_TOPIC", Some(&routine()))
        );
    }

    #[test]
    fn a_name_no_template_knows_has_none() {
        assert_eq!(Help::of("PLUGIN_DUEDAY_TOKEN", Some(&routine())), None);
    }

    /// Every name has words the owner reads instead of it, the template's where one is known.
    #[test]
    fn every_name_is_labelled_in_words() {
        assert_eq!(Help::label("OPENROUTER_API_KEY"), "the OpenRouter key");
        assert_eq!(Help::label("NTFY_TOPIC"), "your notifications topic");
        assert_eq!(
            Help::label("PLUGIN_DUEDAY_TOKEN"),
            "a credential of the dueday plugin"
        );
        assert_eq!(Help::label("SOME_KEY"), "a key this deployment declares");
        assert_eq!(
            Help::label(crate::coding::ROUTINE_TOKEN),
            "the coding routine's token"
        );
    }
}

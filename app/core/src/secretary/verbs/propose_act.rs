use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mail::address::EmailAddress;
use crate::refusal::{Refusal, shown};
use crate::secretary::acts::Act;
use crate::secretary::store::Proposal;
use crate::secretary::verbs::{propose, reasons_of, serves_of};
use crate::secretary::{Call, SecretarySurface};
use crate::settings::Settable;
use crate::verb::{Caught, Verb};

/// Propose an act the box performs on a tap.
pub struct ProposeAct;

#[derive(Deserialize, JsonSchema)]
pub struct ProposeActArgs {
    act: String,
    #[serde(default)]
    account: Caught<Option<String>>,
    #[serde(default)]
    name: Caught<Option<String>>,
    #[serde(default)]
    job: Caught<Option<String>>,
    #[serde(default)]
    models: Caught<Option<Vec<String>>>,
    #[serde(default)]
    reserve: Caught<Option<f64>>,
    #[serde(default)]
    preset: Caught<Option<String>>,
    #[serde(default)]
    workspace: Caught<Option<String>>,
    #[serde(default)]
    expect_commit: Caught<Option<String>>,
    #[serde(default)]
    remove_provider: Caught<Option<String>>,
    #[serde(default)]
    template: Caught<Option<String>>,
    #[serde(default)]
    params: Caught<Option<Vec<String>>>,
    #[serde(default)]
    title: Caught<Option<String>>,
    #[serde(default)]
    start: Caught<Option<String>>,
    #[serde(default)]
    end: Caught<Option<String>>,
    #[serde(default)]
    place: Caught<Option<String>>,
    why: String,
    sources: String,
    serves: Option<String>,
    revises: Option<usize>,
}

// The two acts' own arguments are read only by the act that takes them.
impl Verb for ProposeAct {
    type Surface = SecretarySurface;
    type Args = ProposeActArgs;
    type Answer = Value;
    const NAME: &'static str = "propose_act";
    const DESCRIPTION: &'static str = "PROPOSE AN ACT this box will perform only if the owner taps it. It is \
        STORED with its exact arguments and shown to the owner whole; you cannot run \
        it and there is no verb that can. `act=\"auth\"` with `account=<address>` \
        asks the owner to authorise that Microsoft account for mail sync — the sign-in \
        URL and code arrive in this conversation as turns, so they never need a \
        terminal on the box. Use it when a sync is failing for want of a grant \
        and you can point at the evidence.\n\n\
        `act=\"secret\"` with `name=<NAME>` asks the owner to type a value for a secret \
        this deployment declares and does not currently resolve — `context` \
        names which ones. The value never reaches you; the tap that stores it is \
        the only place it travels. `name` must be one of the declared names or \
        the proposal is refused; do not guess at one context did not list.\n\n\
        `act=\"configure\"` asks the owner to change ONE model setting, which the box \
        reads now as the card's from: `job=<secretary|review|title>` with \
        `models=[\"<backend>/<model>\", ...]` (first choice first, `[]` for the \
        fallback) changes that job's order; `reserve=<0..1>` the share automated \
        jobs leave the owner; `preset=<databricks|phala|openrouter>` (with `workspace` \
        for Databricks, `expect_commit` for Phala) declares a provider; and \
        `remove_provider=<name>` takes one out. Give exactly one of the four. \
        ⚠️ CALL IT FIRST: in a round that has already read mail, a todo, a \
        document, the web or an old answer it is refused. The \
        card shows from and to, and the tap writes only while the file still \
        reads from.\n\n\
        `act=\"connect\"` asks the owner to add one thing to this deployment from the \
        box's catalog: `template=<name>` with `params=[...]`, one of \
        `ics` with `[\"<label>\"]` (a calendar by its secret iCal link, such as \
        `[\"Work\"]`), `google` with `[\"<address>\"]` (a Google account's mail and \
        calendars, by one sign-in with Google), `lieer` with `[\"<address>\"]` (a \
        Gmail account), \
        `imap-device-code` with `[\"<address>\", \"consumers\"]` for Outlook or \
        `[\"<address>\", \"organizations\"]` for Microsoft 365, `bridge` with \
        `[\"<address>\"]` (Proton Mail), `drive-cli` with `[\"<folder>\"]` (a Drive \
        folder you may read), and `ntfy` or `fcm` with `[]` (notifications). The \
        tap writes it into the instance file, and the box then asks on its own \
        cards for each step it needs (a link, a sign-in); never ask for a link or \
        a key yourself. Something already connected is refused. ⚠️ CALL IT \
        FIRST, as for configure: in a round that has read anything but the \
        owner's own words it is refused.\n\n\
        `act=\"calendar_event\"` with `account=<a Google account's address>`, \
        `title`, `start` and `end` (both UTC instants `2026-10-20T14:00:00Z`, or \
        both days `2026-10-20`, `end` the day after the last) and an optional \
        `place` asks the owner to add that event to the account's own calendar. \
        Nothing is written to any calendar unless the owner taps it.\n\n\
        ⚠️ `serves` and `revises` are the same two arguments `propose_email` \
        takes, and mean the same thing.";

    fn run(call: &Call<'_>, args: ProposeActArgs) -> Result<Value, Refusal> {
        let reasons = reasons_of(&args.why, &args.sources)?;
        let act =
            match args.act.as_str() {
                "auth" => {
                    let account_text = args.account.read()?.as_deref().ok_or_else(|| {
                        Refusal::bad_request("account is required for act=\"auth\"")
                    })?;
                    let account = EmailAddress::new(account_text)
                        .map_err(|_| Refusal::bad_request("account must be an email address"))?;
                    let roster = call.caps.mail.accounts();
                    if !roster.iter().any(|known| known == account.as_str()) {
                        return Err(Refusal::bad_request(format!(
                            "account must be one of this deployment's accounts ({}), got {}",
                            roster.join(", "),
                            shown(account.as_str())
                        )));
                    }
                    Act::Auth { account }
                }
                "secret" => {
                    // A name its own card asks for (the Google client, a sign-in with Google)
                    // is no Secret card's.
                    let settables = &call.settables.restricted(|by| !by.own_card());
                    let name_text = args.name.read()?.as_deref().ok_or_else(|| {
                        Refusal::bad_request("name is required for act=\"secret\"")
                    })?;
                    // A bad name's refusal gives the list's size, not its members, so a guess
                    // learns nothing.
                    let name = Settable::mint(settables, name_text).map_err(|_| {
                        Refusal::bad_request(format!(
                            "{} is not one of this deployment's {} declared secret name(s)",
                            shown(name_text),
                            settables.names().len()
                        ))
                    })?;
                    Act::Secret {
                        name: name.name().to_owned(),
                        why: reasons.why().to_owned(),
                    }
                }
                "configure" => Act::Configure {
                    change: configure(call, &args)?,
                },
                "connect" => Act::Connect {
                    connection: connect(call, &args)?,
                },
                "calendar_event" => calendar_event(call, &args)?,
                other => {
                    return Err(Refusal::bad_request(format!(
                        "act must be one of: auth, secret, configure, connect, calendar_event, \
                         got {}",
                        shown(other)
                    )));
                }
            };
        let index = propose(
            call.caps,
            call.conversation,
            Proposal::Act(act.clone()),
            &reasons,
            serves_of(args.serves.as_deref())?,
            args.revises,
        )?;
        let mut answer = json!({
            "proposed": index,
            "why": reasons.why(),
            "sources": reasons.sources().iter().map(ToString::to_string).collect::<Vec<_>>(),
            "next": "The owner taps it, dismisses it, or leaves it standing; you will see which",
        });
        // The answer names its kind, and only the field that kind carries.
        let object = answer.as_object_mut().expect("an object literal");
        object.insert("act".to_owned(), json!(act.kind()));
        let field = match &act {
            Act::Auth { account } => Some(("account", account.as_str().to_owned())),
            Act::Secret { name, .. } => Some(("name", name.clone())),
            Act::Web { request, .. } => Some(("request", request.sentence())),
            Act::Patch { repo, .. } => Some(("repo", repo.to_string())),
            Act::DriveRead { path, .. } => Some(("path", path.to_string())),
            Act::CodingSession { task } => Some(("task", task.as_str().to_owned())),
            Act::Device(enrolment) => Some(("key", enrolment.key.as_str().to_owned())),
            Act::Configure { change } => Some(("change", change.sentence())),
            Act::Connect { connection } => Some(("connection", connection.called())),
            Act::CalendarEvent { event, .. } => Some(("event", event.sentence())),
            // The box's own, never this verb's.
            Act::AuthSend { .. }
            | Act::ClaudeSignIn { .. }
            | Act::SignInCode { .. }
            | Act::UnlockStore { .. }
            | Act::Disconnect { .. }
            | Act::Grant { .. }
            | Act::GoogleClient { .. }
            | Act::GoogleSignIn { .. }
            | Act::Erase { .. } => None,
        };
        if let Some((name, value)) = field {
            object.insert(name.to_owned(), json!(value));
        }
        Ok(answer)
    }
}

/// An event for a Google account's own calendar, as `args` spell it; whether the account is a
/// Google account is the tap's to read, from the instance file as it then stands.
fn calendar_event(call: &Call<'_>, args: &ProposeActArgs) -> Result<Act, Refusal> {
    let required = |field: &Caught<Option<String>>, name: &str| -> Result<String, Refusal> {
        field.read()?.clone().ok_or_else(|| {
            Refusal::bad_request(format!("{name} is required for act=\"calendar_event\""))
        })
    };
    let account = EmailAddress::new(&required(&args.account, "account")?)
        .map_err(|_| Refusal::bad_request("account must be an email address"))?;
    let roster = call.caps.mail.accounts();
    if !roster.iter().any(|known| known == account.as_str()) {
        return Err(Refusal::bad_request(format!(
            "account must be one of this deployment's accounts ({}), got {}",
            roster.join(", "),
            shown(account.as_str())
        )));
    }
    let span = format!(
        "{}/{}",
        required(&args.start, "start")?,
        required(&args.end, "end")?
    );
    let when = crate::calendar::When::parse(&span).map_err(|_| {
        Refusal::bad_request(
            "start and end must be two UTC instants (2026-10-20T14:00:00Z) or two days \
             (2026-10-20), the end after the start",
        )
    })?;
    let event = crate::google::NewEvent::new(
        &required(&args.title, "title")?,
        when,
        args.place.read()?.as_deref().unwrap_or_default(),
    )
    .map_err(Refusal::bad_request)?;
    Ok(Act::CalendarEvent { account, event })
}

/// What a round has read when a Configure or a Connect card is refused in it.
const READ_OUTSIDE: &str = "this round has read something that is not the owner's own words \
                            (mail, a todo, a document, the web, a repository or an old answer)";

/// Why a Configure card is refused in a round that read anything but the owner's own words.
const CONFIGURE_OUTSIDE: &str = "a model setting is a vendor and a trust anchor the owner names, \
                                 never one learned from what was read";

/// Why a Connect card is refused in a round that read anything but the owner's own words.
const CONNECT_OUTSIDE: &str = "an account, a calendar or a folder is one the owner names, never \
                               one learned from what was read";

/// The refusal in a round that read outside the owner's words, or nothing in a clean one.
fn owners_words_only(call: &Call<'_>, why: &str) -> Result<(), Refusal> {
    if crate::secretary::taint::tainted(&crate::secretary::read(
        call.caps.conversations,
        call.conversation,
    )?) {
        return Err(Refusal::forbidden(format!(
            "{READ_OUTSIDE}, and {why}: say so, and the owner asks again in a round of its own"
        )));
    }
    Ok(())
}

/// The connection `args` names, refused where the instance file already holds it or holds no
/// table for it, as the tap would refuse it. A round that read anything but the owner's words
/// proposes none, since an address or a folder could then be an email's.
fn connect(call: &Call<'_>, args: &ProposeActArgs) -> Result<crate::connect::Connection, Refusal> {
    owners_words_only(call, CONNECT_OUTSIDE)?;
    let template = args
        .template
        .read()?
        .as_deref()
        .ok_or_else(|| Refusal::bad_request("template is required for act=\"connect\""))?;
    let params = args.params.read()?.clone().unwrap_or_default();
    let connection = crate::connect::Connection::parse(template, &params)
        .map_err(|refused| Refusal::bad_request(format!("no card was filed: {refused}")))?;
    let declared = call
        .caps
        .configuration
        .declared()
        .map_err(|failed| Refusal::internal(failed.to_string()))?;
    declared
        .with(&connection)
        .map_err(|refused| Refusal::bad_request(format!("no card was filed: {refused}")))?;
    Ok(connection)
}

/// The one setting `args` names, read against what the instance file declares now: that is the
/// card's from. A round that read anything but the owner's words proposes none, since a model,
/// an endpoint or a commit a receipt must name could then be an email's.
fn configure(
    call: &Call<'_>,
    args: &ProposeActArgs,
) -> Result<crate::settings::configure::Configure, Refusal> {
    use crate::settings::configure::Configure;
    owners_words_only(call, CONFIGURE_OUTSIDE)?;
    let now = call
        .caps
        .configuration
        .configured()
        .map_err(|failed| Refusal::internal(failed.to_string()))?;
    let refused = |failed: crate::settings::configure::Refused| {
        Refusal::bad_request(format!("no card was filed: {failed}"))
    };
    let named = [
        args.job.read()?.is_some() || args.models.read()?.is_some(),
        args.reserve.read()?.is_some(),
        args.preset.read()?.is_some(),
        args.remove_provider.read()?.is_some(),
    ];
    if named.iter().filter(|given| **given).count() != 1 {
        return Err(Refusal::bad_request(
            "act=\"configure\" takes exactly one of: job with models, reserve, preset, or \
             remove_provider",
        ));
    }
    if let Some(share) = args.reserve.read()? {
        let share = crate::models::Reserve::parse(*share)
            .map_err(|failed| Refusal::bad_request(failed.to_string()))?;
        return Configure::reserve(&now, share).map_err(refused);
    }
    if let Some(preset) = args.preset.read()? {
        let preset = crate::settings::Preset::ALL
            .into_iter()
            .find(|known| known.as_str() == preset)
            .ok_or_else(|| {
                Refusal::bad_request(format!(
                    "preset must be one of: databricks, phala, openrouter, got {}",
                    shown(preset)
                ))
            })?;
        let setup = crate::settings::ProviderSetup::parse(
            preset,
            args.workspace.read()?.as_deref(),
            args.expect_commit.read()?.as_deref(),
        )
        .map_err(|failed| Refusal::bad_request(failed.to_string()))?;
        return Configure::declare(&now, setup).map_err(refused);
    }
    if let Some(name) = args.remove_provider.read()? {
        return Configure::remove(&now, name).map_err(refused);
    }
    let job = args
        .job
        .read()?
        .as_deref()
        .ok_or_else(|| Refusal::bad_request("models needs the job whose order it is"))
        .and_then(|job| {
            crate::models::Job::parse(job)
                .map_err(|failed| Refusal::bad_request(failed.to_string()))
        })?;
    let models = args
        .models
        .read()?
        .as_ref()
        .ok_or_else(|| Refusal::bad_request("job needs its models, first choice first"))?;
    let models: Vec<&str> = models.iter().map(String::as_str).collect();
    Configure::order(&now, job, &models).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{Table, TemplateId};

    /// The model reads the catalog only from this verb's words, so every template a Connect card
    /// can write is named there, with its parameters, and none it cannot.
    #[test]
    fn the_description_names_every_template_a_card_can_write() {
        for id in TemplateId::ALL {
            let named = ProposeAct::DESCRIPTION.contains(&format!("`{}`", id.as_str()));
            match id.template().table {
                Table::Link(_) | Table::Account(_) | Table::Folder | Table::Channel(_) => {
                    assert!(named, "{} is not named", id.as_str());
                }
                Table::Always | Table::Backend(_) => {
                    assert!(!named, "{} writes no table", id.as_str());
                }
            }
        }
    }
}

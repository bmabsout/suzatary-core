use super::{
    Agent, Answers, Briefings, Calendar, Clock, Corpus, Drive, Junk, MailSync, Mailbox, Manuals,
    Model, Notify, OlderMail, Render, Repositories, Repository, Secretary, Secrets, Sessions,
    Usage, Weather, Web, Whois,
};
use crate::record::TodoEvent;
use crate::store::Prodrome;

/// What the serving process supplies: the secretary's effects, since it runs the
/// secretary, plus peers, phones and usage. Nothing here sends mail; only a route holding
/// a `Tap` the gate mints reaches the transport, and that is not an effect a record holds.
#[derive(Clone, Copy)]
pub struct Server<'a> {
    /// This chain's store.
    pub chain: &'a dyn Prodrome<TodoEvent>,
    /// The triage queue.
    pub corpus: &'a dyn Corpus,
    /// The mail index, read-only.
    pub mail: &'a dyn Mailbox,
    /// The repository's gates.
    pub repository: &'a dyn Repository,
    /// The manuals.
    pub manuals: &'a dyn Manuals,
    /// Typst, for confirmed markup.
    pub render: &'a dyn Render,
    /// The instant every write is stamped with.
    pub clock: &'a dyn Clock,
    /// The model the secretary asks.
    pub model: &'a dyn Model,
    /// Where each exchange is written down.
    pub conversations: crate::secretary::conversations::Conversations<'a>,
    /// Where a proposal is filed for a person to tap.
    pub inbox: &'a dyn crate::store::Prodrome<crate::secretary::inbox_event::InboxEvent>,
    /// The secretary's memory.
    pub memories: crate::secretary::memories::Memories<'a>,
    /// The recorded decisions, which a tapped proposal writes.
    pub decisions: crate::decisions::Decisions<'a>,
    /// The topics on the secretary's rounds and notes, box-local.
    pub topics: crate::secretary::topics::stores::TopicStores<'a>,
    /// The saved briefings.
    pub briefings: &'a dyn Briefings,
    /// The day's forecast.
    pub weather: &'a dyn Weather,
    /// The owner's calendars, read-only, in the box's clock.
    pub calendar: &'a dyn Calendar,
    /// The agent's standing batches beside this chain.
    pub proposals: &'a dyn crate::store::Replica<TodoEvent>,
    /// Whether each declared secret resolves, never a value.
    pub secrets: &'a dyn Secrets,
    /// The web, within the allow rules.
    pub web: &'a dyn Web,
    /// The owner's repositories, read-only.
    pub repositories: &'a dyn Repositories,
    /// The owner's Drive, within the allowed folders.
    pub drive: &'a dyn Drive,
    /// The Microsoft accounts' Junk folders, read-only.
    pub junk: &'a dyn Junk,
    /// The timer's own mail sync, started early.
    pub sync: &'a dyn MailSync,
    /// The mail servers, searched for mail older than the routine sync's window.
    pub older: &'a dyn OlderMail,
    /// The owner's uploaded photos and PDFs, read-only.
    pub uploads: &'a dyn crate::store::Doc<crate::attachment::AttachmentId, crate::upload::Upload>,
    /// The words of the uploads, for the secretary's search.
    pub upload_index: &'a dyn super::UploadIndex,
    /// The files a verb keeps for the model.
    pub kept: &'a dyn crate::store::Doc<crate::attachment::AttachmentId, Vec<u8>>,
    /// The model settings the instance file declares, read-only.
    pub configuration: &'a dyn crate::settings::configure::Configuration,
    /// The watches on mail the owner expects.
    pub watches: &'a crate::mail::watch::Watches,
    /// Who owns a peer address: the only source of a `Login`.
    pub whois: &'a dyn Whois,
    /// Push to the owner's phones.
    pub notify: &'a dyn Notify,
    /// Typst, for a model's answer blocks.
    pub answers: &'a dyn Answers,
    /// The phones that asked to be woken.
    pub devices: &'a dyn crate::store::Log<String, crate::devices::Device>,
    /// What the box has spent.
    pub usage: &'a dyn Usage,
    /// The session-holding model, when there is one.
    pub sessions: &'a dyn Sessions,
}

impl<'a> Server<'a> {
    /// The secretary's share of these effects.
    #[must_use]
    pub fn secretary(&self) -> Secretary<'a> {
        Secretary {
            chain: self.chain,
            corpus: self.corpus,
            mail: self.mail,
            repository: self.repository,
            manuals: self.manuals,
            render: self.render,
            clock: self.clock,
            model: self.model,
            conversations: self.conversations,
            inbox: self.inbox,
            memories: self.memories,
            decisions: self.decisions,
            briefings: self.briefings,
            weather: self.weather,
            calendar: self.calendar,
            proposals: self.proposals,
            secrets: self.secrets,
            web: self.web,
            repositories: self.repositories,
            drive: self.drive,
            junk: self.junk,
            sync: self.sync,
            older: self.older,
            uploads: self.uploads,
            upload_index: self.upload_index,
            kept: self.kept,
            configuration: self.configuration,
            watches: self.watches,
        }
    }

    /// The agent's share of these effects.
    #[must_use]
    pub fn agent(&self) -> Agent<'a> {
        self.secretary().agent()
    }
}

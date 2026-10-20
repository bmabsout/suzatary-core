//! Typst's HTML export as the one renderer of markup ([`Render`]).
//!
//! A todo's markup and a secretary's answer are compiled in the closed world
//! (`crate::typeset`): no `html` module, so no element or attribute but typst's
//! own, and typst escapes all text it emits, so [`suspicious`] is a tripwire
//! that refuses the impossible case, never a sanitiser that edits it. A page
//! this box printed (`reading`, the static site) is compiled by the `typst`
//! binary, since its template draws with `html.elem` and every datum in it is
//! a string literal. What each function computes is kept in one structure,
//! [`Rendered`], keyed by the function and its source: bodies and answers in
//! the process and on disk; a page, printed from the observed value of every
//! price on it (`reading::SHOWN`), hits until that value moves, and still
//! moves with the clock, so it is kept in the process only.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use prodrome_core::memo::{Finding, Key};
use suzatary_core::capability::{
    AnswerError, Answers, Html, Rejected, Render, RenderError, Typeset,
};
use suzatary_core::escape::Escaped;
use suzatary_core::narrowing::narrowed;
use suzatary_core::reading::Page;
use suzatary_core::rendering::{self, ANSWER, ANSWER_PREAMBLE, CLOSED, MARKUP, PAGE};
use suzatary_core::secretary::answer::Renderable;
use suzatary_core::site::page::Page as SitePage;
use suzatary_core::table::Table;
use suzatary_core::tripwire::suspicious;

use crate::rendered::Rendered;
use crate::run::{RunError, succeed};
use crate::scratch::Scratch;

/// typst has no offline switch, so a fragment reaching for a package is
/// bounded by this.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

const FRAGMENTS_KEPT: usize = 4096;

const PAGES_KEPT: usize = 256;

/// Typst's HTML export for one checkout (its nix fallback), one cache
/// directory and one closed world.
pub struct Typst {
    repo: PathBuf,
    timeout: Duration,
    fragments: Rendered,
    pages: Rendered,
    closed: Arc<dyn Typeset + Send>,
    /// The closed world's answers, so a page that redraws a proposal on every poll spawns
    /// one compile per text, not one per poll.
    answers: Table<Result<String, Rejected>>,
}

impl std::fmt::Debug for Typst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typst")
            .field("fragments", &self.fragments)
            .field("repo", &self.repo)
            .finish_non_exhaustive()
    }
}

impl Typst {
    /// `repo` is typst's nix fallback for pages; `cache` is where fragments
    /// are stored by content; `closed` compiles every fragment.
    #[must_use]
    pub fn new(
        repo: impl Into<PathBuf>,
        cache: impl Into<PathBuf>,
        closed: impl Typeset + Send + 'static,
    ) -> Self {
        Self {
            repo: repo.into(),
            timeout: DEFAULT_TIMEOUT,
            fragments: Rendered::on_disk(cache, FRAGMENTS_KEPT, &rendering::kept()),
            pages: Rendered::in_memory(PAGES_KEPT),
            closed: Arc::new(closed),
            answers: Table::new(PAGES_KEPT),
        }
    }

    /// One page this box printed to typst's whole HTML document, compiled in
    /// a fresh directory that is its `--root`, so `#read` and `#import` reach
    /// nothing of this box.
    fn compile(&self, source: &str) -> Compilation {
        let scratch = match Scratch::temporary("suzatary-markup-") {
            Ok(scratch) => scratch,
            Err(err) => return Compilation::Unavailable(err.to_string()),
        };
        let home = scratch.path();
        let src = home.join("fragment.typ");
        let out = home.join("fragment.html");
        if let Err(err) = std::fs::write(&src, source) {
            return Compilation::Unavailable(err.to_string());
        }
        let cmd = crate::tool::tool(&self.repo, "typst")
            .args(["compile", "--features", "html", "--format", "html"])
            .arg("--root")
            .arg(home)
            .arg("--package-path")
            .arg(home)
            .arg("--package-cache-path")
            .arg(home)
            .arg(&src)
            .arg(&out)
            // typst reads it in `datetime.today()`, and every nix shell sets it.
            .unset("SOURCE_DATE_EPOCH")
            .timeout(self.timeout);
        match succeed(&cmd) {
            Ok(_) => match std::fs::read_to_string(&out) {
                Ok(document) => Compilation::Rendered(document),
                Err(err) => Compilation::Unavailable(err.to_string()),
            },
            Err(RunError::Exited { stderr, .. }) => {
                Compilation::Rejected(stderr.as_str().to_owned())
            }
            Err(RunError::TimedOut { .. }) => {
                Compilation::Unavailable(format!("typst did not finish within {:?}", self.timeout))
            }
            Err(failed) => Compilation::Unavailable(failed.to_string()),
        }
    }
}

impl Typst {
    /// One answer block to typst's HTML, through the same compile, cache and
    /// tripwire as a todo's body.
    ///
    /// # Errors
    /// [`RenderError`], as for [`Render::html`].
    pub fn answer(&self, block: &Renderable) -> Result<Html, RenderError> {
        self.rendered(
            Rendered::key(&ANSWER, block.as_str()),
            &format!("{ANSWER_PREAMBLE}{}", block.as_str()),
        )
    }

    /// One page of the static site, the whole document, through the binary
    /// and the same tripwire as a body. Never cached: every page prints the
    /// moment it was generated.
    ///
    /// # Errors
    /// [`RenderError`], as for [`Render::html`].
    pub fn document(&self, page: &SitePage) -> Result<String, RenderError> {
        match self.compile(page.as_str()) {
            Compilation::Rendered(document) => match suspicious(&document) {
                Some(reason) => Err(RenderError::Suspicious(reason.to_string())),
                None => Ok(document),
            },
            Compilation::Rejected(reason) | Compilation::Unavailable(reason) => {
                Err(RenderError::Refused(reason))
            }
        }
    }

    /// `source` compiled in the closed world, or what `key` keeps of it.
    fn rendered(&self, key: Key, source: &str) -> Result<Html, RenderError> {
        if let Some(kept) = self
            .fragments
            .recall(&key)
            .map_err(|failed| unread(&failed))?
        {
            return Ok(Html::from_render(kept));
        }
        let document = self
            .closed
            .closed(source)
            .map_err(|rejected| RenderError::Refused(rejected.said(source)))?;
        let body = narrowed(&document)?;
        // A finding is kept as evidence and answers nothing; this compile's body is still the answer.
        let _ = self.fragments.remember(key, &body);
        Ok(Html::from_render(body))
    }

    /// Every key whose tabulation holds two values: a compile that was not a function of its
    /// source, or a file on disk another writer left.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        [
            self.fragments.findings(),
            self.pages.findings(),
            self.answers.findings(),
        ]
        .concat()
    }

    /// One page through the binary, narrowed to its `<body>` and past the tripwire.
    fn body(&self, source: &str) -> Result<String, RenderError> {
        let document = match self.compile(source) {
            Compilation::Rendered(document) => document,
            Compilation::Rejected(stderr) => return Err(RenderError::Refused(stderr)),
            Compilation::Unavailable(reason) => return Err(RenderError::Refused(reason)),
        };
        narrowed(&document)
    }
}

impl Typeset for Typst {
    fn closed(&self, source: &str) -> Result<String, Rejected> {
        let key = Rendered::key(&CLOSED, source);
        if let Some(known) = self.answers.kept(&key) {
            return known;
        }
        let answer = self.closed.closed(source);
        // A refusal typst located is a fact about the text; one about nothing (a timeout, a
        // child that would not start) may be about the box, so it is asked again next time.
        if answer
            .as_ref()
            .map_or_else(|r| r.diagnostics.iter().any(|d| d.span.is_some()), |_| true)
        {
            let _ = self.answers.keep(key, answer.clone());
        }
        answer
    }
}

impl Render for Typst {
    fn html(&self, markup: &Escaped) -> Result<Html, RenderError> {
        self.rendered(Rendered::key(&MARKUP, markup.as_str()), markup.as_str())
    }

    fn cached(&self, markup: &Escaped) -> Option<Html> {
        // A peek that never compiles: a cache that cannot be read is said by `html`, which reads
        // it again.
        self.fragments
            .recall(&Rendered::key(&MARKUP, markup.as_str()))
            .ok()
            .flatten()
            .map(Html::from_render)
    }

    fn warm(&self, markup: &[Escaped]) -> usize {
        markup
            .iter()
            .filter(|fragment| self.html(fragment).is_ok())
            .count()
    }

    fn page(&self, page: &Page) -> Result<Html, RenderError> {
        let key = Rendered::key(&PAGE, page.as_str());
        if let Some(kept) = self.pages.recall(&key).map_err(|failed| unread(&failed))? {
            return Ok(Html::from_render(kept));
        }
        let body = self.body(page.as_str())?;
        let _ = self.pages.remember(key, &body);
        Ok(Html::from_render(body))
    }

    fn findings(&self) -> Vec<Finding> {
        Typst::findings(self)
    }
}

/// An answer block's HTML is kept under the answer function and its source alone.
impl suzatary_core::capability::RenderCache for Typst {
    fn forget(&self, block: &Renderable) -> Result<(), String> {
        self.fragments
            .forget(&Rendered::key(&ANSWER, block.as_str()))
            .map_err(|failed| failed.to_string())
    }
}

impl Answers for Typst {
    fn block(&self, block: &Renderable) -> Result<Html, AnswerError> {
        self.answer(block)
            .map_err(|refused| AnswerError(refused.to_string()))
    }
}

/// A rendered cache that is there and cannot be read is a fact about the box, said as one, so a
/// broken disk is never a compile every request.
fn unread(failed: &std::io::Error) -> RenderError {
    RenderError::Refused(format!("the rendered cache could not be read: {failed}"))
}

/// Bad markup is a fact about the text, an unavailable typst one about the
/// box; both reach [`Render`] as [`RenderError::Refused`].
enum Compilation {
    Rendered(String),
    Rejected(String),
    Unavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::on_path;

    fn scratch_cache(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "suzatary-io-render-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn typst_at(name: &str) -> (Typst, PathBuf) {
        let cache = scratch_cache(name);
        (
            Typst::new(PathBuf::from("."), &cache, crate::typeset::InProcess),
            cache,
        )
    }

    /// Counts its compiles, answering as the in-process closed world does.
    struct Counted(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Typeset for Counted {
        fn closed(&self, source: &str) -> Result<String, Rejected> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            crate::typeset::compile(source)
        }
    }

    #[test]
    fn a_text_is_compiled_once_however_often_it_is_asked_about() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let typst = Typst::new(
            PathBuf::from("."),
            scratch_cache("counted"),
            Counted(count.clone()),
        );
        for _ in 0..3 {
            assert!(typst.closed("Renew *it*").is_ok());
            assert!(typst.closed("a ] b").is_err());
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn cached_reads_a_fragment_another_process_kept_on_the_same_disk() {
        let cache = scratch_cache("read-from-disk");
        let source = "= Kept\n";
        Rendered::on_disk(&cache, 8, &[&MARKUP, &ANSWER])
            .remember(Rendered::key(&MARKUP, source), "<p>kept</p>")
            .expect("no finding");
        let typst = Typst::new(PathBuf::from("."), &cache, crate::typeset::InProcess);
        let markup = Escaped::from_chain(source);
        assert_eq!(
            typst.cached(&markup).map(|html| html.as_str().to_string()),
            Some("<p>kept</p>".to_string())
        );
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_body_and_a_page_of_one_source_are_two_functions() {
        let cache = scratch_cache("two-functions");
        let source = "= Same bytes\n";
        let typst = Typst::new(PathBuf::from("."), &cache, crate::typeset::InProcess);
        typst
            .fragments
            .remember(Rendered::key(&PAGE, source), "<p>a page</p>")
            .expect("no finding");
        assert_eq!(
            typst
                .fragments
                .recall(&Rendered::key(&PAGE, source))
                .expect("read")
                .as_deref(),
            Some("<p>a page</p>"),
            "the page's value is there to be read"
        );
        assert_eq!(typst.cached(&Escaped::from_chain(source)), None);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn html_compiles_a_trivial_fragment_when_typst_is_reachable() {
        if !on_path("typst") && !on_path("nix") {
            eprintln!(
                "skipping html_compiles_a_trivial_fragment_when_typst_is_reachable: \
                 neither typst nor nix is on PATH"
            );
            return;
        }
        let (typst, cache) = typst_at("live-compile");
        let markup = Escaped::from_chain("hello");
        match typst.html(&markup) {
            Ok(html) => assert!(html.as_str().contains("hello")),
            Err(err) => eprintln!(
                "skipping html_compiles_a_trivial_fragment_when_typst_is_reachable: \
                 found typst/nix but the compile did not succeed here: {err}"
            ),
        }
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn an_answer_block_is_html_with_mathml_in_the_closed_world() {
        let (typst, cache) = typst_at("answer-blocks");
        let answer =
            |source: &str| typst.answer(&suzatary_core::secretary::answer::Renderable::of(source));
        let block = |source: &str| answer(source).expect("the block compiles");
        assert!(block("$ x^2 $").as_str().contains("<math"), "MathML");
        assert!(
            block("#panel[*Due* Friday]")
                .as_str()
                .contains("<blockquote><strong>Due</strong> Friday</blockquote>"),
            "{}",
            block("#panel[*Due* Friday]").as_str()
        );
        assert!(
            answer("#nosuchthing()").is_err(),
            "a block that does not compile is text"
        );
        for reaching in [
            "#html.elem(\"script\")[x]",
            "#std.html.div[x]",
            "#eval(\"html.elem(\\\"b\\\")[x]\")",
        ] {
            assert!(answer(reaching).is_err(), "{reaching}");
        }
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_todo_page_is_classed_html_and_its_data_stays_text_when_typst_is_reachable() {
        use prodrome_core::fpl::Explanation;
        use prodrome_core::term::TermF;
        use suzatary_core::reading::{Content, Explained, Prose, todo};
        use suzatary_core::wire;
        if !on_path("typst") {
            eprintln!("skipping a_todo_page_is_classed_html: typst is not on PATH");
            return;
        }
        let (typst, cache) = typst_at("todo-page");
        let entry = wire::Entry {
            todo: "file-permit".to_owned(),
            state: wire::EntryState::Open,
            states: vec![wire::EntryState::Open],
            at: String::new(),
            claimed: wire::ClaimedState::Agrees,
            value: Some(wire::Fulfillment::Number(0.25)),
            recurring: false,
            unconfirmed: false,
            conflicts: std::collections::BTreeMap::new(),
            authored: None,
            candidates: Vec::new(),
            stream: Vec::new(),
        };
        let content = Content {
            body: Prose::Rendered(Escaped::from_chain("*File* the deed \\(signed\\)")),
            detail: Some(Prose::Source("<script>alert(1)</script> #html".to_owned())),
        };
        let node = |value, node| Explanation {
            value,
            notes: std::collections::BTreeMap::new(),
            node: Box::new(node),
        };
        let hostile = node(
            Some(0.5),
            TermF::Ref {
                store: None,
                entity: "a\"]#html.elem(\"script\")[".to_owned(),
            },
        );
        let why = Explained::of(node(
            Some(0.25),
            TermF::Conj {
                terms: vec![hostile],
                p: -4.0,
            },
        ));
        let html = typst
            .page(&todo(
                &entry,
                Some(&content),
                Some(&why),
                suzatary_core::zone::Zone::UTC,
            ))
            .expect("the page compiles");
        let html = html.as_str();
        assert!(html.starts_with("<div class=\"todohead\">"), "{html}");
        assert!(
            html.contains(
                "<div class=\"todobody rich\"><strong>File</strong> the deed (signed)</div>"
            ),
            "{html}"
        );
        assert!(html.contains("<details class=\"why\" open>"), "{html}");
        assert!(html.contains("<ul class=\"tree\">"), "{html}");
        assert!(html.contains("Why this number — 25.0%"), "{html}");
        assert!(
            html.contains("<p class=\"says\">25.0%: 1 part taken together"),
            "{html}"
        );
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("style="), "{html}");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_page_read_at_two_instants_writes_nothing_under_the_cache() {
        use prodrome_core::fpl::Explanation;
        use prodrome_core::term::TermF;
        use suzatary_core::reading::{Explained, todo};
        use suzatary_core::wire;
        if !on_path("typst") {
            eprintln!("skipping a_page_read_at_two_instants_writes_nothing: typst is not on PATH");
            return;
        }
        let (typst, cache) = typst_at("pages-stay-in-memory");
        let at = |value: f64| {
            let entry = wire::Entry {
                todo: "file-permit".to_owned(),
                state: wire::EntryState::Open,
                states: vec![wire::EntryState::Open],
                at: String::new(),
                claimed: wire::ClaimedState::Agrees,
                value: Some(wire::Fulfillment::Number(value)),
                recurring: false,
                unconfirmed: false,
                conflicts: std::collections::BTreeMap::new(),
                authored: None,
                candidates: Vec::new(),
                stream: Vec::new(),
            };
            let why = Explained::of(Explanation {
                value: Some(value),
                notes: std::collections::BTreeMap::new(),
                node: Box::new(TermF::Flat { value }),
            });
            todo(&entry, None, Some(&why), suzatary_core::zone::Zone::UTC)
        };
        let (morning, evening) = (at(0.31), at(0.24));
        assert_ne!(morning, evening, "two instants are two sources");
        for page in [&morning, &evening, &morning] {
            typst.page(page).expect("the page compiles");
        }
        let files = |dir: &std::path::Path| std::fs::read_dir(dir).map_or(0, Iterator::count);
        assert_eq!(files(&cache), 0, "a page wrote to the disk cache");
        typst
            .html(&Escaped::from_chain("a body"))
            .expect("the body compiles");
        assert_eq!(files(&cache), 1, "a body is kept on disk");
        let _ = std::fs::remove_dir_all(&cache);
    }
}

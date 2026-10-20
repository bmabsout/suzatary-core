//! The manuals, as `suzatary_core::capability::Manuals`. No method takes a path, so the agent
//! holds no file read. A missing file is `None`, an answer the agent can act
//! on, never an error.

use std::path::PathBuf;

use suzatary_core::capability::{Manual, Manuals};
use suzatary_core::persona::Persona;

/// The repository's manuals.
#[derive(Debug)]
pub struct Handbook {
    repo: PathBuf,
    persona: Persona,
}

impl Handbook {
    /// Point the reads at a repository's manuals. [`Manual::Operator`] and [`Manual::Secretary`] have their `{{persona.*}}` placeholders
    /// filled from `persona` at read time, so nothing is written back.
    #[must_use]
    pub fn new(repo: impl Into<PathBuf>, persona: Persona) -> Self {
        Self {
            repo: repo.into(),
            persona,
        }
    }
}

impl Manuals for Handbook {
    fn document(&self, which: Manual) -> Option<String> {
        let path = match which {
            Manual::Constitution => self.repo.join("CLAUDE.md"),
            Manual::Operator => self.repo.join("OPERATOR.md"),
            Manual::Secretary => self.repo.join("SECRETARY.md"),
        };
        let text = std::fs::read_to_string(path).ok()?;
        // Only the two files that name the person are rendered; the constitution is this
        // repository's own prose.
        Some(match which {
            Manual::Operator | Manual::Secretary => self.persona.render(&text),
            Manual::Constitution => text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona() -> Persona {
        Persona {
            name: "Test Person".to_owned(),
            city: "Nowhere".to_owned(),
            zone: suzatary_core::zone::Zone::UTC,
            prose: suzatary_core::persona::Prose::default(),
        }
    }

    /// An absent manual is absence, not a failure.
    #[test]
    fn a_manual_that_is_not_there_is_none() {
        let handbook = Handbook::new("/nonexistent/repo", persona());
        assert!(handbook.document(Manual::Constitution).is_none());
    }

    /// Operator and secretary are rendered against the persona; in the constitution a stray
    /// `{{persona.name}}` surfaces as itself.
    #[test]
    fn only_operator_and_secretary_are_rendered_against_the_persona() {
        let dir = std::env::temp_dir().join(format!(
            "suzatary-manuals-render-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        for (name, text) in [
            ("CLAUDE.md", "constitution names {{persona.name}}"),
            ("OPERATOR.md", "operator manual for {{persona.name}}"),
            ("SECRETARY.md", "secretary manual for {{persona.name}}"),
        ] {
            std::fs::write(dir.join(name), text).expect("a writable scratch file");
        }
        let handbook = Handbook::new(&dir, persona());
        assert_eq!(
            handbook.document(Manual::Constitution).as_deref(),
            Some("constitution names {{persona.name}}"),
            "the constitution is not this deployment's person to render"
        );
        assert_eq!(
            handbook.document(Manual::Operator).as_deref(),
            Some("operator manual for Test Person")
        );
        assert_eq!(
            handbook.document(Manual::Secretary).as_deref(),
            Some("secretary manual for Test Person")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

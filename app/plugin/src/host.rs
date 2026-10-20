//! The host: one engine and one linker, and the check a component passes to be loaded.

use std::sync::Arc;
use std::thread;

use prodrome_core::declared::{Declaration, Refusal};
use suzatary_core::plugin::{Manifest, World};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine};

use crate::budget::TICK;
use crate::state::State;
use crate::world::SourcePre;
use crate::world::actor::ActorPre;
use crate::{Actor, Budget, Source};

/// One wasmtime engine, shared by every plugin, and the linker that holds the worlds' imports.
///
/// The engine counts epochs on a thread of its own, one tick every 10 ms; a call's deadline is
/// a number of ticks, so a plugin in a loop is interrupted without the host waiting on it. The
/// thread ends with the last value holding the engine.
pub struct Host {
    engine: Engine,
    linker: Linker<State>,
    budget: Budget,
}

/// Why the host could not start.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// The engine refused its configuration.
    #[error("the plugin engine could not start: {0}")]
    Engine(String),
    /// The thread that advances the epoch could not start.
    #[error("the plugin clock could not start: {0}")]
    Ticker(String),
    /// The worlds' imports could not be linked.
    #[error("the plugin worlds could not be linked: {0}")]
    Link(String),
}

/// Why a component was not loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    /// Not a WebAssembly component this engine compiles.
    #[error("not a component: {0}")]
    NotAComponent(String),
    /// It imports what its world does not give, so it is never instantiated.
    #[error("imports {0:?}, which its world does not give")]
    ForeignImports(Vec<String>),
    /// It does not export what its world must.
    #[error("does not export {0}, so it does not target its world")]
    WrongWorld(&'static str),
    /// Its imports or exports have the right names and the wrong types.
    #[error("does not match its world's types: {0}")]
    Mismatch(String),
    /// Its manifest names one world and it was loaded as another.
    #[error("its manifest names the {named} world, and it was loaded as {loading}")]
    OtherWorld {
        /// The world the manifest names.
        named: World,
        /// The world it was loaded as.
        loading: World,
    },
    /// Its declared schema breaks a law of admission, which the refusal names.
    #[error("its schema is {0}")]
    Schema(#[from] Refusal),
}

impl Host {
    /// Start the engine and link the worlds' imports, once; every call gets `budget`.
    pub fn new(budget: Budget) -> Result<Self, HostError> {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).map_err(|e| HostError::Engine(e.to_string()))?;
        let weak = engine.weak();
        thread::Builder::new()
            .name("plugin-epoch".to_owned())
            .spawn(move || {
                while let Some(engine) = weak.upgrade() {
                    engine.increment_epoch();
                    drop(engine);
                    thread::sleep(TICK);
                }
            })
            .map_err(|e| HostError::Ticker(e.to_string()))?;
        let mut linker = Linker::new(&engine);
        crate::world::Source::add_to_linker::<State, HasSelf<State>>(&mut linker, |state| state)
            .map_err(|e| HostError::Link(e.to_string()))?;
        Ok(Self {
            engine,
            linker,
            budget,
        })
    }

    /// Admit `schema`, the plugin's declared schema, then compile `component`, check it against
    /// `manifest`'s world, and prepare it to run.
    ///
    /// A schema that breaks a law of admission is refused by that law, and a component that
    /// imports anything its world does not name is refused by name, before anything of it runs:
    /// the world's imports are the plugin's whole authority, and its schema the whole shape of
    /// what it may write.
    pub fn load(
        &self,
        manifest: Manifest,
        component: &[u8],
        schema: &str,
    ) -> Result<Source, LoadError> {
        let (schema, compiled) = self.admitted(&manifest, World::Source, component, schema)?;
        let pre = self
            .linker
            .instantiate_pre(&compiled)
            .and_then(SourcePre::new)
            .map_err(|e| LoadError::Mismatch(e.to_string()))?;
        Ok(Source::new(Arc::new(manifest), schema, pre, self.budget))
    }

    /// [`Host::load`], for an `actor`: its schema admitted by the same laws, its component held
    /// to the actor world's imports, which reach nothing outside the call.
    pub fn load_actor(
        &self,
        manifest: Manifest,
        component: &[u8],
        schema: &str,
    ) -> Result<Actor, LoadError> {
        let (schema, compiled) = self.admitted(&manifest, World::Actor, component, schema)?;
        let pre = self
            .linker
            .instantiate_pre(&compiled)
            .and_then(ActorPre::new)
            .map_err(|e| LoadError::Mismatch(e.to_string()))?;
        Ok(Actor::new(Arc::new(manifest), schema, pre, self.budget))
    }

    /// The schema admitted and the component compiled and checked against `world`, which the
    /// manifest must name: every world's loading is this one check.
    fn admitted(
        &self,
        manifest: &Manifest,
        world: World,
        component: &[u8],
        schema: &str,
    ) -> Result<(Declaration, Component), LoadError> {
        if manifest.world() != world {
            return Err(LoadError::OtherWorld {
                named: manifest.world(),
                loading: world,
            });
        }
        let schema = Declaration::admit(schema)?;
        let compiled = Component::from_binary(&self.engine, component)
            .map_err(|e| LoadError::NotAComponent(e.to_string()))?;
        check(&self.engine, &compiled, world)?;
        Ok((schema, compiled))
    }
}

/// The component imports only what `world` gives and exports what it must.
fn check(engine: &Engine, component: &Component, world: World) -> Result<(), LoadError> {
    let shape = component.component_type();
    let foreign: Vec<String> = shape
        .imports(engine)
        .map(|(name, _)| name)
        .filter(|name| !world.imports().contains(name))
        .map(str::to_owned)
        .collect();
    if !foreign.is_empty() {
        return Err(LoadError::ForeignImports(foreign));
    }
    if shape.get_export(engine, world.export()).is_none() {
        return Err(LoadError::WrongWorld(world.export()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use prodrome_core::declared::Law;

    use super::*;

    fn manifest() -> Manifest {
        toml::from_str(
            r#"
            name = "probe"
            component = "probe.wasm"
            schema = "probe.schema"
            world = "source@0.3.0"
            "#,
        )
        .expect("a manifest")
    }

    /// A schema that admission takes: one event, stamped, keyed, writing one register.
    const SCHEMA: &str = "Schema(key='line', events=(Event(name='Said', fields=(Field(name='line', \
        type=Text()), Field(name='at', type=Instant()), Field(name='actor', type=Text()))),), \
        registers=(Register(name='line', order=Discrete(), inflationary=False, \
        writes=(Write(event='Said', field='line'),)),))";

    fn loaded(bytes: &[u8]) -> Result<Source, LoadError> {
        Host::new(Budget::default())
            .expect("the host starts")
            .load(manifest(), bytes, SCHEMA)
    }

    #[test]
    fn bytes_that_are_not_a_component_are_refused() {
        assert!(matches!(
            loaded(b"not wasm at all"),
            Err(LoadError::NotAComponent(_))
        ));
        // An empty core module: WebAssembly, and not a component.
        assert!(matches!(
            loaded(b"\0asm\x01\0\0\0"),
            Err(LoadError::NotAComponent(_))
        ));
    }

    #[test]
    fn a_schema_that_breaks_a_law_is_refused_by_that_law_before_the_component_is_read() {
        let unstamped = SCHEMA.replace(", Field(name='at', type=Instant())", "");
        let Err(LoadError::Schema(refused)) = loaded_with(b"not wasm at all", &unstamped) else {
            panic!("an unstamped schema is refused");
        };
        assert_eq!(refused.law, Law::Stamp);
        let Err(LoadError::Schema(refused)) = loaded_with(b"", &format!("{SCHEMA}\n")) else {
            panic!("a schema not in its canonical print is refused");
        };
        assert_eq!(refused.law, Law::Canonical);
    }

    fn loaded_with(bytes: &[u8], schema: &str) -> Result<Source, LoadError> {
        Host::new(Budget::default())
            .expect("the host starts")
            .load(manifest(), bytes, schema)
    }

    #[test]
    fn a_component_that_exports_nothing_does_not_target_the_world() {
        // An empty component: it imports nothing, and exports no `sync`.
        assert!(matches!(
            loaded(b"\0asm\x0d\0\x01\0"),
            Err(LoadError::WrongWorld("suzatary:plugin/sync@0.3.0"))
        ));
    }
}

//! Utility Vest for Horizon (SDK mod API 0.2).
//!
//! The pure rules live in [`runtime`] and are unit-tested without the game; [`support`] is the
//! only module that reads and writes the game's state. The one `impl Hooks` is below.
use mod_api::{
    Flow, Hooks, RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent,
    ServerHost,
};

pub mod runtime;
pub mod support;

/// The mod keeps no Rust state: one instance serves every hook.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the five vest items' nine-slot-bank containers.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        support::declare(r);
    }

    /// Register the `/utilityvest` server actions (upstream's keybinds are client-only here).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::register_commands(event);
    }

    /// Dispatch a parsed `/utilityvest` line.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::run(host, event)
    }
}

#[cfg(test)]
mod tests {
    use crate::Mod;

    /// The engine refuses a package whose Parts it cannot attach, so run this package's own
    /// `register` through the SDK's registration path and assert the engine's own verdict.
    #[test]
    fn the_engine_accepts_every_declaration_of_this_package() {
        let category = |_: &str| false;
        let position = |_: &str| None;
        let schema = mod_api::__private::RuleSchema {
            category: &category,
            position: &position,
            rule_count: 0,
        };
        let mods = vec![mod_api::ModEntry::new("utilityvest", Box::new(Mod))];
        let refused =
            mod_api::__private::install_parts(&mods, &schema, |_: &mod_api::DimensionSettings| {
                false
            });
        assert!(
            refused.is_ok(),
            "the engine refuses this package: {refused:?}"
        );
    }
}

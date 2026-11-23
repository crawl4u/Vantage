//! Read-only introspection of the current instance's explicit grants.

use super::{State, cinnabar::extension::permissions};
use crate::ModGrants;

impl permissions::Host for State {
    fn read(&mut self) -> anyhow::Result<Vec<permissions::Permission>> {
        self.charge()?;
        Ok(ModGrants::CAPABILITIES
            .iter()
            .map(|capability| permissions::Permission {
                name: capability.name.into(),
                label: capability.label.into(),
                granted: (capability.enabled)(&self.grants),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_trusted_mod_holds_wasi_and_a_frame_transaction_starts_without_it() {
        let state = |grants| State::new(grants, String::new(), super::super::Declared::default());
        assert!(state(ModGrants::default()).wasi.is_none());
        let trusted = state(ModGrants::trusted(Vec::new()));
        assert!(trusted.wasi.is_some());
        assert!(trusted.frame_transaction().wasi.is_none(), "the caller moves the real context in");
    }

    #[test]
    fn a_trusted_mod_may_grow_its_memory_past_the_sandbox_limit() {
        use wasmtime::ResourceLimiter as _;
        let state = |grants| State::new(grants, String::new(), super::super::Declared::default());
        let past = crate::MEMORY_BYTES * 8;
        assert!(state(ModGrants::trusted(Vec::new())).limits.memory_growing(0, past, None).unwrap());
        assert!(state(ModGrants::default()).limits.memory_growing(0, past, None).is_err(), "the sandbox traps on growth past its limit");
    }

    #[test]
    fn a_callback_begun_by_a_trusted_mod_is_not_held_to_the_import_call_count() {
        let state = |grants| State::new(grants, String::new(), super::super::Declared::default());
        state(ModGrants::trusted(Vec::new())).begin_callback();
        assert_eq!(crate::runtime::max_import_writes(), u32::MAX);
        state(ModGrants::default()).begin_callback();
        assert_eq!(crate::runtime::max_import_writes(), crate::runtime::MAX_IMPORT_WRITES);
    }

    #[test]
    fn the_account_is_readable_only_with_its_grant() {
        use super::super::cinnabar::extension::account::Host as _;
        crate::runtime::set_account(Some(crate::runtime::AccountData {
            display_name: "Steve".into(),
            xuid: "1".into(),
            uuid: "u".into(),
            access_token: Some("secret".into()),
        }));
        let state = |grants| State::new(grants, String::new(), super::super::Declared::default());
        let mut denied = state(ModGrants::default());
        assert!(denied.read_account().unwrap().is_err());
        assert!(denied.read_sign_in().unwrap().is_err());
        let mut allowed = state(ModGrants { account: true, ..Default::default() });
        assert_eq!(allowed.read_account().unwrap().unwrap().display_name, "Steve");
        assert_eq!(allowed.read_sign_in().unwrap().unwrap().access_token, "secret");
        crate::runtime::set_account(None);
        assert!(allowed.read_account().unwrap().is_err());
    }

    #[test]
    fn reports_every_capability_including_denials_without_granting_authority() {
        let mut state = State::new(
            ModGrants {
                inventory: true,
                ..Default::default()
            },
            String::new(),
            super::super::Declared::default(),
        );
        let permissions = permissions::Host::read(&mut state).unwrap();
        assert_eq!(permissions.len(), ModGrants::CAPABILITIES.len());
        for (permission, capability) in permissions.iter().zip(ModGrants::CAPABILITIES) {
            assert_eq!(permission.name, capability.name);
            assert_eq!(permission.label, capability.label);
            assert_eq!(permission.granted, capability.name == "inventory");
        }
        assert!(!state.grants.camera);
    }
}

//! A native-only portrait of the admitted local profile, never a default skin.

use std::sync::Arc;

use ui::mod_panel::local_player_head as identity;

use render_api::StandardSkin;
use render_model::UiTexturePage;

use super::UiPresentationRuntime;

/// The current authority identity and its extracted portrait. Skin allocations are shared.
#[derive(Default)]
pub(super) struct LocalPlayerIdentity {
    source: Option<Source>,
    portrait: Option<UiTexturePage>,
    /// The name and Xbox user ID last told to the mod host, so it is told again only when they change.
    reported: Option<(Arc<str>, Arc<str>)>,
    /// The signed-in player's name for the first-party menu, whatever their skin is.
    menu_name: Option<Arc<str>>,
    /// The default skin's head, worked out once, for a player whose own skin has none to show.
    default_head: Option<UiTexturePage>,
}

/// The head of the default player skin: the vanilla texture once the pack has installed it, else the generated one.
/// The menu shows it for a player whose own skin has no head to read.
fn default_portrait() -> Option<UiTexturePage> {
    let rgba8 = render_model::actor::default_actor_skin_rgba8();
    let side = render_model::actor::actor_skin_side(&rgba8)? as u32;
    render_model::actor::skin_portrait::extract(&StandardSkin {
        width: side,
        height: side,
        rgba8,
        cape: None,
        geometry: None,
    })
}

#[derive(PartialEq, Eq)]
struct Source {
    session: u64,
    actor: u64,
    name: Arc<str>,
    skin: StandardSkin,
}

impl LocalPlayerIdentity {
    /// Re-extracts on profile, skin, actor or session changes, and drops absent identities.
    fn observe(&mut self, source: Option<Source>) {
        if self.source == source {
            return;
        }
        self.portrait = source
            .as_ref()
            .and_then(|source| render_model::actor::skin_portrait::extract(&source.skin));
        self.source = source;
    }

    /// Discards a profile captured for an earlier session before any panel paints.
    fn observe_session(&mut self, session: u64) {
        if self
            .source
            .as_ref()
            .is_some_and(|source| source.session != session)
        {
            self.observe(None);
        }
    }

    /// A name accompanies only a usable portrait, with a neutral fallback otherwise.
    fn name(&self) -> &str {
        self.source
            .as_ref()
            .filter(|_| self.portrait.is_some())
            .map_or(identity::DEFAULT_NAME, |source| source.name.as_ref())
    }
}

impl UiPresentationRuntime {
    /// Captures only the current world's admitted local profile. Missing or unsupported
    /// skins clear the old identity; launcher sample/default skins are never substituted.
    pub(super) fn capture_local_player_identity(
        &mut self,
        local: Option<client_world::ui_read::LocalPlayerUi<'_>>,
        session: u64,
    ) {
        // Mods granted account access read who is signed in; the host is told when it changes.
        let account = local.as_ref().and_then(|local| local.profile()).filter(|p| !p.username.trim().is_empty()).map(|p| (Arc::clone(&p.username), Arc::clone(&p.xuid)));
        if account != self.local_player_identity.reported {
            mod_host::set_account(account.as_ref().map(|(name, xuid)| mod_host::AccountData {
                display_name: name.to_string(),
                xuid: xuid.to_string(),
                uuid: String::new(),
                access_token: None,
            }));
            self.local_player_identity.reported = account;
        }
        self.local_player_identity.menu_name = local
            .as_ref()
            .and_then(|local| local.profile())
            .map(|profile| Arc::clone(&profile.username))
            .filter(|name| !name.trim().is_empty());
        let source = local.and_then(|local| {
            let actor = local.runtime_id();
            let profile = local.profile()?;
            let protocol::PlayerSkin::Standard(skin) = &profile.skin else {
                return None;
            };
            if profile.username.trim().is_empty() {
                return None;
            }
            Some(Source {
                session,
                actor,
                name: Arc::clone(&profile.username),
                skin: skin.clone(),
            })
        });
        self.local_player_identity.observe(source);
    }

    /// Installs the current portrait in whichever atlas owns this frame's engine, then
    /// publishes read-only bindings after all extension-owned values have been prepared.
    pub(super) fn publish_local_player_identity(&mut self, session: u64) {
        self.local_player_identity.observe_session(session);
        if let Some(engine) = self.engine.as_deref() {
            engine
                .textures
                .lock()
                .set_local_player_head(self.local_player_identity.portrait.as_ref());
            // The first-party menu shows a head always: the player's own, else the default skin's.
            let identity = &mut self.local_player_identity;
            if identity.portrait.is_none() && identity.default_head.is_none() {
                identity.default_head = default_portrait();
            }
            let menu_head = identity.portrait.as_ref().or(identity.default_head.as_ref());
            engine.textures.lock().set_menu_player_head(menu_head);
        }
        self.screens
            .mod_ui
            .set_menu_player_name(self.local_player_identity.menu_name.as_ref());
        self.screens.mod_ui.set_local_player_panel_identity(
            self.local_player_identity.name(),
            self.local_player_identity.portrait.is_some(),
        );
    }
}

#[cfg(test)]
mod tests;

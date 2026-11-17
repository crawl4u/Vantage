use bevy::{prelude::Resource, render::extract_resource::ExtractResource};
/// Whether custom skyboxes retain the ordinary Bedrock vertical translation.
#[derive(Resource, ExtractResource, Clone, Copy, Debug, PartialEq, Eq)]
#[extract_app(bevy::render::RenderApp)]
pub struct SkyboxSettings {
    pub native_vertical_offset: bool,
    /// Admission from Fancy Skies and active camera vision effects.
    pub visible: bool,
}

/// Whether a pack's sky cube is dimmed with the time of day. A cube is drawn over the stock sky, so
/// without this it looks the same at noon and midnight; the time changer asks for it.
#[derive(Resource, ExtractResource, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[extract_app(bevy::render::RenderApp)]
pub struct SkyboxFollowsTime(pub bool);

impl Default for SkyboxSettings {
    fn default() -> Self {
        Self {
            native_vertical_offset: assets::DEFAULT_NATIVE_SKYBOX_OFFSET,
            visible: true,
        }
    }
}

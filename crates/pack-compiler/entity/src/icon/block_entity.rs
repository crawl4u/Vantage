//! Inventory models whose placed visuals live outside the terrain mesh carrier.

use std::path::Path;

use assets::{AssetError, EntityGeometry, IconSprite, block_entity_geometry as geometry};

use super::{
    blocks::IconBlocks,
    model::{Model, TexturedQuad},
};

struct Part {
    texture: String,
    logical: [f32; 2],
    faces: Vec<geometry::ModelFace>,
    color: [u8; 3],
}

/// Expands model-part boxes into textured faces without changing their authoring frame.
fn boxes(texture: &str, logical: [f32; 2], boxes: &[geometry::ModelBox]) -> Part {
    Part {
        texture: texture.to_owned(),
        logical,
        faces: boxes
            .iter()
            .flat_map(|&(origin, size, uv)| geometry::box_faces(origin, size, uv, 0.0))
            .collect(),
        color: [255; 3],
    }
}

/// The texture stem of a bed item: `minecraft:bed` takes its dye from `metadata`, and a coloured
/// `minecraft:<colour>_bed` names it.
fn bed_color(name: &str, metadata: u32) -> Option<&'static str> {
    let colors = &geometry::SHULKER_COLORS[1..];
    if name == "minecraft:bed" {
        return colors.get(usize::try_from(metadata).ok()?).copied();
    }
    let stem = name.strip_prefix("minecraft:")?.strip_suffix("_bed")?;
    let stem = if stem == "light_gray" { "silver" } else { stem };
    colors.iter().copied().find(|color| *color == stem)
}

/// A bed lying along the view's depth axis, head away, scaled so its length spans the cube. The
/// boxes are authored standing on end; each corner is turned a quarter revolution about X, as the
/// placed bed is, so the mattress lies flat.
fn bed(color: &str) -> Part {
    let faces = geometry::BED_BOXES
        .iter()
        .flat_map(|&(origin, size, uv)| geometry::box_faces(origin, size, uv, 0.0))
        .map(|(corners, texels)| {
            (
                corners.map(|[x, y, z]| {
                    [
                        (y - 16.0) / 32.0 + 0.5,
                        (9.0 - z - 4.5) / 32.0 + 0.5,
                        (8.0 - x) / 32.0 + 0.5,
                    ]
                }),
                texels,
            )
        })
        .collect();
    Part {
        texture: format!("textures/entity/bed/{color}"),
        logical: geometry::BED_TEXTURE_SIZE,
        faces,
        color: [255; 3],
    }
}

/// Only banner dye changes these block models by item metadata.
pub(super) fn metadata_variant(name: &str, metadata: u32) -> u32 {
    if is_banner(name) { metadata } else { 0 }
}

/// Identifies the standing inventory model shared by both placed banner forms.
fn is_banner(name: &str) -> bool {
    renderer(name) == Some(assets::facts::BlockEntityRenderer::Banner)
}

/// Uses the registry-owned block-entity route for cold inventory model selection.
fn renderer(name: &str) -> Option<assets::facts::BlockEntityRenderer> {
    assets::facts::BlockEntityFacts::pinned()
        .by_block_identifier(name)
        .map(|entry| entry.renderer)
}

/// Bakes an entity-drawn block inventory model, refusing missing optional textures.
pub(super) fn raster(
    root: &Path,
    name: &str,
    metadata: u32,
    geometries: &[EntityGeometry],
) -> Result<Option<IconSprite>, AssetError> {
    raster_with(name, metadata, geometries, super::model::SIDE, &mut |path| {
        IconBlocks::model_texture(root, path)
    })
}

/// The chest texture a block's inventory model draws, or `None` for blocks that are not chests.
fn chest_texture(name: &str) -> Option<&'static str> {
    use assets::facts::{BlockEntityRenderer, BlockModelVariant, ChestKind, CopperAge};
    if renderer(name) == Some(BlockEntityRenderer::EnderChest) {
        return Some("textures/entity/chest/ender");
    }
    let BlockModelVariant::Chest(kind) = assets::facts::block_render_fact(name).variant? else {
        return None;
    };
    Some(match kind {
        ChestKind::Normal => "textures/entity/chest/normal",
        ChestKind::Trapped => "textures/entity/chest/trapped",
        ChestKind::Copper(CopperAge::Unaffected) => "textures/entity/chest/copper_default",
        ChestKind::Copper(CopperAge::Exposed) => "textures/entity/chest/copper_exposed",
        ChestKind::Copper(CopperAge::Weathered) => "textures/entity/chest/copper_weathered",
        ChestKind::Copper(CopperAge::Oxidized) => "textures/entity/chest/copper_oxidized",
    })
}

/// A closed chest in the ordinary cube frame, its latch toward the viewer. The boxes follow the
/// placed model's: body, lid and latch over the 64-texel chest unwrap.
fn chest(texture: &str) -> Part {
    let mut part = boxes(
        texture,
        [64.0, 64.0],
        &[
            ([-7.0, 0.0, -7.0], [14.0, 10.0, 14.0], [0.0, 19.0]),
            ([-7.0, 9.0, -7.0], [14.0, 5.0, 14.0], [0.0, 0.0]),
            ([-1.0, 7.0, -8.0], [2.0, 4.0, 1.0], [0.0, 0.0]),
        ],
    );
    for (corners, _) in &mut part.faces {
        // A half turn about the vertical axis puts the latch on the south face.
        for [x, y, z] in corners {
            *x = (8.0 - *x) / 16.0;
            *y /= 16.0;
            *z = (8.0 - *z) / 16.0;
        }
    }
    part
}

/// [`raster`] with the textures supplied by `texture`, which resolves a pack-relative path such
/// as `textures/entity/chest/normal` to its pixels, so a session can draw from its own pack. The
/// sprite is `side` pixels across.
pub(super) fn raster_with(
    name: &str,
    metadata: u32,
    geometries: &[EntityGeometry],
    side: usize,
    texture: &mut dyn FnMut(&str) -> Result<Option<IconSprite>, AssetError>,
) -> Result<Option<IconSprite>, AssetError> {
    let mut projection: fn([f32; 3]) -> [f32; 3] = assets::gui_item::project_cube;
    let mut parts = if let Some(color) = geometry::shulker_color_from_block_name(name) {
        let mut part = boxes(
            &geometry::shulker_texture(color),
            geometry::SHULKER_TEXTURE_SIZE,
            &[geometry::SHULKER_BASE, geometry::SHULKER_LID],
        );
        for (corners, _) in &mut part.faces {
            for [x, y, z] in corners {
                *x = (*x + 8.0) / 16.0;
                *y /= 16.0;
                *z = (*z + 8.0) / 16.0;
            }
        }
        vec![part]
    } else if let Some(color) = bed_color(name, metadata) {
        vec![bed(color)]
    } else if let Some(chest_texture) = chest_texture(name) {
        vec![chest(chest_texture)]
    } else if let Some(skull_type) = assets::vanilla_skull_type(name) {
        let Some(part) = skull(skull_type, geometries) else {
            return Ok(None);
        };
        vec![part]
    } else {
        match renderer(name) {
            Some(assets::facts::BlockEntityRenderer::Conduit) => {
                projection = assets::gui_item::project_conduit;
                vec![boxes(
                    geometry::CONDUIT_TEXTURE.0,
                    geometry::CONDUIT_TEXTURE.1,
                    &[geometry::CONDUIT_SHELL],
                )]
            }
            Some(assets::facts::BlockEntityRenderer::DecoratedPot) => {
                projection = assets::gui_item::project_decorated_pot;
                let mut base = boxes(
                    geometry::POT_BASE_TEXTURE.0,
                    geometry::POT_BASE_TEXTURE.1,
                    &[],
                );
                for ((origin, size, uv), inflate) in [
                    (geometry::POT_NECK, geometry::POT_NECK_INFLATE),
                    (geometry::POT_LIP, geometry::POT_LIP_INFLATE),
                ] {
                    base.faces
                        .extend(geometry::box_faces(origin, size, uv, inflate));
                }
                for (texels, y) in geometry::POT_PLANES {
                    let half = geometry::POT_BODY_HALF;
                    base.faces.push((
                        [
                            [half, y, half],
                            [-half, y, half],
                            [-half, y, -half],
                            [half, y, -half],
                        ],
                        texels,
                    ));
                }
                vec![
                    base,
                    Part {
                        texture: geometry::POT_SIDE_TEXTURE.0.to_owned(),
                        logical: geometry::POT_SIDE_TEXTURE.1,
                        faces: geometry::pot_sides()
                            .into_iter()
                            .map(|corners| (corners, geometry::POT_SIDE_TEXELS))
                            .collect(),
                        color: [255; 3],
                    },
                ]
            }
            Some(assets::facts::BlockEntityRenderer::Statue) => {
                projection = assets::gui_item::project_statue;
                let Some(parts) = statue(name, geometries) else {
                    return Ok(None);
                };
                parts
            }
            Some(assets::facts::BlockEntityRenderer::Lectern) => lectern(),
            Some(assets::facts::BlockEntityRenderer::Banner) => {
                projection = assets::gui_item::project_banner;
                let frame = boxes(
                    geometry::BANNER_TEXTURE.0,
                    geometry::BANNER_TEXTURE.1,
                    &[geometry::BANNER_POLE, geometry::BANNER_BAR],
                );
                let mut cloth = boxes(
                    geometry::BANNER_TEXTURE.0,
                    geometry::BANNER_TEXTURE.1,
                    &[geometry::BANNER_CLOTH],
                );
                cloth.color = assets::banner::color_rgb(i64::from(metadata));
                for (corners, _) in &mut cloth.faces {
                    for point in corners {
                        for (value, offset) in point.iter_mut().zip(geometry::BANNER_CLOTH_PIVOT) {
                            *value += offset;
                        }
                    }
                }
                vec![frame, cloth]
            }
            _ => return Ok(None),
        }
    };
    let mut textures = Vec::with_capacity(parts.len());
    for part in &parts {
        let Some(mut texture) = texture(&part.texture)? else {
            return Ok(None);
        };
        if part.color != [255; 3] {
            for pixel in std::sync::Arc::make_mut(&mut texture.rgba8)
                .as_chunks_mut::<4>()
                .0
            {
                for (value, tint) in pixel[..3].iter_mut().zip(part.color) {
                    *value = (u16::from(*value) * u16::from(tint) / 255) as u8;
                }
            }
        }
        textures.push(texture);
    }
    let mut quads = Vec::new();
    for (part, texture) in parts.iter_mut().zip(&textures) {
        let [width, height] = part.logical;
        for &(corners, [u, v, w, h]) in &part.faces {
            if w <= 0.0 || h == 0.0 {
                continue;
            }
            let uvs = [[u, v], [u + w, v], [u + w, v + h], [u, v + h]]
                .map(|[u, v]| [u / width, v / height]);
            quads.push(TexturedQuad {
                corners,
                uvs,
                texture,
                face_dimming: true,
            });
        }
    }
    let model = Model::textured(quads, projection, 1.0);
    Ok(Some(
        if is_banner(name)
            || assets::vanilla_skull_type(name).is_some()
            || renderer(name) == Some(assets::facts::BlockEntityRenderer::Statue)
        {
            model.unshaded()
        } else {
            model
        }
        .raster_filled(side),
    ))
}

/// Builds the default standing statue with its eye layer above the body artwork.
fn statue(name: &str, geometries: &[EntityGeometry]) -> Option<Vec<Part>> {
    use assets::facts::BlockModelVariant;
    let BlockModelVariant::Statue(age) = assets::facts::block_render_fact(name).variant? else {
        return None;
    };
    let geometry = geometries
        .iter()
        .find(|geometry| geometry.identifier.as_ref() == geometry::heads::STATUE_GEOMETRIES[0])?;
    let model = geometry::heads::HeadModel::build_tree(geometry, None, 1.0, false)?;
    let faces: Vec<_> = model
        .boxes
        .iter()
        .flat_map(geometry::heads::HeadBox::faces)
        .collect();
    let [body, eyes] = geometry::heads::statue_textures(age);
    // Equal-depth opaque samples keep the first layer in the offline rasterizer.
    Some(
        [eyes, body]
            .map(|texture| Part {
                texture,
                logical: model.texture,
                faces: faces.clone(),
                color: [255; 3],
            })
            .into(),
    )
}

#[cfg(test)]
mod pot_texture_tests {
    use super::*;

    #[test]
    fn pot_side_texture_border_stays_outside_the_preview() {
        let root = tempfile::tempdir().unwrap();
        let texture_path = |name: &str| root.path().join(format!("{name}.png"));
        let base = texture_path(geometry::POT_BASE_TEXTURE.0);
        std::fs::create_dir_all(base.parent().unwrap()).unwrap();
        image::RgbaImage::new(32, 32).save(base).unwrap();
        let side = texture_path(geometry::POT_SIDE_TEXTURE.0);
        let mut image = image::RgbaImage::from_pixel(16, 16, image::Rgba([180, 90, 60, 255]));
        image.save(&side).unwrap();
        let opaque = raster(root.path(), "minecraft:decorated_pot", 0, &[])
            .unwrap()
            .unwrap();
        assert!(
            opaque
                .rgba8
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 0)
        );
        for y in 0..16 {
            for x in [0, 15] {
                image.put_pixel(x, y, image::Rgba([0; 4]));
            }
        }
        image.save(side).unwrap();
        let bordered = raster(root.path(), "minecraft:decorated_pot", 0, &[])
            .unwrap()
            .unwrap();
        assert_eq!(bordered.rgba8, opaque.rgba8);
    }
}

/// A head in the ordinary cube frame, from `geometries` for dragon and piglin heads. Like other
/// block items' fronts, the face turns south, and the head's uninflated bounds fill the cube
/// (an 8-texel head box spans it). Both are provisional until vanilla's skull GUI transform is
/// measured.
fn skull(skull_type: u8, geometries: &[EntityGeometry]) -> Option<Part> {
    let (model, texture) = geometry::heads::skull_model(skull_type, geometries)?;
    let mut bounds = [[f32::MAX; 3], [f32::MIN; 3]];
    for head_box in &model.boxes {
        let bare = geometry::heads::HeadBox {
            matrix: head_box.matrix,
            spec: head_box.spec.inflated(0.0),
        };
        for point in bare.faces().into_iter().flat_map(|(corners, _)| corners) {
            for axis in 0..3 {
                bounds[0][axis] = bounds[0][axis].min(point[axis]);
                bounds[1][axis] = bounds[1][axis].max(point[axis]);
            }
        }
    }
    let [min, max] = bounds;
    let extent = (0..3).map(|axis| max[axis] - min[axis]).fold(0.0, f32::max);
    if !extent.is_finite() || extent <= 0.0 {
        return None;
    }
    let center = [0, 2].map(|axis| (min[axis] + max[axis]) / 2.0);
    let faces = model
        .boxes
        .iter()
        .flat_map(geometry::heads::HeadBox::faces)
        .map(|(corners, texels)| {
            // A half turn about Y, centered, bottom on the floor.
            let corners = corners.map(|[x, y, z]| {
                [
                    (center[0] - x) / extent + 0.5,
                    (y - min[1]) / extent,
                    (center[1] - z) / extent + 0.5,
                ]
            });
            (corners, texels)
        })
        .collect();
    Some(Part {
        texture: texture.to_owned(),
        logical: model.texture,
        faces,
        color: [255; 3],
    })
}

/// Groups the authored lectern faces by their terrain texture.
/// Turns the shared north-facing stand into its south-facing inventory pose.
fn lectern() -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for (texture_index, (corners, texels)) in geometry::lectern_faces() {
        let texture = geometry::LECTERN_TEXTURES[texture_index];
        let face = (
            corners.map(|[x, y, z]| [(8.0 - x) / 16.0, y / 16.0, (8.0 - z) / 16.0]),
            texels,
        );
        if let Some(part) = parts.iter_mut().find(|part| part.texture == texture) {
            part.faces.push(face);
        } else {
            parts.push(Part {
                texture: texture.to_owned(),
                logical: [16.0; 2],
                faces: vec![face],
                color: [255; 3],
            });
        }
    }
    parts
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod chest_icon_tests {
    use super::*;

    /// A 64-texel chest unwrap of one colour.
    fn unwrap_of(colour: [u8; 4]) -> IconSprite {
        IconSprite {
            width: 64,
            height: 64,
            rgba8: colour.repeat(64 * 64).into(),
        }
    }

    /// Every chest block draws the pack's chest texture, so a pack's art reaches its inventory slot.
    #[test]
    fn every_chest_icon_is_drawn_from_the_texture_supplied_for_it() {
        for (name, texture) in [
            ("minecraft:chest", "textures/entity/chest/normal"),
            ("minecraft:trapped_chest", "textures/entity/chest/trapped"),
            ("minecraft:ender_chest", "textures/entity/chest/ender"),
            (
                "minecraft:copper_chest",
                "textures/entity/chest/copper_default",
            ),
            (
                "minecraft:oxidized_copper_chest",
                "textures/entity/chest/copper_oxidized",
            ),
        ] {
            let mut asked = Vec::new();
            let red = raster_with(name, 0, &[], 32, &mut |path| {
                asked.push(path.to_owned());
                Ok(Some(unwrap_of([200, 20, 20, 255])))
            })
            .unwrap()
            .unwrap_or_else(|| panic!("{name} has an inventory model"));
            assert_eq!(asked, [texture], "{name}");
            let blue = raster_with(name, 0, &[], 32, &mut |_| {
                Ok(Some(unwrap_of([20, 20, 200, 255])))
            })
            .unwrap()
            .unwrap();
            assert_ne!(red.rgba8, blue.rgba8, "{name} shows its supplied texture");
            let drawn = red.rgba8.as_chunks::<4>().0.iter().filter(|p| p[3] != 0);
            assert!(drawn.clone().count() > 100, "{name} fills the slot");
            assert!(
                drawn.clone().all(|p| p[0] > p[2]),
                "{name} is only the red texture, shaded"
            );
            assert!(
                raster_with(name, 0, &[], 32, &mut |_| Ok(None))
                    .unwrap()
                    .is_none(),
                "a pack without the texture keeps the vanilla icon"
            );
        }
        assert!(
            raster_with("minecraft:stone", 0, &[], 32, &mut |_| Ok(Some(unwrap_of(
                [1; 4]
            ))))
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn an_entity_drawn_block_icon_fills_its_slot_like_a_full_block() {
        for name in ["minecraft:chest", "minecraft:red_shulker_box", "minecraft:ender_chest"] {
            let icon = raster_with(name, 0, &[], 64, &mut |_| {
                Ok(Some(unwrap_of([200, 20, 20, 255])))
            })
            .unwrap()
            .unwrap();
            let side = usize::from(icon.width);
            let covered = |index: usize| icon.rgba8[index * 4 + 3] != 0;
            let (mut low, mut high) = ([side; 2], [0usize; 2]);
            for index in (0..side * side).filter(|&index| covered(index)) {
                for (axis, value) in [index % side, index / side].into_iter().enumerate() {
                    low[axis] = low[axis].min(value);
                    high[axis] = high[axis].max(value);
                }
            }
            let extent = (high[0] - low[0]).max(high[1] - low[1]) + 1;
            assert!(extent >= side * 7 / 8, "{name} spans {extent} of {side}");
            assert!(low.iter().all(|&edge| edge > 0) && high.iter().all(|&edge| edge < side - 1));
        }
    }

    #[test]
    fn a_bed_item_draws_its_dyed_texture_lying_flat_with_the_pillow_far_right() {
        let texture = |color: [u8; 4]| IconSprite {
            width: 64,
            height: 64,
            rgba8: color.repeat(64 * 64).into(),
        };
        let mut asked = Vec::new();
        let red = raster_with("minecraft:bed", 14, &[], 64, &mut |path| {
            asked.push(path.to_owned());
            Ok(Some(texture([200, 20, 20, 255])))
        })
        .unwrap()
        .expect("a bed has an inventory model");
        assert_eq!(asked, ["textures/entity/bed/red"]);
        let named = raster_with("minecraft:red_bed", 0, &[], 64, &mut |_| {
            Ok(Some(texture([200, 20, 20, 255])))
        })
        .unwrap()
        .unwrap();
        assert_eq!(red.rgba8, named.rgba8, "metadata and name pick the same colour");
        // Lying flat, the bed is wider than it is tall.
        let pixels = red.rgba8.as_chunks::<4>().0;
        let rows: Vec<usize> = (0..64)
            .filter(|row| pixels[row * 64..][..64].iter().any(|p| p[3] != 0))
            .collect();
        let columns: Vec<usize> = (0..64)
            .filter(|column| (0..64).any(|row| pixels[row * 64 + column][3] != 0))
            .collect();
        assert!(columns.len() > rows.len() * 5 / 4);
        assert!(raster_with("minecraft:bed", 16, &[], 64, &mut |_| Ok(Some(texture([1; 4]))))
            .unwrap()
            .is_none());
    }
}

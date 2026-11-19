#[test]
fn positional_materials_keep_one_cell_per_quad_on_every_cube_face() {
    let base = RuntimeAssets::diagnostic();
    let texture = TextureRef::new(1, 0).unwrap();
    let mut page = base.texture_array().clone();
    page.layers = 2;
    for mip in &mut page.mips {
        mip.rgba8 = mip.rgba8.repeat(2).into();
    }
    let mut overlay = assets::BlockOverlay {
        visuals: vec![BlockVisual {
            native_material: assets::native_material::NativeMaterialFacts::Unresolved,
            faces: [0; 6],
            flags: BlockFlags::CUBE_GEOMETRY | BlockFlags::OCCLUDES_FULL_FACE,
            kind: VisualKind::Cube,
            support: assets::VisualSupport::VanillaFallback,
            contributor_role: assets::ContributorRole::Primary,
            model_template: NO_MODEL_TEMPLATE,
            animation: NO_ANIMATION,
            variant: 0,
            destroy: assets::compiled::block_destroy::BlockDestroyFacts::Unresolved,
        }],
        light_properties: vec![assets::LightProperties::OPAQUE_DARK],
        materials: vec![
            Material {
                texture,
                variation_start: 1,
                variation_count: 2,
                ..Material::unvaried()
            },
            Material {
                texture,
                variation_weight: 0.5_f32.to_bits(),
                ..Material::unvaried()
            },
            Material {
                texture: TextureRef::new(1, 1).unwrap(),
                variation_weight: 0.5_f32.to_bits(),
                ..Material::unvaried()
            },
        ],
        textures: vec![page],
        ..Default::default()
    };
    let sub = uniform(1);
    for positional in [true, false] {
        if !positional {
            overlay.materials[0].variation_start = 0;
            overlay.materials[0].variation_count = 0;
        }
        let assets = base.with_block_overlay(1, &overlay).unwrap();
        let mesh = mesh_sub_chunk(
            &classifier(),
            &assets,
            NetworkIdMode::Sequential,
            &Neighbourhood::empty(),
            &sub,
        );
        assert_eq!(mesh.quad_count(), if positional { 6 * 16 * 16 } else { 6 });
        if positional {
            assert!(
                mesh.quads()
                    .iter()
                    .all(|q| q.width() == 1 && q.height() == 1)
            );
        }
    }
}

#[test]
fn mixed_biome_option_keeps_tinted_faces_one_block_wide() {
    let base = RuntimeAssets::diagnostic();
    let texture = TextureRef::new(1, 0).unwrap();
    let mut page = base.texture_array().clone();
    page.layers = 2;
    for mip in &mut page.mips {
        mip.rgba8 = mip.rgba8.repeat(2).into();
    }
    let overlay = assets::BlockOverlay {
        visuals: vec![BlockVisual {
            native_material: assets::native_material::NativeMaterialFacts::Unresolved,
            faces: [0; 6],
            flags: BlockFlags::CUBE_GEOMETRY | BlockFlags::OCCLUDES_FULL_FACE,
            kind: VisualKind::Cube,
            support: assets::VisualSupport::VanillaFallback,
            contributor_role: assets::ContributorRole::Primary,
            model_template: NO_MODEL_TEMPLATE,
            animation: NO_ANIMATION,
            variant: 0,
            destroy: assets::compiled::block_destroy::BlockDestroyFacts::Unresolved,
        }],
        light_properties: vec![assets::LightProperties::OPAQUE_DARK],
        materials: vec![Material {
            texture,
            flags: assets::MATERIAL_FLAG_GRASS_TINT,
            ..Material::unvaried()
        }],
        textures: vec![page],
        ..Default::default()
    };
    let assets = base.with_block_overlay(1, &overlay).unwrap();
    let sub = uniform(1);
    for split in [false, true] {
        let mesh = meshing::mesh_sub_chunk_in_neighbourhood_with_options(
            &classifier(),
            &assets,
            NetworkIdMode::Sequential,
            &world::MeshNeighbourhood::new(&sub),
            &meshing::lighting::FullBrightLightSampler,
            meshing::MeshOptions {
                split_tinted_faces: split,
                ..Default::default()
            },
        );
        if split {
            assert_eq!(mesh.quad_count(), 6 * 16 * 16);
            assert!(
                mesh.quads()
                    .iter()
                    .all(|q| q.width() == 1 && q.height() == 1)
            );
        } else {
            assert_eq!(
                mesh.quad_count(),
                6,
                "uniform biomes keep merged tinted faces"
            );
        }
    }
}

#[test]
fn connected_texture_faces_record_which_neighbours_continue_them() {
    let base = RuntimeAssets::diagnostic();
    let texture = TextureRef::new(1, 0).unwrap();
    let mut page = base.texture_array().clone();
    page.layers = 2;
    for mip in &mut page.mips {
        mip.rgba8 = mip.rgba8.repeat(2).into();
    }
    let tiles = u32::from(assets::CONNECTED_TILE_COUNT);
    let mut materials = vec![Material {
        texture,
        variation_start: 1,
        variation_count: tiles,
        mask_strength: assets::compiled::connected_mask_strength(),
        ..Material::unvaried()
    }];
    materials.extend((0..tiles).map(|_| Material {
        texture,
        variation_weight: (1.0_f32 / tiles as f32).to_bits(),
        ..Material::unvaried()
    }));
    let overlay = assets::BlockOverlay {
        visuals: vec![BlockVisual {
            native_material: assets::native_material::NativeMaterialFacts::Unresolved,
            faces: [0; 6],
            flags: BlockFlags::CUBE_GEOMETRY | BlockFlags::OCCLUDES_FULL_FACE,
            kind: VisualKind::Cube,
            support: assets::VisualSupport::VanillaFallback,
            contributor_role: assets::ContributorRole::Primary,
            model_template: NO_MODEL_TEMPLATE,
            animation: NO_ANIMATION,
            variant: 0,
            destroy: assets::compiled::block_destroy::BlockDestroyFacts::Unresolved,
        }],
        light_properties: vec![assets::LightProperties::OPAQUE_DARK],
        materials,
        textures: vec![page],
        ..Default::default()
    };
    let assets = base.with_block_overlay(1, &overlay).unwrap();
    let mesh = mesh_sub_chunk(
        &classifier(),
        &assets,
        NetworkIdMode::Sequential,
        &Neighbourhood::empty(),
        &uniform(1),
    );
    // Every face is one block, so each can name its own tile.
    assert_eq!(mesh.quad_count(), 6 * 16 * 16);
    let tile_at = |origin: [u8; 3]| {
        mesh.quads()
            .iter()
            .find(|quad| quad.face() == meshing::Face::PositiveZ && quad.origin() == origin)
            .map(|quad| quad.connection())
    };
    // A viewer in front of +Z sees +X to the right and +Y above. The open interior of the face
    // is fully surrounded; its corners keep only the two neighbours they have.
    assert_eq!(tile_at([5, 5, 15]), Some(26));
    assert_eq!(tile_at([0, 15, 15]), Some(13));
    assert_eq!(tile_at([15, 15, 15]), Some(15));
    assert_eq!(tile_at([0, 0, 15]), Some(37));
    assert_eq!(tile_at([15, 0, 15]), Some(39));
    assert_eq!(tile_at([5, 15, 15]), Some(14));

    // Loaded sections beside the face continue it across the boundary, and a diagonal section
    // decides the corner dot.
    let (centre, east, above, north_east) = (uniform(1), uniform(1), uniform(1), uniform(1));
    let corner = |with_diagonal: bool| {
        let mut neighbourhood = world::MeshNeighbourhood::new(&centre);
        assert!(neighbourhood.insert([1, 0, 0], &east));
        assert!(neighbourhood.insert([0, 1, 0], &above));
        if with_diagonal {
            assert!(neighbourhood.insert([1, 1, 0], &north_east));
        }
        let mesh = meshing::mesh_sub_chunk_in_neighbourhood(
            &classifier(),
            &assets,
            NetworkIdMode::Sequential,
            &neighbourhood,
        );
        let tile = |origin: [u8; 3]| {
            mesh.quads()
                .iter()
                .find(|quad| quad.face() == meshing::Face::PositiveZ && quad.origin() == origin)
                .map(|quad| quad.connection())
        };
        (tile([15, 15, 15]), tile([15, 5, 15]))
    };
    assert_eq!(corner(false), (Some(44), Some(26)));
    assert_eq!(corner(true), (Some(26), Some(26)));
}

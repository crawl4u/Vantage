struct Projection { clip_from_hand: mat4x4<f32> }
@group(0) @binding(0) var<uniform> projection: Projection;
@group(0) @binding(1) var page0: texture_2d_array<f32>;
@group(0) @binding(2) var page1: texture_2d_array<f32>;
@group(0) @binding(3) var page2: texture_2d_array<f32>;
@group(0) @binding(4) var page3: texture_2d_array<f32>;
@group(0) @binding(5) var page4: texture_2d_array<f32>;
@group(0) @binding(6) var pages: sampler;
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) slot: vec2<u32>,
    @location(2) @interpolate(flat) tint: u32,
}
@vertex fn hand_vertex(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) slot: vec2<u32>,
    @location(3) tint: u32,
) -> VertexOut {
    var output: VertexOut;
    output.position = projection.clip_from_hand * vec4<f32>(position, 1.0);
    output.uv = uv;
    output.slot = slot;
    output.tint = tint;
    return output;
}
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
@fragment fn hand_fragment(input: VertexOut) -> @location(0) vec4<f32> {
    // Gradients come from uniform control flow so the page switch below keeps mip selection.
    let dx = dpdx(input.uv);
    let dy = dpdy(input.uv);
    let layer = i32(input.slot.y);
    var color: vec4<f32>;
    switch input.slot.x {
        case 0u: { color = textureSampleGrad(page0, pages, input.uv, layer, dx, dy); }
        case 1u: { color = textureSampleGrad(page1, pages, input.uv, layer, dx, dy); }
        case 2u: { color = textureSampleGrad(page2, pages, input.uv, layer, dx, dy); }
        case 3u: { color = textureSampleGrad(page3, pages, input.uv, layer, dx, dy); }
        default: { color = textureSampleGrad(page4, pages, input.uv, layer, dx, dy); }
    }
    var rgb = color.rgb;
    var alpha = color.a;
    if (input.tint & 0x1000000u) != 0u {
        let tint = vec3<f32>(
            f32(input.tint & 255u),
            f32((input.tint >> 8u) & 255u),
            f32((input.tint >> 16u) & 255u),
        ) / 255.0;
        if (input.tint & 0x2000000u) != 0u {
            // The alpha says where the tint lands; the face itself is opaque.
            rgb = mix(rgb, rgb * tint, alpha);
            alpha = 1.0;
        } else {
            rgb = rgb * tint;
        }
    }
    if alpha < 0.5 { discard; }
    // The pages are sampled as stored, so the colour leaves gamma space only here.
    return vec4<f32>(srgb_to_linear(rgb), 1.0);
}

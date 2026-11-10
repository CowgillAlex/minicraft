@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};
@vertex fn vs_hud(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) uv: vec2<f32>) -> Out {
    var out: Out;
    out.position = vec4(position, 0.0, 1.0);
    out.color = color;
    out.uv = uv;
    return out;
}
@fragment fn fs_hud(v: Out) -> @location(0) vec4<f32> {
    // UI colours are authored in sRGB; the render target performs the final encoding.
    let linear = select(v.color.rgb / 12.92, pow((v.color.rgb + 0.055) / 1.055, vec3(2.4)), v.color.rgb > vec3(0.04045));
    return vec4(linear, v.color.a * textureSample(atlas, atlas_sampler, v.uv).r);
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out; o.position = vec4(p * 2.0 - 1.0, 0.0, 1.0); o.uv = p * vec2(1.0,-1.0) + vec2(0.0,1.0); return o;
}
@fragment fn fs(v: Out) -> @location(0) vec4<f32> { return textureSampleLevel(image,image_sampler,v.uv,0.0); }

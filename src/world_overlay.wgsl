@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(1) @binding(0) var scene_depth: texture_depth_2d;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) anchor_uv: vec2<f32>,
};
@vertex fn vs(@location(0) position:vec3<f32>, @location(1) color:vec4<f32>, @location(2) uv:vec2<f32>, @location(3) anchor_uv:vec2<f32>) -> Out {
    var o:Out; o.position=vec4(position,1.0); o.color=color; o.uv=uv; o.anchor_uv=anchor_uv; return o;
}
@fragment fn fs(v:Out) -> @location(0) vec4<f32> {
    let size=vec2<i32>(textureDimensions(scene_depth));
    let pixel=clamp(vec2<i32>(v.anchor_uv*vec2<f32>(size)),vec2(0),size-1);
    // Test the world anchor once per fragment so glyphs stay whole at silhouettes.
    if textureLoad(scene_depth,pixel,0)+0.000002 < v.position.z { discard; }
    let linear=select(v.color.rgb/12.92,pow((v.color.rgb+0.055)/1.055,vec3(2.4)),v.color.rgb>vec3(0.04045));
    return vec4(linear,v.color.a*textureSampleLevel(atlas,atlas_sampler,v.uv,0.0).r);
}

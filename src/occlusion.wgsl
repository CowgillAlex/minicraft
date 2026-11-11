struct Uniforms {
    view_proj: mat4x4<f32>, inverse_view_proj: mat4x4<f32>, light_view_proj: mat4x4<f32>,
    camera: vec4<f32>, sun: vec4<f32>, options: vec4<f32>,
    daylight: vec4<f32>, light_color: vec4<f32>, view: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: Uniforms;
@group(1) @binding(0) var depth_map: texture_depth_2d;
@group(1) @binding(1) var normal_map: texture_2d<f32>;
@group(1) @binding(2) var ao_map: texture_2d<f32>;
@group(1) @binding(3) var color_map: texture_2d<f32>;
@group(1) @binding(4) var bloom_map: texture_2d<f32>;
@group(1) @binding(5) var bloom_sampler: sampler;
@vertex fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4(p, 0.0, 1.0);
}
fn full_pixel(p: vec2<i32>) -> vec2<i32> { return clamp(p, vec2(0), vec2<i32>(textureDimensions(depth_map)) - 1); }
fn world_position(p: vec2<i32>) -> vec3<f32> {
    let pixel = full_pixel(p);
    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(depth_map));
    let depth = textureLoad(depth_map, pixel, 0);
    let projected = scene.inverse_view_proj * vec4(uv * vec2(2.0,-2.0) + vec2(-1.0,1.0), depth, 1.0);
    return projected.xyz / projected.w;
}
@fragment fn fs_ao(@builtin(position) screen: vec4<f32>) -> @location(0) f32 {
    let pixel = full_pixel(vec2<i32>(screen.xy) * 2 + 1);
    let packed = textureLoad(normal_map, pixel, 0);
    if packed.a < 0.001 || textureLoad(depth_map, pixel, 0) > 0.99998 { return 1.0; }
    let point = world_position(pixel);
    let normal = normalize(packed.xyz * 2.0 - 1.0);
    let distance = length(point - scene.camera.xyz);
    let radius = 0.55;
    let radius_pixels = clamp(radius / max(distance,0.05) * f32(textureDimensions(depth_map).y) * 0.78, 2.0, 150.0);
    let half_pixel = vec2<i32>(screen.xy);
    let rotation = f32((half_pixel.x & 3) + (half_pixel.y & 3) * 4) * 0.392699;
    var occlusion = 0.0;
    var samples = 0.0;
    for (var ray = 0; ray < 8; ray++) {
        let angle = f32(ray) * 0.785398 + rotation;
        let direction = vec2(cos(angle),sin(angle));
        for (var step = 1; step <= 3; step++) {
            let fraction = f32(step) / 3.0;
            let offset = vec2<i32>(round(direction * radius_pixels * fraction * fraction));
            let sample_pixel = full_pixel(pixel + offset);
            if textureLoad(depth_map, sample_pixel, 0) > 0.99998 { continue; }
            let delta = world_position(sample_pixel) - point;
            let separation = length(delta);
            let horizon = max(dot(normal, delta) / max(separation,0.001) - 0.065, 0.0);
            let range_weight = 1.0 - smoothstep(0.03,radius,separation);
            occlusion += horizon * range_weight;
            samples += 1.0;
        }
    }
    return pow(clamp(1.0 - occlusion / max(samples,1.0) * 3.0, 0.18, 1.0), 1.35);
}
fn tonemap(color: vec3<f32>) -> vec3<f32> {
    let x=color*mix(1.55,1.1,scene.daylight.x);
    return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14),vec3(0.0),vec3(1.0));
}
@fragment fn fs_composite(@builtin(position) screen: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = full_pixel(vec2<i32>(screen.xy));
    let raw_color=textureLoad(color_map,pixel,0).rgb;
    if scene.options.w>0.5 {return vec4(raw_color,1.0);}
    let bloom=textureSampleLevel(bloom_map,bloom_sampler,screen.xy/vec2<f32>(textureDimensions(color_map)),0.0).rgb*mix(0.16,0.08,scene.daylight.x);
    let color=raw_color;
    let packed = textureLoad(normal_map,pixel,0);
    if scene.options.z < 0.5 || packed.a < 0.001 { return vec4(tonemap(color+bloom),1.0); }
    let point = world_position(pixel);
    let normal = normalize(packed.xyz*2.0-1.0);
    let half_pos = (vec2<f32>(pixel)+0.5)*0.5-0.5;
    let half_center = vec2<i32>(floor(half_pos));
    var total = 0.0;
    var weight_sum = 0.0;
    // Depth- and normal-aware reconstruction prevents dark halos at silhouettes.
    for (var y = -1; y <= 1; y++) { for (var x = -1; x <= 1; x++) {
        let p = clamp(half_center+vec2(x,y),vec2(0),vec2<i32>(textureDimensions(ao_map))-1);
        let full = full_pixel(p*2+1);
        let other = textureLoad(normal_map,full,0);
        if length(other.xyz*2.0-1.0)<0.5 { continue; }
        let difference = world_position(full)-point;
        let spatial = vec2<f32>(p)-half_pos;
        let weight = exp(-dot(spatial,spatial)*0.6) * exp(-length(difference)*10.0)
            * pow(max(dot(normal,normalize(other.xyz*2.0-1.0)),0.0),16.0);
        total += textureLoad(ao_map,p,0).r * weight;
        weight_sum += weight;
    }}
    let ao = select(1.0,total/max(weight_sum,0.00001),weight_sum>0.00001);
    return vec4(tonemap(color * (1.0-packed.a*(1.0-ao))+bloom),1.0);
}

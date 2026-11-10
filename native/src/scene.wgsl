struct Uniforms {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    sun: vec4<f32>,
    options: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: Uniforms;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) ao: vec4<f32>,
    @location(5) uv: vec2<f32>,
    @location(4) material: f32,
};
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) ao: vec4<f32>,
    @location(5) uv: vec2<f32>,
    @location(4) @interpolate(flat) material: f32,
};
@vertex fn vs_world(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = scene.view_proj * vec4(v.position, 1.0);
    out.world = v.position;
    out.normal = v.normal;
    out.color = v.color;
    out.ao = v.ao;
    out.uv = v.uv;
    out.material = v.material;
    return out;
}
@vertex fn vs_shadow(v: VertexIn) -> @builtin(position) vec4<f32> {
    return scene.light_view_proj * vec4(v.position, 1.0);
}
fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let altitude = max(direction.y, 0.0);
    let horizon = vec3(0.23, 0.27, 0.27);
    let zenith = vec3(0.035, 0.09, 0.15);
    var color = mix(horizon, zenith, pow(altitude, 0.48));
    let sun_dot = max(dot(direction, scene.sun.xyz), 0.0);
    color += vec3(0.52, 0.32, 0.13) * pow(sun_dot, 28.0);
    color += vec3(2.5, 1.8, 0.8) * smoothstep(0.9994, 0.9998, sun_dot);
    return color;
}
fn shadow_visibility(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    let projected = scene.light_view_proj * vec4(world + normal * 0.004, 1.0);
    let ndc = projected.xyz / projected.w;
    let uv = ndc.xy * vec2(0.5, -0.5) + 0.5;
    // The orthographic light matrix has perpendicular, scaled basis rows.
    // Transform the geometric face normal into light clip space analytically.
    // Screen derivatives amplified interpolation errors into triangle-shaped shadows.
    let rows = transpose(scene.light_view_proj);
    let plane = vec3(
        dot(normal,rows[0].xyz)/dot(rows[0].xyz,rows[0].xyz),
        dot(normal,rows[1].xyz)/dot(rows[1].xyz,rows[1].xyz),
        dot(normal,rows[2].xyz)/dot(rows[2].xyz,rows[2].xyz)
    );
    if dot(normal,scene.sun.xyz)<=0.0 { return 1.0; }
    let gradient = vec2(-2.0*plane.x,2.0*plane.y)/plane.z;
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) || ndc.z > 1.0 || ndc.z < 0.0 { return 1.0; }
    // Bound the depth variation across the comparison filter's texel footprint.
    // This keeps a receiver lit regardless of where its triangles cross that grid.
    let resolution = vec2<f32>(textureDimensions(shadow_map));
    let bias = dot(abs(gradient),1.0/resolution) + 0.000015;
    return textureSampleCompareLevel(shadow_map,shadow_sampler,uv,ndc.z-bias);

}
struct SurfaceOut { @location(0) color: vec4<f32>, @location(1) normal: vec4<f32> };
@fragment fn fs_world(v: VertexOut) -> SurfaceOut {
    let normal = normalize(v.normal);
    let direction = normalize(v.world - scene.camera.xyz);
    let sunlight = max(dot(normal, scene.sun.xyz), 0.0);
    let visibility = shadow_visibility(v.world, normal);
    if scene.options.w > 0.5 {
        var out: SurfaceOut;
        out.color=vec4(vec3(visibility),1.0);
        out.normal=vec4(normal*0.5+0.5,0.0);
        return out;
    }
    var base = pow(v.color, vec3(2.2));
    let voxel = floor(v.world * 16.0 + normal * -0.01);
    let noise = fract(sin(dot(voxel, vec3(12.9898, 78.233, 39.425))) * 43758.5453);
    base *= 0.94 + noise * 0.12;
    let voxel_ao = mix(mix(v.ao.x,v.ao.y,v.uv.x),mix(v.ao.w,v.ao.z,v.uv.x),v.uv.y);
    let ambient = base * vec3(0.29, 0.38, 0.47) * (0.62 + 0.38 * max(normal.y, 0.0)) * mix(1.0,voxel_ao,scene.options.z);
    let direct = base * vec3(1.28, 1.09, 0.80) * sunlight * visibility;
    var color = ambient + direct;
    let luma = vec3(0.2126,0.7152,0.0722);
    var ambient_fraction = clamp(dot(ambient,luma)/max(dot(color,luma),0.00001),0.0,1.0);
    if v.material == 9.0 { color = base * 1.4 + vec3(0.4, 0.18, 0.025); ambient_fraction = 0.0; }
    if v.material == 10.0 {
        ambient_fraction = 0.0;
        let fresnel = pow(1.0 - max(dot(-direction, normal), 0.0), 3.0);
        let ripples = sin(v.world.x * 14.0 + scene.options.x) * sin(v.world.z * 9.0 - scene.options.x * 0.7);
        color = mix(color, sky_color(reflect(direction, normal)), 0.25 + fresnel * 0.6);
        color += vec3(0.025, 0.04, 0.035) * max(0.0, ripples);
    }
    if scene.options.y > 0.5 {
        let grid_pos = fract(v.world * 16.0);
        let edge = min(grid_pos, vec3(1.0) - grid_pos);
        let plane_edge = select(select(min(edge.x,edge.z),min(edge.x,edge.y),abs(normal.z)>0.5),min(edge.y,edge.z),abs(normal.x)>0.5);
        let aa = max(fwidth(plane_edge), 0.005);
        color *= 0.65 + 0.35 * smoothstep(0.0, aa * 0.8, plane_edge);
    }
    let distance = length(v.world - scene.camera.xyz);
    let fog = 1.0 - exp(-distance * distance * 0.00012);
    color = mix(color, sky_color(vec3(direction.x, 0.015, direction.z)), min(fog, 0.92));
    var out: SurfaceOut;
    out.color = vec4(color,1.0);
    out.normal = vec4(normal*0.5+0.5,ambient_fraction*(1.0-min(fog,0.92)));
    return out;
}
struct SkyOut { @builtin(position) position: vec4<f32>, @location(0) clip: vec2<f32> };
@vertex fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let p = vec2(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    out.position = vec4(p, 0.99999, 1.0);
    out.clip = p;
    return out;
}
@fragment fn fs_sky(v: SkyOut) -> SurfaceOut {
    let world = scene.inverse_view_proj * vec4(v.clip, 1.0, 1.0);
    let direction = normalize(world.xyz / world.w - scene.camera.xyz);
    var out: SurfaceOut;
    out.color = vec4(sky_color(direction),1.0);
    out.normal = vec4(0.5,0.5,0.5,0.0);
    return out;
}

@vertex fn vs_selection(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return scene.view_proj * vec4(position, 1.0);
}
@fragment fn fs_selection() -> @location(0) vec4<f32> {
    return vec4(0.45, 1.0, 0.7, 1.0);
}

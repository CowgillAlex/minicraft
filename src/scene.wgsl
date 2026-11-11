struct Uniforms {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    sun: vec4<f32>,
    options: vec4<f32>,
    daylight: vec4<f32>,
    light_color: vec4<f32>,
    view: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: Uniforms;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(0) var light_volume: texture_3d<f32>;
@group(2) @binding(1) var light_sampler: sampler;
struct LightRegion {origin:vec4<f32>,size:vec4<f32>};
@group(2) @binding(2) var<uniform> light_region:LightRegion;
fn lighting_at(world: vec3<f32>,normal: vec3<f32>) -> vec2<f32> {
    let position = world + normal * 0.14;
    let uv=(position-light_region.origin.xyz)/light_region.size.xyz;
    if any(uv<vec3(0.0)) || any(uv>vec3(1.0)) { return vec2(0.0,select(1.0,0.0,light_region.origin.w>0.5)); }
    return textureSampleLevel(light_volume,light_sampler,uv,0.0).rg;
}
fn tonemap(color: vec3<f32>) -> vec3<f32> {
    let x=color*mix(1.55,1.1,scene.daylight.x);
    return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14),vec3(0.0),vec3(1.0));
}
struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) ao: vec4<f32>,
    @location(5) uv: vec2<f32>,
};
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) ao: vec4<f32>,
    @location(5) uv: vec2<f32>,
    @location(4) @interpolate(flat) material: f32,
    @location(6) @interpolate(flat) coarse: f32,
};
@vertex fn vs_world(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = scene.view_proj * vec4(v.position, 1.0);
    out.world = v.position;
    out.normal = v.normal.xyz;
    out.color = v.color.rgb;
    out.ao = v.ao;
    out.uv = v.uv;
    out.material = round(v.color.a * 255.0);
    out.coarse = v.normal.w;
    return out;
}
@vertex fn vs_shadow(v: VertexIn) -> @builtin(position) vec4<f32> {
    return scene.light_view_proj * vec4(v.position, 1.0);
}
fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let altitude=max(direction.y,0.0);
    let day=scene.daylight.x;
    let dusk=scene.daylight.y;
    let horizon=mix(vec3(0.008,0.017,0.04),vec3(0.25,0.34,0.43),day)+vec3(0.17,0.055,0.012)*dusk;
    let zenith=mix(vec3(0.002,0.006,0.02),vec3(0.035,0.14,0.29),day);
    var color=mix(horizon,zenith,pow(altitude,0.5));
    let light_dot=max(dot(direction,scene.sun.xyz),0.0);
    color+=scene.light_color.rgb*pow(light_dot,40.0)*scene.sun.w*0.18;
    return color;
}
// Celestial discs and stars belong only to the depth-tested sky pass.
fn visible_sky(direction:vec3<f32>) -> vec3<f32> {
    var color=sky_color(direction);
    let altitude=max(direction.y,0.0);
    let day=scene.daylight.x;
    let light_dot=max(dot(direction,scene.sun.xyz),0.0);
    color+=scene.light_color.rgb*smoothstep(0.99955,0.9998,light_dot)*mix(0.3,6.0,day);
    let star_cell=floor(direction*1200.0);
    let star=fract(sin(dot(star_cell,vec3(127.1,311.7,74.7)))*43758.5453);
    color+=vec3(0.3,0.42,0.65)*pow(max((star-0.9992)/0.0008,0.0),3.0)*(1.0-day)*smoothstep(0.05,0.4,altitude);
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
// Full-height worlds keep only nearby fine meshes resident. Fog covers the cache edge.
fn streamed_world() -> bool { return light_region.origin.w>0.5 || textureDimensions(light_volume).y > 48u; }
fn terrain_fog(distance:f32) -> f32 {
    if scene.view.w<0.5 { return 0.0; }
    let haze=min(1.0-exp(-distance*distance*0.00012*scene.view.z),0.92);
    let edge=smoothstep(scene.view.y,scene.view.x,distance)*scene.view.z;
    return clamp(max(haze,edge),0.0,1.0);
}
fn sky_exposure(skylight:f32) -> f32 {
    return smoothstep(0.02,0.45,skylight);
}
fn fog_sky(direction:vec3<f32>,skylight:f32) -> vec3<f32> {
    var color=sky_color(vec3(direction.x,0.015,direction.z));
    if streamed_world() { color=sky_color(direction); }
    return color*sky_exposure(skylight);
}
struct SurfaceOut { @location(0) color: vec4<f32>, @location(1) normal: vec4<f32> };
@fragment fn fs_world(v: VertexOut) -> SurfaceOut {
    var normal = normalize(v.normal);
    let plane=normalize(cross(dpdx(v.world),dpdy(v.world)));
    if v.coarse>0.5 {
        normal=select(-plane,plane,dot(plane,normal)>=0.0);
    }
    let direction = normalize(v.world - scene.camera.xyz);
    let sunlight = max(dot(normal, scene.sun.xyz), 0.0);
    let field=lighting_at(v.world,normal);
    var visibility=0.0;
    if !streamed_world() || field.y>=0.001 || scene.options.w>0.5 { visibility=shadow_visibility(v.world,normal); }
    if scene.options.w > 0.5 {
        var out: SurfaceOut;
        out.color=vec4(vec3(visibility),1.0);
        out.normal=vec4(normal*0.5+0.5,0.0);
        return out;
    }
    var base = pow(v.color, vec3(2.2));
    let voxel = floor(v.world * 16.0 + normal * -0.01);
    let noise = fract(sin(dot(voxel, vec3(12.9898, 78.233, 39.425))) * 43758.5453);
    let footprint=max(length(dpdx(v.world)),length(dpdy(v.world)))*16.0;
    let detail=1.0-smoothstep(0.5,2.0,footprint);
    base *= 1.0 + (noise-0.5)*0.12*detail;
    let voxel_ao = mix(1.0,mix(mix(v.ao.x,v.ao.y,v.uv.x),mix(v.ao.w,v.ao.z,v.uv.x),v.uv.y),1.0-smoothstep(0.75,2.5,footprint));
    let sky_tint=mix(vec3(0.018,0.035,0.075),vec3(0.29,0.41,0.59),scene.daylight.x);
    let sky_light=max(sky_tint*pow(field.y,1.35),vec3(0.48,0.55,0.64)*scene.light_color.w)*(0.7+0.3*max(normal.y,0.0));
    let block_light=vec3(1.0,0.78,0.55)*pow(field.x,2.0)*3.2;
    let ambient=base*(sky_light+block_light)*mix(1.0,voxel_ao,scene.options.z);
    let direct=base*scene.light_color.rgb*scene.sun.w*sunlight*visibility;
    var color = ambient + direct;
    let luma = vec3(0.2126,0.7152,0.0722);
    var ambient_fraction = clamp(dot(ambient,luma)/max(dot(color,luma),0.00001),0.0,1.0);
    if v.material == 9.0 { color = vec3(1.0,0.78,0.55)*7.0; ambient_fraction = 0.0; }
    if scene.options.y > 0.5 {
        let grid_pos = fract(v.world * 16.0);
        let edge = min(grid_pos, vec3(1.0) - grid_pos);
        let plane_edge = select(select(min(edge.x,edge.z),min(edge.x,edge.y),abs(normal.z)>0.5),min(edge.y,edge.z),abs(normal.x)>0.5);
        let aa = max(fwidth(plane_edge), 0.005);
        color *= 0.65 + 0.35 * smoothstep(0.0, aa * 0.8, plane_edge);
    }
    let distance = length(v.world - scene.camera.xyz);
    let fog = terrain_fog(distance);
    color = mix(color, fog_sky(direction,field.y), fog);
    var out: SurfaceOut;
    out.color = vec4(color,1.0);
    out.normal = vec4(normal*0.5+0.5,ambient_fraction*(1.0-fog));
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
    // The camera may occupy an empty region. Its nearest loaded light brick
    // still describes the surroundings; clamp to that brick's air boundary.
    let camera_uv=(scene.camera.xyz-light_region.origin.xyz)/light_region.size.xyz;
    let exposure=sky_exposure(textureSampleLevel(light_volume,light_sampler,clamp(camera_uv,vec3(0.0),vec3(1.0)),0.0).g);
    out.color = vec4(visible_sky(direction)*exposure,1.0);
    out.normal = vec4(0.5,0.5,0.5,0.0);
    return out;
}

@vertex fn vs_selection(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return scene.view_proj * vec4(position, 1.0);
}
@fragment fn fs_selection() -> @location(0) vec4<f32> {
    return vec4(0.45, 1.0, 0.7, 1.0);
}

@fragment fn fs_transparent(v: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if v.material == 10.0 { return shade_water(v, front); }
    let normal=normalize(v.normal);
    let view=normalize(v.world-scene.camera.xyz);
    let fresnel=pow(1.0-abs(dot(view,normal)),5.0);
    let field=lighting_at(v.world,normal);
    let reflection=sky_color(reflect(view,normal));
    let color=reflection*0.75+vec3(0.025,0.055,0.065)+vec3(1.0,0.78,0.55)*pow(field.x,2.0)*0.12;
    let border=min(min(v.uv.x,1.0-v.uv.x),min(v.uv.y,1.0-v.uv.y));
    let edge=1.0-smoothstep(0.0,max(fwidth(border)*1.5,0.002),border);
    let alpha=(0.10+fresnel*0.48+edge*0.12)*(1.0-terrain_fog(length(v.world-scene.camera.xyz)));
    return vec4(tonemap(color),alpha);
}


@group(3) @binding(0) var opaque_depth: texture_depth_2d;

fn shade_water(v: VertexOut, front: bool) -> vec4<f32> {
    let normal = select(-normalize(v.normal), normalize(v.normal), front);
    let view = normalize(v.world - scene.camera.xyz);
    let phase = scene.options.x;
    let wave = normalize(normal + vec3(
        sin(v.world.x*3.1+phase*0.7)*cos(v.world.z*2.3+phase*0.3), 0.0,
        cos(v.world.x*2.7-phase*0.4)*sin(v.world.z*3.7+phase*0.5)
    ) * 0.025);
    let size = vec2<i32>(textureDimensions(opaque_depth));
    let pixel = clamp(vec2<i32>(v.position.xy), vec2(0), size-1);
    let depth = textureLoad(opaque_depth,pixel,0);
    let uv = (vec2<f32>(pixel)+0.5) / vec2<f32>(size);
    let projected = scene.inverse_view_proj * vec4(uv*vec2(2.0,-2.0)+vec2(-1.0,1.0),depth,1.0);
    let floor_point = projected.xyz / projected.w;
    let distance = length(v.world-scene.camera.xyz);
    let thickness = clamp(select(distance, length(floor_point-scene.camera.xyz)-distance, front), 0.0,24.0);
    let fresnel = 0.02 + 0.98*pow(1.0-abs(dot(view,wave)),5.0);
    let reflected = 0.04+fresnel*0.85;
    // Beer-Lambert attenuation: a shallow bottom stays clear; depth gradually adds tint.
    let transmission = exp(-thickness*0.26)*(1.0-reflected);
    let alpha = clamp(1.0-transmission,0.08,0.98);
    let field = lighting_at(v.world,normal);
    var visibility=0.0;
    if !streamed_world() || field.y>=0.001 { visibility=shadow_visibility(v.world,normalize(v.normal)); }
    let sky = mix(vec3(0.018,0.035,0.075),vec3(0.29,0.41,0.59),scene.daylight.x)*pow(field.y,1.35);
    let lamp = vec3(1.0,0.78,0.55)*pow(field.x,2.0)*3.2;
    let sunlight = scene.light_color.rgb*scene.sun.w*max(dot(normal,scene.sun.xyz),0.0)*visibility;
    let tint = vec3(0.025,0.16,0.20)*max(sky+lamp+sunlight,vec3(0.48,0.55,0.64)*scene.light_color.w);
    var reflection=vec3(0.0);
    if field.y>0.001 { reflection=sky_color(reflect(view,wave))*pow(field.y,1.35)*mix(0.25,1.0,visibility); }
    let specular = pow(max(dot(reflect(-scene.sun.xyz,wave),-view),0.0),96.0);
    var color = tint*(1.0-reflected/max(alpha,0.0001)) + reflection*reflected/max(alpha,0.0001);
    color += scene.light_color.rgb*scene.sun.w*specular*visibility*0.45;
    let fog = terrain_fog(distance);
    color = mix(color,fog_sky(view,field.y),fog);
    return vec4(tonemap(max(color,vec3(0.0))),alpha);
}

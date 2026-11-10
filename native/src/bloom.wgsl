@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@vertex fn vs_fullscreen(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32> {
    return vec4(vec2(f32((index<<1u)&2u),f32(index&2u))*2.0-1.0,0.0,1.0);
}
@fragment fn fs_down(@builtin(position) screen:vec4<f32>)->@location(0) vec4<f32> {
    let size=vec2<f32>(textureDimensions(image));
    let uv=screen.xy*4.0/size;
    var color=vec3(0.0);
    for(var y=0;y<2;y++){for(var x=0;x<2;x++){
        let value=textureSampleLevel(image,linear_sampler,uv+(vec2(f32(x),f32(y))*2.0-1.0)/size,0.0).rgb;
        let brightness=max(value.r,max(value.g,value.b));
        color+=value*max(brightness-1.1,0.0)/max(brightness,0.001)*0.25;
    }}
    return vec4(color,1.0);
}
fn blur(uv:vec2<f32>,axis:vec2<f32>)->vec4<f32> {
    var color=textureSampleLevel(image,linear_sampler,uv,0.0).rgb*0.227027;
    color+=(textureSampleLevel(image,linear_sampler,uv+axis*1.384615,0.0).rgb+textureSampleLevel(image,linear_sampler,uv-axis*1.384615,0.0).rgb)*0.316216;
    color+=(textureSampleLevel(image,linear_sampler,uv+axis*3.230769,0.0).rgb+textureSampleLevel(image,linear_sampler,uv-axis*3.230769,0.0).rgb)*0.070270;
    return vec4(color,1.0);
}
@fragment fn fs_horizontal(@builtin(position) screen:vec4<f32>)->@location(0) vec4<f32> {
    let size=vec2<f32>(textureDimensions(image));return blur(screen.xy/size,vec2(1.0/size.x,0.0));
}
@fragment fn fs_vertical(@builtin(position) screen:vec4<f32>)->@location(0) vec4<f32> {
    let size=vec2<f32>(textureDimensions(image));return blur(screen.xy/size,vec2(0.0,1.0/size.y));
}

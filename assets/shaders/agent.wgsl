#import bevy_sprite::mesh2d_functions as mesh_functions
#import bevy_sprite::mesh2d_view_bindings::globals

struct AgentParams {
    atlas_grid: vec2<f32>,
    pulse_base: f32,
    pulse_amplitude: f32,
    pulse_period: f32,
}

struct AgentInstance {
    sprite_ids: array<u32, 6>,
    layer_count: u32,
    outfit_colors: u32, // packed: head | body<<8 | legs<<16 | feet<<24
    bbox_min: vec2<f32>,
    bbox_size: vec2<f32>,
    shift: vec2<f32>,
    highlighted: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var atlas_tex: texture_2d<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var atlas_smp: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> params: AgentParams;

@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<storage, read> instances: array<AgentInstance>;

const COLOR_TABLE: array<vec3<f32>, 133> = array<vec3<f32>, 133>(
    vec3<f32>(1.0000, 1.0000, 1.0000),
    vec3<f32>(1.0000, 0.8314, 0.7490),
    vec3<f32>(1.0000, 0.9137, 0.7490),
    vec3<f32>(1.0000, 1.0000, 0.7490),
    vec3<f32>(0.9137, 1.0000, 0.7490),
    vec3<f32>(0.8314, 1.0000, 0.7490),
    vec3<f32>(0.7490, 1.0000, 0.7490),
    vec3<f32>(0.7490, 1.0000, 0.8314),
    vec3<f32>(0.7490, 1.0000, 0.9137),
    vec3<f32>(0.7490, 1.0000, 1.0000),
    vec3<f32>(0.7490, 0.9137, 1.0000),
    vec3<f32>(0.7490, 0.8314, 1.0000),
    vec3<f32>(0.7490, 0.7490, 1.0000),
    vec3<f32>(0.8314, 0.7490, 1.0000),
    vec3<f32>(0.9137, 0.7490, 1.0000),
    vec3<f32>(1.0000, 0.7490, 1.0000),
    vec3<f32>(1.0000, 0.7490, 0.9137),
    vec3<f32>(1.0000, 0.7490, 0.8314),
    vec3<f32>(1.0000, 0.7490, 0.7490),
    vec3<f32>(0.8549, 0.8549, 0.8549),
    vec3<f32>(0.7490, 0.6235, 0.5608),
    vec3<f32>(0.7490, 0.6863, 0.5608),
    vec3<f32>(0.7490, 0.7490, 0.5608),
    vec3<f32>(0.6863, 0.7490, 0.5608),
    vec3<f32>(0.6235, 0.7490, 0.5608),
    vec3<f32>(0.5608, 0.7490, 0.5608),
    vec3<f32>(0.5608, 0.7490, 0.6235),
    vec3<f32>(0.5608, 0.7490, 0.6863),
    vec3<f32>(0.5608, 0.7490, 0.7490),
    vec3<f32>(0.5608, 0.6863, 0.7490),
    vec3<f32>(0.5608, 0.6235, 0.7490),
    vec3<f32>(0.5608, 0.5608, 0.7490),
    vec3<f32>(0.6235, 0.5608, 0.7490),
    vec3<f32>(0.6863, 0.5608, 0.7490),
    vec3<f32>(0.7490, 0.5608, 0.7490),
    vec3<f32>(0.7490, 0.5608, 0.6863),
    vec3<f32>(0.7490, 0.5608, 0.6235),
    vec3<f32>(0.7490, 0.5608, 0.5608),
    vec3<f32>(0.7137, 0.7137, 0.7098),
    vec3<f32>(0.7490, 0.4980, 0.3725),
    vec3<f32>(0.7490, 0.6235, 0.3725),
    vec3<f32>(0.7490, 0.7490, 0.3725),
    vec3<f32>(0.6235, 0.7490, 0.3725),
    vec3<f32>(0.4980, 0.7490, 0.3725),
    vec3<f32>(0.3725, 0.7490, 0.3725),
    vec3<f32>(0.3725, 0.7490, 0.4980),
    vec3<f32>(0.3725, 0.7490, 0.6235),
    vec3<f32>(0.3725, 0.7490, 0.7490),
    vec3<f32>(0.3725, 0.6235, 0.7490),
    vec3<f32>(0.3725, 0.4980, 0.7490),
    vec3<f32>(0.3725, 0.3725, 0.7490),
    vec3<f32>(0.4980, 0.3725, 0.7490),
    vec3<f32>(0.6235, 0.3725, 0.7490),
    vec3<f32>(0.7490, 0.3725, 0.7490),
    vec3<f32>(0.7490, 0.3725, 0.6235),
    vec3<f32>(0.7490, 0.3725, 0.4980),
    vec3<f32>(0.7490, 0.3725, 0.3725),
    vec3<f32>(0.5686, 0.5686, 0.5647),
    vec3<f32>(0.7490, 0.4157, 0.2471),
    vec3<f32>(0.7490, 0.5804, 0.2471),
    vec3<f32>(0.7490, 0.7490, 0.2471),
    vec3<f32>(0.5804, 0.7490, 0.2471),
    vec3<f32>(0.4157, 0.7490, 0.2471),
    vec3<f32>(0.2471, 0.7490, 0.2471),
    vec3<f32>(0.2471, 0.7490, 0.4157),
    vec3<f32>(0.2471, 0.7490, 0.5804),
    vec3<f32>(0.2471, 0.7490, 0.7490),
    vec3<f32>(0.2471, 0.5804, 0.7490),
    vec3<f32>(0.2471, 0.4157, 0.7490),
    vec3<f32>(0.2471, 0.2471, 0.7490),
    vec3<f32>(0.4157, 0.2471, 0.7490),
    vec3<f32>(0.5804, 0.2471, 0.7490),
    vec3<f32>(0.7490, 0.2471, 0.7490),
    vec3<f32>(0.7490, 0.2471, 0.5804),
    vec3<f32>(0.7490, 0.2471, 0.4157),
    vec3<f32>(0.7490, 0.2471, 0.2471),
    vec3<f32>(0.4275, 0.4275, 0.4275),
    vec3<f32>(1.0000, 0.3333, 0.0000),
    vec3<f32>(1.0000, 0.6667, 0.0000),
    vec3<f32>(1.0000, 1.0000, 0.0000),
    vec3<f32>(0.6667, 1.0000, 0.0000),
    vec3<f32>(0.3294, 1.0000, 0.0000),
    vec3<f32>(0.0000, 1.0000, 0.0000),
    vec3<f32>(0.0000, 1.0000, 0.3294),
    vec3<f32>(0.0000, 1.0000, 0.6667),
    vec3<f32>(0.0000, 1.0000, 1.0000),
    vec3<f32>(0.0000, 0.6627, 1.0000),
    vec3<f32>(0.0000, 0.3333, 1.0000),
    vec3<f32>(0.0000, 0.0000, 1.0000),
    vec3<f32>(0.3333, 0.0000, 1.0000),
    vec3<f32>(0.6627, 0.0000, 1.0000),
    vec3<f32>(0.9961, 0.0000, 1.0000),
    vec3<f32>(1.0000, 0.0000, 0.6667),
    vec3<f32>(1.0000, 0.0000, 0.3333),
    vec3<f32>(1.0000, 0.0000, 0.0000),
    vec3<f32>(0.2824, 0.2824, 0.2667),
    vec3<f32>(0.7490, 0.2471, 0.0000),
    vec3<f32>(0.7490, 0.4980, 0.0000),
    vec3<f32>(0.7490, 0.7490, 0.0000),
    vec3<f32>(0.4980, 0.7490, 0.0000),
    vec3<f32>(0.2471, 0.7490, 0.0000),
    vec3<f32>(0.0000, 0.7490, 0.0000),
    vec3<f32>(0.0000, 0.7490, 0.2471),
    vec3<f32>(0.0000, 0.7490, 0.4980),
    vec3<f32>(0.0000, 0.7490, 0.7490),
    vec3<f32>(0.0000, 0.4980, 0.7490),
    vec3<f32>(0.0000, 0.2471, 0.7490),
    vec3<f32>(0.0000, 0.0000, 0.7490),
    vec3<f32>(0.2471, 0.0000, 0.7490),
    vec3<f32>(0.4980, 0.0000, 0.7490),
    vec3<f32>(0.7490, 0.0000, 0.7490),
    vec3<f32>(0.7490, 0.0000, 0.4980),
    vec3<f32>(0.7490, 0.0000, 0.2471),
    vec3<f32>(0.7490, 0.0000, 0.0000),
    vec3<f32>(0.1412, 0.1412, 0.1412),
    vec3<f32>(0.4980, 0.1647, 0.0000),
    vec3<f32>(0.4980, 0.3333, 0.0000),
    vec3<f32>(0.4980, 0.4980, 0.0000),
    vec3<f32>(0.3333, 0.4980, 0.0000),
    vec3<f32>(0.1647, 0.4980, 0.0000),
    vec3<f32>(0.0000, 0.4980, 0.0000),
    vec3<f32>(0.0000, 0.4980, 0.1647),
    vec3<f32>(0.0000, 0.4980, 0.3333),
    vec3<f32>(0.0000, 0.4980, 0.4980),
    vec3<f32>(0.0000, 0.3294, 0.4980),
    vec3<f32>(0.0000, 0.1647, 0.4980),
    vec3<f32>(0.0000, 0.0000, 0.4980),
    vec3<f32>(0.1647, 0.0000, 0.4980),
    vec3<f32>(0.3294, 0.0000, 0.4980),
    vec3<f32>(0.4980, 0.0000, 0.4980),
    vec3<f32>(0.4980, 0.0000, 0.3333),
    vec3<f32>(0.4980, 0.0000, 0.1647),
    vec3<f32>(0.4980, 0.0000, 0.0000),
);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) instance_index: u32,
}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>
}

fn calculate_world_pos(
    position: vec3<f32>,
    mesh_size: f32,
    bbox_min: vec2<f32>,
    bbox_size: vec2<f32>,
    shift: vec2<f32>,
    instance_index: u32
) -> vec4<f32> {
    let local01 = position.xy / mesh_size;
    var scaled = local01 * bbox_size;
    let bbox_center = (bbox_min * 2.0 + bbox_size - mesh_size) * vec2<f32>(0.5, -0.5);
    let displacement = shift * vec2<f32>(1.0, -1.0);
    let final_local = scaled + bbox_center - displacement;
    var world_from_local = mesh_functions::get_world_from_local(instance_index);
    return mesh_functions::mesh2d_position_local_to_world(
        world_from_local,
        vec4<f32>(final_local, position.z, 1.0)
    );
}

fn adjust_uv_to_bbox(
    uv: vec2<f32>,
    mesh_size: f32,
    bbox_min: vec2<f32>,
    bbox_size: vec2<f32>
) -> vec2<f32> {
    let bbox_min_n = bbox_min / mesh_size;
    let bbox_size_n = bbox_size / mesh_size;
    return bbox_min_n + uv * bbox_size_n;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let inst_index = mesh_functions::get_tag(vertex.instance_index);
    let inst = instances[inst_index];
    out.world_position = calculate_world_pos(
        vertex.position, 64.0, inst.bbox_min, inst.bbox_size, inst.shift, vertex.instance_index
    );
    out.position = mesh_functions::mesh2d_position_world_to_clip(out.world_position);
    out.uv = adjust_uv_to_bbox(vertex.uv, 64.0, inst.bbox_min, inst.bbox_size);
    out.instance_index = inst_index;
    return out;
}

fn atlas_uv(base_uv: vec2<f32>, index: u32) -> vec2<f32> {
    let tile_size = vec2<f32>(1.0 / params.atlas_grid.x, 1.0 / params.atlas_grid.y);
    let col = index % u32(params.atlas_grid.x);
    let row = index / u32(params.atlas_grid.x);
    let offset = vec2<f32>(f32(col) * tile_size.x, f32(row) * tile_size.y);
    return offset + base_uv * tile_size;
}

fn recolor(sample: vec4<f32>, outfit_colors: u32) -> vec4<f32> {
    if (sample.a < 0.01) { return vec4<f32>(0.0); }
    let rgb = sample.rgb;
    let head = COLOR_TABLE[outfit_colors & 0xffu];
    let body = COLOR_TABLE[(outfit_colors >> 8u) & 0xffu];
    let legs = COLOR_TABLE[(outfit_colors >> 16u) & 0xffu];
    let feet = COLOR_TABLE[(outfit_colors >> 24u) & 0xffu];
    if (distance(rgb, vec3<f32>(1.0, 0.0, 0.0)) < 0.01) {
        return vec4<f32>(head, sample.a);
    }
    if (distance(rgb, vec3<f32>(0.0, 1.0, 0.0)) < 0.01) {
        return vec4<f32>(body, sample.a);
    }
    if (distance(rgb, vec3<f32>(0.0, 0.0, 1.0)) < 0.01) {
        return vec4<f32>(legs, sample.a);
    }
    if (distance(rgb, vec3<f32>(1.0, 1.0, 0.0)) < 0.01) {
        return vec4<f32>(feet, sample.a);
    }
    return vec4<f32>(0.0);
}

fn alpha_blend(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let out_rgb = src.rgb * src.a + dst.rgb * (1.0 - src.a);
    let out_a = src.a + dst.a * (1.0 - src.a);
    return vec4<f32>(out_rgb, out_a);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec4<f32>(0.0);
    let instance = instances[in.instance_index];

    for (var i: u32 = 0u; i < instance.layer_count; i = i + 1u) {
        let uv = atlas_uv(in.uv, instance.sprite_ids[i]);
        let sample = textureSample(atlas_tex, atlas_smp, uv);

        if (i % 2u == 0u) {
            color = alpha_blend(color, sample);
        } else {
            let tint = recolor(sample, instance.outfit_colors);
            let factor = mix(vec3<f32>(1.0), tint.rgb, sample.a);
            color = vec4<f32>(color.rgb * factor, color.a);
        }
    }
    if (instance.highlighted != 0u) {
        let wave = 0.5 + 0.5 * sin(6.2831853 * globals.time / params.pulse_period);
        let k = params.pulse_base + params.pulse_amplitude * wave;
        color = vec4<f32>(mix(color.rgb, vec3<f32>(1.0), k), color.a);
    }
    return color;
}

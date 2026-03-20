// Particle compute shader for firework simulation

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    color: vec4<f32>,
    life: f32,
    size: f32,
    _padding: vec2<f32>,
};

struct SimParams {
    delta_time: f32,
    gravity: f32,
    drag: f32,
    _padding: f32,
};

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> params: SimParams;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let idx = id.x;
    if idx >= arrayLength(&particles) {
        return;
    }

    var p = particles[idx];

    if p.life <= 0.0 {
        return;
    }

    // Apply gravity
    p.velocity.y += params.gravity * params.delta_time;

    // Apply drag
    p.velocity *= (1.0 - params.drag * params.delta_time);

    // Update position
    p.position += p.velocity * params.delta_time;

    // Decay life
    p.life -= params.delta_time;

    // Fade out
    p.color.a = clamp(p.life * 2.0, 0.0, 1.0);

    // Shrink
    p.size = max(p.size - params.delta_time * 2.0, 0.5);

    particles[idx] = p;
}

// Render pass for particles
struct ParticleVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) point_coord: vec2<f32>,
};

struct RenderUniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<storage, read> particles_render: array<Particle>;

@group(0) @binding(1)
var<uniform> render_uniforms: RenderUniforms;

// Each particle is a quad made of 6 vertices (2 triangles)
@vertex
fn vs_particle(@builtin(vertex_index) vertex_index: u32) -> ParticleVertexOutput {
    let particle_index = vertex_index / 6u;
    let corner_index = vertex_index % 6u;

    let p = particles_render[particle_index];

    var out: ParticleVertexOutput;

    if p.life <= 0.0 {
        out.clip_position = vec4<f32>(0.0, 0.0, -2.0, 1.0);
        out.color = vec4<f32>(0.0);
        out.point_coord = vec2<f32>(0.0);
        return out;
    }

    // Quad corners: 0=TL, 1=TR, 2=BL, 3=BL, 4=TR, 5=BR
    var offsets = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let offset = offsets[corner_index] * p.size;
    let pixel_pos = p.position + offset;

    let ndc = vec2<f32>(
        (pixel_pos.x / render_uniforms.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / render_uniforms.screen_size.y) * 2.0
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = p.color;
    out.point_coord = offsets[corner_index];
    return out;
}

@fragment
fn fs_particle(in: ParticleVertexOutput) -> @location(0) vec4<f32> {
    // Circle shape
    let dist = length(in.point_coord);
    if dist > 1.0 {
        discard;
    }
    let alpha = (1.0 - dist) * in.color.a;
    return vec4<f32>(in.color.rgb, alpha);
}

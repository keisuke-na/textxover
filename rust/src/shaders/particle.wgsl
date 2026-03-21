// Particle firework shader — compute (physics) + render (visual)

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    color: vec4<f32>,
    life: f32,
    size: f32,
    phase: f32,        // 0=launch, 1=burst, 2=waiting
    initial_life: f32, // original life for color/timing
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

    p.life -= params.delta_time;

    if p.phase == 0.0 {
        // === Launch phase ===
        // Rise upward, decelerating due to gravity
        p.velocity.y += 1200.0 * params.delta_time; // gravity deceleration
        p.position += p.velocity * params.delta_time;
        p.color.a = 1.0;

        if p.life <= 0.0 {
            // Launch particle dies
            p.life = 0.0;
        }
    } else if p.phase == 2.0 {
        // === Waiting for burst ===
        // Count down until burst time (life > initial_life means still waiting)
        if p.life <= p.initial_life {
            // Time to burst!
            p.phase = 1.0;
            p.color.a = 1.0;
        }
        // Stay invisible while waiting
    } else {
        // === Burst phase ===
        // Gravity
        p.velocity.y += params.gravity * params.delta_time;

        // Drag
        p.velocity *= (1.0 - params.drag * params.delta_time);

        // Move
        p.position += p.velocity * params.delta_time;

        // Life ratio for effects
        let ratio = clamp(p.life / p.initial_life, 0.0, 1.0);

        // Color transition: original color → warm gold → dim red as life decreases
        if ratio < 0.3 {
            // Final phase: fade to dim red/orange
            let t = ratio / 0.3;
            p.color.r = mix(0.8, p.color.r, t);
            p.color.g = mix(0.2, p.color.g, t);
            p.color.b = mix(0.05, p.color.b, t);
        }

        // Twinkle/blink near end of life
        if ratio < 0.2 {
            let flicker = sin(p.life * 30.0) * 0.5 + 0.5;
            p.color.a = flicker * ratio * 5.0;
        } else {
            // Normal fade
            p.color.a = clamp(ratio * 1.5, 0.0, 1.0);
        }

        // Shrink
        p.size = max(p.size - params.delta_time * 1.5, 0.5);
    }

    particles[idx] = p;
}

// ====== Render pass ======

struct ParticleVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) point_coord: vec2<f32>,
    @location(2) velocity: vec2<f32>,
    @location(3) phase: f32,
};

struct RenderUniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<storage, read> particles_render: array<Particle>;

@group(0) @binding(1)
var<uniform> render_uniforms: RenderUniforms;

@vertex
fn vs_particle(@builtin(vertex_index) vertex_index: u32) -> ParticleVertexOutput {
    let particle_index = vertex_index / 6u;
    let corner_index = vertex_index % 6u;

    let p = particles_render[particle_index];

    var out: ParticleVertexOutput;

    // Hide dead or waiting particles
    if p.life <= 0.0 || p.phase == 2.0 {
        out.clip_position = vec4<f32>(0.0, 0.0, -2.0, 1.0);
        out.color = vec4<f32>(0.0);
        out.point_coord = vec2<f32>(0.0);
        out.velocity = vec2<f32>(0.0);
        out.phase = p.phase;
        return out;
    }

    // Elongate quad along velocity for trail effect
    let speed = length(p.velocity);
    var trail_stretch = 1.0;
    if p.phase == 1.0 && speed > 10.0 {
        trail_stretch = clamp(speed / 150.0, 1.0, 4.0);
    } else if p.phase == 0.0 && speed > 10.0 {
        // Launch trail is longer and wider
        trail_stretch = clamp(speed / 60.0, 1.0, 10.0);
    }

    // Build a rotated quad aligned to velocity direction
    var dir = vec2<f32>(0.0, -1.0); // default up
    if speed > 1.0 {
        dir = normalize(p.velocity);
    }
    let perp = vec2<f32>(-dir.y, dir.x);

    // Quad corners in local space
    var offsets = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let local = offsets[corner_index];
    // x=perpendicular (width), y=along velocity (length with trail)
    let world_offset = perp * local.x * p.size + dir * local.y * p.size * trail_stretch;
    let pixel_pos = p.position + world_offset;

    let ndc = vec2<f32>(
        (pixel_pos.x / render_uniforms.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / render_uniforms.screen_size.y) * 2.0
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = p.color;
    out.point_coord = local;
    out.velocity = p.velocity;
    out.phase = p.phase;
    return out;
}

@fragment
fn fs_particle(in: ParticleVertexOutput) -> @location(0) vec4<f32> {
    let coord = in.point_coord;

    if in.phase == 0.0 {
        // Launch: simple straight trail, fading toward tail
        let dx = abs(coord.x);
        if dx > 0.6 {
            discard;
        }
        let along = (coord.y + 1.0) * 0.5; // 0=tail, 1=head
        let brightness = 1.0 - dx / 0.6;
        let alpha = brightness * along * in.color.a;
        let core_color = mix(in.color.rgb, vec3<f32>(1.0, 1.0, 1.0), brightness * 0.5);
        return vec4<f32>(core_color, alpha);
    }

    // Burst: circle with trail fade
    let speed = length(in.velocity);
    if speed > 50.0 {
        // Comet shape: bright head, fading tail
        let along = (coord.y + 1.0) * 0.5; // 0=tail, 1=head
        let dx = abs(coord.x);
        let width = mix(1.0, 0.3, 1.0 - along); // wider at head
        if dx > width {
            discard;
        }
        let brightness = along * (1.0 - dx / width);
        let alpha = brightness * in.color.a;
        return vec4<f32>(in.color.rgb, alpha);
    }

    // Slow particle: simple circle with glow
    let dist = length(coord);
    if dist > 1.0 {
        discard;
    }
    let glow = (1.0 - dist * dist) * in.color.a;
    return vec4<f32>(in.color.rgb, glow);
}

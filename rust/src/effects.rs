use crate::types::Particle;
use rand::Rng;

/// CPU-side particle factory. Only responsible for spawning initial particle data.
/// Physics simulation runs on GPU via compute shader.
pub struct EffectManager {
    pending: Vec<Particle>,
    max_particles: usize,
}

impl EffectManager {
    pub fn new(max_particles: usize) -> Self {
        EffectManager {
            pending: Vec::new(),
            max_particles,
        }
    }

    pub fn spawn_firework(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        let mut rng = rand::thread_rng();

        let origin_x = x * screen_width;
        let origin_y = y * screen_height;
        let hue: f32 = rng.gen_range(0.0..360.0);

        // Launch particle: fixed initial velocity, gravity decelerates
        // Physics: y = y0 + vy*t + 0.5*g*t^2 (g=400 in shader)
        let launch_speed = 1200.0;
        let gravity = 400.0;
        let vx = rng.gen_range(-15.0..15.0);

        // Solve for t when particle reaches origin_y:
        // origin_y = screen_height - launch_speed*t + 0.5*400*t^2
        // 200*t^2 - launch_speed*t + (screen_height - origin_y) = 0 (not needed, just use quadratic)
        let distance = screen_height - origin_y;
        // t = (v - sqrt(v^2 - 2*g*d)) / g
        let discriminant = launch_speed * launch_speed - 2.0 * gravity * distance;
        let launch_life = if discriminant > 0.0 {
            (launch_speed - discriminant.sqrt()) / gravity
        } else {
            distance / launch_speed // fallback
        };

        // Calculate exact final position using same physics
        let final_x = origin_x + vx * launch_life;
        let final_y = screen_height + (-launch_speed) * launch_life + 0.5 * gravity * launch_life * launch_life;

        self.pending.push(Particle {
            position: [origin_x, screen_height],
            velocity: [vx, -launch_speed],
            color: [1.0, 0.9, 0.6, 1.0], // warm white
            life: launch_life,
            size: 12.0,
            phase: 0.0, // launch phase
            initial_life: launch_life,
        });

        // Burst particles: spawn at calculated final position
        let burst_count = rng.gen_range(200..=400);
        let second_hue = hue + rng.gen_range(90.0..180.0);

        for i in 0..burst_count {
            if self.pending.len() >= self.max_particles {
                break;
            }

            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed: f32 = rng.gen_range(150.0..350.0);

            let is_inner = (i as f32) < (burst_count as f32 * 0.2);
            let (r, g, b) = if is_inner {
                let h = second_hue + rng.gen_range(-20.0..20.0);
                hsv_to_rgb(h.rem_euclid(360.0), 0.7, 1.0)
            } else {
                let h = hue + rng.gen_range(-30.0..30.0);
                hsv_to_rgb(h.rem_euclid(360.0), 0.8, 1.0)
            };

            let inner_speed = if is_inner { speed * 0.5 } else { speed };
            let burst_life = rng.gen_range(1.5..3.0);

            self.pending.push(Particle {
                position: [final_x, final_y],
                velocity: [angle.cos() * inner_speed, angle.sin() * inner_speed],
                color: [r, g, b, 0.0], // alpha=0, invisible until burst
                life: burst_life + launch_life, // includes launch delay
                size: rng.gen_range(4.0..10.0),
                phase: 2.0, // waiting for burst (delay = launch_life)
                initial_life: burst_life,
            });
        }
    }

    /// Drain newly spawned particles for upload to GPU buffer.
    pub fn drain_pending(&mut self) -> Vec<Particle> {
        std::mem::take(&mut self.pending)
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (r + m, g + m, b + m)
}

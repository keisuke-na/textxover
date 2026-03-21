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
        let count = rng.gen_range(200..=500);

        let hue: f32 = rng.gen_range(0.0..360.0);

        for _ in 0..count {
            if self.pending.len() >= self.max_particles {
                break;
            }

            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed: f32 = rng.gen_range(50.0..400.0);

            let h = hue + rng.gen_range(-30.0..30.0);
            let (r, g, b) = hsv_to_rgb(h.rem_euclid(360.0), 0.8, 1.0);

            self.pending.push(Particle {
                position: [x * screen_width, y * screen_height],
                velocity: [angle.cos() * speed, angle.sin() * speed],
                color: [r, g, b, 1.0],
                life: rng.gen_range(1.5..3.0),
                size: rng.gen_range(4.0..10.0),
                _padding: [0.0; 2],
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

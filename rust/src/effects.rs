use crate::types::Particle;
use rand::Rng;

pub struct EffectManager {
    particles: Vec<Particle>,
    max_particles: usize,
}

impl EffectManager {
    pub fn new(max_particles: usize) -> Self {
        EffectManager {
            particles: Vec::new(),
            max_particles,
        }
    }

    pub fn spawn_firework(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        let mut rng = rand::thread_rng();
        let count = rng.gen_range(200..=500);

        // Choose a random base color
        let hue: f32 = rng.gen_range(0.0..360.0);

        for _ in 0..count {
            if self.particles.len() >= self.max_particles {
                break;
            }

            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed: f32 = rng.gen_range(50.0..400.0);

            // Vary hue slightly per particle
            let h = hue + rng.gen_range(-30.0..30.0);
            let (r, g, b) = hsv_to_rgb(h.rem_euclid(360.0), 0.8, 1.0);

            let particle = Particle {
                position: [x * screen_width, y * screen_height],
                velocity: [angle.cos() * speed, angle.sin() * speed],
                color: [r, g, b, 1.0],
                life: rng.gen_range(1.5..3.0),
                size: rng.gen_range(2.0..6.0),
                _padding: [0.0; 2],
            };

            self.particles.push(particle);
        }
    }

    pub fn update(&mut self, dt: f32) {
        let gravity = 150.0;
        let drag = 0.5;

        for p in &mut self.particles {
            if p.life <= 0.0 {
                continue;
            }

            p.velocity[1] += gravity * dt;
            p.velocity[0] *= 1.0 - drag * dt;
            p.velocity[1] *= 1.0 - drag * dt;
            p.position[0] += p.velocity[0] * dt;
            p.position[1] += p.velocity[1] * dt;
            p.life -= dt;
            p.color[3] = (p.life * 2.0).clamp(0.0, 1.0);
            p.size = (p.size - dt * 2.0).max(0.5);
        }

        self.particles.retain(|p| p.life > 0.0);
    }

    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    pub fn active_count(&self) -> u32 {
        self.particles.len() as u32
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

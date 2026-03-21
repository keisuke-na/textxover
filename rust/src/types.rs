use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommentType {
    Scroll = 0,
    Top = 1,
    Bottom = 2,
}

impl From<u8> for CommentType {
    fn from(v: u8) -> Self {
        match v {
            1 => CommentType::Top,
            2 => CommentType::Bottom,
            _ => CommentType::Scroll,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: u32,
    pub comment_type: CommentType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub life: f32, // seconds remaining (for top/bottom)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentRequest {
    pub text: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_size")]
    pub size: String,
    #[serde(default = "default_type")]
    pub r#type: String,
}

fn default_color() -> String { "#FFFFFF".to_string() }
fn default_size() -> String { "medium".to_string() }
fn default_type() -> String { "scroll".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectRequest {
    pub r#type: String,
    #[serde(default = "default_pos")]
    pub x: f32,
    #[serde(default = "default_pos")]
    pub y: f32,
}

fn default_pos() -> f32 { 0.5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRequest {
    pub speed: Option<f32>,
    pub font_size_medium: Option<u32>,
    pub font_size_big: Option<u32>,
    pub font_size_small: Option<u32>,
    pub opacity: Option<f32>,
    pub display_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub speed: f32,
    pub font_size_medium: u32,
    pub font_size_big: u32,
    pub font_size_small: u32,
    pub opacity: f32,
    pub display_index: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            speed: 1.0,
            font_size_medium: 36,
            font_size_big: 48,
            font_size_small: 24,
            opacity: 1.0,
            display_index: 0,
        }
    }
}

impl Config {
    pub fn apply(&mut self, req: &ConfigRequest) {
        if let Some(v) = req.speed { self.speed = v; }
        if let Some(v) = req.font_size_medium { self.font_size_medium = v; }
        if let Some(v) = req.font_size_big { self.font_size_big = v; }
        if let Some(v) = req.font_size_small { self.font_size_small = v; }
        if let Some(v) = req.opacity { self.opacity = v; }
        if let Some(v) = req.display_index { self.display_index = v; }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub active_comments: u32,
    pub active_particles: u32,
    pub fps: u32,
    pub config: Config,
}

// Poll types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollStartRequest {
    pub question: String,
    pub choices: Vec<PollChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollChoice {
    pub key: String,   // e.g. "A", "B", "1", "2"
    pub label: String, // e.g. "Agree", "Disagree"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollState {
    pub active: bool,
    pub question: String,
    pub choices: Vec<PollChoiceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollChoiceResult {
    pub key: String,
    pub label: String,
    pub count: u32,
}

/// Vertex for textured quad rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub tex_coord: [f32; 2],
}

/// Particle for firework effect
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub color: [f32; 4],
    pub life: f32,
    pub size: f32,
    pub phase: f32,        // 0=launch, 1=burst
    pub initial_life: f32, // original life for color transition ratio
}

use crate::types::{Comment, CommentType};
use rand::Rng;

/// Manages active comments and lane assignment
pub struct CommentManager {
    comments: Vec<Comment>,
    screen_width: f32,
    screen_height: f32,
    speed: f32, // pixels per second base speed
    next_id: u32,
}

impl CommentManager {
    pub fn new(width: f32, height: f32) -> Self {
        CommentManager {
            comments: Vec::new(),
            screen_width: width,
            screen_height: height,
            speed: 200.0,
            next_id: 1,
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    pub fn set_speed(&mut self, multiplier: f32) {
        self.speed = 200.0 * multiplier;
    }

    pub fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_comment(&mut self, id: u32, comment_type: CommentType, width: f32, height: f32, y_position: f32) {
        let (x, y, life) = match comment_type {
            CommentType::Scroll => {
                (self.screen_width, y_position, -1.0)
            }
            CommentType::Top => {
                ((self.screen_width - width) / 2.0, y_position, 3.0)
            }
            CommentType::Bottom => {
                ((self.screen_width - width) / 2.0, y_position, 3.0)
            }
        };

        self.comments.push(Comment {
            id,
            comment_type,
            x,
            y,
            width,
            height,
            life,
        });
    }

    /// Assign a Y position for a new scroll comment to avoid overlap.
    /// Picks randomly from free lanes within the top 70% of the screen.
    pub fn assign_lane(&self, height: f32) -> f32 {
        let lane_height = height + 4.0; // 4px gap
        let usable_height = self.screen_height * 0.7;
        let max_lanes = (usable_height / lane_height) as usize;

        let mut free_lanes = Vec::new();

        for lane in 0..max_lanes.max(1) {
            let y = lane as f32 * lane_height;
            let occupied = self.comments.iter().any(|c| {
                c.comment_type == CommentType::Scroll
                    && c.y >= y
                    && c.y < y + lane_height
                    && (c.x + c.width) > self.screen_width - 50.0
            });
            if !occupied {
                free_lanes.push(y);
            }
        }

        let mut rng = rand::thread_rng();

        if !free_lanes.is_empty() {
            free_lanes[rng.gen_range(0..free_lanes.len())]
        } else {
            // All lanes busy — pick a random lane
            let lane = rng.gen_range(0..max_lanes.max(1));
            lane as f32 * lane_height
        }
    }

    /// Assign Y position for top-fixed comments
    pub fn assign_top_lane(&self, height: f32) -> f32 {
        let lane_height = height + 4.0;
        let max_lanes = (self.screen_height / 3.0 / lane_height) as usize;

        for lane in 0..max_lanes.max(1) {
            let y = lane as f32 * lane_height;
            let occupied = self.comments.iter().any(|c| {
                c.comment_type == CommentType::Top && c.y >= y && c.y < y + lane_height
            });
            if !occupied {
                return y;
            }
        }
        0.0
    }

    /// Assign Y position for bottom-fixed comments
    pub fn assign_bottom_lane(&self, height: f32) -> f32 {
        let lane_height = height + 4.0;
        let max_lanes = (self.screen_height / 3.0 / lane_height) as usize;

        for lane in 0..max_lanes.max(1) {
            let y = self.screen_height - (lane as f32 + 1.0) * lane_height;
            let occupied = self.comments.iter().any(|c| {
                c.comment_type == CommentType::Bottom && c.y >= y && c.y < y + lane_height
            });
            if !occupied {
                return y;
            }
        }
        self.screen_height - lane_height
    }

    /// Update all comments by delta time. Returns IDs of comments that have expired.
    pub fn update(&mut self, dt: f32) -> Vec<u32> {
        let speed = self.speed;

        let mut expired = Vec::new();

        self.comments.retain(|c| {
            match c.comment_type {
                CommentType::Scroll => {
                    if c.x + c.width < 0.0 {
                        expired.push(c.id);
                        return false;
                    }
                }
                CommentType::Top | CommentType::Bottom => {
                    if c.life <= 0.0 {
                        expired.push(c.id);
                        return false;
                    }
                }
            }
            true
        });

        for c in &mut self.comments {
            match c.comment_type {
                CommentType::Scroll => {
                    c.x -= speed * dt;
                }
                CommentType::Top | CommentType::Bottom => {
                    c.life -= dt;
                }
            }
        }

        expired
    }

    /// Remove a comment by ID
    pub fn remove_comment(&mut self, id: u32) {
        self.comments.retain(|c| c.id != id);
    }

    pub fn active_comments(&self) -> &[Comment] {
        &self.comments
    }

    pub fn active_count(&self) -> u32 {
        self.comments.len() as u32
    }
}

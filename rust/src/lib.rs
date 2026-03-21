mod comments;
mod effects;
mod renderer;
mod server;
mod types;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::RwLock;
use renderer::Renderer;
use server::ServerMessage;
use std::collections::VecDeque;
use std::ffi::CString;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use types::{CommentType, Config, PollChoiceResult, PollState};

/// Pending comment stored for Swift to poll
struct PendingComment {
    id: u32,
    comment_type: u8,
    color: u32,
    size: u8,
    text: CString,
}

/// C-compatible struct for returning pending comment data
#[repr(C)]
pub struct TxoPendingComment {
    pub comment_id: u32,
    pub comment_type: u8,
    pub color: u32,
    pub size: u8,
    pub text: *const std::ffi::c_char,
    pub text_len: u32,
}

struct AppState {
    renderer: Renderer,
    rx: Receiver<ServerMessage>,
    tx: Sender<ServerMessage>,
    active_comments: Arc<AtomicU32>,
    active_particles: Arc<AtomicU32>,
    last_frame_time: std::time::Instant,
    pending_comments: VecDeque<PendingComment>,
    next_comment_id: u32,
    last_polled_text: Option<CString>,
    poll: Arc<RwLock<PollState>>,
    // Poll overlay
    poll_overlay_id: Option<u32>,
    poll_dirty: bool,
}

fn parse_hex_color(s: &str) -> u32 {
    let s = s.trim_start_matches('#');
    u32::from_str_radix(s, 16).unwrap_or(0xFFFFFF)
}

fn parse_size(s: &str) -> u8 {
    match s {
        "big" => 1,
        "small" => 2,
        _ => 0, // medium
    }
}

fn parse_comment_type(s: &str) -> u8 {
    match s {
        "top" => 1,
        "bottom" => 2,
        _ => 0, // scroll
    }
}

#[no_mangle]
pub extern "C" fn txo_init(
    metal_layer_ptr: *mut std::ffi::c_void,
    width: u32,
    height: u32,
) -> *mut std::ffi::c_void {
    env_logger::try_init().ok();
    log::info!("txo_init: {}x{}", width, height);

    let config = Arc::new(RwLock::new(Config::default()));
    let renderer = Renderer::new(metal_layer_ptr, width, height, config.clone());

    let (tx, rx) = crossbeam_channel::unbounded();
    let active_comments = Arc::new(AtomicU32::new(0));
    let active_particles = Arc::new(AtomicU32::new(0));
    let poll = Arc::new(RwLock::new(PollState {
        active: false,
        question: String::new(),
        choices: Vec::new(),
    }));

    let state = Box::new(AppState {
        renderer,
        rx,
        tx,
        active_comments,
        active_particles,
        last_frame_time: std::time::Instant::now(),
        pending_comments: VecDeque::new(),
        next_comment_id: 1,
        last_polled_text: None,
        poll,
        poll_overlay_id: None,
        poll_dirty: false,
    });

    Box::into_raw(state) as *mut std::ffi::c_void
}

#[no_mangle]
pub extern "C" fn txo_destroy(handle: *mut std::ffi::c_void) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut AppState);
        }
        log::info!("txo_destroy");
    }
}

#[no_mangle]
pub extern "C" fn txo_resize(handle: *mut std::ffi::c_void, width: u32, height: u32) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    state.renderer.resize(width, height);
    log::info!("txo_resize: {}x{}", width, height);
}

/// Poll for a pending comment. Returns 1 if available, 0 if none.
/// The text pointer is valid until the next call to txo_poll_comment.
#[no_mangle]
pub extern "C" fn txo_poll_comment(
    handle: *mut std::ffi::c_void,
    out: *mut TxoPendingComment,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return 0;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };

    // First, drain HTTP messages into pending queue
    while let Ok(msg) = state.rx.try_recv() {
        match msg {
            ServerMessage::Comment(req) => {
                let id = state.next_comment_id;
                state.next_comment_id += 1;

                let pending = PendingComment {
                    id,
                    comment_type: parse_comment_type(&req.r#type),
                    color: parse_hex_color(&req.color),
                    size: parse_size(&req.size),
                    text: CString::new(req.text).unwrap_or_default(),
                };
                state.pending_comments.push_back(pending);
            }
            ServerMessage::Effect(req) => {
                let (w, h) = state.renderer.dimensions();
                state
                    .renderer
                    .effect_manager
                    .spawn_firework(req.x, req.y, w as f32, h as f32);
            }
            ServerMessage::Config(_req) => {
                let config = state.renderer.config.read();
                state.renderer.comment_manager.set_speed(config.speed);
            }
            ServerMessage::PollStart(_) | ServerMessage::PollStop => {
                state.poll_dirty = true;
            }
        }
    }

    // Mark poll dirty if votes changed
    {
        let poll = state.poll.read();
        if poll.active {
            state.poll_dirty = true;
        }
    }

    if let Some(pending) = state.pending_comments.pop_front() {
        let text_len = pending.text.as_bytes().len() as u32;
        let text_ptr = pending.text.as_ptr();

        unsafe {
            (*out).comment_id = pending.id;
            (*out).comment_type = pending.comment_type;
            (*out).color = pending.color;
            (*out).size = pending.size;
            (*out).text = text_ptr;
            (*out).text_len = text_len;
        }

        // Keep CString alive until next poll
        state.last_polled_text = Some(pending.text);
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn txo_submit_texture(
    handle: *mut std::ffi::c_void,
    comment_id: u32,
    width: u32,
    height: u32,
    rgba_data: *const u8,
    data_len: u32,
) {
    if handle.is_null() || rgba_data.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let data = unsafe { std::slice::from_raw_parts(rgba_data, data_len as usize) };
    state.renderer.submit_texture(comment_id, width, height, data);
}

#[no_mangle]
pub extern "C" fn txo_start_comment(
    handle: *mut std::ffi::c_void,
    comment_id: u32,
    comment_type: u8,
    y_position: f32,
) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let ct = CommentType::from(comment_type);

    // Get texture dimensions
    let (w, h) = if let Some(tex) = state.renderer.comment_textures_ref().get(&comment_id) {
        (tex.width as f32, tex.height as f32)
    } else {
        return;
    };

    // Auto-assign lane if y_position < 0
    let y = if y_position < 0.0 {
        match ct {
            CommentType::Scroll => state.renderer.comment_manager.assign_lane(h),
            CommentType::Top => state.renderer.comment_manager.assign_top_lane(h),
            CommentType::Bottom => state.renderer.comment_manager.assign_bottom_lane(h),
        }
    } else {
        y_position
    };

    state
        .renderer
        .comment_manager
        .add_comment(comment_id, ct, w, h, y);
}

#[no_mangle]
pub extern "C" fn txo_trigger_effect(handle: *mut std::ffi::c_void, _effect_type: u8) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let (w, h) = state.renderer.dimensions();
    state
        .renderer
        .effect_manager
        .spawn_firework(0.5, 0.5, w as f32, h as f32);
}

#[no_mangle]
pub extern "C" fn txo_update_config(
    handle: *mut std::ffi::c_void,
    speed: f32,
    opacity: f32,
) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let mut config = state.renderer.config.write();
    config.speed = speed;
    config.opacity = opacity;
    state.renderer.comment_manager.set_speed(speed);
}

#[no_mangle]
pub extern "C" fn txo_start_server(handle: *mut std::ffi::c_void, port: u16) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let config = state.renderer.config.clone();
    let active_comments = state.active_comments.clone();
    let active_particles = state.active_particles.clone();
    let tx = state.tx.clone();
    let poll = state.poll.clone();
    server::start_server(port, tx, config, active_comments, active_particles, poll);
    log::info!("Server started on port {}", port);
}

#[no_mangle]
pub extern "C" fn txo_render_frame(handle: *mut std::ffi::c_void) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };

    // Calculate delta time
    let now = std::time::Instant::now();
    let dt = now.duration_since(state.last_frame_time).as_secs_f32();
    state.last_frame_time = now;

    // Update counters
    state
        .active_comments
        .store(state.renderer.comment_manager.active_count(), std::sync::atomic::Ordering::Relaxed);
    state
        .active_particles
        .store(state.renderer.particle_count(), std::sync::atomic::Ordering::Relaxed);

    // Render
    state.renderer.render(dt);
}

#[no_mangle]
pub extern "C" fn txo_remove_comment(handle: *mut std::ffi::c_void, comment_id: u32) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    state.renderer.comment_manager.remove_comment(comment_id);
    state.renderer.remove_texture(comment_id);
}

/// Update texture and reset life for an existing comment (no flicker)
#[no_mangle]
pub extern "C" fn txo_update_texture(
    handle: *mut std::ffi::c_void,
    comment_id: u32,
    width: u32,
    height: u32,
    rgba_data: *const u8,
    data_len: u32,
) {
    if handle.is_null() || rgba_data.is_null() {
        return;
    }
    let state = unsafe { &mut *(handle as *mut AppState) };
    let data = unsafe { std::slice::from_raw_parts(rgba_data, data_len as usize) };
    state.renderer.submit_texture(comment_id, width, height, data);
    state.renderer.comment_manager.reset_life(comment_id, 999.0);
}

#[no_mangle]
pub extern "C" fn txo_get_poll_json(handle: *mut std::ffi::c_void) -> *mut std::ffi::c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let state = unsafe { &*(handle as *mut AppState) };
    let poll = state.poll.read();
    let json = serde_json::to_string(&*poll).unwrap_or_default();
    let cstr = CString::new(json).unwrap_or_default();
    cstr.into_raw()
}

#[no_mangle]
pub extern "C" fn txo_free_string(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

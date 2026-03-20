# Architecture

This document describes the technical architecture of textxover for developers and AI assistants working on the codebase.

## Overview

textxover is a hybrid Swift + Rust macOS app. Swift handles windowing and text rasterization. Rust handles GPU rendering and the HTTP server. They communicate via C FFI.

```
Swift (AppKit)              Rust (dylib)
─────────────               ────────────
OverlayWindow ──────────┐
  CAMetalLayer ──────────┼── wgpu Surface (Metal backend)
                         │
TextRasterizer ──────────┼── Texture storage + quad rendering
  CoreText → RGBA bytes  │
                         │
AppDelegate ─────────────┼── FFI calls (txo_*)
  CVDisplayLink callback │
  Menu bar UI            │
                         ├── Comment manager (lanes, animation)
                         ├── Effect manager (particles)
                         └── axum HTTP server (localhost:8080)
```

## Project Structure

```
textxover/
├── rust/src/
│   ├── lib.rs          # FFI entry points (#[no_mangle] extern "C")
│   ├── renderer.rs     # wgpu setup, render loop, texture management
│   ├── comments.rs     # Comment lifecycle, lane assignment
│   ├── effects.rs      # Particle system (fireworks)
│   ├── server.rs       # axum HTTP server, WebUI serving
│   ├── types.rs        # Shared types (Comment, Config, Particle, etc.)
│   └── shaders/
│       ├── comment.wgsl   # Textured quad shader
│       └── particle.wgsl  # Particle compute + render shader
├── swift/
│   ├── main.swift            # NSApplication entry point
│   ├── AppDelegate.swift     # Menu bar, display link, tunnel management
│   ├── OverlayWindow.swift   # Transparent borderless window config
│   ├── TextRasterizer.swift  # CoreText → RGBA bitmap
│   └── DisplaySelector.swift # Multi-display support
├── web/
│   └── index.html      # WebUI (served via include_str at /ui)
├── include/
│   └── textxover.h     # C FFI header
├── resources/
│   └── Info.plist       # LSUIElement=true (no Dock icon)
└── justfile             # Build commands
```

## Data Flow

### Comment flow (HTTP → screen)

```
1. HTTP POST /comment → axum receives JSON
2. axum sends CommentRequest via crossbeam channel
3. Swift polls via txo_poll_comment() on each display link frame
4. Swift rasterizes text with CoreText → RGBA bitmap
5. Swift calls txo_submit_texture() → wgpu texture created in GPU
6. Swift calls txo_start_comment() → lane assigned, animation starts
7. Each frame: comment X position decremented, quad drawn with texture
8. Comment exits left edge → texture freed
```

### Render loop

```
CVDisplayLink callback (60fps, called from Swift)
  ├── AppDelegate.processComments()  — poll + rasterize + submit
  └── txo_render_frame()             — called into Rust
        ├── Calculate delta time
        ├── Update comment positions (CPU)
        ├── Update particle positions (CPU)
        ├── wgpu Render Pass (clear transparent)
        │   ├── Comment quads (alpha blend)
        │   └── Particle quads (additive blend)
        └── Present
```

## FFI Interface

Direction: Swift → Rust only. No Rust → Swift callbacks.

| Function | Purpose |
|----------|---------|
| `txo_init` | Create renderer from CAMetalLayer pointer |
| `txo_destroy` | Clean up |
| `txo_resize` | Handle display change |
| `txo_poll_comment` | Get next pending comment from HTTP queue |
| `txo_submit_texture` | Upload RGBA bitmap as GPU texture |
| `txo_start_comment` | Begin comment animation (auto lane assignment if y < 0) |
| `txo_trigger_effect` | Fire a particle effect |
| `txo_update_config` | Change speed/opacity |
| `txo_start_server` | Launch HTTP server on a thread |
| `txo_render_frame` | Render one frame |

## Key Design Decisions

### Text as GPU texture
Text is rasterized once by CoreText (CPU), uploaded as a GPU texture, then moved as a simple quad each frame. This avoids per-frame text shaping and allows hundreds of simultaneous comments.

### No Rust → Swift callback
The FFI surface is kept minimal. Instead of callbacks, Swift polls for pending comments on each frame via `txo_poll_comment`. This avoids threading complexity across the FFI boundary.

### Lane assignment with randomness
Scroll comments are assigned to random free lanes within the top 70% of the screen, rather than always filling from the top. This looks more natural with sparse comments.

### cloudflared lazy download
The cloudflared binary (~38MB) is not bundled with the app. It is downloaded on first use to `~/Library/Application Support/textxover/` to avoid bloating the app for users who do not need the sharing feature.

### WebUI via include_str
The HTML file is embedded in the Rust binary at compile time via `include_str!()`. This eliminates the need for static file serving infrastructure.

## Dependencies

### Rust
- **wgpu** — GPU rendering (Metal backend)
- **axum** — HTTP server
- **tokio** — async runtime
- **crossbeam-channel** — lock-free message queue between HTTP and render
- **parking_lot** — fast mutex for shared config
- **bytemuck** — safe casting for GPU buffer data
- **tower-http** — CORS middleware

### Swift
- **AppKit** — window management, menu bar
- **CoreText** — text rasterization
- **QuartzCore** — CAMetalLayer, CVDisplayLink

## Build

```bash
just run   # cargo build --release → swiftc → bundle → open
```

The Swift binary links against `libtextxover.dylib`. The `install_name_tool` step in the justfile ensures the dylib is found at `@executable_path/` inside the app bundle.

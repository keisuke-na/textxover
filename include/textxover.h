#ifndef TEXTXOVER_H
#define TEXTXOVER_H

#include <stdint.h>

// Pending comment returned by txo_poll_comment
typedef struct {
    uint32_t comment_id;
    uint8_t  comment_type;  // 0=scroll, 1=top, 2=bottom
    uint32_t color;         // 0xRRGGBB
    uint8_t  size;          // 0=medium, 1=big, 2=small
    const char* text;       // UTF-8, valid until next poll call
    uint32_t text_len;
} TxoPendingComment;

// Lifecycle
void* txo_init(void* metal_layer_ptr, uint32_t width, uint32_t height);
void  txo_destroy(void* handle);
void  txo_resize(void* handle, uint32_t width, uint32_t height);

// Poll for pending comments from HTTP server.
// Returns 1 if a comment is available (written to out), 0 if none.
int   txo_poll_comment(void* handle, TxoPendingComment* out);

// Texture registration (RGBA from CoreText -> Rust)
void  txo_submit_texture(void* handle,
                         uint32_t comment_id,
                         uint32_t width,
                         uint32_t height,
                         const uint8_t* rgba_data,
                         uint32_t data_len);

// Start comment animation
void  txo_start_comment(void* handle,
                        uint32_t comment_id,
                        uint8_t  comment_type,
                        float    y_position);

// Trigger effect
void  txo_trigger_effect(void* handle, uint8_t effect_type);

// Update config
void  txo_update_config(void* handle,
                        float speed,
                        float opacity);

// Start HTTP server on separate thread
void  txo_start_server(void* handle, uint16_t port);

// Render one frame (called from CVDisplayLink)
void  txo_render_frame(void* handle);

#endif

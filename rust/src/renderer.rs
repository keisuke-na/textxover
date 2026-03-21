use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use wgpu::util::DeviceExt;

use crate::comments::CommentManager;
use crate::effects::EffectManager;
use crate::types::{Config, Particle, QuadVertex};

const MAX_PARTICLES: usize = 10000;

pub struct CommentTexture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    // Comment rendering
    comment_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    comment_textures: HashMap<u32, CommentTexture>,
    // Particle compute + render
    particle_compute_pipeline: wgpu::ComputePipeline,
    particle_render_pipeline: wgpu::RenderPipeline,
    particle_compute_bind_group_layout: wgpu::BindGroupLayout,
    particle_render_bind_group_layout: wgpu::BindGroupLayout,
    particle_buffer: wgpu::Buffer,
    particle_count: u32,   // total active slots (capped at MAX_PARTICLES)
    particle_cursor: u32,  // next write position (wraps around)
    // Shared state
    pub comment_manager: CommentManager,
    pub effect_manager: EffectManager,
    pub config: Arc<RwLock<Config>>,
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(
        metal_layer: *mut std::ffi::c_void,
        width: u32,
        height: u32,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        let surface = unsafe {
            let layer = metal_layer as *mut objc::runtime::Object;
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    layer as *mut _,
                ))
                .expect("Failed to create surface")
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("textxover device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .expect("Failed to create device");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // --- Comment pipeline ---
        let comment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("comment shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/comment.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("comment bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let comment_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("comment pipeline layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let comment_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("comment pipeline"),
            layout: Some(&comment_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &comment_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &comment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Particle shader ---
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        // Pre-allocate persistent particle storage buffer on GPU
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle buffer"),
            size: (MAX_PARTICLES * std::mem::size_of::<Particle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Particle compute pipeline ---
        let particle_compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particle compute bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("particle compute pipeline layout"),
                bind_group_layouts: &[&particle_compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let particle_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("particle compute pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &particle_shader,
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });

        // --- Particle render pipeline (additive blend) ---
        let particle_render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particle render bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("particle render pipeline layout"),
                bind_group_layouts: &[&particle_render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let particle_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("particle render pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &particle_shader,
                    entry_point: Some("vs_particle"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &particle_shader,
                    entry_point: Some("fs_particle"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Renderer {
            device,
            queue,
            surface,
            surface_config,
            comment_pipeline,
            sampler,
            uniform_bind_group_layout,
            comment_textures: HashMap::new(),
            particle_compute_pipeline,
            particle_render_pipeline,
            particle_compute_bind_group_layout,
            particle_render_bind_group_layout,
            particle_buffer,
            particle_count: 0,
            particle_cursor: 0,
            comment_manager: CommentManager::new(width as f32, height as f32),
            effect_manager: EffectManager::new(MAX_PARTICLES),
            config,
            width,
            height,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.comment_manager.resize(width as f32, height as f32);
    }

    pub fn submit_texture(&mut self, comment_id: u32, width: u32, height: u32, rgba_data: &[u8]) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("comment texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());

        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comment uniform buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("comment bind group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.comment_textures.insert(
            comment_id,
            CommentTexture {
                texture,
                view,
                bind_group,
                width,
                height,
            },
        );
    }

    pub fn remove_texture(&mut self, comment_id: u32) {
        self.comment_textures.remove(&comment_id);
    }

    pub fn comment_textures_ref(&self) -> &HashMap<u32, CommentTexture> {
        &self.comment_textures
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn particle_count(&self) -> u32 {
        self.particle_count
    }

    pub fn render(&mut self, dt: f32) {
        let opacity = self.config.read().opacity;

        // Update comments
        let expired = self.comment_manager.update(dt);
        for id in expired {
            self.remove_texture(id);
        }

        // Upload any newly spawned particles to GPU buffer (ring buffer)
        let new_particles = self.effect_manager.drain_pending();
        if !new_particles.is_empty() {
            let stride = std::mem::size_of::<Particle>();
            let mut written = 0;
            while written < new_particles.len() {
                let cursor = self.particle_cursor as usize;
                let space_to_end = MAX_PARTICLES - cursor;
                let batch = (new_particles.len() - written).min(space_to_end);
                let offset = cursor * stride;
                self.queue.write_buffer(
                    &self.particle_buffer,
                    offset as u64,
                    bytemuck::cast_slice(&new_particles[written..written + batch]),
                );
                written += batch;
                self.particle_cursor = ((cursor + batch) % MAX_PARTICLES) as u32;
            }
            self.particle_count = (self.particle_count as usize + new_particles.len())
                .min(MAX_PARTICLES) as u32;
        }

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to get surface texture: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        // --- Compute pass: update particle physics on GPU ---
        if self.particle_count > 0 {
            let sim_params: [f32; 4] = [dt, 150.0, 0.5, 0.0]; // delta_time, gravity, drag, padding
            let sim_uniform =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sim params"),
                        contents: bytemuck::cast_slice(&sim_params),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

            let compute_bind_group =
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("particle compute bind group"),
                    layout: &self.particle_compute_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.particle_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: sim_uniform.as_entire_binding(),
                        },
                    ],
                });

            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("particle compute pass"),
                    timestamp_writes: None,
                });
            compute_pass.set_pipeline(&self.particle_compute_pipeline);
            compute_pass.set_bind_group(0, &compute_bind_group, &[]);
            let workgroups = (self.particle_count + 63) / 64;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // --- Render pass ---
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            // Draw comments
            render_pass.set_pipeline(&self.comment_pipeline);

            let vertices: [QuadVertex; 6] = [
                QuadVertex { position: [0.0, 0.0], tex_coord: [0.0, 0.0] },
                QuadVertex { position: [1.0, 0.0], tex_coord: [1.0, 0.0] },
                QuadVertex { position: [0.0, 1.0], tex_coord: [0.0, 1.0] },
                QuadVertex { position: [0.0, 1.0], tex_coord: [0.0, 1.0] },
                QuadVertex { position: [1.0, 0.0], tex_coord: [1.0, 0.0] },
                QuadVertex { position: [1.0, 1.0], tex_coord: [1.0, 1.0] },
            ];

            let vertex_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("quad vertex buffer"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

            let screen_size = [self.width as f32, self.height as f32];

            for comment in self.comment_manager.active_comments() {
                if let Some(ct) = self.comment_textures.get(&comment.id) {
                    let uniforms: [f32; 8] = [
                        screen_size[0],
                        screen_size[1],
                        comment.x,
                        comment.y,
                        ct.width as f32,
                        ct.height as f32,
                        opacity,
                        0.0,
                    ];

                    let uniform_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("comment uniform"),
                                contents: bytemuck::cast_slice(&uniforms),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    let bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("comment bind group"),
                            layout: &self.uniform_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: uniform_buffer.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(&ct.view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                                },
                            ],
                        });

                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.draw(0..6, 0..1);
                }
            }

            // Draw particles (after compute has updated them)
            if self.particle_count > 0 {
                let screen_uniforms: [f32; 4] = [
                    self.width as f32,
                    self.height as f32,
                    0.0,
                    0.0,
                ];
                let render_uniform =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("particle render uniform"),
                            contents: bytemuck::cast_slice(&screen_uniforms),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });

                let render_bind_group =
                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("particle render bind group"),
                        layout: &self.particle_render_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: self.particle_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: render_uniform.as_entire_binding(),
                            },
                        ],
                    });

                render_pass.set_pipeline(&self.particle_render_pipeline);
                render_pass.set_bind_group(0, &render_bind_group, &[]);
                render_pass.draw(0..(self.particle_count * 6), 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

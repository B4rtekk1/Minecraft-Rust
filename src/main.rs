use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;

use bytemuck;
use cgmath::{InnerSpace, Matrix4, Rad};
use wgpu::util::DeviceExt;
use wgpu_glyph::{GlyphBrush, GlyphBrushBuilder, Section, Text, ab_glyph};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowBuilder},
};

use render3d::{
    BlockType, CHUNK_SIZE, Camera, DEFAULT_WORLD_FILE, DiggingState, InputState, NUM_SUBCHUNKS,
    RENDER_DISTANCE, SUBCHUNK_HEIGHT, SavedWorld, TEXTURE_SIZE, Uniforms, Vertex, World,
    build_crosshair, extract_frustum_planes, generate_texture_atlas, load_texture_atlas_from_file,
    load_world, save_world,
};

#[cfg_attr(rustfmt, rustfmt_skip)]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    crosshair_pipeline: wgpu::RenderPipeline,
    sun_vertex_buffer: wgpu::Buffer,
    sun_index_buffer: wgpu::Buffer,
    crosshair_vertex_buffer: wgpu::Buffer,
    crosshair_index_buffer: wgpu::Buffer,
    num_crosshair_indices: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    shadow_texture_view: wgpu::TextureView,
    #[allow(dead_code)]
    shadow_sampler: wgpu::Sampler,
    world: World,
    camera: Camera,
    input: InputState,
    digging: DiggingState,
    window: Arc<Window>,
    frame_count: u32,
    last_fps_update: Instant,
    current_fps: f32,
    last_frame: Instant,
    mouse_captured: bool,
    chunks_rendered: u32,
    subchunks_rendered: u32,
    game_start_time: Instant,
    coords_vertex_buffer: Option<wgpu::Buffer>,
    coords_index_buffer: Option<wgpu::Buffer>,
    coords_num_indices: u32,
    #[allow(dead_code)]
    texture_atlas: wgpu::Texture,
    #[allow(dead_code)]
    texture_view: wgpu::TextureView,
    #[allow(dead_code)]
    texture_sampler: wgpu::Sampler,
    glyph_brush: GlyphBrush<(), ab_glyph::FontArc>,
    staging_belt: wgpu::util::StagingBelt,
}

impl State {
    async fn new(window: Window) -> Self {
        let window = Arc::new(window);
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::FifoRelaxed,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_texture = Self::create_depth_texture(&device, &config);

        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terrain.wgsl").into()),
        });

        let water_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Water Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/water.wgsl").into()),
        });

        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });

        let sun_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sun Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sun.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                view_proj: Matrix4::from_scale(1.0).into(),
                sun_view_proj: Matrix4::from_scale(1.0).into(),
                camera_pos: [0.0, 0.0, 0.0],
                time: 0.0,
                sun_position: [0.4, 0.8, 0.3],
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (atlas_data, atlas_width, atlas_height) =
            match load_texture_atlas_from_file("assets/textures.png") {
                Ok((data, width, height)) => {
                    println!("Loaded texture atlas from PNG: {}x{}", width, height);
                    (data, width, height)
                }
                Err(e) => {
                    eprintln!("Failed to load assets/textures.png: {}", e);
                    match load_texture_atlas_from_file("assets/textures.jpg") {
                        Ok((data, width, height)) => {
                            println!("Loaded texture atlas from JPG: {}x{}", width, height);
                            (data, width, height)
                        }
                        Err(e) => {
                            eprintln!("Failed to load assets/textures.jpg: {}", e);
                            match load_texture_atlas_from_file("textures.png") {
                                Ok((data, width, height)) => {
                                    println!(
                                        "Loaded texture atlas from textures.png: {}x{}",
                                        width, height
                                    );
                                    (data, width, height)
                                }
                                Err(e) => {
                                    eprintln!("Failed to load textures.png: {}", e);
                                    match load_texture_atlas_from_file("textures.jpg") {
                                        Ok((data, width, height)) => {
                                            println!(
                                                "Loaded texture atlas from textures.jpg: {}x{}",
                                                width, height
                                            );
                                            (data, width, height)
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to load textures.jpg: {}", e);
                                            println!("Using procedural texture atlas generation.");
                                            let data = generate_texture_atlas();
                                            (data, TEXTURE_SIZE, TEXTURE_SIZE)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };

        let texture_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture Array"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 16,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture_atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas_width),
                rows_per_image: Some(atlas_height),
            },
            wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 16,
            },
        );

        let texture_view = texture_atlas.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Texture Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let shadow_map_size = 2048;
        let shadow_map_desc = wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: shadow_map_size,
                height: shadow_map_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let shadow_texture = device.create_texture(&shadow_map_desc);
        let shadow_texture_view =
            shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
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
                            view_dimension: wgpu::TextureViewDimension::D2Array,
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            });

        let shadow_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shadow_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
            label: Some("uniform_bind_group"),
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shadow_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("shadow_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[&shadow_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let water_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Water Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &water_shader,
                entry_point: Some("vs_water"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &water_shader,
                entry_point: Some("fs_water"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let crosshair_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_ui"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_ui"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sun_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sun Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &sun_shader,
                entry_point: Some("vs_sun"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sun_shader,
                entry_point: Some("fs_sun"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sun_vertices = vec![
            Vertex {
                position: [-1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 1.0, 1.0],
                uv: [0.0, 0.0],
                tex_index: 0.0,
            },
            Vertex {
                position: [1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 1.0, 1.0],
                uv: [1.0, 0.0],
                tex_index: 0.0,
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 1.0, 1.0],
                uv: [1.0, 1.0],
                tex_index: 0.0,
            },
            Vertex {
                position: [-1.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 1.0, 1.0],
                uv: [0.0, 1.0],
                tex_index: 0.0,
            },
        ];
        let sun_indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];

        let sun_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Vertex Buffer"),
            contents: bytemuck::cast_slice(&sun_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let sun_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sun Index Buffer"),
            contents: bytemuck::cast_slice(&sun_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        println!("Generating world...");
        let world = World::new();
        let spawn = world.find_spawn_point();
        let camera = Camera::new(spawn);
        println!("World generated! Spawn: {:?}", spawn);

        let (crosshair_vertices, crosshair_indices) = build_crosshair();
        let num_crosshair_indices = crosshair_indices.len() as u32;

        let crosshair_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Crosshair Vertex Buffer"),
                contents: bytemuck::cast_slice(&crosshair_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let crosshair_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Crosshair Index Buffer"),
            contents: bytemuck::cast_slice(&crosshair_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let staging_belt = wgpu::util::StagingBelt::new(1024);
        let font = ab_glyph::FontArc::try_from_slice(include_bytes!("c:/Windows/Fonts/arial.ttf"))
            .or_else(|_| {
                ab_glyph::FontArc::try_from_slice(include_bytes!("c:/Windows/Fonts/consola.ttf"))
            })
            .expect("Could not load font");
        let glyph_brush = GlyphBrushBuilder::using_font(font).build(&device, surface_format);

        Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            water_pipeline,
            sun_pipeline,
            shadow_pipeline,
            crosshair_pipeline,
            sun_vertex_buffer,
            sun_index_buffer,
            crosshair_vertex_buffer,
            crosshair_index_buffer,
            num_crosshair_indices,
            uniform_buffer,
            uniform_bind_group,
            shadow_bind_group,
            depth_texture,
            shadow_texture_view,
            shadow_sampler,
            world,
            camera,
            input: InputState::default(),
            digging: DiggingState::default(),
            window,
            frame_count: 0,
            last_fps_update: Instant::now(),
            current_fps: 0.0,
            last_frame: Instant::now(),
            mouse_captured: false,
            chunks_rendered: 0,
            subchunks_rendered: 0,
            game_start_time: Instant::now(),
            coords_vertex_buffer: None,
            coords_index_buffer: None,
            coords_num_indices: 0,
            texture_atlas,
            texture_view,
            texture_sampler,
            glyph_brush,
            staging_belt,
        }
    }

    fn update_coords_ui(&mut self) {
        let x = self.camera.position.x;
        let y = self.camera.position.y;
        let z = self.camera.position.z;

        let text = format!("X:{:.0} Y:{:.0} Z:{:.0}", x, y, z);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let char_width = 0.018;
        let char_height = 0.032;
        let line_thickness = 0.004;
        let char_spacing = char_width * 0.6;
        let gap_spacing = char_width + 0.005;

        let mut total_width = 0.0;
        for ch in text.chars() {
            if ch == ' ' {
                total_width += char_spacing;
            } else {
                total_width += gap_spacing;
            }
        }

        let start_x = 0.98 - total_width;
        let start_y = 0.95;

        let mut cursor_x = start_x;
        let cursor_y = start_y;
        let color = [1.0, 1.0, 1.0];
        let normal = [0.0, 0.0, 1.0];

        let add_segment =
            |x1: f32, y1: f32, x2: f32, y2: f32, verts: &mut Vec<Vertex>, inds: &mut Vec<u32>| {
                let base_idx = verts.len() as u32;
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.001 {
                    return;
                }
                let nx = -dy / len * line_thickness * 0.5;
                let ny = dx / len * line_thickness * 0.5;

                verts.push(Vertex {
                    position: [x1 - nx, y1 - ny, 0.0],
                    normal,
                    color,
                    uv: [0.0, 0.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x2 - nx, y2 - ny, 0.0],
                    normal,
                    color,
                    uv: [1.0, 0.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x2 + nx, y2 + ny, 0.0],
                    normal,
                    color,
                    uv: [1.0, 1.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x1 + nx, y1 + ny, 0.0],
                    normal,
                    color,
                    uv: [0.0, 1.0],
                    tex_index: 0.0,
                });
                inds.extend_from_slice(&[
                    base_idx,
                    base_idx + 1,
                    base_idx + 2,
                    base_idx,
                    base_idx + 2,
                    base_idx + 3,
                ]);
            };

        fn get_char_segments(ch: char) -> Vec<(f32, f32, f32, f32)> {
            let seg_top = (0.0, 1.0, 1.0, 1.0);
            let seg_tr = (1.0, 1.0, 1.0, 0.5);
            let seg_br = (1.0, 0.5, 1.0, 0.0);
            let seg_bot = (0.0, 0.0, 1.0, 0.0);
            let seg_bl = (0.0, 0.5, 0.0, 0.0);
            let seg_tl = (0.0, 1.0, 0.0, 0.5);
            let seg_mid = (0.0, 0.5, 1.0, 0.5);

            match ch {
                '0' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_bl, seg_tl],
                '1' => vec![seg_tr, seg_br],
                '2' => vec![seg_top, seg_tr, seg_mid, seg_bl, seg_bot],
                '3' => vec![seg_top, seg_tr, seg_mid, seg_br, seg_bot],
                '4' => vec![seg_tl, seg_mid, seg_tr, seg_br],
                '5' => vec![seg_top, seg_tl, seg_mid, seg_br, seg_bot],
                '6' => vec![seg_top, seg_tl, seg_mid, seg_br, seg_bot, seg_bl],
                '7' => vec![seg_top, seg_tr, seg_br],
                '8' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_bl, seg_tl, seg_mid],
                '9' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_tl, seg_mid],
                'X' => vec![(0.0, 1.0, 1.0, 0.0), (0.0, 0.0, 1.0, 1.0)],
                'Y' => vec![
                    (0.0, 1.0, 0.5, 0.5),
                    (1.0, 1.0, 0.5, 0.5),
                    (0.5, 0.5, 0.5, 0.0),
                ],
                'Z' => vec![seg_top, (1.0, 1.0, 0.0, 0.0), seg_bot],
                ':' => vec![(0.4, 0.7, 0.6, 0.7), (0.4, 0.3, 0.6, 0.3)],
                '.' => vec![(0.4, 0.1, 0.6, 0.1)],
                '-' => vec![seg_mid],
                _ => vec![],
            }
        }

        for ch in text.chars() {
            if ch == ' ' {
                cursor_x += char_spacing;
                continue;
            }

            let segments = get_char_segments(ch);
            for (x1, y1, x2, y2) in segments {
                let px1 = cursor_x + x1 * char_width;
                let py1 = cursor_y - char_height + y1 * char_height;
                let px2 = cursor_x + x2 * char_width;
                let py2 = cursor_y - char_height + y2 * char_height;
                add_segment(px1, py1, px2, py2, &mut vertices, &mut indices);
            }

            cursor_x += gap_spacing;
        }

        if vertices.is_empty() {
            self.coords_vertex_buffer = None;
            self.coords_index_buffer = None;
            self.coords_num_indices = 0;
            return;
        }

        let vb = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Coords Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let ib = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Coords Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        self.coords_vertex_buffer = Some(vb);
        self.coords_index_buffer = Some(ib);
        self.coords_num_indices = indices.len() as u32;
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Self::create_depth_texture(&self.device, &self.config);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.camera.update(&self.world, dt, &self.input);

        self.world
            .update_chunks_around_player(self.camera.position.x, self.camera.position.z);

        self.update_coords_ui();

        if self.mouse_captured && self.input.left_mouse {
            if let Some((bx, by, bz, _, _, _)) = self.camera.raycast(&self.world, 5.0) {
                let target = (bx, by, bz);
                let block = self.world.get_block(bx, by, bz);
                let break_time = block.break_time();

                if break_time.is_finite() && break_time > 0.0 {
                    if self.digging.target == Some(target) {
                        self.digging.progress += dt;
                        if self.digging.progress >= break_time {
                            self.world.set_block_player(bx, by, bz, BlockType::Air);
                            self.mark_chunk_dirty(bx, by, bz);
                            self.digging.target = None;
                            self.digging.progress = 0.0;
                        }
                    } else {
                        self.digging.target = Some(target);
                        self.digging.progress = 0.0;
                        self.digging.break_time = break_time;
                    }
                }
            } else {
                self.digging.target = None;
                self.digging.progress = 0.0;
            }
        } else {
            self.digging.target = None;
            self.digging.progress = 0.0;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.staging_belt.recall();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let aspect = self.config.width as f32 / self.config.height as f32;
        let proj = cgmath::perspective(Rad(std::f32::consts::FRAC_PI_2), aspect, 0.1, 500.0);
        let view_mat = self.camera.view_matrix();
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view_mat;
        let view_proj_array: [[f32; 4]; 4] = view_proj.into();

        let time = self.game_start_time.elapsed().as_secs_f32();
        let sun_angle = time * 0.05;
        let sun_x = sun_angle.cos();
        let sun_y = 0.8;
        let sun_z = sun_angle.sin();
        let sun_dir = cgmath::Vector3::new(sun_x, sun_y, sun_z).normalize();

        let sun_pos = cgmath::Point3::new(
            self.camera.position.x + sun_dir.x * 100.0,
            self.camera.position.y + sun_dir.y * 100.0,
            self.camera.position.z + sun_dir.z * 100.0,
        );
        let sun_view =
            Matrix4::look_at_rh(sun_pos, self.camera.position, cgmath::Vector3::unit_y());
        let ortho_size = 150.0;
        let sun_proj = cgmath::ortho(
            -ortho_size,
            ortho_size,
            -ortho_size,
            ortho_size,
            500.0,
            -500.0,
        );
        let sun_view_proj = OPENGL_TO_WGPU_MATRIX * sun_proj * sun_view;
        let sun_view_proj_array: [[f32; 4]; 4] = sun_view_proj.into();

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms {
                view_proj: view_proj_array,
                sun_view_proj: sun_view_proj_array,
                camera_pos: self.camera.eye_position().into(),
                time,
                sun_position: [sun_x, sun_y, sun_z],
                _padding: 0.0,
            }]),
        );

        let frustum_planes = extract_frustum_planes(&view_proj);

        let player_cx = (self.camera.position.x as i32) / CHUNK_SIZE;
        let player_cz = (self.camera.position.z as i32) / CHUNK_SIZE;

        {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            shadow_pass.set_pipeline(&self.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.shadow_bind_group, &[]);

            let shadow_dist = RENDER_DISTANCE;
            for cx in (player_cx - shadow_dist)..=(player_cx + shadow_dist) {
                for cz in (player_cz - shadow_dist)..=(player_cz + shadow_dist) {
                    if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                        for subchunk in &chunk.subchunks {
                            if subchunk.is_empty || subchunk.num_indices == 0 {
                                continue;
                            }
                            if let (Some(vb), Some(ib)) =
                                (&subchunk.vertex_buffer, &subchunk.index_buffer)
                            {
                                shadow_pass.set_vertex_buffer(0, vb.slice(..));
                                shadow_pass
                                    .set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                                shadow_pass.draw_indexed(0..subchunk.num_indices, 0, 0..1);
                            }
                        }
                    }
                }
            }
        }

        let mut meshes_to_build: Vec<(i32, i32, i32)> = Vec::new();

        for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
            for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                    for (sy, subchunk) in chunk.subchunks.iter().enumerate() {
                        if subchunk.mesh_dirty && !subchunk.is_empty {
                            meshes_to_build.push((cx, cz, sy as i32));
                        }
                    }
                }
            }
        }

        let max_meshes_per_frame = 8;
        meshes_to_build.truncate(max_meshes_per_frame);
        let built_meshes: Vec<(
            i32,
            i32,
            i32,
            (Vec<Vertex>, Vec<u32>),
            (Vec<Vertex>, Vec<u32>),
        )> = meshes_to_build
            .par_iter()
            .map(|&(cx, cz, sy)| {
                let meshes = self.world.build_subchunk_mesh(cx, cz, sy);
                (cx, cz, sy, meshes.0, meshes.1)
            })
            .collect();

        for (cx, cz, sy, (vertices, indices), (w_vertices, w_indices)) in built_meshes {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                let subchunk = &mut chunk.subchunks[sy as usize];

                subchunk.num_indices = indices.len() as u32;
                if !vertices.is_empty() {
                    subchunk.vertex_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Subchunk Vertex Buffer"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                    subchunk.index_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Subchunk Index Buffer"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    ));
                } else {
                    subchunk.vertex_buffer = None;
                    subchunk.index_buffer = None;
                }

                subchunk.num_water_indices = w_indices.len() as u32;
                if !w_vertices.is_empty() {
                    subchunk.water_vertex_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Water Vertex Buffer"),
                            contents: bytemuck::cast_slice(&w_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                    subchunk.water_index_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Water Index Buffer"),
                            contents: bytemuck::cast_slice(&w_indices),
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    ));
                } else {
                    subchunk.water_vertex_buffer = None;
                    subchunk.water_index_buffer = None;
                }

                subchunk.mesh_dirty = false;
            }
        }

        let mut chunks_rendered = 0u32;
        let mut subchunks_rendered = 0u32;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.53,
                            g: 0.81,
                            b: 0.98,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
                for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                    if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                        let mut chunk_visible = false;
                        for subchunk in &chunk.subchunks {
                            if subchunk.is_empty || subchunk.num_indices == 0 {
                                continue;
                            }
                            if !subchunk.aabb.is_visible(&frustum_planes) {
                                continue;
                            }
                            if let (Some(vb), Some(ib)) =
                                (&subchunk.vertex_buffer, &subchunk.index_buffer)
                            {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                                render_pass
                                    .set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                                render_pass.draw_indexed(0..subchunk.num_indices, 0, 0..1);
                                subchunks_rendered += 1;
                                chunk_visible = true;
                            }
                        }
                        if chunk_visible {
                            chunks_rendered += 1;
                        }
                    }
                }
            }

            render_pass.set_pipeline(&self.water_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
                for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                    if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                        for subchunk in &chunk.subchunks {
                            if subchunk.is_empty || subchunk.num_water_indices == 0 {
                                continue;
                            }
                            if !subchunk.aabb.is_visible(&frustum_planes) {
                                continue;
                            }
                            if let (Some(vb), Some(ib)) =
                                (&subchunk.water_vertex_buffer, &subchunk.water_index_buffer)
                            {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                                render_pass
                                    .set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                                render_pass.draw_indexed(0..subchunk.num_water_indices, 0, 0..1);
                            }
                        }
                    }
                }
            }

            render_pass.set_pipeline(&self.sun_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sun_vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(self.sun_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }

        self.chunks_rendered = chunks_rendered;
        self.subchunks_rendered = subchunks_rendered;

        {
            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            ui_pass.set_pipeline(&self.crosshair_pipeline);
            ui_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            ui_pass.set_vertex_buffer(0, self.crosshair_vertex_buffer.slice(..));
            ui_pass.set_index_buffer(
                self.crosshair_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            ui_pass.draw_indexed(0..self.num_crosshair_indices, 0, 0..1);

            if let (Some(vb), Some(ib)) = (&self.coords_vertex_buffer, &self.coords_index_buffer) {
                if self.coords_num_indices > 0 {
                    ui_pass.set_vertex_buffer(0, vb.slice(..));
                    ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    ui_pass.draw_indexed(0..self.coords_num_indices, 0, 0..1);
                }
            }
        }

        if self.digging.target.is_some() && self.digging.break_time > 0.0 {
            let progress = (self.digging.progress / self.digging.break_time).min(1.0);

            let bar_width = 0.15;
            let bar_height = 0.015;
            let bar_y = -0.05;

            let bg_color = [0.2, 0.2, 0.2];
            let prog_color = [1.0 - progress, progress, 0.0];

            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let normal = [0.0, 0.0, 1.0];

            vertices.push(Vertex {
                position: [-bar_width, bar_y - bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [0.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [bar_width, bar_y - bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [1.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [bar_width, bar_y + bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [1.0, 1.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [-bar_width, bar_y + bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [0.0, 1.0],
                tex_index: 0.0,
            });
            indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

            let prog_width = bar_width * 2.0 * progress - bar_width;
            vertices.push(Vertex {
                position: [-bar_width + 0.005, bar_y - bar_height + 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [0.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [prog_width - 0.005, bar_y - bar_height + 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [1.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [prog_width - 0.005, bar_y + bar_height - 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [1.0, 1.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [-bar_width + 0.005, bar_y + bar_height - 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [0.0, 1.0],
                tex_index: 0.0,
            });
            indices.extend_from_slice(&[4, 5, 6, 4, 6, 7]);

            let progress_vb = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Progress Bar VB"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let progress_ib = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Progress Bar IB"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            let mut progress_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Progress Bar Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            progress_pass.set_pipeline(&self.crosshair_pipeline);
            progress_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            progress_pass.set_vertex_buffer(0, progress_vb.slice(..));
            progress_pass.set_index_buffer(progress_ib.slice(..), wgpu::IndexFormat::Uint32);
            progress_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        {
            let fps_text = format!("FPS: {:.0}", self.current_fps);
            self.glyph_brush.queue(Section {
                screen_position: (10.0, 10.0),
                bounds: (self.config.width as f32, self.config.height as f32),
                text: vec![
                    Text::new(&fps_text)
                        .with_color([1.0, 1.0, 1.0, 1.0])
                        .with_scale(40.0),
                ],
                ..Section::default()
            });

            self.glyph_brush
                .draw_queued(
                    &self.device,
                    &mut self.staging_belt,
                    &mut encoder,
                    &view,
                    self.config.width,
                    self.config.height,
                )
                .expect("Draw queued");
        }

        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn handle_mouse_input(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.input.left_mouse = pressed,
            MouseButton::Right => self.input.right_mouse = pressed,
            _ => {}
        }

        if !self.mouse_captured {
            return;
        }

        if button == MouseButton::Right && pressed {
            if let Some((_, _, _, px, py, pz)) = self.camera.raycast(&self.world, 5.0) {
                self.world.set_block_player(px, py, pz, BlockType::Stone);
                self.mark_chunk_dirty(px, py, pz);
            }
        }
    }

    fn mark_chunk_dirty(&mut self, x: i32, y: i32, z: i32) {
        let cx = x / CHUNK_SIZE;
        let cz = z / CHUNK_SIZE;
        let sy = y / SUBCHUNK_HEIGHT;

        if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
            if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                chunk.subchunks[sy as usize].mesh_dirty = true;
            }
        }

        let lx = x % CHUNK_SIZE;
        let lz = z % CHUNK_SIZE;
        let ly = y % SUBCHUNK_HEIGHT;

        if lx == 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx - 1, cz)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lx == CHUNK_SIZE - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx + 1, cz)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lz == 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz - 1)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lz == CHUNK_SIZE - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz + 1)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if ly == 0 && sy > 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                chunk.subchunks[(sy - 1) as usize].mesh_dirty = true;
            }
        }
        if ly == SUBCHUNK_HEIGHT - 1 && sy < NUM_SUBCHUNKS - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                chunk.subchunks[(sy + 1) as usize].mesh_dirty = true;
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("🎮 Mini Minecraft 256x256 | Loading...")
        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new(window));

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    state.resize(size);
                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    state.frame_count += 1;
                    let now = Instant::now();
                    let elapsed = now.duration_since(state.last_fps_update).as_secs_f32();

                    if elapsed >= 0.5 {
                        state.current_fps = state.frame_count as f32 / elapsed;
                        state.frame_count = 0;
                        state.last_fps_update = now;
                    }

                    state.update();

                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.window.inner_size()),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => eprintln!("Render error: {:?}", e),
                    }

                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    physical_key: PhysicalKey::Code(key),
                                    state: key_state,
                                    ..
                                },
                            ..
                        },
                    ..
                } => {
                    let pressed = key_state == ElementState::Pressed;
                    match key {
                        KeyCode::KeyW => state.input.forward = pressed,
                        KeyCode::KeyS => state.input.backward = pressed,
                        KeyCode::KeyA => state.input.left = pressed,
                        KeyCode::KeyD => state.input.right = pressed,
                        KeyCode::Space => state.input.jump = pressed,
                        KeyCode::ShiftLeft => state.input.sprint = pressed,
                        KeyCode::Escape if pressed => {
                            state.mouse_captured = false;
                            let _ = state.window.set_cursor_grab(CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                        KeyCode::F11 if pressed => {
                            if state.window.fullscreen().is_some() {
                                state.window.set_fullscreen(None);
                            } else {
                                state.window.set_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(None),
                                ));
                            }
                        }
                        KeyCode::F5 if pressed => {
                            let saved = SavedWorld::from_world(
                                &state.world.chunks,
                                state.world.seed,
                                (
                                    state.camera.position.x,
                                    state.camera.position.y,
                                    state.camera.position.z,
                                ),
                                (state.camera.yaw, state.camera.pitch),
                            );
                            match save_world(DEFAULT_WORLD_FILE, &saved) {
                                Ok(_) => println!("✅ Świat zapisany do {}", DEFAULT_WORLD_FILE),
                                Err(e) => println!("❌ Błąd zapisu: {}", e),
                            }
                        }
                        KeyCode::F9 if pressed => match load_world(DEFAULT_WORLD_FILE) {
                            Ok(saved) => {
                                println!("🔄 Regenerating world with seed {}...", saved.seed);
                                state.world = World::new_with_seed(saved.seed);
                                state.camera.position.x = saved.player_x;
                                state.camera.position.y = saved.player_y;
                                state.camera.position.z = saved.player_z;
                                state.camera.yaw = saved.player_yaw;
                                state.camera.pitch = saved.player_pitch;

                                for chunk_data in &saved.chunks {
                                    for block in &chunk_data.blocks {
                                        state.world.set_block(
                                            block.x,
                                            block.y,
                                            block.z,
                                            block.block_type,
                                        );
                                    }
                                    let cx = chunk_data.cx;
                                    let cz = chunk_data.cz;
                                    if let Some(chunk) = state.world.chunks.get_mut(&(cx, cz)) {
                                        chunk.player_modified = true;
                                    }
                                }

                                for chunk in state.world.chunks.values_mut() {
                                    for subchunk in &mut chunk.subchunks {
                                        subchunk.mesh_dirty = true;
                                    }
                                }

                                println!(
                                    "✅ Świat wczytany z {} (seed: {})",
                                    DEFAULT_WORLD_FILE, saved.seed
                                );
                            }
                            Err(e) => println!("❌ Błąd wczytywania: {}", e),
                        },
                        _ => {}
                    }
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::MouseInput {
                            state: btn_state,
                            button,
                            ..
                        },
                    ..
                } => {
                    let pressed = btn_state == ElementState::Pressed;

                    if pressed && !state.mouse_captured {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(CursorGrabMode::Confined)
                            .or_else(|_| state.window.set_cursor_grab(CursorGrabMode::Locked));
                        state.window.set_cursor_visible(false);
                        let _ = state.window.set_cursor_position(PhysicalPosition::new(
                            state.config.width / 2,
                            state.config.height / 2,
                        ));
                    } else {
                        state.handle_mouse_input(button, pressed);
                    }
                }
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => {
                    if state.mouse_captured {
                        let sensitivity = 0.002;
                        state.camera.yaw += delta.0 as f32 * sensitivity;
                        state.camera.pitch -= delta.1 as f32 * sensitivity;
                        state.camera.pitch = state.camera.pitch.clamp(
                            -std::f32::consts::FRAC_PI_2 + 0.1,
                            std::f32::consts::FRAC_PI_2 - 0.1,
                        );
                    }
                }
                Event::AboutToWait => {
                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                _ => {}
            }
        })
        .unwrap();
}

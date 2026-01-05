//! GPU Indirect Drawing Manager
//!
//! Manages unified vertex/index buffers and indirect draw commands for GPU-driven rendering.
//! This eliminates per-subchunk draw call overhead by batching all geometry into unified buffers
//! and using compute shader culling with indirect drawing.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

use crate::core::vertex::Vertex;
use crate::render::frustum::AABB;

/// Maximum number of subchunks that can be stored in unified buffers
const MAX_SUBCHUNKS: usize = 16384;
/// Maximum vertices across all subchunks (~252MB at 56 bytes per vertex - under 256MB wgpu limit)
const MAX_VERTICES: usize = 4_500_000;
/// Maximum indices across all subchunks (128MB at 4 bytes per index)
const MAX_INDICES: usize = 32_000_000;

/// wgpu DrawIndexedIndirect command structure (matches GPU layout)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct DrawIndexedIndirect {
    /// Number of indices to draw
    pub index_count: u32,
    /// Number of instances to draw (always 1 for us)
    pub instance_count: u32,
    /// First index in the index buffer
    pub first_index: u32,
    /// Value added to vertex indices before indexing into vertex buffer
    pub base_vertex: i32,
    /// First instance to draw (always 0)
    pub first_instance: u32,
}

/// Metadata for a subchunk stored on GPU for culling
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SubchunkGpuMeta {
    /// AABB min (xyz) + padding
    pub aabb_min: [f32; 4],
    /// AABB max (xyz) + subchunk_id in w
    pub aabb_max: [f32; 4],
    /// Draw command data: index_count, first_index, base_vertex, flags
    pub draw_data: [u32; 4],
}

/// Culling uniforms - frustum planes + subchunk count
/// Note: Must match cull.wgsl CullUniforms struct layout exactly
/// WGSL vec3<u32> has 16-byte alignment, so we need extra padding
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CullUniforms {
    /// 6 frustum planes (each is vec4: xyz=normal, w=distance)
    pub frustum_planes: [[f32; 4]; 6],
    /// Number of active subchunks
    pub subchunk_count: u32,
    /// Padding to match WGSL alignment (vec3<u32> is 16 bytes in WGSL)
    /// Total size must be 128 bytes: 96 (planes) + 4 (count) + 28 (padding) = 128
    pub _padding: [u32; 7],
}

/// Key for identifying a subchunk
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SubchunkKey {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub subchunk_y: i32,
}

/// Allocation info for a subchunk in unified buffers
#[derive(Copy, Clone, Debug)]
struct SubchunkAlloc {
    vertex_offset: u32,
    vertex_count: u32,
    index_offset: u32,
    index_count: u32,
    slot_index: usize,
}

/// Manages GPU indirect drawing resources
pub struct IndirectManager {
    // Unified geometry buffers
    unified_vertex_buffer: wgpu::Buffer,
    unified_index_buffer: wgpu::Buffer,

    // Draw command buffers
    #[allow(dead_code)]
    draw_commands_buffer: wgpu::Buffer,
    visible_draw_commands_buffer: wgpu::Buffer,

    // Subchunk metadata for GPU culling
    subchunk_meta_buffer: wgpu::Buffer,

    // Atomic counter for visible subchunks
    visible_count_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    visible_count_staging: wgpu::Buffer,

    // Tracking allocations
    allocations: HashMap<SubchunkKey, SubchunkAlloc>,
    next_vertex_offset: u32,
    next_index_offset: u32,
    active_subchunk_count: u32,

    // Compute pipeline for culling
    cull_pipeline: wgpu::ComputePipeline,
    cull_bind_group_layout: wgpu::BindGroupLayout,
    cull_bind_group: Option<wgpu::BindGroup>,

    // Culling uniforms buffer (frustum + count)
    cull_uniforms_buffer: wgpu::Buffer,
}

impl IndirectManager {
    pub fn new(device: &wgpu::Device) -> Self {
        // Create unified vertex buffer
        let unified_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Unified Vertex Buffer"),
            size: (MAX_VERTICES * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create unified index buffer
        let unified_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Unified Index Buffer"),
            size: (MAX_INDICES * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Draw commands (one per possible subchunk)
        let draw_commands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Draw Commands Buffer"),
            size: (MAX_SUBCHUNKS * std::mem::size_of::<DrawIndexedIndirect>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Visible draw commands (filtered by compute shader)
        let visible_draw_commands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visible Draw Commands Buffer"),
            size: (MAX_SUBCHUNKS * std::mem::size_of::<DrawIndexedIndirect>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Subchunk metadata for GPU culling
        let subchunk_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Subchunk Metadata Buffer"),
            size: (MAX_SUBCHUNKS * std::mem::size_of::<SubchunkGpuMeta>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Visible count (atomic counter)
        let visible_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visible Count Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let visible_count_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visible Count Staging"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Culling uniforms buffer (frustum planes + count)
        let cull_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cull Uniforms Buffer"),
            size: std::mem::size_of::<CullUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute pipeline for culling
        let cull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cull Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cull.wgsl").into()),
        });

        let cull_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cull Bind Group Layout"),
                entries: &[
                    // Culling uniforms (frustum planes + count)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Subchunk metadata (read)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Visible draw commands (write)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Visible count (atomic)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let cull_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cull Pipeline Layout"),
            bind_group_layouts: &[&cull_bind_group_layout],
            immediate_size: 0,
        });

        let cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Cull Pipeline"),
            layout: Some(&cull_pipeline_layout),
            module: &cull_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            unified_vertex_buffer,
            unified_index_buffer,
            draw_commands_buffer,
            visible_draw_commands_buffer,
            subchunk_meta_buffer,
            visible_count_buffer,
            visible_count_staging,
            allocations: HashMap::new(),
            next_vertex_offset: 0,
            next_index_offset: 0,
            active_subchunk_count: 0,
            cull_pipeline,
            cull_bind_group_layout,
            cull_bind_group: None,
            cull_uniforms_buffer,
        }
    }

    /// Recreate bind group (call after buffer changes)
    fn recreate_bind_group(&mut self, device: &wgpu::Device) {
        self.cull_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cull Bind Group"),
            layout: &self.cull_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.cull_uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.subchunk_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.visible_draw_commands_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.visible_count_buffer.as_entire_binding(),
                },
            ],
        }));
    }

    /// Upload a subchunk's mesh data to unified buffers
    /// Returns true if successful, false if buffer is full
    pub fn upload_subchunk(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: SubchunkKey,
        vertices: &[Vertex],
        indices: &[u32],
        aabb: &AABB,
    ) -> bool {
        // Remove old allocation if exists
        if let Some(old_alloc) = self.allocations.remove(&key) {
            // For now, we don't reclaim space (simple append-only allocator)
            // In production, you'd want a free-list or defragmentation
            _ = old_alloc;
        }

        if vertices.is_empty() || indices.is_empty() {
            return true; // Empty is considered success (nothing to upload)
        }

        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        // Check if we have space
        if self.next_vertex_offset + vertex_count > MAX_VERTICES as u32
            || self.next_index_offset + index_count > MAX_INDICES as u32
        {
            // Buffer full - fall back to per-subchunk rendering
            return false;
        }

        let slot_index = self.allocations.len();
        if slot_index >= MAX_SUBCHUNKS {
            // Max slots reached
            return false;
        }

        // Allocate space
        let alloc = SubchunkAlloc {
            vertex_offset: self.next_vertex_offset,
            vertex_count,
            index_offset: self.next_index_offset,
            index_count,
            slot_index,
        };

        // Upload vertex data
        let vertex_byte_offset = alloc.vertex_offset as u64 * std::mem::size_of::<Vertex>() as u64;
        queue.write_buffer(
            &self.unified_vertex_buffer,
            vertex_byte_offset,
            bytemuck::cast_slice(vertices),
        );

        // Upload index data (rebased to 0, will use base_vertex in draw command)
        let index_byte_offset = alloc.index_offset as u64 * std::mem::size_of::<u32>() as u64;
        queue.write_buffer(
            &self.unified_index_buffer,
            index_byte_offset,
            bytemuck::cast_slice(indices),
        );

        // Upload subchunk metadata for GPU culling
        let subchunk_meta = SubchunkGpuMeta {
            aabb_min: [aabb.min.x, aabb.min.y, aabb.min.z, 0.0],
            aabb_max: [aabb.max.x, aabb.max.y, aabb.max.z, slot_index as f32],
            draw_data: [
                index_count,
                alloc.index_offset,
                alloc.vertex_offset,
                1, // enabled flag
            ],
        };
        let meta_byte_offset = slot_index * std::mem::size_of::<SubchunkGpuMeta>();
        queue.write_buffer(
            &self.subchunk_meta_buffer,
            meta_byte_offset as u64,
            bytemuck::bytes_of(&subchunk_meta),
        );

        self.next_vertex_offset += vertex_count;
        self.next_index_offset += index_count;
        self.allocations.insert(key, alloc);
        self.active_subchunk_count = self.allocations.len() as u32;

        // Recreate bind group since metadata changed
        self.recreate_bind_group(device);

        true
    }

    /// Remove a subchunk
    pub fn remove_subchunk(&mut self, queue: &wgpu::Queue, key: SubchunkKey) {
        if let Some(alloc) = self.allocations.remove(&key) {
            // Disable this slot by zeroing the draw data
            let subchunk_meta = SubchunkGpuMeta {
                aabb_min: [0.0; 4],
                aabb_max: [0.0; 4],
                draw_data: [0, 0, 0, 0], // enabled = 0
            };
            let meta_byte_offset = alloc.slot_index * std::mem::size_of::<SubchunkGpuMeta>();
            queue.write_buffer(
                &self.subchunk_meta_buffer,
                meta_byte_offset as u64,
                bytemuck::bytes_of(&subchunk_meta),
            );
            self.active_subchunk_count = self.allocations.len() as u32;
        }
    }

    /// Dispatch GPU culling compute shader
    pub fn dispatch_culling(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        frustum_planes: &[[f32; 4]; 6],
    ) {
        if self.active_subchunk_count == 0 {
            return;
        }

        // Reset visible count to 0
        queue.write_buffer(&self.visible_count_buffer, 0, &0u32.to_le_bytes());

        // Upload culling uniforms
        let uniforms = CullUniforms {
            frustum_planes: *frustum_planes,
            subchunk_count: self.active_subchunk_count,
            _padding: [0; 7],
        };
        queue.write_buffer(&self.cull_uniforms_buffer, 0, bytemuck::bytes_of(&uniforms));

        if let Some(bind_group) = &self.cull_bind_group {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Culling Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.cull_pipeline);
            cpass.set_bind_group(0, bind_group, &[]);

            // Dispatch enough workgroups for all subchunks (64 threads per group)
            let workgroup_count = (self.active_subchunk_count + 63) / 64;
            cpass.dispatch_workgroups(workgroup_count, 1, 1);
        }
    }

    /// Get unified vertex buffer for rendering
    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.unified_vertex_buffer
    }

    /// Get unified index buffer for rendering
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.unified_index_buffer
    }

    /// Get visible draw commands buffer for indirect rendering
    pub fn draw_commands(&self) -> &wgpu::Buffer {
        &self.visible_draw_commands_buffer
    }

    /// Get number of active subchunks (upper bound for draw count)
    pub fn active_count(&self) -> u32 {
        self.active_subchunk_count
    }

    /// Check if a subchunk is already uploaded
    pub fn has_subchunk(&self, key: &SubchunkKey) -> bool {
        self.allocations.contains_key(key)
    }

    /// Clear all allocations (e.g., on world reload)
    pub fn clear(&mut self) {
        self.allocations.clear();
        self.next_vertex_offset = 0;
        self.next_index_offset = 0;
        self.active_subchunk_count = 0;
    }
}

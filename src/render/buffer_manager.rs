use crate::core::vertex::Vertex;
use crate::core::{BufferAllocation, DrawIndexedIndirect};
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub offset: u32,
    pub size: u32,
}

pub struct RangeAllocator {
    pub free_ranges: Vec<Range>,
    pub max_size: u32,
}

impl RangeAllocator {
    pub fn new(max_size: u32) -> Self {
        Self {
            free_ranges: vec![Range {
                offset: 0,
                size: max_size,
            }],
            max_size,
        }
    }

    pub fn allocate(&mut self, size: u32) -> Option<u32> {
        if size == 0 {
            return Some(0);
        }

        let mut found_idx = None;
        for (i, range) in self.free_ranges.iter().enumerate() {
            if range.size >= size {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            let range = self.free_ranges[idx];
            let offset = range.offset;

            if range.size == size {
                self.free_ranges.remove(idx);
            } else {
                self.free_ranges[idx] = Range {
                    offset: range.offset + size,
                    size: range.size - size,
                };
            }
            Some(offset)
        } else {
            None
        }
    }

    pub fn deallocate(&mut self, offset: u32, size: u32) {
        if size == 0 {
            return;
        }

        // Add range back and try to merge
        self.free_ranges.push(Range { offset, size });
        self.free_ranges.sort_by_key(|r| r.offset);

        let mut merged = Vec::new();
        if let Some(first) = self.free_ranges.first() {
            let mut current = *first;
            for next in self.free_ranges.iter().skip(1) {
                if next.offset == current.offset + current.size {
                    current.size += next.size;
                } else {
                    merged.push(current);
                    current = *next;
                }
            }
            merged.push(current);
        }
        self.free_ranges = merged;
    }
}

pub struct ChunkBufferManager {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub indirect_buffer: wgpu::Buffer,

    pub vertex_allocator: RangeAllocator,
    pub index_allocator: RangeAllocator,
}

impl ChunkBufferManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let max_vertices = 10_000_000; // ~480MB
        let max_indices = 20_000_000; // ~80MB
        let max_draw_calls = 50_000;

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Vertex Buffer"),
            size: (max_vertices * std::mem::size_of::<Vertex>() as u32) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Index Buffer"),
            size: (max_indices * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Indirect Draw Buffer"),
            size: (max_draw_calls * std::mem::size_of::<DrawIndexedIndirect>() as u32) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        Self {
            vertex_buffer,
            index_buffer,
            indirect_buffer,
            vertex_allocator: RangeAllocator::new(max_vertices),
            index_allocator: RangeAllocator::new(max_indices),
        }
    }

    pub fn allocate(
        &mut self,
        queue: &wgpu::Queue,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Option<BufferAllocation> {
        let v_count = vertices.len() as u32;
        let i_count = indices.len() as u32;

        let v_offset = self.vertex_allocator.allocate(v_count)?;
        let i_offset = match self.index_allocator.allocate(i_count) {
            Some(o) => o,
            None => {
                self.vertex_allocator.deallocate(v_offset, v_count);
                return None;
            }
        };

        if !vertices.is_empty() {
            queue.write_buffer(
                &self.vertex_buffer,
                v_offset as u64 * std::mem::size_of::<Vertex>() as u64,
                bytemuck::cast_slice(vertices),
            );
        }
        if !indices.is_empty() {
            queue.write_buffer(
                &self.index_buffer,
                i_offset as u64 * 4,
                bytemuck::cast_slice(indices),
            );
        }

        Some(BufferAllocation {
            index_offset: i_offset,
            index_count: i_count,
            vertex_count: v_count,
            base_vertex: v_offset as i32,
        })
    }

    pub fn deallocate(&mut self, alloc: BufferAllocation) {
        self.vertex_allocator
            .deallocate(alloc.base_vertex as u32, alloc.vertex_count);
        self.index_allocator
            .deallocate(alloc.index_offset, alloc.index_count);
    }
}

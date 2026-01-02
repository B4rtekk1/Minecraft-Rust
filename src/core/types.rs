#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndexedIndirect {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct BufferAllocation {
    pub index_offset: u32,
    pub index_count: u32,
    pub vertex_count: u32,
    pub base_vertex: i32,
}

use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CloudUniforms {
    pub time: f32,
    pub wind_direction: [f32; 2],
    pub sun_direction: [f32; 2],
    pub coverage: f32,
    pub density: f32,
    pub steps: u32,
    pub _padding: [u32; 2],
}

pub fn create_cloud_texture(
    device: &wgpu::Device,
    swapchain_width: u32,
    swapchain_height: u32,
) -> wgpu::Texture {
    let clouds_size = wgpu::Extent3d {
        width: swapchain_width / 2,
        height: swapchain_height / 2,
        depth_or_array_layers: 1,
    };
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("clouds"),
        size: clouds_size,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        dimension: wgpu::TextureDimension::D2,
        mip_level_count: 1,
        sample_count: 1,
        view_formats: &[],
    })
}

pub fn create_cloud_uniform_buffer(
    device: &wgpu::Device,
    uniforms: &CloudUniforms,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Cloud Uniform Buffer"),
        contents: bytemuck::bytes_of(uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

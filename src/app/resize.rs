use glyphon::Resolution;

use super::state::State;

impl State {
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let msaa_sample_count: u32 = 4;
            self.depth_texture =
                Self::create_depth_texture(&self.device, &self.config, msaa_sample_count);
            self.msaa_texture_view = Self::create_msaa_texture(
                &self.device,
                &self.config,
                self.surface_format,
                msaa_sample_count,
            );

            self.ssr_color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SSR Color Texture"),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.ssr_color_view = self
                .ssr_color_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            self.ssr_depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("SSR Depth Texture"),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.ssr_depth_view = self
                .ssr_depth_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            self.ssr_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("SSR Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            });

            self.water_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.water_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.shadow_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.ssr_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&self.ssr_depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&self.ssr_sampler),
                    },
                ],
                label: Some("water_bind_group"),
            });

            self.depth_resolve_bind_group =
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Depth Resolve Bind Group"),
                    layout: &self.depth_resolve_pipeline.get_bind_group_layout(0),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.depth_texture),
                    }],
                });

            self.viewport.update(
                &self.queue,
                Resolution {
                    width: new_size.width,
                    height: new_size.height,
                },
            );

            self.scene_color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Scene Color Texture"),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.scene_color_view = self
                .scene_color_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let composite_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Composite Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let composite_bind_group_layout = self.composite_pipeline.get_bind_group_layout(0);
            self.composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Composite Bind Group"),
                layout: &composite_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.scene_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&composite_sampler),
                    },
                ],
            });

            let new_hiz_size = [new_size.width, new_size.height];
            if new_hiz_size != self.hiz_size {
                self.hiz_size = new_hiz_size;
                let hiz_max_dim = new_size.width.max(new_size.height);
                let hiz_mips_count = (hiz_max_dim as f32).log2().floor() as u32 + 1;
                let hiz_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Hi-Z Texture"),
                    size: wgpu::Extent3d {
                        width: new_hiz_size[0],
                        height: new_hiz_size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: hiz_mips_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let new_hiz_view = hiz_texture.create_view(&wgpu::TextureViewDescriptor::default());
                let new_hiz_mips: Vec<_> = (0..hiz_mips_count)
                    .map(|i| {
                        hiz_texture.create_view(&wgpu::TextureViewDescriptor {
                            label: Some(&format!("Hi-Z Mip View {}", i)),
                            base_mip_level: i,
                            mip_level_count: Some(1),
                            ..Default::default()
                        })
                    })
                    .collect();
                let new_hiz_bind_groups: Vec<_> = (0..hiz_mips_count - 1)
                    .map(|i| {
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Hi-Z Bind Group {}", i)),
                            layout: &self.hiz_bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(
                                        &new_hiz_mips[i as usize],
                                    ),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(
                                        &new_hiz_mips[(i + 1) as usize],
                                    ),
                                },
                            ],
                        })
                    })
                    .collect();
                self.indirect_manager
                    .update_bind_group(&self.device, &new_hiz_view);
                self.water_indirect_manager
                    .update_bind_group(&self.device, &new_hiz_view);
                self.hiz_texture = hiz_texture;
                self.hiz_view = new_hiz_view;
                self.hiz_mips = new_hiz_mips;
                self.hiz_bind_groups = new_hiz_bind_groups;
            }
        }
    }
}

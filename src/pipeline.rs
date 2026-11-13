//! GPU pipeline — single fullscreen draw call via WGSL fragment shader.

use cosmic::iced::Rectangle;
use cosmic::iced_wgpu::graphics::Viewport;
use cosmic::iced_wgpu::primitive::{self, Primitive};
use cosmic::iced_wgpu::wgpu;

// ── Uniforms (CPU→GPU) ─────────────────────────────────────────────

/// Must match the WGSL `Uniforms` struct layout exactly.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub bar_pos: [[f32; 4]; 3],     // 10 positions packed in 3 vec4s
    pub bar_colors: [[f32; 4]; 10], // RGB + padding per bar
    pub bar_half: f32,
    pub blend: f32,
    pub time: f32,
    pub scale: f32, // display scale factor (physical px / logical px)
    pub n: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
    pub k_crt: f32,
    pub fold_angle: f32,      // kaleidoscope axis rotation (slow drift)
    pub fold_zoom: f32,       // zoom factor
    pub fold_count: u32,      // current kaleidoscope fold
    pub prev_fold_count: u32, // previous fold (for crossfade)
    pub fold_blend: f32,      // 0 = prev fold, 1 = current fold
    pub prev_fold_angle: f32, // prev mode's axis angle
    pub zoom_t: f32,          // 0 = fully zoomed (far), 1 = settled at screen
    pub flags: f32,     // bit 1: CRT post-process off
    pub field_mix: f32, // 0 = raster bars, 1 = the field scene
    pub _pad: [f32; 3],
}

// Must be 288 bytes (18 × 16) to match WGSL struct layout.
const _: () = assert!(std::mem::size_of::<Uniforms>() == 288);

impl Uniforms {
    pub fn pack_positions(positions: &[f32; 10]) -> [[f32; 4]; 3] {
        [
            [positions[0], positions[1], positions[2], positions[3]],
            [positions[4], positions[5], positions[6], positions[7]],
            [positions[8], positions[9], 0.0, 0.0],
        ]
    }

    pub fn pack_color(c: cosmic::iced::Color) -> [f32; 4] {
        [c.r, c.g, c.b, 1.0]
    }
}

// ── Primitive ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DreamPrimitive {
    pub uniforms: Uniforms,
}

impl Primitive for DreamPrimitive {
    type Pipeline = DreamPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let mut uniforms = self.uniforms;
        uniforms.scale = viewport.scale_factor() as f32;
        queue.write_buffer(&pipeline.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
        true
    }
}

// ── Pipeline ────────────────────────────────────────────────────────

pub struct DreamPipeline {
    render_pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl primitive::Pipeline for DreamPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dream"),
            source: wgpu::ShaderSource::Wgsl(include_str!("dream.wgsl").into()),
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dream_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dream_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dream_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dream_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dream_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            uniform_buf,
            bind_group,
        }
    }
}

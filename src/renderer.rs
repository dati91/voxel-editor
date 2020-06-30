use crate::camera::CameraWrapper;
use cgmath;
use std::time;
use wgpu;
use winit::{
    event::{self, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

static DEFAULT_MESH_RESOLUTION: u16 = 16;

#[cfg_attr(rustfmt, rustfmt_skip)]
#[allow(unused)]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

pub async fn run_async(event_loop: EventLoop<()>, window: Window) {
    log::info!("Initializing the surface...");

    let instance = wgpu::Instance::new();
    let (size, surface) = unsafe {
        let size = window.inner_size();
        let surface = instance.create_surface(&window);
        (size, surface)
    };

    let adapter = instance
        .request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
        )
        .await
        .unwrap();

    let trace_dir = std::env::var("WGPU_TRACE");
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                extensions: wgpu::Extensions::empty(),
                limits: wgpu::Limits::default(),
            },
            trace_dir.ok().as_ref().map(std::path::Path::new),
        )
        .await
        .unwrap();

    let mut sc_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8Unorm,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };
    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    log::info!("Initializing the Renderer...");
    let mut renderer = Renderer::init(&sc_desc, &device, DEFAULT_MESH_RESOLUTION);

    let mut last_update_inst = time::Instant::now();

    log::info!("Entering render loop...");
    event_loop.run(move |event, _, control_flow| {
        let _ = (&instance, &adapter); // force ownership by the closure
        *control_flow = if cfg!(feature = "metal-auto-capture") {
            ControlFlow::Exit
        } else {
            ControlFlow::WaitUntil(time::Instant::now() + time::Duration::from_millis(10))
        };
        match event {
            event::Event::MainEventsCleared => {
                if last_update_inst.elapsed() > time::Duration::from_millis(20) {
                    window.request_redraw();
                    last_update_inst = time::Instant::now();
                }
            }
            event::Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                log::info!("Resizing to {:?}", size);
                sc_desc.width = size.width;
                sc_desc.height = size.height;
                renderer.resize(&sc_desc, &device, &queue);
            }
            event::Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                        event::KeyboardInput {
                            virtual_keycode: Some(event::VirtualKeyCode::Escape),
                            state: event::ElementState::Pressed,
                            ..
                        },
                    ..
                }
                | WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {
                    renderer.update(event, &sc_desc, &queue);
                }
            },
            event::Event::RedrawRequested(_) => {
                let frame = match swap_chain.get_next_frame() {
                    Ok(frame) => frame,
                    Err(_) => {
                        swap_chain = device.create_swap_chain(&surface, &sc_desc);
                        swap_chain
                            .get_next_frame()
                            .expect("Failed to acquire next swap chain texture!")
                    }
                };

                let command_buf = renderer.render(&frame.output, &device, &queue);
                queue.submit(Some(command_buf));
            }
            _ => {}
        }
    });
}

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    _pos: [f32; 4],
    _col: [f32; 4],
}

unsafe impl Pod for Vertex {}
unsafe impl Zeroable for Vertex {}

fn white_vertex(pos: [f32; 3]) -> Vertex {
    vertex(pos, [1.0; 4])
}

fn vertex(pos: [f32; 3], col: [f32; 4]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1.0],
        _col: [col[0], col[1], col[2], col[3]],
    }
}

fn generate_mesh_vertices(resolution: u16) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertex_data = Vec::new();
    let mut index_data: Vec<u16> = Vec::new();

    // X axis
    vertex_data.push(vertex([0.0, 0.0, 0.0], RED));
    index_data.push((vertex_data.len() - 1) as u16);
    vertex_data.push(vertex([1.0, 0.0, 0.0], RED));
    index_data.push((vertex_data.len() - 1) as u16);

    // Y axis
    vertex_data.push(vertex([0.0, 0.0, 0.0], GREEN));
    index_data.push((vertex_data.len() - 1) as u16);
    vertex_data.push(vertex([0.0, 1.0, 0.0], GREEN));
    index_data.push((vertex_data.len() - 1) as u16);

    // Z axis
    vertex_data.push(vertex([0.0, 0.0, 0.0], BLUE));
    index_data.push((vertex_data.len() - 1) as u16);
    vertex_data.push(vertex([0.0, 0.0, 1.0], BLUE));
    index_data.push((vertex_data.len() - 1) as u16);


    let step = 1.0 / resolution as f32;
    for i in 1..(resolution + 1)
    {
        // bottom
        vertex_data.push(white_vertex([0.0, 0.0 + i as f32 * step, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([1.0, 0.0 + i as f32 * step, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);

        vertex_data.push(white_vertex([0.0 + i as f32 * step, 0.0, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([0.0 + i as f32 * step, 1.0, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);

        // left
        vertex_data.push(white_vertex([0.0, 0.0 + i as f32 * step, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([0.0, 0.0 + i as f32 * step, 1.0]));
        index_data.push((vertex_data.len() - 1) as u16);

        vertex_data.push(white_vertex([0.0, 0.0, 0.0 + i as f32 * step]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([0.0, 1.0, 0.0 + i as f32 * step]));
        index_data.push((vertex_data.len() - 1) as u16);

        // back
        vertex_data.push(white_vertex([0.0 + i as f32 * step, 0.0, 0.0]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([0.0 + i as f32 * step, 0.0, 1.0]));
        index_data.push((vertex_data.len() - 1) as u16);

        vertex_data.push(white_vertex([0.0, 0.0, 0.0 + i as f32 * step]));
        index_data.push((vertex_data.len() - 1) as u16);
        vertex_data.push(white_vertex([1.0, 0.0, 0.0 + i as f32 * step]));
        index_data.push((vertex_data.len() - 1) as u16);
    }

    (vertex_data, index_data)
}

struct Pipeline {
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: usize,
}

impl Pipeline {
    fn draw<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_index_buffer(self.index_buf.slice(..));
        render_pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
        render_pass.draw_indexed(0..self.index_count as u32, 0, 0..1);
    }
}

pub struct Renderer {
    camera: CameraWrapper,
    mesh_pipeline: Pipeline,
    _mesh_resolution: u16,
}

impl Renderer {
    pub fn init(
        sc_desc: &wgpu::SwapChainDescriptor,
        device: &wgpu::Device,
        mesh_resolution: u16,
    ) -> Self {
        use std::mem;

        // Create the vertex and index buffers
        let vertex_size = mem::size_of::<Vertex>();
        let (vertex_data, index_data) = generate_mesh_vertices(mesh_resolution);

        let vertex_buf_mesh = device.create_buffer_with_data(
            bytemuck::cast_slice(&vertex_data),
            wgpu::BufferUsage::VERTEX,
        );

        let index_buf_mesh = device
            .create_buffer_with_data(bytemuck::cast_slice(&index_data), wgpu::BufferUsage::INDEX);

        // Create pipeline layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            bindings: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });

        let mut camera = CameraWrapper::new(sc_desc.width as f32 / sc_desc.height as f32);

        let mx = camera.mvp_matrix(sc_desc.width as f32 / sc_desc.height as f32);
        let mx_ref = mx.as_ref();
        let uniform_buf = device.create_buffer_with_data(
            bytemuck::cast_slice(mx_ref),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        // Create bind group
        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(uniform_buf.slice(..)),
                },
            ],
            label: None,
        });

        // Create the render pipeline
        let vs_bytes = include_bytes!("shader.vert.spv");
        let fs_bytes = include_bytes!("shader.frag.spv");
        let vs_module = device
            .create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&vs_bytes[..])).unwrap());
        let fs_module = device
            .create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&fs_bytes[..])).unwrap());

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::LineList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: sc_desc.format,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: vertex_size as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttributeDescriptor {
                            format: wgpu::VertexFormat::Float4,
                            offset: 0,
                            shader_location: 0,
                        },
                        // Color
                        wgpu::VertexAttributeDescriptor {
                            format: wgpu::VertexFormat::Float4,
                            offset: 4 * 4,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        // Done
        Renderer {
            camera,
            mesh_pipeline: Pipeline {
                pipeline: mesh_pipeline,
                bind_group: mesh_bind_group,
                uniform_buf,
                vertex_buf: vertex_buf_mesh,
                index_buf: index_buf_mesh,
                index_count: index_data.len(),
            },
            _mesh_resolution: mesh_resolution,
        }
    }

    pub fn update(
        &mut self,
        event: winit::event::WindowEvent,
        sc_desc: &wgpu::SwapChainDescriptor,
        queue: &wgpu::Queue,
    ) {
        self.camera.update(&event);
        let mx = self.camera.mvp_matrix(sc_desc.width as f32 / sc_desc.height as f32);
        let mx_ref = mx.as_ref();
        queue.write_buffer(&self.mesh_pipeline.uniform_buf, 0, bytemuck::cast_slice(mx_ref));
    }

    pub fn resize(
        &mut self,
        sc_desc: &wgpu::SwapChainDescriptor,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let mx = self.camera.mvp_matrix(sc_desc.width as f32 / sc_desc.height as f32);
        let mx_ref = mx.as_ref();
        queue.write_buffer(&self.mesh_pipeline.uniform_buf, 0, bytemuck::cast_slice(mx_ref));
    }

    pub fn render(
        &mut self,
        frame: &wgpu::SwapChainTexture,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> wgpu::CommandBuffer {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color {
                        r: 0.0,
                        g: 0.8,
                        b: 1.0,
                        a: 1.0,
                    },
                }],
                depth_stencil_attachment: None,
            });
            self.mesh_pipeline.draw(&mut rpass);
        }

        encoder.finish()
    }
}
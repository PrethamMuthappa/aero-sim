use env_logger::Env;
use log::{info, error};
use winit::{
    event::*,
    event_loop::EventLoop,
    window::WindowBuilder,
};

mod gpu;
mod render;
mod sim;
mod gui;
pub use gpu::{run_compute_test, run_lbm_tests};
pub use render::{Renderer, Tracers};
pub use gui::{GuiState, VizMode};

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Starting Aero 2D Aerodynamics Simulator");
    let run_tests = std::env::args().any(|a| a == "--tests");
    let event_loop = EventLoop::new().unwrap();
    let window = std::sync::Arc::new(
        WindowBuilder::new()
            .with_title("Aero - 2D Aerodynamics Simulator")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 512.0))
            .build(&event_loop)
            .unwrap(),
    );
    info!("Window created successfully");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();
    info!("GPU Adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .unwrap();
    let device = std::sync::Arc::new(device);
    let queue = std::sync::Arc::new(queue);
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    info!("surface format: {:?}", format);
    let size = window.inner_size();
    let mut scfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &scfg);
    if run_tests {
        info!("Running compute + LBM tests...");
        run_compute_test(&device, &queue).unwrap();
        run_lbm_tests(device.clone(), queue.clone()).unwrap();
        info!("All tests passed, exiting");
        return;
    }
    for arg in std::env::args().skip(1) {
        if arg.starts_with("--stability=") {
            let re_max: f32 = arg["--stability=".len()..].parse().unwrap_or(500.0);
            let mask = sim::obstacle::default_circle(gpu::lbm::W, gpu::lbm::H);
            let d = 2.0 * gpu::lbm::H as f32 / 20.0;
            let u = 0.05;
            let mut re = 250.0f32;
            while re <= re_max {
                let nu = u * d / re;
                let tau = (3.0 * nu + 0.5).max(0.51);
                let mut lbm = gpu::lbm::Lbm::new(device.clone(), queue.clone(), u, tau, &mask);
                lbm.step(5000);
                let m = lbm.read_macro();
                let bad = m.iter().any(|v| !v.is_finite());
                let maxu = m.chunks_exact(4).map(|c| (c[1] * c[1] + c[2] * c[2]).sqrt()).fold(0.0f32, f32::max);
                info!("stability Re={:.0} tau={:.4} max|u|={:.4} nan={}", re, tau, maxu, bad);
                if bad || maxu > 0.3 {
                    info!("stability ceiling reached at Re={:.0}", re);
                    return;
                }
                re += 50.0;
            }
            info!("stability: clean through Re={:.0}", re_max);
            return;
        }
        if arg == "--tracer-sample" {
            let mask = sim::obstacle::default_circle(gpu::lbm::W, gpu::lbm::H);
            let mut lbm = gpu::lbm::Lbm::new(device.clone(), queue.clone(), 0.05, 0.536, &mask);
            let mut tracers = Tracers::new(device.clone(), queue.clone(), format, gpu::lbm::W, gpu::lbm::H, &lbm.macro_buf);
            tracers.log_sample(10);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            lbm.dispatch_into(&mut enc);
            tracers.advect_into(&mut enc, 1, 0.05);
            queue.submit([enc.finish()]);
            device.poll(wgpu::Maintain::Wait);
            tracers.log_sample(10);
            return;
        }
        if arg.starts_with("--mask-test=") {
            let path = std::path::Path::new(&arg["--mask-test=".len()..]);
            match sim::obstacle::from_png(path, gpu::lbm::W, gpu::lbm::H) {
                Ok(o) => info!(
                    "mask-test {}: {} solid cells of {}",
                    o.name,
                    o.mask.iter().filter(|&&v| v == 1).count(),
                    o.mask.len()
                ),
                Err(e) => info!("mask-test {} FAILED: {}", path.display(), e),
            }
            return;
        }
    }
    let mut gui_state = GuiState::default();
    let tau = gui_state.tau(2.0 * gpu::lbm::H as f32 / 20.0);
    let mask = sim::obstacle::default_circle(gpu::lbm::W, gpu::lbm::H);
    let mut lbm = gpu::lbm::Lbm::new(device.clone(), queue.clone(), gui_state.wind_speed, tau, &mask);
    let renderer = Renderer::new(device.clone(), queue.clone(), format, gpu::lbm::W, gpu::lbm::H, &lbm.macro_buf);
    let mut tracers = Tracers::new(device.clone(), queue.clone(), format, gpu::lbm::W, gpu::lbm::H, &lbm.macro_buf);
    let egui_ctx = egui::Context::default();
    let mut egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::viewport::ViewportId::ROOT,
        &window,
        Some(window.scale_factor() as f32),
        None,
    );
    let mut egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1);
    info!("Renderer ready, entering frame loop");
    let mut frames: u64 = 0;
    let mut t0 = std::time::Instant::now();
    let mut last_wind = gui_state.wind_speed;
    let mut last_re = gui_state.reynolds;
    let mut last_viz = gui_state.visualization;
    renderer.set_viz(last_viz.as_u32(), (2.0 * last_wind).max(0.02), gpu::lbm::W, gpu::lbm::H);
    let r = event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event: ref we, .. } = event {
            let _ = egui_winit.on_window_event(&window, we);
        }
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                elwt.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(ns),
                window_id,
            } if window_id == window.id() => {
                scfg.width = ns.width.max(1);
                scfg.height = ns.height.max(1);
                surface.configure(&device, &scfg);
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == window.id() => {
                if (gui_state.wind_speed - last_wind).abs() > f32::EPSILON
                    || (gui_state.reynolds - last_re).abs() > f32::EPSILON
                {
                    lbm.set_u_inlet(gui_state.wind_speed);
                    lbm.set_tau(gui_state.tau(lbm.char_len()));
                    renderer.set_viz(
                        gui_state.visualization.as_u32(),
                        (2.0 * gui_state.wind_speed).max(0.02),
                        gpu::lbm::W,
                        gpu::lbm::H,
                    );
                    last_wind = gui_state.wind_speed;
                    last_re = gui_state.reynolds;
                    last_viz = gui_state.visualization;
                } else if gui_state.visualization != last_viz {
                    renderer.set_viz(
                        gui_state.visualization.as_u32(),
                        (2.0 * gui_state.wind_speed).max(0.02),
                        gpu::lbm::W,
                        gpu::lbm::H,
                    );
                    last_viz = gui_state.visualization;
                }
                if gui_state.reset_requested {
                    lbm.init_equilibrium(gui_state.wind_speed);
                    gui_state.total_steps = 0;
                    gui_state.reset_requested = false;
                }
                let raw_input = egui_winit.take_egui_input(&window);
                let full_output = egui_ctx.run(raw_input, |ctx| {
                    egui::SidePanel::right("controls")
                        .resizable(false)
                        .exact_width(220.0)
                        .show(ctx, |ui| {
                            ui.heading("Aero 2D");
                            ui.separator();
                            ui.label("Obstacle");
                            ui.label(format!("Current: {}", gui_state.obstacle_name));
                            ui.horizontal(|ui| {
                                if ui.button("Load PNG...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Images", &["png", "jpg", "jpeg"])
                                        .pick_file()
                                    {
                                        gui_state.pending_obstacle_load = Some(path);
                                    }
                                }
                                if ui.button("Circle").clicked() {
                                    gui_state.pending_obstacle_load =
                                        Some(std::path::PathBuf::from("__circle__"));
                                }
                            });
                            ui.separator();
                            ui.label("Wind speed (lattice)");
                            ui.add(egui::Slider::new(&mut gui_state.wind_speed, 0.01..=0.10).fixed_decimals(3));
                            ui.label("Reynolds number");
                            ui.add(egui::Slider::new(&mut gui_state.reynolds, 10.0..=500.0).logarithmic(true));
                            ui.label("Steps per frame");
                            ui.add(egui::Slider::new(&mut gui_state.steps_per_frame, 1..=10));
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button(if gui_state.paused { "Resume" } else { "Pause" }).clicked() {
                                    gui_state.paused = !gui_state.paused;
                                }
                                if ui.button("Reset").clicked() {
                                    gui_state.reset_requested = true;
                                }
                            });
                            ui.separator();
                            ui.label("Visualization");
                            ui.radio_value(&mut gui_state.visualization, VizMode::VelocityMagnitude, "Velocity magnitude");
                            ui.radio_value(&mut gui_state.visualization, VizMode::Pressure, "Pressure");
                            ui.radio_value(&mut gui_state.visualization, VizMode::Vorticity, "Vorticity");
                            ui.separator();
                            ui.checkbox(&mut gui_state.show_tracers, "Particle tracers");
                            ui.separator();
                            ui.label(format!("FPS: {:.1}", gui_state.fps));
                            ui.label(format!("Steps: {}", gui_state.total_steps));
                            ui.label(format!("Grid: {}x{}", gpu::lbm::W, gpu::lbm::H));
                        });
                });
                egui_winit.handle_platform_output(&window, full_output.platform_output);
                if let Some(path) = gui_state.pending_obstacle_load.take() {
                    if path.to_string_lossy() == "__circle__" {
                        let obs = sim::obstacle::from_circle(gpu::lbm::W, gpu::lbm::H);
                        lbm.set_mask(&obs.mask);
                        lbm.init_equilibrium(gui_state.wind_speed);
                        gui_state.total_steps = 0;
                        gui_state.obstacle_name = obs.name.clone();
                        log::info!("Loaded obstacle: circle");
                    } else {
                        match sim::obstacle::from_png(&path, gpu::lbm::W, gpu::lbm::H) {
                            Ok(obs) => {
                                lbm.set_mask(&obs.mask);
                                lbm.init_equilibrium(gui_state.wind_speed);
                                gui_state.total_steps = 0;
                                gui_state.obstacle_name = obs.name.clone();
                                log::info!(
                                    "Loaded obstacle: {} ({} solid cells)",
                                    obs.name,
                                    obs.mask.iter().filter(|&&v| v == 1).count()
                                );
                            }
                            Err(e) => {
                                log::warn!("Failed to load obstacle, keeping '{}': {}", gui_state.obstacle_name, e);
                            }
                        }
                    }
                }
                let frame = match surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost) => {
                        surface.configure(&device, &scfg);
                        return;
                    }
                    Err(e) => {
                        error!("surface error: {:?}", e);
                        return;
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                if !gui_state.paused {
                    for _ in 0..gui_state.steps_per_frame {
                        lbm.dispatch_into(&mut enc);
                    }
                    gui_state.total_steps += gui_state.steps_per_frame as u64;
                }
                if !gui_state.paused && gui_state.show_tracers {
                    tracers.advect_into(&mut enc, gui_state.steps_per_frame, gui_state.wind_speed);
                }
                {
                    let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    renderer.draw(&mut rpass);
                    if gui_state.show_tracers {
                        tracers.draw(&mut rpass);
                    }
                }
                let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                let screen_desc = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [scfg.width, scfg.height],
                    pixels_per_point: window.scale_factor() as f32,
                };
                for (id, delta) in &full_output.textures_delta.set {
                    egui_renderer.update_texture(&device, &queue, *id, delta);
                }
                egui_renderer.update_buffers(&device, &queue, &mut enc, &paint_jobs, &screen_desc);
                {
                    let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("gui"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    egui_renderer.render(&mut rpass, &paint_jobs, &screen_desc);
                }
                for id in &full_output.textures_delta.free {
                    egui_renderer.free_texture(id);
                }
                queue.submit([enc.finish()]);
                frame.present();
                frames += 1;
                let dt = t0.elapsed();
                if dt.as_secs() >= 1 {
                    gui_state.fps = frames as f64 / dt.as_secs_f64();
                    info!("FPS: {:.0}", gui_state.fps);
                    frames = 0;
                    t0 = std::time::Instant::now();
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => (),
        }
    });
    let _ = r;
}

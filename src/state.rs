use crate::{
    config::{Config, DisplayGroup, RenderSource, RenderTarget, ResizeKind},
    display::Display,
    region::{Region, TupleVecExt},
};
use image::{imageops::FilterType, RgbaImage};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::{Shm, ShmHandler},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use wayland_client::{
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

pub struct State {
    pub config: Config,

    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub shm: Shm,

    pub exit: bool,
    pub first_configure: bool,
    pub pointer: Option<wl_pointer::WlPointer>,

    pub displays: HashMap<String, Display>,
    pub render_pass_resizes: HashMap<usize, RgbaImage>,
    pub render_pass_rotate_index: HashMap<usize, Arc<AtomicUsize>>,
}

impl State {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        for (index, pass) in self.config.render_passes.iter().enumerate() {
            let image = match &pass.source {
                RenderSource::Single(image) => image,
                RenderSource::Many { images, .. } => &{
                    let rotate_index = self.render_pass_rotate_index.get(&index).unwrap();
                    &images[rotate_index.load(Ordering::Acquire) % images.len()]
                },
            };

            let image = &self.config.images.get(image).unwrap().image;

            let total_region = match &pass.target {
                RenderTarget::Display(d) => self.displays.get(d).unwrap().region,
                RenderTarget::Group(g) => self
                    .group_region(self.config.groups.get(g).unwrap())
                    .unwrap(),
            };

            // TODO: cache resize results
            // if true {
            // !self.render_pass_resizes.contains_key(&index) {
            self.render_pass_resizes.insert(
                index,
                match pass.resize {
                    ResizeKind::Cover => {
                        let original_dims = image.dimensions().to_vec2().map(From::from);

                        let scale = total_region
                            .dim
                            .zip(original_dims, |l, r| l as f64 / r as f64);
                        let scale = f64::max(scale.x, scale.y);

                        let new_dims = (original_dims * scale).map(|i| i.round() as u32);

                        let temp_image = image::imageops::resize(
                            image,
                            new_dims.x,
                            new_dims.y,
                            FilterType::Nearest,
                        );

                        image::imageops::crop_imm(
                            &temp_image,
                            ((new_dims.x as i32 - total_region.dim.x) / 2).max(0) as u32,
                            ((new_dims.y as i32 - total_region.dim.y) / 2).max(0) as u32,
                            total_region.dim.x as u32,
                            total_region.dim.y as u32,
                        )
                        .to_image()
                    }
                    ResizeKind::Stretch => image::imageops::resize(
                        image,
                        total_region.dim.x as u32,
                        total_region.dim.y as u32,
                        FilterType::Nearest,
                    ),
                },
            );
            // }
            let scaled_image = self.render_pass_resizes.get(&index).unwrap();

            if let RenderTarget::Display(s) = &pass.target {
                let display = self.displays.get_mut(s).unwrap();

                display.draw(qh, &scaled_image, total_region);
            } else if let RenderTarget::Group(s) = &pass.target {
                let group = self.config.groups.get(s).unwrap();

                for display in &group.displays {
                    let display = self.displays.get_mut(display).unwrap();
                    display.draw(qh, &scaled_image, total_region);
                }
            }
        }
    }

    pub fn group_region(&self, group: &DisplayGroup) -> Option<Region> {
        group
            .displays
            .iter()
            .map(|d| self.displays.get(d).unwrap().region)
            .fold(None, |r, n| Some(r.map_or(n, |r| r.combine(n))))
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        self.displays.retain(|_, v| v.layer.0 != *layer);
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        for (_id, disp) in &mut self.displays {
            if disp.layer.0 != *layer {
                continue;
            }

            disp.damaged.store(true, Ordering::Release);
            disp.first = false;
        }

        self.draw(qh);
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for State {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        _events: &[PointerEvent],
    ) {
    }
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_layer!(State);
delegate_compositor!(State);
delegate_output!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_pointer!(State);
delegate_registry!(State);

use cgmath::Vector2;
use image::DynamicImage;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputInfo, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::{Shm, ShmHandler},
};
use std::collections::HashMap;
use wayland_client::{
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

use crate::{
    config::{Config, RenderSource, RenderTarget, ResizeKind},
    display::Display,
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

    pub min: Vector2<i32>,
    pub max: Vector2<i32>,

    pub displays: HashMap<String, Display>,
}

impl State {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        for pass in &self.config.render_passes {
            let RenderSource::Single(image) = &pass.source;
            let image = self.config.images.get(image).unwrap();

            if let RenderTarget::Display(s) = &pass.target {
                let display = self.displays.get_mut(s).unwrap();

                display.draw(qh, &image.image, pass.resize);
            }
            if let RenderTarget::Group(s) = &pass.target {}

            // use ResizeKind::*;
            // match pass.resize {
            //     None => {

            //     },
            //     Cover => {
            //         let dyn_image = DynamicImage::ImageRgba8(image.clone);

            //         dyn_image.resize(nwidth, nheight, filter)
            //         DynamicImage::resize(&self, , nheight, filter
            //     },
            //     Stretch => {
            //         DynamicImage::resize_exact(&self, nwidth, nheight, filter)
            //     },
            // }
        }

        // for (_, disp) in &mut self.displays {
        //     disp.draw(qh, self.min, self.max);
        // }
    }

    // pub fn group_region(&self, group: String) ->
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
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        for (_id, disp) in &mut self.displays {
            if disp.layer.0 != *layer {
                continue;
            }

            // disp.width = NonZeroU32::new(configure.new_size.0).map_or(256, NonZeroU32::get);
            // disp.height = NonZeroU32::new(configure.new_size.1).map_or(256, NonZeroU32::get);
            disp.damaged = true;
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
            println!("Set pointer capability");
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
            println!("Unset pointer capability");
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
        events: &[PointerEvent],
    ) {
        for event in events {
            println!("{:?}", event.position);
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    println!("Pointer entered @{:?}", event.position);
                }
                PointerEventKind::Leave { .. } => {
                    println!("Pointer left");
                }
                _ => (),
            }
        }
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

pub fn print_output(info: &OutputInfo) {
    println!("{}", info.model);

    if let Some(name) = info.name.as_ref() {
        println!("\tname: {name}");
    }

    if let Some(description) = info.description.as_ref() {
        println!("\tdescription: {description}");
    }

    println!("\tmake: {}", info.make);
    println!("\tx: {}, y: {}", info.location.0, info.location.1);
    println!("\tsubpixel: {:?}", info.subpixel);
    println!(
        "\tphysical_size: {}×{}mm",
        info.physical_size.0, info.physical_size.1
    );
    if let Some((x, y)) = info.logical_position.as_ref() {
        println!("\tlogical x: {x}, y: {y}");
    }
    if let Some((width, height)) = info.logical_size.as_ref() {
        println!("\tlogical width: {width}, height: {height}");
    }
    println!("\tmodes:");

    for mode in &info.modes {
        println!("\t\t{mode}");
    }
}

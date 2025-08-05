use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        WaylandSurface,
    },
    shm::{multi::MultiPool, slot::SlotPool, Shm},
};
use state::State;
use wayland_client::{globals::registry_queue_init, Connection};

use crate::display::Display;

pub mod display;
pub mod state;

fn main() {
    env_logger::init();

    // All Wayland apps start by connecting the compositor (server).
    let conn = Connection::connect_to_env().unwrap();

    // Enumerate the list of globals to get the protocols the server implements.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let registry_state = RegistryState::new(&globals);
    let seat_state = SeatState::new(&globals, &qh);
    let output_state = OutputState::new(&globals, &qh);

    let mut state = State {
        registry_state,
        seat_state,
        output_state,
        compositor_state,
        shm,
        first_configure: true,
        exit: false,
        pointer: None,
        displays: HashMap::new(),
    };

    event_queue.roundtrip(&mut state).unwrap();

    for output in state.output_state.outputs() {
        let info = &state.output_state.info(&output).unwrap();

        let surface = state.compositor_state.create_surface(&qh);
        let layer = layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Bottom,
            Some(format!("wanipaper_layer_{}", info.id)),
            Some(&output),
        );
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_anchor(Anchor::all());

        let (width, height) = info.logical_size.unwrap();
        let (x, y) = info.logical_position.unwrap();

        layer.set_size(width as u32, height as u32);
        layer.commit();
        let pool = MultiPool::new(&state.shm).unwrap();

        state.displays.insert(
            info.id,
            Display {
                id: info.id,
                layer: (layer, 0),
                pool,
                buffer: None,
                width: width as u32,
                height: height as u32,
                x,
                y,
                first: true,
                damaged: true,
            },
        );
    }

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        if state.exit {
            break;
        }
    }
}

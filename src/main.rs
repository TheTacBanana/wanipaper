use crate::{config::Config, display::Display};
use cgmath::{Vector2, Zero};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        WaylandSurface,
    },
    shm::{multi::MultiPool, Shm},
};
use state::State;
use std::collections::HashMap;
use wayland_client::{globals::registry_queue_init, Connection};

pub mod config;
pub mod display;
pub mod region;
pub mod state;

fn main() {
    env_logger::init();

    let config = match Config::load("./wani.config") {
        Ok(c) => c,
        Err(e) => {
            println!("{e}");
            return;
        }
    };

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
        config,
        registry_state,
        seat_state,
        output_state,
        compositor_state,
        shm,
        first_configure: true,
        exit: false,
        pointer: None,
        displays: HashMap::new(),
        min: Vector2::zero(),
        max: Vector2::zero(),
    };

    event_queue.roundtrip(&mut state).unwrap();

    {
        let display_map = state
            .config
            .displays
            .iter()
            .map(|(n, d)| (&d.name, n))
            .collect::<HashMap<_, _>>();

        for output in state.output_state.outputs() {
            let info = &state.output_state.info(&output).unwrap();

            // Skip if display has no name
            // TODO: Support other identification methods
            if info.name.is_none() {
                continue;
            }

            let Some(&name) = display_map.get(info.name.as_ref().unwrap()) else {
                continue;
            };

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

            let min = Vector2::new(x, y);
            let max = Vector2::new(x + width, y + height);
            let dim = Vector2::new(width, height);

            layer.set_size(width as u32, height as u32);
            layer.commit();
            let pool = MultiPool::new(&state.shm).unwrap();

            state.displays.insert(
                name.clone(),
                Display {
                    layer: (layer, 0),
                    pool,
                    first: true,
                    damaged: true,
                    min,
                    max,
                    dim,
                    transform: info.transform,
                },
            );
        }
    }

    println!("Final {:?} {:?}", state.min, state.max);

    loop {
        event_queue.blocking_dispatch(&mut state).unwrap();

        if state.exit {
            break;
        }
    }
}

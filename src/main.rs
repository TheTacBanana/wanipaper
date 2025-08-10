use crate::{
    config::{Config, RenderSource, RenderTarget},
    display::Display,
    region::Region,
};
use cgmath::Vector2;
use log::{error, info};
use mq::EventKind;
use nix::{errno::Errno, sys::epoll::*};
use rand::random_range;
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
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use wayland_client::{globals::registry_queue_init, Connection};

pub mod config;
pub mod display;
pub mod mq;
pub mod region;
pub mod state;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_target(false)
        .format_timestamp(None)
        .format_module_path(true)
        .init();

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            error!("{e}");
            return;
        }
    };

    info!("config loaded");

    let epoll = Epoll::new(EpollCreateFlags::empty()).unwrap();
    let (mq_send, mq_recv) = mq::new::<()>(&epoll, EventKind::Mq as u64).unwrap();

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
        render_pass_resizes: HashMap::new(),
        render_pass_rotate_index: HashMap::new(),
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

            layer.set_size(width as u32, height as u32);
            layer.commit();
            let pool = MultiPool::new(&state.shm).unwrap();

            state.displays.insert(
                name.clone(),
                Display {
                    layer: (layer, 0),
                    pool,
                    first: true,
                    damaged: Arc::new(AtomicBool::new(true)),
                    region: Region::new(min, max),
                },
            );
        }
    }

    let mut threads = Vec::new();
    for (index, pass) in state.config.render_passes.iter().enumerate() {
        if let RenderSource::Many {
            images,
            rotate: Some(timing),
            rand,
            ..
        } = &pass.source
        {
            let len = images.len();
            let atomic = Arc::new(AtomicUsize::new(0));
            let timing = *timing;
            let rand = *rand;
            let send = mq_send.clone();

            let mut targets = Vec::new();
            match &pass.target {
                RenderTarget::Display(display) => {
                    targets.push(state.displays.get(display).unwrap().damaged.clone());
                }
                RenderTarget::Group(group) => {
                    for display in &state.config.groups.get(group).unwrap().displays {
                        targets.push(state.displays.get(display).unwrap().damaged.clone());
                    }
                }
            }

            state.render_pass_rotate_index.insert(index, atomic.clone());

            threads.push(std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(timing as u64));
                atomic.fetch_add(
                    if rand { random_range(1..len) } else { 1 },
                    Ordering::Acquire,
                );

                send.send(()).expect("Failed to redraw");

                for damaged in targets.iter() {
                    damaged.store(true, Ordering::Release);
                }
            }));
        }
    }

    while !state.exit {
        event_queue.flush().unwrap();

        let wayland_read_guard = if let Some(wayland_read_guard) = event_queue.prepare_read() {
            wayland_read_guard
        } else {
            event_queue.dispatch_pending(&mut state).unwrap();
            event_queue
                .prepare_read()
                .expect("unknown wayland event queue error")
        };

        epoll
            .add(
                wayland_read_guard.connection_fd(),
                EpollEvent::new(EpollFlags::EPOLLIN, EventKind::Wayland as u64),
            )
            .unwrap();

        let mut events = [EpollEvent::empty()];
        let ret = epoll.wait(&mut events, EpollTimeout::NONE);

        epoll.delete(wayland_read_guard.connection_fd()).unwrap();

        let event = match ret {
            Ok(_) => events[0],
            Err(Errno::EINTR) => continue,
            Err(err) => Err(err).unwrap(),
        };

        match event.data().into() {
            EventKind::Mq => {
                std::mem::drop(wayland_read_guard);
                let _ = mq_recv.recv().unwrap();
                state.draw(&qh);
            }
            EventKind::Wayland => {
                if wayland_read_guard.read().is_ok() {
                    event_queue.dispatch_pending(&mut state).unwrap();
                }
            }
            EventKind::Unknown => error!("unknown event queue msg"),
        }
    }
}

use crate::state::State;
use smithay_client_toolkit::{
    shell::{wlr_layer::LayerSurface, WaylandSurface},
    shm::{
        multi::MultiPool,
        slot::{Buffer, SlotPool},
    },
};
use wayland_client::{protocol::wl_shm, QueueHandle};

pub struct Display {
    pub id: u32,
    pub buffer: Option<Buffer>,

    pub layer: (LayerSurface, usize),
    pub pool: MultiPool<(LayerSurface, usize)>,

    pub first: bool,
    pub damaged: bool,

    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

impl Display {
    pub fn draw(&mut self, qh: &QueueHandle<State>) {
        if self.first || !self.damaged {
            println!("Skipped first {} not damaged {}", self.first, !self.damaged);
            return;
        }

        let width = self.width;
        let height = self.height;
        let stride = self.width as i32 * 4;

        let layer = &self.layer.0;

        for i in 0..2 {
            self.layer.1 = i;
            if let Ok((offset, buffer, slice)) = self.pool.create_buffer(
                width as i32,
                stride,
                height as i32,
                &self.layer,
                wl_shm::Format::Argb8888,
            ) {
                {
                    let shift = 0;
                    slice
                        .chunks_exact_mut(4)
                        .enumerate()
                        .for_each(|(index, chunk)| {
                            let x = ((index + shift as usize) % width as usize) as u32;
                            let y = (index / width as usize) as u32;

                            let a = 0xFF;
                            let r = u32::min(
                                ((width - x) * 0xFF) / width,
                                ((height - y) * 0xFF) / height,
                            );
                            let g = u32::min((x * 0xFF) / width, ((height - y) * 0xFF) / height);
                            let b = u32::min(((width - x) * 0xFF) / width, (y * 0xFF) / height);
                            let color = (a << 24) + (r << 16) + (g << 8) + b;

                            let array: &mut [u8; 4] = chunk.try_into().unwrap();
                            *array = color.to_le_bytes();
                        });
                }

                // Damage the entire window
                self.layer
                    .0
                    .wl_surface()
                    .damage_buffer(0, 0, width as i32, height as i32);

                // Request our next frame
                self.layer
                    .0
                    .wl_surface()
                    .frame(qh, self.layer.0.wl_surface().clone());

                self.layer.0.attach(Some(buffer), 0, 0);

                // // Attach and commit to present.
                // buffer
                //     .attach_to(self.layer.0.wl_surface())
                //     .expect("buffer attach");
                self.layer.0.commit();

                return;
            }
        }

        // let buffer = self.buffer.get_or_insert_with(|| {
        //     println!("create new buffer");
        //     self.pool
        //         .create_buffer(
        //             width as i32,
        //             height as i32,
        //             stride,
        //             wl_shm::Format::Argb8888,
        //         )
        //         .expect("create buffer")
        //         .0
        // });

        // let canvas = match self.pool.canvas(buffer) {
        //     Some(canvas) => canvas,
        //     None => {
        //         // panic!();
        //         // This should be rare, but if the compositor has not released the previous
        //         // buffer, we need double-buffering.
        //         println!("create new buffer");
        //         let (second_buffer, canvas) = self
        //             .pool
        //             .create_buffer(
        //                 self.width as i32,
        //                 self.height as i32,
        //                 stride,
        //                 wl_shm::Format::Argb8888,
        //             )
        //             .expect("create buffer");
        //         *buffer = second_buffer;
        //         canvas
        //     }
        // };

        // Draw to the window:

        // TODO save and reuse buffer when the window size is unchanged.  This is especially
        // useful if you do damage tracking, since you don't need to redraw the undamaged parts
        // of the canvas.
    }
}

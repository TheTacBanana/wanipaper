use crate::{config::ResizeKind, state::State};
use cgmath::Vector2;
use image::{DynamicImage, RgbaImage};
use smithay_client_toolkit::{
    shell::{wlr_layer::LayerSurface, WaylandSurface},
    shm::multi::MultiPool,
};
use wayland_client::{
    protocol::{wl_output::Transform, wl_shm},
    QueueHandle,
};

pub struct Display {
    pub layer: (LayerSurface, usize),
    pub pool: MultiPool<(LayerSurface, usize)>,

    pub first: bool,
    pub damaged: bool,

    pub min: Vector2<i32>,
    pub max: Vector2<i32>,
    pub dim: Vector2<i32>,
    pub transform: Transform,
}

impl Display {
    pub fn draw(&mut self, qh: &QueueHandle<State>, image: &RgbaImage, resize: ResizeKind) {
        if self.first || !self.damaged {
            return;
        }

        let layer = &self.layer.0;

        for i in 0..2 {
            self.layer.1 = i;
            let Ok((_offset, buffer, canvas)) = self.pool.create_buffer(
                self.dim.x,
                self.dim.x * 4,
                self.dim.y,
                &self.layer,
                wl_shm::Format::Argb8888,
            ) else {
                continue;
            };

            let image = image::imageops::resize(
                image,
                self.dim.x as u32,
                self.dim.y as u32,
                image::imageops::FilterType::Nearest,
            );

            for (pixel, argb) in image.pixels().zip(canvas.chunks_exact_mut(4)) {
                argb[3] = pixel.0[3];
                argb[2] = pixel.0[0];
                argb[1] = pixel.0[1];
                argb[0] = pixel.0[2];
            }

            // let len = canvas.len() as i32;
            // canvas
            //     .chunks_exact_mut(4)
            //     .enumerate()
            //     .for_each(|(index, chunk)| {
            //         let index = index as i32;

            //         // Local position
            //         let pos = Vector2::new(
            //             self.min.x + index % self.dim.x,
            //             self.min.y + index / self.dim.x,
            //         );

            //         // Normalised global position
            //         let global_dim = max - min;
            //         let Vector2 { x, y } = pos - min;

            //         let width = global_dim.x;
            //         let height = global_dim.y;

            //         if index == 0 || index == (len / 4) - 1 {
            //             println!("{index} {:?} {:?} {:?}", pos, self.dim, global_dim);
            //         }

            //         let a = 0xFF;
            //         let r = i32::min(((width - x) * 0xFF) / width, ((height - y) * 0xFF) / height);
            //         let g = i32::min((x * 0xFF) / width, ((height - y) * 0xFF) / height);
            //         let b = i32::min(((width - x) * 0xFF) / width, (y * 0xFF) / height);
            //         // let r = (x * 255) / global_dim.x;
            //         // let g = if y % 100 == 0 { 255 } else { 0 };
            //         // let b = if x % 100 == 0 { 255 } else { 0 };
            //         let color = (a << 24) + (r << 16) + (g << 8) + b;
            //         // if index == 0 {
            //         //     color = i32::MAX;
            //         // }

            //         let array: &mut [u8; 4] = chunk.try_into().unwrap();
            //         *array = color.to_le_bytes();
            //     });

            // Damage the entire window
            layer
                .wl_surface()
                .damage_buffer(0, 0, self.dim.x, self.dim.y);
            // Request our next frame
            layer
                .wl_surface()
                .frame(qh, self.layer.0.wl_surface().clone());
            layer.attach(Some(buffer), 0, 0);
            layer.commit();

            self.damaged = false;

            return;
        }
    }

    // pub fn convert_coordinates(&self) -> Vector2<i32> {}
}

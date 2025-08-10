use crate::{region::Region, state::State};
use image::RgbaImage;
use smithay_client_toolkit::{
    shell::{wlr_layer::LayerSurface, WaylandSurface},
    shm::multi::MultiPool,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wayland_client::{protocol::wl_shm, QueueHandle};

pub struct Display {
    pub layer: (LayerSurface, usize),
    pub pool: MultiPool<(LayerSurface, usize)>,
    pub first: bool,
    pub damaged: Arc<AtomicBool>,
    pub region: Region,
}

impl Display {
    pub fn draw(&mut self, qh: &QueueHandle<State>, image: &RgbaImage, total: Region) {
        if self.first || !self.damaged.load(Ordering::Acquire) {
            return;
        }

        let layer = &self.layer.0;

        for i in 0..2 {
            self.layer.1 = i;
            let Ok((_offset, buffer, canvas)) = self.pool.create_buffer(
                self.region.dim.x,
                self.region.dim.x * 4,
                self.region.dim.y,
                &self.layer,
                wl_shm::Format::Argb8888,
            ) else {
                continue;
            };

            if self.region == total {
                for (pixel, argb) in image.pixels().zip(canvas.chunks_exact_mut(4)) {
                    argb[3] = pixel.0[3];
                    argb[2] = pixel.0[0];
                    argb[1] = pixel.0[1];
                    argb[0] = pixel.0[2];
                }
            } else {
                let image = image::imageops::crop_imm(
                    image,
                    (self.region.min.x - total.min.x) as u32,
                    (self.region.min.y - total.min.y) as u32,
                    self.region.dim.x as u32,
                    self.region.dim.y as u32,
                )
                .to_image();

                for (pixel, argb) in image.pixels().zip(canvas.chunks_exact_mut(4)) {
                    argb[3] = pixel.0[3];
                    argb[2] = pixel.0[0];
                    argb[1] = pixel.0[1];
                    argb[0] = pixel.0[2];
                }
            }

            // Damage the entire window
            layer
                .wl_surface()
                .damage_buffer(0, 0, self.region.dim.x, self.region.dim.y);
            // Request our next frame
            layer
                .wl_surface()
                .frame(qh, self.layer.0.wl_surface().clone());
            layer.attach(Some(buffer), 0, 0);
            layer.commit();

            self.damaged.store(false, Ordering::Release);

            return;
        }
    }
}

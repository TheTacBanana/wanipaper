use cgmath::Vector2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub min: Vector2<i32>,
    pub max: Vector2<i32>,
    pub dim: Vector2<i32>,
}

impl Region {
    pub fn new(min: Vector2<i32>, max: Vector2<i32>) -> Self {
        Region {
            min,
            max,
            dim: max - min,
        }
    }

    pub fn combine(self, rhs: Region) -> Region {
        let min = self.min.zip(rhs.min, i32::min);
        let max = self.max.zip(rhs.max, i32::max);
        Region {
            min,
            max,
            dim: max - min,
        }
    }
}

pub trait TupleVecExt {
    fn to_vec2(self) -> Vector2<i32>;
}

impl TupleVecExt for (u32, u32) {
    fn to_vec2(self) -> Vector2<i32> {
        Vector2::new(self.0 as i32, self.1 as i32)
    }
}

impl TupleVecExt for (i32, i32) {
    fn to_vec2(self) -> Vector2<i32> {
        Vector2::new(self.0, self.1)
    }
}

use crate::math::Vec3;

pub struct Mat4 {
    pub cell: [f32; 16]
}

impl Mat4 {
    pub fn new() -> Mat4 {
        Mat4 {
            cell: [0.0; 16]
        }
    }

    pub fn identity() -> Mat4 {
        let mut mat = Self::new();
        mat.cell[0] = 1.0;
        mat.cell[5] = 1.0;
        mat.cell[10] = 1.0;
        mat.cell[15] = 1.0;

        mat
    }

    pub fn rotate(vec: Vec3, a: f32) -> Mat4 {
        let mut m = Self::new();

        let u = vec.x;
        let v = vec.y;
        let w = vec.z;
        let ca = a.cos();
        let sa = a.sin();
        m.cell[0] = u * u + (v * v + w * w) * ca;
        m.cell[1] = u * v * (1.0 - ca) - w * sa;
        m.cell[2] = u * w * (1.0 - ca) + v * sa;
        m.cell[4] = u * v * (1.0 - ca) + w * sa;
        m.cell[5] = v * v + (u * u + w * w) * ca;
        m.cell[6] = v * w * (1.0 - ca) - u * sa;
        m.cell[8] = u * w * (1.0 - ca) - v * sa;
        m.cell[9] = v * w * (1.0 - ca) + u * sa;
        m.cell[10] = w * w + (u * u + v * v) * ca;
        m.cell[3] = 0.0;
        m.cell[7] = 0.0;
        m.cell[11] = 0.0;
        m.cell[12] = 0.0;
        m.cell[13] = 0.0;
        m.cell[14] = 0.0;
        m.cell[15] = 1.0;

        m
    }

    pub fn rotate_x(a: f32) -> Mat4 {
        let mut m = Self::new();

        let ca = a.cos();
        let sa = a.sin();
        m.cell[5] = ca;
        m.cell[6] = -sa;
        m.cell[9] = sa;
        m.cell[10] = ca;

        m
    }

    pub fn rotate_y(a: f32) -> Mat4 {
        let mut m = Self::new();

        let ca = a.cos();
        let sa = a.sin();
        m.cell[0] = ca;
        m.cell[2] = sa;
        m.cell[8] = -sa;
        m.cell[10] = ca;

        m
    }

    pub fn rotate_z(a: f32) -> Mat4 {
        let mut m = Self::new();

        let ca = a.cos();
        let sa = a.sin();
        m.cell[0] = ca;
        m.cell[1] = -sa;
        m.cell[4] = sa;
        m.cell[5] = ca;

        m
    }

    pub fn invert(&mut self) {
        // from MESA, via http://stackoverflow.com/questions/1148309/inverting-a-4x4-matrix
        let inv: [f32; 16] = [
            self.cell[5] * self.cell[10] * self.cell[15] - self.cell[5] * self.cell[11] * self.cell[14] - self.cell[9] * self.cell[6] * self.cell[15] +
                self.cell[9] * self.cell[7] * self.cell[14] + self.cell[13] * self.cell[6] * self.cell[11] - self.cell[13] * self.cell[7] * self.cell[10],
            -self.cell[1] * self.cell[10] * self.cell[15] + self.cell[1] * self.cell[11] * self.cell[14] + self.cell[9] * self.cell[2] * self.cell[15] -
                self.cell[9] * self.cell[3] * self.cell[14] - self.cell[13] * self.cell[2] * self.cell[11] + self.cell[13] * self.cell[3] * self.cell[10],
            self.cell[1] * self.cell[6] * self.cell[15] - self.cell[1] * self.cell[7] * self.cell[14] - self.cell[5] * self.cell[2] * self.cell[15] +
                self.cell[5] * self.cell[3] * self.cell[14] + self.cell[13] * self.cell[2] * self.cell[7] - self.cell[13] * self.cell[3] * self.cell[6],
            -self.cell[1] * self.cell[6] * self.cell[11] + self.cell[1] * self.cell[7] * self.cell[10] + self.cell[5] * self.cell[2] * self.cell[11] -
                self.cell[5] * self.cell[3] * self.cell[10] - self.cell[9] * self.cell[2] * self.cell[7] + self.cell[9] * self.cell[3] * self.cell[6],
            -self.cell[4] * self.cell[10] * self.cell[15] + self.cell[4] * self.cell[11] * self.cell[14] + self.cell[8] * self.cell[6] * self.cell[15] -
                self.cell[8] * self.cell[7] * self.cell[14] - self.cell[12] * self.cell[6] * self.cell[11] + self.cell[12] * self.cell[7] * self.cell[10],
            self.cell[0] * self.cell[10] * self.cell[15] - self.cell[0] * self.cell[11] * self.cell[14] - self.cell[8] * self.cell[2] * self.cell[15] +
                self.cell[8] * self.cell[3] * self.cell[14] + self.cell[12] * self.cell[2] * self.cell[11] - self.cell[12] * self.cell[3] * self.cell[10],
            -self.cell[0] * self.cell[6] * self.cell[15] + self.cell[0] * self.cell[7] * self.cell[14] + self.cell[4] * self.cell[2] * self.cell[15] -
                self.cell[4] * self.cell[3] * self.cell[14] - self.cell[12] * self.cell[2] * self.cell[7] + self.cell[12] * self.cell[3] * self.cell[6],
            self.cell[0] * self.cell[6] * self.cell[11] - self.cell[0] * self.cell[7] * self.cell[10] - self.cell[4] * self.cell[2] * self.cell[11] +
                self.cell[4] * self.cell[3] * self.cell[10] + self.cell[8] * self.cell[2] * self.cell[7] - self.cell[8] * self.cell[3] * self.cell[6],
            self.cell[4] * self.cell[9] * self.cell[15] - self.cell[4] * self.cell[11] * self.cell[13] - self.cell[8] * self.cell[5] * self.cell[15] +
                self.cell[8] * self.cell[7] * self.cell[13] + self.cell[12] * self.cell[5] * self.cell[11] - self.cell[12] * self.cell[7] * self.cell[9],
            -self.cell[0] * self.cell[9] * self.cell[15] + self.cell[0] * self.cell[11] * self.cell[13] + self.cell[8] * self.cell[1] * self.cell[15] -
                self.cell[8] * self.cell[3] * self.cell[13] - self.cell[12] * self.cell[1] * self.cell[11] + self.cell[12] * self.cell[3] * self.cell[9],
            self.cell[0] * self.cell[5] * self.cell[15] - self.cell[0] * self.cell[7] * self.cell[13] - self.cell[4] * self.cell[1] * self.cell[15] +
                self.cell[4] * self.cell[3] * self.cell[13] + self.cell[12] * self.cell[1] * self.cell[7] - self.cell[12] * self.cell[3] * self.cell[5],
            -self.cell[0] * self.cell[5] * self.cell[11] + self.cell[0] * self.cell[7] * self.cell[9] + self.cell[4] * self.cell[1] * self.cell[11] -
                self.cell[4] * self.cell[3] * self.cell[9] - self.cell[8] * self.cell[1] * self.cell[7] + self.cell[8] * self.cell[3] * self.cell[5],
            -self.cell[4] * self.cell[9] * self.cell[14] + self.cell[4] * self.cell[10] * self.cell[13] + self.cell[8] * self.cell[5] * self.cell[14] -
                self.cell[8] * self.cell[6] * self.cell[13] - self.cell[12] * self.cell[5] * self.cell[10] + self.cell[12] * self.cell[6] * self.cell[9],
            self.cell[0] * self.cell[9] * self.cell[14] - self.cell[0] * self.cell[10] * self.cell[13] - self.cell[8] * self.cell[1] * self.cell[14] +
                self.cell[8] * self.cell[2] * self.cell[13] + self.cell[12] * self.cell[1] * self.cell[10] - self.cell[12] * self.cell[2] * self.cell[9],
            -self.cell[0] * self.cell[5] * self.cell[14] + self.cell[0] * self.cell[6] * self.cell[13] + self.cell[4] * self.cell[1] * self.cell[14] -
                self.cell[4] * self.cell[2] * self.cell[13] - self.cell[12] * self.cell[1] * self.cell[6] + self.cell[12] * self.cell[2] * self.cell[5],
            self.cell[0] * self.cell[5] * self.cell[10] - self.cell[0] * self.cell[6] * self.cell[9] - self.cell[4] * self.cell[1] * self.cell[10] +
                self.cell[4] * self.cell[2] * self.cell[9] + self.cell[8] * self.cell[1] * self.cell[6] - self.cell[8] * self.cell[2] * self.cell[5]
        ];

        let det = self.cell[0] * inv[0] + self.cell[1] * inv[4] + self.cell[2] * inv[8] + self.cell[3] * inv[12];

        if det != 0.0 {
            let inv_det = 1.0 / det;

            for i in 0..16 {
                self.cell[i] = inv[i] * inv_det;
            }
        }
    }
}

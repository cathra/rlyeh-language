// ===== 三维向量（i64 定点分量）=====
// 向量运算全部返回定点量纲（×1000），由 fixed 模块保证数值一致性。

import fixed::SCALE;
import fixed::fmul;
import fixed::fdiv;
import fixed::fsqrt;

pub struct Vec3 {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl Vec3 {
    pub fn new(x: i64, y: i64, z: i64) -> Vec3 {
        Vec3 { x: x, y: y, z: z }
    }

    // 逐分量加
    pub fn add(&self, o: Vec3) -> Vec3 {
        Vec3 { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z }
    }

    // 逐分量减
    pub fn sub(&self, o: Vec3) -> Vec3 {
        Vec3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z }
    }

    // 分量乘标量（k 为定点数，×1000）
    pub fn scale(&self, k: i64) -> Vec3 {
        Vec3 { x: fmul(self.x, k), y: fmul(self.y, k), z: fmul(self.z, k) }
    }

    // 点积（结果 ×1000）
    pub fn dot(&self, o: Vec3) -> i64 {
        let ax = fmul(self.x, o.x);
        let by = fmul(self.y, o.y);
        let cz = fmul(self.z, o.z);
        ax + by + cz
    }

    // 叉积
    pub fn cross(&self, o: Vec3) -> Vec3 {
        let xa = fmul(self.y, o.z);
        let xb = fmul(self.z, o.y);
        let ya = fmul(self.z, o.x);
        let yb = fmul(self.x, o.z);
        let za = fmul(self.x, o.y);
        let zb = fmul(self.y, o.x);
        Vec3 { x: xa - xb, y: ya - yb, z: za - zb }
    }

    // 长度（×1000）。
    // dot(v,v) = |v|² / SCALE，因此 |v|_fixed = sqrt(SCALE * dot(v,v))。
    pub fn len(&self) -> i64 {
        let d = self.dot(*self);
        fsqrt(SCALE * d)
    }

    // 单位化（分量 ×1000）
    pub fn normalize(&self) -> Vec3 {
        let l = self.len();
        if l <= 0 {
            return Vec3 { x: 0, y: 0, z: 0 };
        }
        Vec3 { x: fdiv(self.x, l), y: fdiv(self.y, l), z: fdiv(self.z, l) }
    }
}

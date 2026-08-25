// ===== 相机：正交基构建 + 主光线生成 =====
//
// 正交基：forward = normalize(look_at - eye)，
//         right   = normalize(forward × world_up)，
//         up      = right × forward。
// 光线方向：forward·focal + right·u + up·v（u/v 为像素归一化坐标 ×1000），
// focal 即"焦距"比例（越大视野越窄），无需三角函数。

import fixed::SCALE;
import vec3::Vec3;
import ray::Ray;

pub struct Camera {
    pub eye: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub focal: i64,
}

impl Camera {
    pub fn new(eye: Vec3, look_at: Vec3, focal: i64) -> Camera {
        let f = look_at.sub(eye).normalize();
        let world_up = Vec3 { x: 0, y: SCALE, z: 0 };
        let r = f.cross(world_up).normalize();
        let u = r.cross(f).normalize();
        Camera {
            eye: eye,
            forward: f,
            right: r,
            up: u,
            focal: focal,
        }
    }

    // 由像素归一化坐标 (u, v)（各 ×1000，范围约 [-1000, 1000]）生成主光线
    pub fn ray(&self, u: i64, v: i64) -> Ray {
        let fd = self.forward.scale(self.focal);
        let ru = self.right.scale(u);
        let uv = self.up.scale(v);
        let d0 = fd.add(ru);
        let d = d0.add(uv).normalize();
        Ray::new(self.eye, d)
    }
}

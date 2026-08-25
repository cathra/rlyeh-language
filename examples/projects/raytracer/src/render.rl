// ===== 渲染核心：trace（光照 / 阴影 / 反射）+ 逐像素渲染 =====
//
// 渲染模型（简化 Phong）：
//   color = 环境光 + 漫反射·光照色 + 高光（Blinn 近似）+ 反射（递归 depth 层）
// 阴影：交点向光源发射遮挡检测光线（硬阴影）。
// 定点量纲：颜色分量与光照系数均为 ×1000（0..SCALE = 0..1）。

import fixed::SCALE;
import fixed::fmul;
import fixed::fdiv;
import fixed::clamp;
import fixed::max0;
import fixed::fpow;
import vec3::Vec3;
import color::Color;
import ray::Ray;
import ray::Sphere;
import ray::hit_sphere;
import ray::hit_plane;
import scene::Scene;
import camera::Camera;

// 渲染图像：逐像素 trace 并把量化后的 RGB 追加到 out（PPM 正文，无文件头）。
pub fn render(cam: Camera, s: Scene, width: i64, height: i64, depth: i64, dst: &mut String) {
    let w2 = width / 2;
    let h2 = height / 2;
    let mut py = 0;
    while py < height {
        let v = 0 - fdiv(py - h2, h2);
        let mut px = 0;
        while px < width {
            let u = fdiv(px - w2, w2);
            let r = cam.ray(u, v);
            region 'r adaptive {
                let col = trace(r, s, depth) in 'r;
                let rr = clamp(col.r * 255 / SCALE, 0, 255);
                let gg = clamp(col.g * 255 / SCALE, 0, 255);
                let bb = clamp(col.b * 255 / SCALE, 0, 255);
                dst.push_str(int_to_string(rr));
                dst.push_str(" ");
                dst.push_str(int_to_string(gg));
                dst.push_str(" ");
                dst.push_str(int_to_string(bb));
                dst.push_str(" ");
            }
            px = px + 1;
        }
        dst.push_str("\n");
        py = py + 1;
    }
}

// 光线追踪主函数：遍历球与地面取最近交点，分派着色；无命中返回天空渐变。
pub fn trace(r: Ray, s: Scene, depth: i64) -> Color {
    let mut best_t = 200000;
    let mut best_idx = -1;
    let n = s.spheres.len();
    let mut i = 0;
    while i < n {
        let sph = s.spheres[i];
        let t = hit_sphere(r, sph);
        if t > 0 && t < best_t {
            best_t = t;
            best_idx = i;
        }
        i = i + 1;
    }
    let tg = hit_plane(r, s.ground_y);
    if tg > 0 && tg < best_t {
        best_t = tg;
        best_idx = -2;
    }
    if best_idx == -1 {
        return sky_color(r, s);
    }
    let mut col = Color::new(0, 0, 0);
    if best_idx == -2 {
        col = shade_ground(r, best_t, s);
    } else {
        let sph = s.spheres[best_idx];
        col = shade_sphere(r, best_t, sph, s, depth);
    }
    col.saturate()
}

// 球体着色：漫反射 + 高光 + 环境光 + 反射
fn shade_sphere(r: Ray, t: i64, sph: Sphere, s: Scene, depth: i64) -> Color {
    let p = r.origin.add(r.dir.scale(t));
    let n = p.sub(sph.center).normalize();
    let light_dir = s.light.sub(p).normalize();
    let view_dir = r.dir.scale(0 - SCALE).normalize();
    let diff_k = max0(n.dot(light_dir));
    let shadow = occluded(p, light_dir, s);
    let mut diff = diff_k;
    if shadow == 1 {
        diff = fmul(diff_k, 300);
    }
    let h = light_dir.add(view_dir).normalize();
    let nh = max0(n.dot(h));
    let mut spec = fpow(nh, s.shininess);
    if shadow == 1 {
        spec = fmul(spec, 300);
    }
    let base = sph.color.modulate(s.light_color);
    let dif_term = base.scale(diff);
    let amb = sph.color.scale(s.ambient);
    let spe_term = s.light_color.scale(spec);
    let lit = amb.add(dif_term).add(spe_term);
    if depth <= 0 {
        return lit;
    }
    let refl = reflect_dir(n, view_dir);
    let refl_ray = Ray::new(p, refl);
    let refl_col = trace(refl_ray, s, depth - 1);
    let refl_col2 = refl_col.scale(400);
    lit.add(refl_col2)
}

// 地面着色：棋盘格 + 漫反射 + 环境光
fn shade_ground(r: Ray, t: i64, s: Scene) -> Color {
    let p = r.origin.add(r.dir.scale(t));
    let n = Vec3 { x: 0, y: SCALE, z: 0 };
    let light_dir = s.light.sub(p).normalize();
    let diff = max0(n.dot(light_dir));
    let gx = p.x / SCALE;
    let gz = p.z / SCALE;
    let par = (gx + gz) & 1;
    let mut base = s.ground_color;
    if par == 0 {
        base = base.scale(800);
    } else {
        base = base.scale(600);
    }
    let amb = base.scale(s.ambient);
    let dif = base.scale(diff);
    amb.add(dif)
}

// 天空渐变：按光线方向 y 分量在 top/bottom 间插值
fn sky_color(r: Ray, s: Scene) -> Color {
    let mut k = fdiv(r.dir.y + SCALE, 2000);
    k = clamp(k, 0, SCALE);
    let inv = SCALE - k;
    let rr = fmul(s.sky_top.r, k) + fmul(s.sky_bottom.r, inv);
    let gg = fmul(s.sky_top.g, k) + fmul(s.sky_bottom.g, inv);
    let bb = fmul(s.sky_top.b, k) + fmul(s.sky_bottom.b, inv);
    Color::new(rr, gg, bb)
}

// 阴影检测：p 沿 ldir 是否被球遮挡（>0.05 单位外即算）
fn occluded(p: Vec3, ldir: Vec3, s: Scene) -> i64 {
    let ray = Ray::new(p, ldir);
    let n = s.spheres.len();
    let mut i = 0;
    while i < n {
        let sph = s.spheres[i];
        let t = hit_sphere(ray, sph);
        if t > 50 {
            return 1;
        }
        i = i + 1;
    }
    0
}

// 反射方向：refl = 2(n·v)·n - v（v 为入射方向）
fn reflect_dir(n: Vec3, v: Vec3) -> Vec3 {
    let ndv = n.dot(v);
    let k = fmul(ndv, 2000);
    let proj = n.scale(k);
    proj.sub(v)
}

// ===== raytracer：纯软件光线追踪渲染器（Zeta 示例项目）=====
//
// 功能：渲染三个球 + 棋盘格地面 + 反射的简单场景，输出 P3 格式 PPM 图像。
// 特点：全 i64 定点运算（MVP 无 f64↔i64 转换与数学库），
//       展示结构体/方法/模块化/region 自适应分配/文件 IO。
//
// 运行：zeta run src/main.zeta（或 scripts/run.sh）

module fixed;
module vec3;
module color;
module ray;
module camera;
module scene;
module render;

import fixed::SCALE;
import vec3::Vec3;
import color::Color;
import camera::Camera;
import scene::build_scene;
import render::render;

fn main() {
    // 渲染参数（内置；修改后重新编译即可出图）
    let width = 320;
    let height = 200;
    let depth = 2;               // 反射递归层数

    // 相机：位于 (0, 0.3, 5.5)，看向原点，焦距 1.2（中等视野）
    let eye = Vec3 { x: 0, y: 300, z: 5500 };
    let look = Vec3 { x: 0, y: 0, z: 0 };
    let cam = Camera::new(eye, look, 1200);

    // 场景
    let scene = build_scene();

    // 计时渲染（std 的 `import time::Instant` 全局可见，短名调用）
    let t0 = Instant::now();
    let mut body = String::with_capacity(width * height * 12);
    render(cam, scene, width, height, depth, &mut body);
    let dt = t0.elapsed().millis();

    // 组装 PPM 文件（P3 文本格式）
    let mut header = String::from("P3\n");
    header.push_str(int_to_string(width));
    header.push_str(" ");
    header.push_str(int_to_string(height));
    header.push_str("\n255\n");
    let content = header + body;

    // 写入文件
    let res = write_file(String::from("output.ppm"), content);
    let ok = match res {
        Result::Ok(n) => 1,
        Result::Err(e) => 0,
    };
    println("render done:");
    println(ok);
    println("elapsed millis:");
    println(dt);
    println("size bytes:");
    println(header.len() + body.len());
}

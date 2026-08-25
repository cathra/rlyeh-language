# raytracer — Rlyeh 光线追踪示例

一个纯软件、全 i64 定点的光线追踪渲染器，输出 PPM 图像。

## 设计要点

- **全 i64 定点数学**：MVP 无 f64↔i64 转换与数学库，因此用 `SCALE=1000` 的定点数模拟浮点，并手写 `sqrt`/`pow`。
- **模块组织**：`fixed`/`vec3`/`color`/`ray`/`camera`/`scene`/`render` 多文件协作。
- **核心渲染模型**：简化 Phong（环境+漫反射+Blinn 高光+阴影+一层反射）。
- **区域内存**：`region 'r adaptive` 包裹逐像素 trace，批量释放临时向量。
- **文件 IO**：`write_file` 输出 PPM；`Instant::now` 计时。

## 构建与运行

```bash
scripts/build.sh     # 编译为 ./raytracer
scripts/run.sh       # 编译并生成 output.ppm（macOS 同时生成 output.png）
scripts/bench.sh     # rlyeh bench 基准
```

或直接：

```bash
rlyeh run src/main.rl    # 或 rlyeh build src/main.rl -o raytracer
```

## 调整参数

编辑 `src/main.rl` 中的内置常量：

- `width` / `height`：图像尺寸
- `depth`：反射递归层数
- `eye` / `look` / `focal`：相机位置、目标与焦距

场景球体、光源、颜色在 `src/scene.rl` 中定义。

## 产物

- `output.ppm`：P3 格式图片，可用任意图像查看器打开（macOS 脚本会自动转 output.png）。

//! 変形@FlowType_H（原作 Deform@FlowType_H）。原作 filters/intern/deform/intern/deform.cpp の移植。
//!
//! 画像を 2D のアフィン変換（位置・回転・せん断・スケール・ピボット）で描き直す。計算は原作と同じく単精度で、
//! Eigen の `Transform::translate / rotate / scale` は右から掛ける。

use aviutl2::filter::{
    FilterConfigItemSliceExt, FilterConfigItems, FilterPlugin, FilterPluginTable, FilterProcVideo, ImageResource, RgbaPixel, SamplerMode,
    VertexList, VertexTexture,
};
use aviutl2::AnyResult;

use super::{fail, information, stop, zeros, LABEL};
use crate::rotation::to_rad;

#[derive(aviutl2::filter::FilterConfigSelectItems, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    #[item(name = "最近傍")]
    Nearest = 0,
    #[item(name = "バイリニア")]
    Bilinear = 1,
}

#[aviutl2::filter::filter_config_items]
#[derive(Debug, Clone)]
pub struct Config {
    #[group(name = "ピボット", opened = true)]
    pivot: group! {
        #[track(name = "ピボット::X", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::ピボット")]
        pivot_x: f64,
        #[track(name = "ピボット::Y", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::ピボット")]
        pivot_y: f64,
    },
    #[group(name = "位置", opened = true)]
    position: group! {
        #[track(name = "位置::X", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::位置")]
        position_x: f64,
        #[track(name = "位置::Y", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::位置")]
        position_y: f64,
    },
    #[group(name = "スケール", opened = true)]
    scale: group! {
        #[track(name = "スケール::X", range = -10000.0..=10000.0, step = 0.01, default = 100.0, group = "Group::スケール")]
        scale_x: f64,
        #[track(name = "スケール::Y", range = -10000.0..=10000.0, step = 0.01, default = 100.0, group = "Group::スケール")]
        scale_y: f64,
    },
    #[group(name = "せん断", opened = true)]
    skew: group! {
        #[track(name = "せん断::角度", range = -70.0..=70.0, step = 0.01, default = 0.0)]
        skew_angle: f64,
        #[track(name = "せん断::軸", range = -3600.0..=3600.0, step = 0.01, default = 0.0)]
        skew_axis: f64,
    },
    #[track(name = "回転", range = -3600.0..=3600.0, step = 0.01, default = 0.0)]
    rotation: f64,
    #[track(name = "不透明度", range = 0.0..=100.0, step = 0.01, default = 100.0)]
    opacity: f64,
    #[select(name = "サンプリング", items = Sampling, default = Sampling::Bilinear)]
    sampling: Sampling,
}

#[aviutl2::plugin(FilterPlugin)]
pub struct Deform;

impl FilterPlugin for Deform {
    type Userdata = ();

    fn new(_info: aviutl2::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: "変形@FlowType_H".to_string(),
            label: Some(LABEL.to_string()),
            information: information("変形"),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags { video: true }),
            config_items: Config::to_config_items(),
        }
    }

    fn proc_video(&self, config: &[aviutl2::filter::FilterConfigItem], video: &mut FilterProcVideo<()>) -> AnyResult<()> {
        let cfg: Config = config.to_struct();
        deform(&cfg, video)
    }
}

/// 2D のアフィン変換（線形部分 l と平行移動 t）。単精度。
#[derive(Debug, Clone, Copy)]
pub struct Affine {
    l: [[f32; 2]; 2],
    t: [f32; 2],
}

impl Affine {
    const IDENTITY: Affine = Affine { l: [[1.0, 0.0], [0.0, 1.0]], t: [0.0, 0.0] };

    fn mul_linear(a: [[f32; 2]; 2], b: [[f32; 2]; 2]) -> [[f32; 2]; 2] {
        [
            [a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
            [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]],
        ]
    }

    /// `Transform::translate`（右から平行移動を掛ける: t += l * v）
    fn translate(&mut self, v: [f32; 2]) {
        self.t[0] += self.l[0][0] * v[0] + self.l[0][1] * v[1];
        self.t[1] += self.l[1][0] * v[0] + self.l[1][1] * v[1];
    }

    /// `Transform::rotate(Rotation2D)`（l = l * R）
    fn rotate(&mut self, angle: f32) {
        let (s, c) = angle.sin_cos();
        self.l = Self::mul_linear(self.l, [[c, -s], [s, c]]);
    }

    /// `Transform::scale`（l = l * diag(s)）
    fn scale(&mut self, s: [f32; 2]) {
        self.l = Self::mul_linear(self.l, [[s[0], 0.0], [0.0, s[1]]]);
    }

    pub fn apply(&self, v: [f32; 2]) -> [f32; 2] {
        [
            self.l[0][0] * v[0] + self.l[0][1] * v[1] + self.t[0],
            self.l[1][0] * v[0] + self.l[1][1] * v[1] + self.t[1],
        ]
    }
}

pub fn build_transform(cfg: &Config) -> Affine {
    let mut t = Affine::IDENTITY;
    t.translate([cfg.position_x as f32, cfg.position_y as f32]);
    t.rotate(to_rad(cfg.rotation) as f32);
    let axis = to_rad(cfg.skew_axis) as f32;
    let shear = [[1.0f32, 0.0], [(to_rad(cfg.skew_angle) as f32).tan(), 1.0]];
    t.rotate(-axis);
    t.l = Affine::mul_linear(t.l, shear);
    t.rotate(axis);
    t.scale([cfg.scale_x as f32 * 0.01, cfg.scale_y as f32 * 0.01]);
    t.translate([-(cfg.pivot_x as f32), -(cfg.pivot_y as f32)]);
    t
}

fn deform(cfg: &Config, video: &mut FilterProcVideo<()>) -> AnyResult<()> {
    let alpha = cfg.opacity as f32 * 0.01;
    let t = build_transform(cfg);

    let cx = video.video_object.width as f32 * 0.5;
    let cy = video.video_object.height as f32 * 0.5;
    let verts = [[-cx, -cy], [cx, -cy], [cx, cy], [-cx, cy]].map(|v| t.apply(v));

    let max_x = verts.iter().map(|v| v[0].abs()).fold(f32::MIN, f32::max);
    let max_y = verts.iter().map(|v| v[1].abs()).fold(f32::MIN, f32::max);
    let w = max_x.ceil() as i32 * 2;
    let h = max_y.ceil() as i32 * 2;
    if w <= 0 || h <= 0 {
        return stop(video);
    }
    let (w, h) = (w as u32, h as u32);

    video.set_sampler_mode(if cfg.sampling == Sampling::Nearest { SamplerMode::Dot } else { SamplerMode::Clamp });

    let result = ImageResource::Resource("result".to_string());
    video.create_image_resource(&result, &zeros(w, h), w, h)?;
    if video.clear_image_resource(&result, RgbaPixel { r: 0, g: 0, b: 0, a: 0 }).is_err() {
        return fail(video, "Failed to clear buffer");
    }

    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let quad: [VertexTexture; 4] = std::array::from_fn(|i| VertexTexture {
        x: verts[i][0],
        y: verts[i][1],
        z: 0.0,
        u: uv[i][0],
        v: uv[i][1],
        a: alpha,
    });
    if video
        .draw_poly_to_resource(&result, &VertexList::QuadTexture(vec![quad]), Some(&ImageResource::Object))
        .is_err()
    {
        return fail(video, "Failed to draw image");
    }
    if video.copy_image_resource(&result, &ImageResource::Object).is_err() {
        return fail(video, "Failed to copy buffer");
    }

    if matches!(video.video_object.num, Some(n) if n > 1) {
        let pos = t.apply([video.param.x, video.param.y]);
        video.param.x = pos[0];
        video.param.y = pos[1];
    }
    Ok(())
}

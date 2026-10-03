//! トランスフォーム@FlowType_H（原作 Transform@FlowType_H）。原作 filters/intern/transform/intern/transform.cpp の移植。
//!
//! ピボット・位置・回転・スケールを、オブジェクト自身の座標（ローカル空間）か、基準座標（ワールド空間）に
//! 影響度の割合で足す。回転は四元数・軸角・オイラー角（6 通りの順序）で指定できる。

use aviutl2::filter::{FilterConfigItemSliceExt, FilterConfigItems, FilterPlugin, FilterPluginTable, FilterProcVideo};
use aviutl2::AnyResult;

use super::{information, LABEL};
use crate::math::lerp;
use crate::rotation::{self, Unit};

#[derive(aviutl2::filter::FilterConfigSelectItems, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationMode {
    #[item(name = "クォータニオン")]
    Quaternion = 0,
    #[item(name = "軸角")]
    AxisAngle = 1,
    #[item(name = "XYZオイラー")]
    Xyz = 5,
    #[item(name = "XZYオイラー")]
    Xzy = 7,
    #[item(name = "YXZオイラー")]
    Yxz = 11,
    #[item(name = "YZXオイラー")]
    Yzx = 15,
    #[item(name = "ZXYオイラー")]
    Zxy = 19,
    #[item(name = "ZYXオイラー")]
    Zyx = 21,
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
        #[track(name = "ピボット::Z", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::ピボット")]
        pivot_z: f64,
    },
    #[group(name = "位置", opened = true)]
    position: group! {
        #[track(name = "位置::X", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::位置")]
        position_x: f64,
        #[track(name = "位置::Y", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::位置")]
        position_y: f64,
        #[track(name = "位置::Z", range = -100000.0..=100000.0, step = 0.01, default = 0.0, group = "Group::位置")]
        position_z: f64,
    },
    #[group(name = "回転", opened = true)]
    rotation: group! {
        #[track(name = "回転::W", range = -3600.0..=3600.0, step = 0.01, default = 0.0)]
        rotation_w: f64,
        #[track(name = "回転::X", range = -3600.0..=3600.0, step = 0.01, default = 0.0, group = "Group::回転")]
        rotation_x: f64,
        #[track(name = "回転::Y", range = -3600.0..=3600.0, step = 0.01, default = 0.0, group = "Group::回転")]
        rotation_y: f64,
        #[track(name = "回転::Z", range = -3600.0..=3600.0, step = 0.01, default = 0.0, group = "Group::回転")]
        rotation_z: f64,
        #[select(name = "回転::モード", items = RotationMode, default = RotationMode::Zyx)]
        rotation_mode: RotationMode,
    },
    #[group(name = "スケール", opened = true)]
    scale: group! {
        #[track(name = "スケール::X", range = -10000.0..=10000.0, step = 0.01, default = 100.0, group = "Group::スケール")]
        scale_x: f64,
        #[track(name = "スケール::Y", range = -10000.0..=10000.0, step = 0.01, default = 100.0, group = "Group::スケール")]
        scale_y: f64,
        #[track(name = "スケール::Z", range = -10000.0..=10000.0, step = 0.01, default = 100.0, group = "Group::スケール")]
        scale_z: f64,
    },
    #[group(name = "対象", opened = true)]
    target: group! {
        #[checksection(name = "対象::ローカル空間", default = true, multi_section = false)]
        target_local_space: bool,
        #[checksection(name = "対象::ワールド空間", default = false, multi_section = false)]
        target_world_space: bool,
    },
    #[group(name = "追加オプション", opened = false)]
    additional: group! {
        #[track(name = "影響度", range = 0.0..=100.0, step = 0.01, default = 100.0)]
        influence: f64,
    },
}

#[aviutl2::plugin(FilterPlugin)]
pub struct Transform;

impl FilterPlugin for Transform {
    type Userdata = ();

    fn new(_info: aviutl2::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: "トランスフォーム@FlowType_H".to_string(),
            label: Some(LABEL.to_string()),
            information: information("トランスフォーム"),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags { video: true }),
            config_items: Config::to_config_items(),
        }
    }

    fn proc_video(&self, config: &[aviutl2::filter::FilterConfigItem], video: &mut FilterProcVideo<()>) -> AnyResult<()> {
        let cfg: Config = config.to_struct();
        transform(&cfg, video)
    }
}

fn transform(cfg: &Config, video: &mut FilterProcVideo<()>) -> AnyResult<()> {
    let t = cfg.influence * 0.01;
    let (sx, sy, sz) = (cfg.scale_x * 0.01, cfg.scale_y * 0.01, cfg.scale_z * 0.01);
    let (rw, rx, ry, rz) = (cfg.rotation_w, cfg.rotation_x, cfg.rotation_y, cfg.rotation_z);
    let mode = cfg.rotation_mode as i32;
    let p = &mut video.param;

    if cfg.target_world_space {
        let mut v = [[
            (p.x as f64 - cfg.pivot_x * t) * (1.0 + (sx - 1.0) * t),
            (p.y as f64 - cfg.pivot_y * t) * (1.0 + (sy - 1.0) * t),
            (p.z as f64 - cfg.pivot_z * t) * (1.0 + (sz - 1.0) * t),
        ]];
        rotation::rotate(t, mode, Unit::Degree, rw, rx, ry, rz, &mut v).map_err(anyhow_like)?;
        p.x = v[0][0] as f32;
        p.y = v[0][1] as f32;
        p.z = v[0][2] as f32;
    }

    if cfg.target_local_space {
        let angle = rotation::to_euler(t, mode, Unit::Degree, rw, rx, ry, rz).map_err(anyhow_like)?;
        p.cx += (cfg.pivot_x * t) as f32;
        p.cy += (cfg.pivot_y * t) as f32;
        p.cz += (cfg.pivot_z * t) as f32;
        p.rx += angle[0] as f32;
        p.ry += angle[1] as f32;
        p.rz += angle[2] as f32;
        p.sx = lerp(p.sx as f64, p.sx as f64 * sx, t) as f32;
        p.sy = lerp(p.sy as f64, p.sy as f64 * sy, t) as f32;
        p.sz = lerp(p.sz as f64, p.sz as f64 * sz, t) as f32;
    }

    if cfg.target_local_space || cfg.target_world_space {
        p.x += (cfg.position_x * t) as f32;
        p.y += (cfg.position_y * t) as f32;
        p.z += (cfg.position_z * t) as f32;
    }
    Ok(())
}

fn anyhow_like(e: &'static str) -> aviutl2::anyhow::Error {
    aviutl2::anyhow::Error::msg(e)
}

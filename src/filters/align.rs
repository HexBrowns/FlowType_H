//! 整列@FlowType_H（原作 Align@FlowType_H）。原作 filters/intern/align/intern/align.cpp の移植。
//!
//! 画像の大きさに対する割合（-1000〜1000、100 で画像の半分）でピボットや位置をずらす。

use aviutl2::filter::{FilterConfigItemSliceExt, FilterConfigItems, FilterPlugin, FilterPluginTable, FilterProcVideo};
use aviutl2::AnyResult;

use super::{information, LABEL};

#[derive(aviutl2::filter::FilterConfigSelectItems, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    #[item(name = "ピボット")]
    Pivot = 1,
    #[item(name = "位置")]
    Position = 2,
    #[item(name = "両方")]
    Both = 3,
}

#[aviutl2::filter::filter_config_items]
#[derive(Debug, Clone)]
pub struct Config {
    #[track(name = "水平", range = -1000.0..=1000.0, step = 0.01, default = 0.0)]
    horizontal: f64,
    #[track(name = "垂直", range = -1000.0..=1000.0, step = 0.01, default = 0.0)]
    vertical: f64,
    #[select(name = "対象", items = Target, default = Target::Pivot)]
    target: Target,
    #[checksection(name = "上書き", default = false, multi_section = false)]
    should_overwrite: bool,
}

#[aviutl2::plugin(FilterPlugin)]
pub struct Align;

impl FilterPlugin for Align {
    type Userdata = ();

    fn new(_info: aviutl2::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: "整列@FlowType_H".to_string(),
            label: Some(LABEL.to_string()),
            information: information("整列"),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags { video: true }),
            config_items: Config::to_config_items(),
        }
    }

    fn proc_video(&self, config: &[aviutl2::filter::FilterConfigItem], video: &mut FilterProcVideo<()>) -> AnyResult<()> {
        let cfg: Config = config.to_struct();
        let (mut cx, mut cy, mut ox, mut oy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        if !cfg.should_overwrite {
            (cx, cy, ox, oy) = (video.param.cx, video.param.cy, video.param.x, video.param.y);
        }
        let dx = cfg.horizontal as f32 * video.video_object.width as f32 * 0.005;
        let dy = cfg.vertical as f32 * video.video_object.height as f32 * 0.005;
        let target = cfg.target as i32;
        if target & 1 != 0 {
            video.param.cx = cx + dx;
            video.param.cy = cy + dy;
        }
        if target >> 1 != 0 {
            video.param.x = ox + dx;
            video.param.y = oy + dy;
        }
        Ok(())
    }
}

//! トリミング@FlowType_H（原作 Trim@FlowType_H）。原作 filters/intern/trim/intern/trim.cpp の移植。
//!
//! アルファがしきい値を超える画素の外接矩形に余白を足して切り抜き、アンカー（ピボットか位置）をその分ずらす。
//! しきい値は原作どおり、0〜100 の値をそのままアルファ（0〜1、半精度）と比べる。

use std::mem::ManuallyDrop;

use aviutl2::filter::{
    BlendStateMode, FilterConfigItemSliceExt, FilterConfigItems, FilterPlugin, FilterPluginTable, FilterProcVideo, ImageResource,
    OutputImageResourcePixelFormat, RgbaPixel, SamplerMode,
};
use aviutl2::AnyResult;
use half::f16;

use super::{fail, information, stop, too_large_once, zeroed_bytes, zeros, LABEL};

/// 大きさが上限を超えたとき。描かずに止め、ログは初回だけ出す
fn too_large(video: &mut FilterProcVideo<()>, w: i64, h: i64) -> AnyResult<()> {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    too_large_once(&WARNED, "トリミング@FlowType_H", w, h);
    stop(video)
}

const BLIT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/blit.cso"));

#[derive(aviutl2::filter::FilterConfigSelectItems, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorTarget {
    #[item(name = "なし")]
    None = 0,
    #[item(name = "ピボット")]
    Pivot = 1,
    #[item(name = "位置")]
    Position = 2,
}

#[aviutl2::filter::filter_config_items]
#[derive(Debug, Clone)]
pub struct Config {
    #[track(name = "しきい値", range = 0.0..=100.0, step = 0.01, default = 0.0)]
    threshold: f64,
    #[group(name = "アンカー", opened = true)]
    anchor: group! {
        #[select(name = "アンカー::対象", items = AnchorTarget, default = AnchorTarget::Pivot)]
        anchor_target: AnchorTarget,
        #[checksection(name = "アンカー::上書き", default = false, multi_section = false)]
        anchor_should_overwrite: bool,
    },
    #[group(name = "余白", opened = false)]
    padding: group! {
        #[track(name = "余白::左", range = -1000.0..=1000.0, step = 1.0, default = 0.0)]
        padding_left: f64,
        #[track(name = "余白::右", range = -1000.0..=1000.0, step = 1.0, default = 0.0)]
        padding_right: f64,
        #[track(name = "余白::上", range = -1000.0..=1000.0, step = 1.0, default = 0.0)]
        padding_top: f64,
        #[track(name = "余白::下", range = -1000.0..=1000.0, step = 1.0, default = 0.0)]
        padding_bottom: f64,
    },
}

#[aviutl2::plugin(FilterPlugin)]
pub struct Trim;

impl FilterPlugin for Trim {
    type Userdata = ();

    fn new(_info: aviutl2::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: "トリミング@FlowType_H".to_string(),
            label: Some(LABEL.to_string()),
            information: information("トリミング"),
            flags: aviutl2::bitflag!(aviutl2::filter::FilterPluginFlags { video: true }),
            config_items: Config::to_config_items(),
        }
    }

    fn proc_video(&self, config: &[aviutl2::filter::FilterConfigItem], video: &mut FilterProcVideo<()>) -> AnyResult<()> {
        let cfg: Config = config.to_struct();
        trim(&cfg, video)
    }
}

fn trim(cfg: &Config, video: &mut FilterProcVideo<()>) -> AnyResult<()> {
    let t = f16::from_f64(cfg.threshold);
    let (w, h) = (video.video_object.width, video.video_object.height);
    let tmp = ImageResource::Resource("tmp".to_string());

    if video.copy_image_resource(&ImageResource::Object, &tmp).is_err() {
        return fail(video, "Failed to copy buffer");
    }

    // 半精度 RGBA（1 画素 8 バイト）。確保に失敗しても abort しないように（`zeroed_bytes`）
    let Some(mut data) = zeroed_bytes(w, h, 8) else {
        return too_large(video, w as i64, h as i64);
    };
    if w == 0
        || h == 0
        || video
            .get_image_resource_data(&ImageResource::Object, &mut data, w, h, w * 8, OutputImageResourcePixelFormat::Hf64)
            .is_err()
    {
        return fail(video, "Pixel data is null or corrupted");
    }

    // アルファ（半精度、1 画素 8 バイトの 6〜7 バイト目）がしきい値を超える画素の外接矩形
    let mut bbox: Option<([i32; 2], [i32; 2])> = None;
    for y in 0..h as usize {
        let row = &data[y * w as usize * 8..(y + 1) * w as usize * 8];
        for x in 0..w as usize {
            let a = f16::from_bits(u16::from_le_bytes([row[x * 8 + 6], row[x * 8 + 7]]));
            if a > t {
                let (x, y) = (x as i32, y as i32);
                let b = bbox.get_or_insert(([x, y], [x, y]));
                b.0[0] = b.0[0].min(x);
                b.0[1] = b.0[1].min(y);
                b.1[0] = b.1[0].max(x);
                b.1[1] = b.1[1].max(y);
            }
        }
    }
    let Some((min, max)) = bbox else {
        return stop(video);
    };

    // static_cast<int>（0 方向への切り捨て）
    let (left, right) = (cfg.padding_left as i32, cfg.padding_right as i32);
    let (top, bottom) = (cfg.padding_top as i32, cfg.padding_bottom as i32);
    let origin = [min[0] - left, min[1] - top];
    let size = [max[0] - min[0] + left + right + 1, max[1] - min[1] + top + bottom + 1];
    if size[0] <= 0 || size[1] <= 0 {
        return stop(video);
    }

    let (sw, sh) = (size[0] as u32, size[1] as u32);
    // 余白を大きくすると上限を超える。確保すると本体ごと落ちうるので、描かずに止める
    let Some(pixels) = zeros(sw, sh) else {
        return too_large(video, sw as i64, sh as i64);
    };
    video.set_image_data(&pixels, sw, sh);
    if video.clear_image_resource(&ImageResource::Object, RgbaPixel { r: 0, g: 0, b: 0, a: 0 }).is_err() {
        return fail(video, "Failed to clear buffer");
    }

    let blend = video.get_blend_state(BlendStateMode::Copy).map(ManuallyDrop::new);
    let sampler = video.get_sampler_state(SamplerMode::Clip).map(ManuallyDrop::new);
    let (Some(blend), Some(sampler)) = (blend, sampler) else {
        return fail(video, "Failed to execute pixelshader");
    };
    // cbuffer の int2 origin（16 バイトにそろえて渡す）
    let constant: [i32; 4] = [origin[0], origin[1], 0, 0];
    if video
        .exec_pixelshader_data(BLIT, &ImageResource::Object, &[tmp], constant, &blend, &sampler)
        .is_err()
    {
        return fail(video, "Failed to execute pixelshader");
    }

    let delta = [
        origin[0] as f32 + (size[0] - w as i32) as f32 * 0.5,
        origin[1] as f32 + (size[1] - h as i32) as f32 * 0.5,
    ];
    match cfg.anchor_target {
        AnchorTarget::Pivot => {
            if cfg.anchor_should_overwrite {
                video.param.cx = -delta[0];
                video.param.cy = -delta[1];
            } else {
                video.param.cx -= delta[0];
                video.param.cy -= delta[1];
            }
        }
        AnchorTarget::Position => {
            if cfg.anchor_should_overwrite {
                video.param.x = delta[0];
                video.param.y = delta[1];
            } else {
                video.param.x += delta[0];
                video.param.y += delta[1];
            }
        }
        AnchorTarget::None => {}
    }
    Ok(())
}

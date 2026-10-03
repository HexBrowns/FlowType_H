//! `Kerning@FlowType_H`。原作 modules/intern/text/intern/kerning.cpp の移植。
//!
//! テキストを HarfBuzz で組み、1 文字ごとに「組んだ位置 − 字送りだけで並べた位置」を返す
//! （カーニング@FlowType_H がこのずれを各文字に足す）。合字は切り、プロポーショナル字幅とカーニングを入れる。

use harfbuzz_sys as hb;

use super::{BAD_ENCODING, WRONG_COUNT};
use crate::font::{self, Buffer};
use crate::smp::Smp;
use crate::utf8::{is_ctrl, next};

crate::module_fn!(shift_fn, shift);

super::script_module!(KerningModule, "Kerning", ["shift" => shift_fn]);

const fn tag(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}

const fn feature(t: &[u8; 4], value: u32) -> hb::hb_feature_t {
    hb::hb_feature_t { tag: tag(t), value, start: 0, end: u32::MAX }
}

pub const FEATURES: [hb::hb_feature_t; 7] = [
    feature(b"liga", 0),
    feature(b"clig", 0),
    feature(b"dlig", 0),
    feature(b"palt", 1),
    feature(b"vpal", 1),
    feature(b"kern", 1),
    feature(b"vkrn", 1),
];

/// shift(id, text, size, font, alignment, bold, italic) → 文字ごとの {dx, dy}
fn shift(p: &mut Smp) {
    if p.num() != 7 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let text = p.bytes(1);
    let size = p.double(2);
    let name = p.bytes(3);
    let alignment = p.int(4);
    let is_bold = p.boolean(5);
    let is_italic = p.boolean(6);

    if text.is_empty() {
        return;
    }
    if name.is_empty() || size < 1.0e-6 {
        p.set_error("Font name is empty or size is zero");
        return;
    }
    let direction = if alignment >= 9 { hb::HB_DIRECTION_TTB } else { hb::HB_DIRECTION_LTR };

    let font = match font::load(&String::from_utf8_lossy(&name), is_bold, is_italic) {
        Ok(Some(f)) => f,
        Ok(None) => {
            tracing::warn!("Font file not found");
            let mut pos = 0;
            while pos < text.len() {
                let Some((cp, end)) = next(&text, pos) else {
                    p.set_error(BAD_ENCODING);
                    return;
                };
                pos = end;
                if !is_ctrl(cp) {
                    p.push_array_double(&[0.0, 0.0]);
                }
            }
            return;
        }
        Err(e) => {
            p.set_error(&e);
            return;
        }
    };

    let scale = (size * 64.0) as i32;
    unsafe { hb::hb_font_set_scale(font.font, scale, scale) };
    let buffer = Buffer::new();

    for line in text.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let Some(row) = shape_line(&font, &buffer, line, direction) else {
            p.set_error(BAD_ENCODING);
            return;
        };
        if row.is_empty() {
            continue;
        }
        let last = *row.last().unwrap();
        for mut q in row {
            match alignment {
                1 | 4 | 7 => q[0] -= last[0] * 0.5,
                2 | 5 | 8 => q[0] -= last[0],
                10 | 13 | 16 => q[1] -= last[1] * 0.5,
                11 | 14 | 17 => q[1] -= last[1],
                _ => {}
            }
            p.push_array_double(&q);
        }
    }
}

/// 1 行を組んで、制御文字を除いた文字ごとのずれを返す。不正な UTF-8 なら `None`。
fn shape_line(font: &font::Font, buffer: &Buffer, line: &[u8], direction: hb::hb_direction_t) -> Option<Vec<[f64; 2]>> {
    let (infos, positions) = unsafe {
        hb::hb_buffer_reset(buffer.0);
        hb::hb_buffer_add_utf8(buffer.0, line.as_ptr() as *const std::ffi::c_char, line.len() as i32, 0, line.len() as i32);
        hb::hb_buffer_set_direction(buffer.0, direction);
        hb::hb_buffer_guess_segment_properties(buffer.0);
        hb::hb_shape(font.font, buffer.0, FEATURES.as_ptr(), FEATURES.len() as u32);
        let mut count = 0u32;
        let info = hb::hb_buffer_get_glyph_infos(buffer.0, &mut count);
        let pos = hb::hb_buffer_get_glyph_positions(buffer.0, std::ptr::null_mut());
        if count == 0 || info.is_null() || pos.is_null() {
            return Some(Vec::new());
        }
        (
            std::slice::from_raw_parts(info, count as usize).to_vec(),
            std::slice::from_raw_parts(pos, count as usize).to_vec(),
        )
    };

    let mut row = Vec::with_capacity(infos.len());
    let mut nominal = [0.0f64; 2];
    let mut actual = [0.0f64; 2];
    for (info, pos) in infos.iter().zip(positions.iter()) {
        let (cp, _) = next(line, info.cluster as usize)?;
        if is_ctrl(cp) {
            continue;
        }
        let (mut ox, mut oy) = (0, 0);
        unsafe { hb::hb_font_add_glyph_origin_for_direction(font.font, info.codepoint, direction, &mut ox, &mut oy) };
        let offset = [(pos.x_offset + ox) as f64 / 64.0, -((pos.y_offset + oy) as f64) / 64.0];
        row.push([actual[0] - nominal[0] + offset[0], actual[1] - nominal[1] + offset[1]]);

        if direction == hb::HB_DIRECTION_LTR {
            nominal[0] += unsafe { hb::hb_font_get_glyph_h_advance(font.font, info.codepoint) } as f64 / 64.0;
        } else {
            nominal[1] -= unsafe { hb::hb_font_get_glyph_v_advance(font.font, info.codepoint) } as f64 / 64.0;
        }
        actual[0] += pos.x_advance as f64 / 64.0;
        actual[1] -= pos.y_advance as f64 / 64.0;
    }
    Some(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実際のフォントで組む（DirectWrite で Arial を探し、HarfBuzz で「AV」を組む）。
    #[test]
    fn kerns_av_with_a_real_font() {
        font::init(std::path::Path::new("Z:/__no_such_dir__"));
        let f = font::load("Arial", false, false).unwrap().expect("Arial が見つからない");
        unsafe { hb::hb_font_set_scale(f.font, 100 * 64, 100 * 64) };
        let buf = Buffer::new();
        let row = shape_line(&f, &buf, b"AV", hb::HB_DIRECTION_LTR).unwrap();
        assert_eq!(row.len(), 2);
        assert_eq!(row[0], [0.0, 0.0]);
        // A と V の間は詰まる（字送りだけで並べた位置より左）
        assert!(row[1][0] < -1.0, "{row:?}");
        // 詰めない組み合わせは 0
        let row = shape_line(&f, &buf, b"HH", hb::HB_DIRECTION_LTR).unwrap();
        assert_eq!(row[1], [0.0, 0.0], "{row:?}");
        // 改行などの制御文字は数えない
        let row = shape_line(&f, &buf, b"A\tV", hb::HB_DIRECTION_LTR).unwrap();
        assert_eq!(row.len(), 2);
    }

    #[test]
    fn missing_font_is_none() {
        font::init(std::path::Path::new("Z:/__no_such_dir__"));
        assert!(font::load("__FlowType_H_no_such_font__", false, false).unwrap().is_none());
    }
}

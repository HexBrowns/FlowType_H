//! 「テキストを文字ごとに分解」。原作 editors/.../object/intern/text.cpp の移植。
//!
//! 選んだテキストオブジェクトを消し、同じ場所に グループ制御（標準描画の位置・回転・拡大率を引き継ぐ）→
//! グループ制御（中心の分だけ戻す）→ 1 文字ずつのテキスト を作る。文字の位置は HarfBuzz で組んで決める。

use harfbuzz_sys as hb;

use crate::alias::{Effect, Object};
use crate::font::{self, Buffer};
use crate::math::lerp;
use crate::utf8::{is_ctrl, next};
use crate::EDIT_HANDLE;

/// オブジェクトメニューから呼ばれる。
pub fn run() {
    let result = EDIT_HANDLE.call_edit_section(|edit| split_text(edit));
    if let Err(e) = result {
        tracing::error!("Failed to call edit section: {e}");
    }
}

fn delete_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pos = 0;
    loop {
        let Some(st) = s[pos..].find('<').map(|i| i + pos) else {
            out.push_str(&s[pos..]);
            break;
        };
        out.push_str(&s[pos..st]);
        let Some(ed) = s[st..].find('>').map(|i| i + st) else {
            out.push_str(&s[st..]);
            break;
        };
        pos = ed + 1;
    }
    out
}

/// `[Object.n]` の n を offset だけずらす（後ろに付いていた効果を、作るグループ制御の後ろへ移す）。
fn shift_object_indices(s: &str, offset: i32) -> String {
    const TARGET: &str = "[Object.";
    let mut out = String::with_capacity(s.len());
    let mut pos = 0;
    loop {
        let Some(section) = s[pos..].find(TARGET).map(|i| i + pos) else {
            out.push_str(&s[pos..]);
            break;
        };
        out.push_str(&s[pos..section]);
        let st = section + TARGET.len();
        let Some(ed) = s[st..].find(']').map(|i| i + st) else {
            out.push_str(&s[section..]);
            break;
        };
        match s[st..ed].parse::<i32>() {
            Ok(index) => {
                out.push_str("[Object.");
                out.push_str(&(index + offset).to_string());
                out.push(']');
            }
            Err(_) => out.push_str(&s[section..=ed]),
        }
        pos = ed + 1;
    }
    out
}

/// 「文字揃え」（`中央揃え[中]` `縦書 中央[右]` など）を原作の番号（0〜17）にする。
fn alignment_base(alignment: &str) -> i32 {
    // 原作は末尾の `]` を落としてから、縦書きなら先頭の「縦書 」も落として見る
    let a = alignment.strip_suffix(']').unwrap_or(alignment);
    if let Some(rest) = a.strip_prefix("縦書 ") {
        let y = if rest.ends_with('左') { 2 } else if rest.ends_with('中') { 1 } else { 0 };
        let x = if rest.starts_with('下') { 2 } else if rest.starts_with('中') { 1 } else { 0 };
        x + y * 3 + 9
    } else {
        let y = if a.ends_with('下') { 2 } else if a.ends_with('中') { 1 } else { 0 };
        let x = if a.starts_with('右') { 2 } else if a.starts_with('中') { 1 } else { 0 };
        x + y * 3
    }
}

/// 原作の `layout_text`。文字（制御文字を除く）ごとの位置を返す。
fn layout_text(text: &str, name: &str, size: f64, alignment: &str, is_bold: bool, is_italic: bool) -> Result<Vec<[f64; 2]>, String> {
    let base = alignment_base(alignment);
    let direction = if base >= 9 { hb::HB_DIRECTION_TTB } else { hb::HB_DIRECTION_LTR };
    let ltr = direction == hb::HB_DIRECTION_LTR;

    let font = font::load(name, is_bold, is_italic)?.ok_or("Font file not found")?;
    let scale = (size * 64.0) as i32;
    unsafe { hb::hb_font_set_scale(font.font, scale, scale) };
    let buffer = Buffer::new();

    let mut extents: hb::hb_font_extents_t = unsafe { std::mem::zeroed() };
    unsafe { hb::hb_font_get_extents_for_direction(font.font, direction, &mut extents) };
    let line_height = (extents.ascender - extents.descender + extents.line_gap) as f64 / 64.0;

    let mut rows: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut line_origin = [0.0f64; 2];
    let bytes = text.as_bytes();
    let mut remaining: &[u8] = bytes;

    while !remaining.is_empty() {
        let (line, rest) = match remaining.iter().position(|&b| b == b'\n') {
            Some(nl) => (&remaining[..nl], &remaining[nl + 1..]),
            None => (remaining, &[][..]),
        };
        remaining = rest;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            if ltr {
                line_origin[1] -= line_height;
            } else {
                line_origin[0] -= line_height;
            }
            continue;
        }

        let infos = unsafe {
            hb::hb_buffer_reset(buffer.0);
            hb::hb_buffer_add_utf8(buffer.0, line.as_ptr() as *const std::ffi::c_char, line.len() as i32, 0, line.len() as i32);
            hb::hb_buffer_set_direction(buffer.0, direction);
            hb::hb_buffer_guess_segment_properties(buffer.0);
            hb::hb_shape(font.font, buffer.0, std::ptr::null(), 0);
            let mut count = 0u32;
            let info = hb::hb_buffer_get_glyph_infos(buffer.0, &mut count);
            if info.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(info, count as usize).to_vec()
            }
        };

        let mut row: Vec<[f64; 2]> = Vec::with_capacity(infos.len());
        let mut nominal = [0.0f64; 2];
        for info in &infos {
            let (cp, _) = next(line, info.cluster as usize).ok_or("Character encoding is unsupported in the text")?;
            if is_ctrl(cp) {
                continue;
            }
            row.push([line_origin[0] + nominal[0], line_origin[1] + nominal[1]]);
            if ltr {
                nominal[0] += unsafe { hb::hb_font_get_glyph_h_advance(font.font, info.codepoint) } as f64 / 64.0;
            } else {
                nominal[1] += unsafe { hb::hb_font_get_glyph_v_advance(font.font, info.codepoint) } as f64 / 64.0;
            }
        }

        if !row.is_empty() {
            let n = row.len() - 1;
            match base {
                1 | 4 | 7 => {
                    let origin = lerp(row[n][0], nominal[0], 0.5) - row[n][0] * 0.5;
                    for i in 0..n {
                        row[i][0] = lerp(row[i][0], row[i + 1][0], 0.5) - origin;
                    }
                    row[n][0] *= 0.5;
                }
                2 | 5 | 8 => {
                    let origin = nominal[0];
                    for i in 0..n {
                        row[i][0] = row[i + 1][0] - origin;
                    }
                    row[n][0] = 0.0;
                }
                10 | 13 | 16 => {
                    let origin = lerp(row[n][1], nominal[1], 0.5) - row[n][1] * 0.5;
                    for i in 0..n {
                        row[i][1] = lerp(row[i][1], row[i + 1][1], 0.5) - origin;
                    }
                    row[n][1] *= 0.5;
                }
                11 | 14 | 17 => {
                    let origin = nominal[1];
                    for i in 0..n {
                        row[i][1] = row[i + 1][1] - origin;
                    }
                    row[n][1] = 0.0;
                }
                _ => {}
            }
        }
        rows.push(row);

        if ltr {
            line_origin[1] -= line_height;
        } else {
            line_origin[0] -= line_height;
        }
    }

    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let mut origin = [0.0f64; 2];
    match base {
        3..=5 => origin[1] = (line_origin[1] + line_height) * 0.5,
        6..=8 => origin[1] = line_origin[1] + line_height,
        12..=14 => origin[0] = (line_origin[0] + line_height) * 0.5,
        15..=17 => origin[0] = line_origin[0] + line_height,
        _ => {}
    }
    Ok(rows.iter().flatten().map(|p| [p[0] - origin[0], p[1] - origin[1]]).collect())
}

/// 原作の `negate`（先頭が `-` なら外し、`0` で始まるならそのまま、それ以外は `-` を付ける）
fn negate(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('-') {
        rest.to_string()
    } else if s.starts_with('0') {
        s.to_string()
    } else {
        format!("-{s}")
    }
}

#[allow(clippy::too_many_arguments)]
fn group_alias(meta: &str, x: &str, y: &str, z: &str, rx: &str, ry: &str, rz: &str, zoom: &str, layers: usize, tail: &str) -> String {
    format!(
        "{meta}[Object.0]\neffect.name=グループ制御\nX={x}\nY={y}\nZ={z}\nGroup=1\nX軸回転={rx}\nY軸回転={ry}\nZ軸回転={rz}\n拡大率={zoom}\n対象レイヤー数={layers}\n{tail}"
    )
}

/// 元のテキストが `layer` にあるとき、空いている必要があるレイヤー（内側のグループ制御 1 つと `chars` 文字ぶん）。
/// 外側のグループ制御は元のテキストのレイヤーに置くので入らない
fn target_layers(layer: usize, chars: usize) -> std::ops::RangeInclusive<usize> {
    layer + 1..=layer + 1 + chars
}

/// レイヤーを左から辿り、`[start, end]`（両端を含む）にかかる最初のオブジェクトの範囲を返す。
///
/// `find(from)` は `find_object_after(layer, from)` で見つけたオブジェクトの範囲。SDK（`plugin2.h` の `find_object`）の
/// 説明は「指定のフレーム番号以降にあるオブジェクトを検索」だけで、開始が `from` より前にあって `from` にかかる
/// オブジェクトを返すかは書かれていない（aviutl2-rs 0.48 の `find_object_after` も素通し）。
/// `find(start)` だけを見ると、返さない場合に開始が前にあって範囲にかかるものを見落とすので、どちらでも漏れないように
/// レイヤーの先頭（0）から辿る。
fn first_overlap(mut find: impl FnMut(usize) -> Option<(usize, usize)>, start: usize, end: usize) -> Option<(usize, usize)> {
    let mut from = 0;
    loop {
        let (s, e) = find(from)?;
        if s > end {
            return None;
        }
        if e >= start {
            return Some((s, e));
        }
        if e < from {
            // 先へ進まない（起きないはず）。空いているとは言い切れないので、かかっているとみなす
            return Some((s, e));
        }
        from = e + 1;
    }
}

/// エイリアスから `layer` の `start` に長さ `len` で作り、その位置に置かれたかを読み返す。
/// 長さ 0（自動）だと、重なっても失敗せずに本体が空いている位置へずらして置くので、長さを指定する（ルール au2-rs-plugin）
fn place(edit: &aviutl2::generic::EditSection, alias: &str, layer: usize, start: usize, len: usize) -> Result<aviutl2::generic::ObjectHandle, String> {
    let h = edit.create_object_from_alias(alias, layer, start, len).map_err(|e| e.to_string())?;
    let end = start + len - 1;
    match edit.get_object_layer_frame(h) {
        Ok(f) if (f.layer, f.start, f.end) == (layer, start, end) => Ok(h),
        Ok(f) => Err(format!("{start}〜{end} に置いたつもりが {} レイヤー目の {}〜{} に置かれました", f.layer + 1, f.start, f.end)),
        Err(e) => Err(format!("作ったオブジェクトの位置を読み返せません: {e}")),
    }
}

fn split_text(edit: &mut aviutl2::generic::EditSection) {
    let handle = match edit.get_selected_objects() {
        Ok(v) if !v.is_empty() => {
            // 分解は 1 回に 1 個（原作と同じ）。黙って残りを捨てないように知らせる
            if v.len() > 1 {
                tracing::warn!("テキストを文字ごとに分解: {} 個選ばれていますが、分解するのは 1 個目だけです", v.len());
            }
            v[0]
        }
        _ => {
            tracing::error!("Failed to get the handle of the object");
            return;
        }
    };
    let Ok(alias) = edit.get_object_alias(handle) else {
        tracing::error!("Failed to get the alias of the object");
        return;
    };
    let object = Object::new(alias);
    let (meta, fx_text, fx_xform) = (object.get(-1), object.get(0), object.get(1));
    if meta.alias().is_empty() || fx_text.alias().is_empty() || fx_xform.alias().is_empty() {
        tracing::error!("The alias does not match the expected format");
        return;
    }
    let remaining = object.alias().find("[Object.2]").map(|p| &object.alias()[p..]).unwrap_or("");

    if fx_text.get("effect.name", "") != "テキスト" {
        tracing::error!("Text object is not selected");
        return;
    }
    let raw = fx_text.get("テキスト", "");
    if raw.is_empty() {
        return;
    }
    let text = delete_tags(&Effect::unescape(raw));
    if text.is_empty() {
        return;
    }

    let font_name = fx_text.get("フォント", "");
    let size: f64 = match fx_text.front("サイズ", "").parse::<f64>() {
        Ok(s) if !font_name.is_empty() && s >= 1.0e-6 => s,
        _ => {
            tracing::error!("Font name is empty or size is zero");
            return;
        }
    };
    let alignment = fx_text.get("文字揃え", "");
    let is_bold = fx_text.front("B", "") != "0";
    let is_italic = fx_text.front("I", "") != "0";

    let coords = match layout_text(&format!("{text}\n"), font_name, size, alignment, is_bold, is_italic) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e}");
            return;
        }
    };
    if coords.is_empty() {
        tracing::error!("Text layout computation failed");
        return;
    }

    let Ok(info) = edit.get_object_layer_frame(handle) else {
        tracing::error!("Failed to get the handle of the object");
        return;
    };

    let bytes = text.as_bytes();
    let mut chars: Vec<&str> = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let Some((cp, end)) = next(bytes, pos) else {
            tracing::error!("Character encoding is unsupported in the text");
            return;
        };
        if !is_ctrl(cp) {
            chars.push(&text[pos..end]);
        }
        pos = end;
    }
    if chars.len() == 1 {
        return;
    }
    if chars.len() != coords.len() {
        tracing::error!("Character count mismatch between layout and text");
        return;
    }

    // 元を消す前に、作るもの（内側のグループ制御と 1 文字ずつのテキスト）を置く場所が全部空いているかを確かめる。
    // 外側のグループ制御は元のテキストの場所（消せば空く）に置く
    let len = info.end - info.start + 1;
    for layer in target_layers(info.layer, chars.len()) {
        let found = first_overlap(
            |from| {
                let h = edit.find_object_after(layer, from).ok().flatten()?;
                edit.get_object_layer_frame(h).ok().map(|f| (f.start, f.end))
            },
            info.start,
            info.end,
        );
        if let Some((s, e)) = found {
            tracing::error!(
                "テキストを文字ごとに分解: {} レイヤー目の {}〜{} に別のオブジェクト（{s}〜{e}）があります。何も変えていません",
                layer + 1,
                info.start,
                info.end
            );
            return;
        }
    }

    let meta_alias = meta.alias();
    if edit.delete_object(handle).is_err() {
        tracing::error!("Failed to delete the object");
        return;
    }
    // ここから先で失敗したら、元のテキストは消えたまま途中で止まる。1 回の編集なので Ctrl+Z で戻せることを知らせる
    let failed = |what: &str, layer: usize, e: String| {
        tracing::error!(
            "テキストを文字ごとに分解: 元のテキストを消した後、{what}（{} レイヤー目）を作れませんでした: {e}。途中で止めました。Ctrl+Z で分解する前に戻せます",
            layer + 1
        );
    };

    let outer = group_alias(
        meta_alias,
        fx_xform.get("X", "0.00"),
        fx_xform.get("Y", "0.00"),
        fx_xform.get("Z", "0.00"),
        fx_xform.get("X軸回転", "0.00"),
        fx_xform.get("Y軸回転", "0.00"),
        fx_xform.get("Z軸回転", "0.00"),
        fx_xform.get("拡大率", "100.000"),
        chars.len() + 1,
        &shift_object_indices(remaining, -1),
    );
    match place(edit, &outer, info.layer, info.start, len) {
        Ok(h) => {
            let _ = edit.set_focus_object(Some(h));
        }
        Err(e) => return failed("外側のグループ制御", info.layer, e),
    }

    let inner = group_alias(
        meta_alias,
        &negate(fx_xform.get("中心X", "0.00")),
        &negate(fx_xform.get("中心Y", "0.00")),
        &negate(fx_xform.get("中心Z", "0.00")),
        "0.00",
        "0.00",
        "0.00",
        "100.000",
        chars.len(),
        "",
    );
    if let Err(e) = place(edit, &inner, info.layer + 1, info.start, len) {
        return failed("内側のグループ制御", info.layer + 1, e);
    }

    for (i, ch) in chars.iter().enumerate() {
        let alias = format!(
            "{meta_alias}[Object.0]\neffect.name=テキスト\nサイズ={size}\n字間=0.00\n行間=0.00\n表示速度=0.00\nフォント={font_name}\n文字色={}\n影・縁色={}\n文字装飾={}\n文字揃え={alignment}\nB={}\nI={}\nテキスト={ch}\n文字毎に個別オブジェクト=0\n自動スクロール=0\n移動座標上に表示=0\nオブジェクトの長さを自動調節=0\n[Object.1]\neffect.name=標準描画\nX={}\nY={}\nZ=0.00\nGroup=1\n中心X=0.00\n中心Y=0.00\n中心Z=0.00\nX軸回転=0.00\nY軸回転=0.00\nZ軸回転=0.00\nGroup2=1\n拡大率=100.000\n縦横比={}\n透明度={}\n合成モード={}",
            fx_text.get("文字色", ""),
            fx_text.get("影・縁色", ""),
            fx_text.get("文字装飾", ""),
            fx_text.get("B", ""),
            fx_text.get("I", ""),
            coords[i][0],
            -coords[i][1],
            fx_xform.get("縦横比", "0.000"),
            fx_xform.get("透明度", "0.00"),
            fx_xform.get("合成モード", "通常"),
        );
        if let Err(e) = place(edit, &alias, info.layer + i + 2, info.start, len) {
            return failed(&format!("{} 文字目のテキスト「{ch}」", i + 1), info.layer + i + 2, e);
        }
    }

    tracing::info!("Converted '{text}' into per-character objects");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(delete_tags("a<#ff0000>b<s20>c"), "abc");
        assert_eq!(delete_tags("a<b"), "a<b");
        assert_eq!(shift_object_indices("[Object.2]\nx=1\n[Object.3]\n", -1), "[Object.1]\nx=1\n[Object.2]\n");
        assert_eq!(negate("-1.00"), "1.00");
        assert_eq!(negate("0.00"), "0.00");
        assert_eq!(negate("5.00"), "-5.00");
        assert_eq!(alignment_base("左寄せ[上]"), 0);
        assert_eq!(alignment_base("中央揃え[中]"), 4);
        assert_eq!(alignment_base("右寄せ[下]"), 8);
        assert_eq!(alignment_base("縦書 上寄[右]"), 9);
        assert_eq!(alignment_base("縦書 中央[中]"), 13);
        assert_eq!(alignment_base("縦書 下寄[左]"), 17);
    }

    #[test]
    fn target_layers_cover_inner_group_and_each_char() {
        // 元が 3 レイヤー目（2）で 4 文字: 内側のグループ制御が 3、文字が 4〜7
        assert_eq!(target_layers(2, 4), 3..=7);
        assert_eq!(target_layers(0, 2).count(), 3);
    }

    /// 同じレイヤーの並び（開始順）に対する `find_object_after` の 2 通りの解釈
    fn finder(objects: &'static [(usize, usize)], covering: bool) -> impl FnMut(usize) -> Option<(usize, usize)> {
        move |from| objects.iter().copied().find(|&(s, e)| if covering { e >= from } else { s >= from })
    }

    #[test]
    fn first_overlap_finds_objects_starting_before_the_range() {
        const OBJECTS: &[(usize, usize)] = &[(0, 9), (20, 40), (50, 60)];
        for covering in [false, true] {
            // 開始が前にあって範囲にかかる
            assert_eq!(first_overlap(finder(OBJECTS, covering), 30, 35), Some((20, 40)), "covering={covering}");
            assert_eq!(first_overlap(finder(OBJECTS, covering), 35, 55), Some((20, 40)), "covering={covering}");
            // 端が接するだけでも重なる（両端を含む）
            assert_eq!(first_overlap(finder(OBJECTS, covering), 40, 45), Some((20, 40)), "covering={covering}");
            assert_eq!(first_overlap(finder(OBJECTS, covering), 45, 50), Some((50, 60)), "covering={covering}");
            // 隙間
            assert_eq!(first_overlap(finder(OBJECTS, covering), 10, 19), None, "covering={covering}");
            assert_eq!(first_overlap(finder(OBJECTS, covering), 41, 49), None, "covering={covering}");
            assert_eq!(first_overlap(finder(OBJECTS, covering), 61, 100), None, "covering={covering}");
            assert_eq!(first_overlap(finder(&[], covering), 0, 100), None, "covering={covering}");
        }
        // 前の検査（find(start) の 1 つだけを見る）は、開始より後のものしか返さない解釈だと (20, 40) を見落とす
        let mut f = finder(OBJECTS, false);
        assert_eq!(f(30), Some((50, 60)));
    }

    /// 実際のフォント（Arial）で並べる。
    #[test]
    fn layout_with_a_real_font() {
        crate::font::init(std::path::Path::new("Z:/__no_such_dir__"));
        // 左寄せ[上]: 1 行目は y = 0、2 行目は 1 行ぶん下（負）。x は字送りの累積
        let c = layout_text("ab
cd
", "Arial", 100.0, "左寄せ[上]", false, false).unwrap();
        assert_eq!(c.len(), 4);
        assert_eq!(c[0], [0.0, 0.0]);
        assert!(c[1][0] > 0.0 && c[1][1] == 0.0, "{c:?}");
        assert!(c[2][0] == 0.0 && c[2][1] < -50.0, "{c:?}");
        // 中央揃え[中]: 1 行の左右が対称（a と b の中心が 0 をはさむ）
        let c = layout_text("HH
", "Arial", 100.0, "中央揃え[中]", false, false).unwrap();
        assert_eq!(c.len(), 2);
        assert!((c[0][0] + c[1][0]).abs() < 1e-9, "{c:?}");
        // 制御文字（タブ）は数えない
        let c = layout_text("a	b
", "Arial", 100.0, "左寄せ[上]", false, false).unwrap();
        assert_eq!(c.len(), 2);
    }
}

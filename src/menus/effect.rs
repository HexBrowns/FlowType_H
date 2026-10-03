//! 「エフェクトをコピー」。原作 editors/.../property/intern/effect.cpp の移植。
//!
//! INI 形式はエイリアスの `[Object.n]` を `[Effect.0]` にしたもの（.effect と同じ形）。TOML 形式は
//! モーション@FlowType_H の「エフェクト::パラメータ」に貼れる形（設定項目の種別で値の書き方を変える）。
//! 原作は対象の効果が見つからないと止まらずに回り続けた。こちらは効果が尽きたら諦める。

use std::collections::HashMap;

use aviutl2::generic::{EffectItemType, ObjectHandle};

use super::{clipboard, effect_key};
use crate::alias::Object;
use crate::EDIT_HANDLE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Ini,
    Toml,
}

pub fn copy(format: Format, handle: ObjectHandle, effect: &str, suffix: usize) {
    let fx = effect_key(effect, suffix);
    let alias = EDIT_HANDLE.call_read_section(move |read| read.get_object_alias(handle).ok()).ok().flatten();
    let Some(alias) = alias else {
        tracing::error!("Failed to get the alias of the object");
        return;
    };
    let object = Object::new(alias);

    // 同じ名前の効果の suffix 番目を探す
    let mut index = 0;
    let mut count = 0;
    let found = loop {
        let e = object.get(index);
        if e.alias().is_empty() {
            break None;
        }
        if e.get("effect.name", "") == effect {
            if count == suffix {
                break Some(e);
            }
            count += 1;
        }
        index += 1;
    };
    let Some(found) = found else {
        tracing::error!("Effect '{fx}' was not found in the object");
        return;
    };
    let head_len = format!("[Object.{index}]").len();

    let result = match format {
        Format::Ini => format!("[Effect.0]{}", &found.alias()[head_len..]),
        Format::Toml => {
            let types: HashMap<String, EffectItemType> = EDIT_HANDLE
                .get_effect_items(effect)
                .map(|items| items.into_iter().map(|i| (i.name, i.item_type)).collect())
                .unwrap_or_default();
            let tmp = format!("[\"{effect}\"]{}", &found.alias()[head_len..]);
            to_toml(&tmp, &types)
        }
    };

    let kind = if format == Format::Toml { "TOML" } else { "INI" };
    if clipboard::set_text(&result) {
        tracing::info!("Copied '{fx}' to the clipboard in {kind} format");
    } else {
        tracing::error!("Failed to copy '{fx}' to the clipboard in {kind} format");
    }
}

/// エイリアスの 1 効果分を TOML の表に書き直す。
fn to_toml(tmp: &str, types: &HashMap<String, EffectItemType>) -> String {
    let mut result = String::with_capacity(tmp.len());
    let mut remaining = tmp;
    while !remaining.is_empty() {
        let (line, rest) = match remaining.find('\n') {
            Some(nl) => (&remaining[..nl], &remaining[nl + 1..]),
            None => (remaining, ""),
        };
        remaining = rest;
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            result.push('\n');
            continue;
        }
        if line.starts_with("effect.name") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        let pair = line.split(',').next().unwrap_or(line);
        let Some(eq) = pair.find('=') else {
            result.push_str(pair);
            result.push('\n');
            continue;
        };
        let (k, v) = (&pair[..eq], &pair[eq + 1..]);
        if k.ends_with(".hide") {
            continue;
        }
        let line = match types.get(k) {
            Some(EffectItemType::Color) => format!("\"{k}\"=0x{v}"),
            Some(EffectItemType::Check) => format!("\"{k}\"={}", if v == "0" { "false" } else { "true" }),
            Some(EffectItemType::Integer) => format!("\"{k}\"={v}"),
            Some(EffectItemType::Number) => format!("\"{k}\"={}", trim_zeros(v)),
            _ => format!("\"{k}\"=\"{v}\""),
        };
        result.push_str(&line);
        result.push('\n');
    }
    result
}

/// 小数の末尾の 0 を落とす（`1.500` → `1.5`、`2.000` → `2.0`）。原作と同じく、小数点の直後の 0 は 1 つ残す。
fn trim_zeros(v: &str) -> &str {
    if !v.contains('.') {
        return v;
    }
    match v.rfind(|c| c != '0') {
        Some(ed) if v.as_bytes()[ed] == b'.' => &v[..ed + 2],
        Some(ed) => &v[..ed + 1],
        None => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_lines() {
        assert_eq!(trim_zeros("1.500"), "1.5");
        assert_eq!(trim_zeros("2.000"), "2.0");
        assert_eq!(trim_zeros("10"), "10");
        let mut types = HashMap::new();
        types.insert("範囲".to_string(), EffectItemType::Number);
        types.insert("色".to_string(), EffectItemType::Color);
        types.insert("有効".to_string(), EffectItemType::Check);
        let out = to_toml("[\"ぼかし\"]\neffect.name=ぼかし\n範囲=5.000,10.000,直線移動\n色=ff0000\n有効=1\nグループ.hide=1\n", &types);
        assert_eq!(out, "[\"ぼかし\"]\n\"範囲\"=5.0\n\"色\"=0xff0000\n\"有効\"=true\n");
    }
}

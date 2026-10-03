//! 「現在値で上書き」「値を反転」。原作 editors/.../property/intern/value.cpp の移植。
//!
//! 設定項目の値（`開始,…,終了,移動方法,…|設定`）を区間ごとに書き換える。トラックバー（整数・数値）と
//! チェックボックスだけが対象。

use aviutl2::generic::{EffectItemType, ObjectHandle};

use super::{effect_key, item_type};
use crate::EDIT_HANDLE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Current,
    All,
    Preceding,
    Subsequent,
}

fn is_subject(t: EffectItemType) -> bool {
    matches!(t, EffectItemType::Integer | EffectItemType::Number | EffectItemType::Check)
}

/// `string::to_number(s, int)`（全体が整数として読めたときだけ）
fn to_int(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// `cfg.substr(1, cfg.find('|', 1) - 1)`
fn cfg_head(cfg: &str) -> &str {
    let rest = cfg.get(1..).unwrap_or("");
    match rest.find('|') {
        Some(e) => &rest[..e],
        None => rest,
    }
}

/// 値の文字列を (データ, `|` から後ろ) に分ける。
fn split_props(props: &str) -> (&str, &str) {
    match props.find('|') {
        Some(sep) => (&props[..sep], &props[sep..]),
        None => (props, ""),
    }
}

/// 原作の `values | take(ed + 1) | drop(st)` が空か。
fn targets_empty(len: usize, st: i64, ed: i64) -> bool {
    let take = (ed + 1).clamp(0, len as i64);
    take <= st
}

pub fn overwrite(range: Range, handle: ObjectHandle, effect: &str, index: usize, item: &str) {
    let Some(ty) = item_type(effect, item) else {
        return;
    };
    if !is_subject(ty) {
        return;
    }
    let fx = effect_key(effect, index);
    let (effect, item) = (effect.to_string(), item.to_string());
    let _ = EDIT_HANDLE.call_edit_section(move |edit| {
        let props = edit.get_object_effect_item(handle, &effect, index, &item).unwrap_or_default();
        if props.is_empty() {
            tracing::error!("Failed to get the value");
            return;
        }
        let Ok(Some(focus)) = edit.get_focus_object_section() else {
            tracing::error!("Failed to get the focus section");
            return;
        };
        let mut index_s = focus as i64;
        let mut num = edit.get_object_section_num(handle).unwrap_or(0) as i64;

        let (data, cfg) = split_props(&props);
        let values: Vec<&str> = data.split(',').collect();
        if values.len() < 2 {
            return;
        }
        let commas = data.matches(',').count() as i64;

        if ty == EffectItemType::Check {
            num -= 1;
            if commas != num {
                return;
            }
            if !cfg.is_empty() {
                if let Some(flag) = to_int(cfg_head(cfg)) {
                    if flag & 1 == 0 {
                        return;
                    }
                }
            }
        } else {
            if commas != 3 && commas != num + 2 {
                return;
            }
            if commas == 3 && num > 1 {
                if let Some(flag) = to_int(&data[data.rfind(',').unwrap() + 1..]) {
                    if flag & 4 != 0 {
                        index_s = 0;
                        num = 1;
                    }
                }
            }
        }

        let (st, ed) = match range {
            Range::All => (0, num),
            Range::Subsequent => (index_s, num),
            Range::Preceding => (0, index_s),
            Range::Current => (0, 0),
        };
        if targets_empty(values.len(), st, ed) {
            return;
        }
        let Some(src) = values.get(index_s as usize).copied() else {
            return;
        };
        let mut result = values
            .iter()
            .enumerate()
            .map(|(i, v)| if (i as i64) >= st && (i as i64) <= ed { src } else { *v })
            .collect::<Vec<_>>()
            .join(",");
        result.push_str(cfg);

        if edit.set_object_effect_item(handle, &effect, index, &item, &result).is_ok() {
            tracing::info!("Updated '{fx}:{item}'");
        }
    });
}

pub fn invert(range: Range, handle: ObjectHandle, effect: &str, index: usize, item: &str) {
    let Some(ty) = item_type(effect, item) else {
        return;
    };
    if !is_subject(ty) {
        return;
    }
    let fx = effect_key(effect, index);
    let (effect, item) = (effect.to_string(), item.to_string());
    let _ = EDIT_HANDLE.call_edit_section(move |edit| {
        let mut range = range;
        let props = edit.get_object_effect_item(handle, &effect, index, &item).unwrap_or_default();
        if props.is_empty() {
            tracing::error!("Failed to get the value");
            return;
        }
        let Ok(Some(focus)) = edit.get_focus_object_section() else {
            tracing::error!("Failed to get the focus section");
            return;
        };
        let mut index_s = focus as i64;
        let mut num = edit.get_object_section_num(handle).unwrap_or(0) as i64;

        let (data, cfg) = split_props(&props);
        let values: Vec<&str> = data.split(',').collect();
        if values.len() < 2 {
            index_s = 0;
            num = 0;
        }
        let commas = data.matches(',').count() as i64;

        if ty == EffectItemType::Check {
            num -= 1;
            if commas != num {
                return;
            }
            if !cfg.is_empty() {
                if let Some(flag) = to_int(cfg_head(cfg)) {
                    if flag & 1 == 0 {
                        range = Range::All;
                    }
                }
            }
        } else if commas < 3 {
            index_s = 0;
            num = 0;
        } else if commas == 3 && num > 1 {
            if let Some(flag) = to_int(&data[data.rfind(',').unwrap() + 1..]) {
                if flag & 4 != 0 {
                    index_s = 0;
                    num = 1;
                }
            }
        } else if commas != num + 2 {
            return;
        }

        let (st, ed) = if num != 0 {
            match range {
                Range::Current => (index_s, index_s),
                Range::All => (0, num),
                Range::Subsequent => (index_s, num),
                Range::Preceding => (0, index_s),
            }
        } else {
            (0, 0)
        };
        if targets_empty(values.len(), st, ed) {
            return;
        }

        let in_range = |i: usize| (i as i64) >= st && (i as i64) <= ed;
        let mut result = match ty {
            EffectItemType::Integer | EffectItemType::Number => values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if !v.is_empty() && in_range(i) && !v.starts_with('0') {
                        match v.strip_prefix('-') {
                            Some(rest) => rest.to_string(),
                            None => format!("-{v}"),
                        }
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
            _ => values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if !v.is_empty() && in_range(i) {
                        if *v == "0" { "1" } else { "0" }.to_string()
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
        };
        result.push_str(cfg);

        if edit.set_object_effect_item(handle, &effect, index, &item, &result).is_ok() {
            tracing::info!("Updated '{fx}:{item}'");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(split_props("1,2,3|0|x"), ("1,2,3", "|0|x"));
        assert_eq!(cfg_head("|12|abc"), "12");
        assert_eq!(cfg_head("|12"), "12");
        assert!(targets_empty(3, 3, 5));
        assert!(!targets_empty(3, 0, 0));
    }
}

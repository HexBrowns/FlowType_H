//! 「プロパティ名をコピー」「参照式をコピー」。原作 editors/.../property/intern/expression.cpp の移植。

use aviutl2::generic::{EffectItemType, ObjectHandle};

use super::{clipboard, effect_key, item_type};
use crate::EDIT_HANDLE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Local,
    Object,
    Global,
}

fn copy(text: &str) {
    if clipboard::set_text(text) {
        tracing::info!("Copied '{text}' to the clipboard");
    } else {
        tracing::error!("Failed to copy '{text}' to the clipboard");
    }
}

pub fn copy_prop_name(scope: Scope, handle: ObjectHandle, effect: &str, index: usize, item: &str) {
    let fx = effect_key(effect, index);
    let name = match scope {
        Scope::Local => item.to_string(),
        Scope::Object => format!("{fx}.{item}"),
        Scope::Global => {
            let layer = EDIT_HANDLE
                .call_read_section(move |read| read.get_object_layer_frame(handle).map(|f| f.layer + 1).unwrap_or(0))
                .unwrap_or(0);
            format!("layer{layer}.{fx}.{item}")
        }
    };
    copy(&name);
}

pub fn copy_expression(handle: ObjectHandle, effect: &str, index: usize, item: &str) {
    let Some(ty) = item_type(effect, item) else {
        return;
    };
    if !matches!(ty, EffectItemType::Integer | EffectItemType::Number) {
        return;
    }
    let (effect, item) = (effect.to_string(), item.to_string());
    let props = EDIT_HANDLE
        .call_read_section(move |read| read.get_object_effect_item(handle, &effect, index, &item).unwrap_or_default())
        .unwrap_or_default();
    if props.is_empty() {
        tracing::error!("Failed to get the value");
        return;
    }
    let Some(sep) = props.find('|') else {
        tracing::warn!("Expression not found");
        return;
    };
    let (data, cfg) = (&props[..sep], &props[sep..]);
    if data.matches(',').count() < 2 {
        return;
    }
    if let Ok(flag) = data[data.rfind(',').unwrap() + 1..].parse::<i32>() {
        if flag & 8 == 0 {
            tracing::warn!("Expression not found");
            return;
        }
    }
    let rest = &cfg[1..];
    let result = match rest.find('|') {
        Some(e) => &rest[..e],
        None => rest,
    };
    copy(result);
}

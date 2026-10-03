//! メニュー。原作 plugins/editors の移植。
//!
//! メニューの名前は原作と同じ（もとから日本語）。原作は値の読み取りにも `call_edit_section` を使っていたが、
//! 読むだけのもの（参照式・名前・効果のコピー）は `call_read_section` にした（本体の Undo に触れない。
//! ルール au2-rs-plugin）。書き込み（値の上書き・反転、文字ごとの分解）はメニューを選んだときだけ `call_edit_section` を呼ぶ。

pub mod clipboard;
pub mod effect;
pub mod expression;
pub mod text_split;
pub mod value;

use aviutl2::generic::{EffectItemType, HostAppHandle};

use crate::EDIT_HANDLE;

/// 原作の `fx`（`効果名` か `効果名:n`）を作る。
pub fn effect_key(effect: &str, index: usize) -> String {
    if index == 0 {
        effect.to_string()
    } else {
        format!("{effect}:{index}")
    }
}

/// 効果の設定項目の種別（原作の `enum_effect_item` で名前が一致したもの）。
pub fn item_type(effect: &str, item: &str) -> Option<EffectItemType> {
    let items = EDIT_HANDLE.get_effect_items(effect).ok()?;
    items.into_iter().find(|i| i.name == item).map(|i| i.item_type)
}

pub fn register(host: &mut HostAppHandle) {
    use expression::Scope;
    use value::Range;

    host.register_object_menu("FlowType_H\\テキストを文字ごとに分解", text_split::run);

    for (name, range) in [
        ("FlowType_H\\現在値で上書き\\全ての区間", Range::All),
        ("FlowType_H\\現在値で上書き\\以前の区間", Range::Preceding),
        ("FlowType_H\\現在値で上書き\\以降の区間", Range::Subsequent),
    ] {
        host.register_object_item_menu(name, move |h, effect, index, item| value::overwrite(range, h, effect, index, item));
    }
    for (name, range) in [
        ("FlowType_H\\値を反転\\現在の区間", Range::Current),
        ("FlowType_H\\値を反転\\全ての区間", Range::All),
        ("FlowType_H\\値を反転\\以前の区間", Range::Preceding),
        ("FlowType_H\\値を反転\\以降の区間", Range::Subsequent),
    ] {
        host.register_object_item_menu(name, move |h, effect, index, item| value::invert(range, h, effect, index, item));
    }

    for (name, scope) in [
        ("FlowType_H\\プロパティ名をコピー\\{プロパティ名}", Scope::Local),
        ("FlowType_H\\プロパティ名をコピー\\{エフェクト名}.{プロパティ名}", Scope::Object),
        ("FlowType_H\\プロパティ名をコピー\\{レイヤー名}.{エフェクト名}.{プロパティ名}", Scope::Global),
    ] {
        host.register_object_item_menu(name, move |h, effect, index, item| {
            expression::copy_prop_name(scope, h, effect, index, item)
        });
    }
    host.register_object_item_menu("FlowType_H\\参照式をコピー", expression::copy_expression);

    host.register_object_item_and_effect_menu(
        "FlowType_H\\エフェクトをコピー\\エイリアス形式 (INI 形式)",
        |h, effect, index, _item| effect::copy(effect::Format::Ini, h, effect, index),
    );
    host.register_object_item_and_effect_menu(
        "FlowType_H\\エフェクトをコピー\\FlowType_H 形式 (TOML 形式)",
        |h, effect, index, _item| effect::copy(effect::Format::Toml, h, effect, index),
    );
}

//! `UTF8@FlowType_H`。原作 modules/intern/text/intern/utf8.cpp の移植。

use super::{BAD_ENCODING, WRONG_COUNT};
use crate::smp::Smp;
use crate::utf8::{is_ctrl, next};

crate::module_fn!(count_fn, count);
crate::module_fn!(split_fn, split);

super::script_module!(Utf8Module, "UTF8", ["count" => count_fn, "split" => split_fn]);

/// count(text[, 制御文字を数えない]) → 文字数
fn count(p: &mut Smp) {
    let n = p.num();
    if n != 1 && n != 2 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let text = p.bytes(0);
    let pass_ctrl = p.boolean(1);
    if text.is_empty() {
        p.push_int(0);
        return;
    }
    let mut count = 0;
    let mut pos = 0;
    while pos < text.len() {
        let Some((cp, end)) = next(&text, pos) else {
            p.set_error(BAD_ENCODING);
            return;
        };
        pos = end;
        if pass_ctrl && is_ctrl(cp) {
            continue;
        }
        count += 1;
    }
    p.push_int(count);
}

/// split(text[, 制御文字を除く]) → 1 文字ずつの配列
fn split(p: &mut Smp) {
    let n = p.num();
    if n != 1 && n != 2 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let text = p.bytes(0);
    let pass_ctrl = p.boolean(1);
    if text.is_empty() {
        p.push_array_string(&[]);
        return;
    }
    let mut chars = Vec::with_capacity(text.len() / 2);
    let mut pos = 0;
    while pos < text.len() {
        let Some((cp, end)) = next(&text, pos) else {
            // 原作はここだけ set_error ではなくログに出して何も返さない
            tracing::error!("{BAD_ENCODING}");
            return;
        };
        if !(pass_ctrl && is_ctrl(cp)) {
            chars.push(text[pos..end].to_vec());
        }
        pos = end;
    }
    p.push_array_string(&chars);
}

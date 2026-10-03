//! `Regex@FlowType_H`。原作 modules/intern/text/intern/regex.cpp の移植。
//!
//! 原作は RE2（UTF-8・先頭一致優先）。こちらは regex クレートの bytes 版で、同じく UTF-8 として読む。
//! どちらも後方参照や先読みを持たず、構文はほぼ同じ。`Match(text, pos, …)` と同じく、検索の開始位置を
//! 進めても `^` や `\b` は文字列全体を文脈に判定する（regex の `captures_read_at`）。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;
use regex::bytes::Regex;

use super::{BAD_ENCODING, WRONG_COUNT};
use crate::smp::Smp;
use crate::utf8::{is_ctrl, next};

crate::module_fn!(mark_fn, mark);

super::script_module!(RegexModule, "Regex", ["mark" => mark_fn]);

/// コンパイル済みの正規表現。失敗したものも覚える（毎フレーム警告を出し直すのは原作と同じ）。
static CACHE: LazyLock<Mutex<HashMap<Vec<u8>, Option<Arc<Regex>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const CACHE_LIMIT: usize = 1024;

pub fn reset() {
    CACHE.lock().clear();
}

fn compile(pattern: &[u8]) -> Option<Arc<Regex>> {
    let mut cache = CACHE.lock();
    if let Some(re) = cache.get(pattern) {
        return re.clone();
    }
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    let re = std::str::from_utf8(pattern).ok().and_then(|s| Regex::new(s).ok()).map(Arc::new);
    cache.insert(pattern.to_vec(), re.clone());
    re
}

/// mark(id, text, pattern[, group]) → 文字ごとに {当たったか, 制御文字か}
fn mark(p: &mut Smp) {
    let n = p.num();
    if n != 3 && n != 4 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let input = p.bytes(1);
    if input.is_empty() {
        return;
    }
    let pattern = p.bytes(2);
    if pattern.is_empty() {
        fallback(p, &input);
        return;
    }
    let idx = p.int(3);
    let Some(re) = compile(&pattern) else {
        tracing::warn!("Regex pattern syntax is incorrect");
        fallback(p, &input);
        return;
    };

    let mut hits = vec![false; input.len()];
    let nsubmatch = re.captures_len() as i32;
    let idx = if idx >= 0 && idx < nsubmatch { idx as usize } else { 0 };
    let mut locs = re.capture_locations();
    let mut pos = 0;
    while pos <= input.len() && re.captures_read_at(&mut locs, &input, pos).is_some() {
        // 指定のグループが空（当たっていない・長さ 0）なら全体を使う
        let (st, ed) = match locs.get(idx) {
            Some((s, e)) if e > s => (s, e),
            _ => locs.get(0).unwrap_or((pos, pos)),
        };
        for h in &mut hits[st..ed] {
            *h = true;
        }
        if ed > st {
            pos = ed;
            continue;
        }
        if st >= input.len() {
            break;
        }
        let Some((_, end)) = next(&input, st) else {
            p.set_error(BAD_ENCODING);
            return;
        };
        pos = end;
    }

    let mut pos = 0;
    while pos < input.len() {
        let Some((cp, end)) = next(&input, pos) else {
            p.set_error(BAD_ENCODING);
            return;
        };
        let hit = hits[pos..end].iter().any(|&h| h);
        p.push_array_boolean(&[hit, is_ctrl(cp)]);
        pos = end;
    }
}

/// パターンが空・不正のとき。全部の文字を「当たった」にする。
fn fallback(p: &mut Smp, input: &[u8]) {
    let mut pos = 0;
    while pos < input.len() {
        let Some((cp, end)) = next(input, pos) else {
            p.set_error(BAD_ENCODING);
            return;
        };
        p.push_array_boolean(&[true, is_ctrl(cp)]);
        pos = end;
    }
}

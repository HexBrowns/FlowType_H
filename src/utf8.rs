//! 原作が使う utfcpp の `utf8::next` と同じ判定で 1 文字ずつ読む。
//!
//! utfcpp は不正な並び（途中で切れた列・続きのバイトでない・冗長な表現・サロゲート・0x10FFFF 超）で例外を投げる。
//! Rust の UTF-8 検査も同じものを弾くので、先頭バイトから長さを決めて `std::str::from_utf8` に通す。

/// `pos` から 1 文字読み、(コードポイント, 次の位置) を返す。不正なら `None`。
pub fn next(s: &[u8], pos: usize) -> Option<(u32, usize)> {
    let lead = *s.get(pos)?;
    let len = match lead {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let end = pos.checked_add(len)?;
    if end > s.len() {
        return None;
    }
    let c = std::str::from_utf8(&s[pos..end]).ok()?.chars().next()?;
    Some((c as u32, end))
}

/// 原作が数えない制御文字（C0・DEL・C1）。
pub fn is_ctrl(cp: u32) -> bool {
    cp <= 0x1f || cp == 0x7f || (0x80..=0x9f).contains(&cp)
}

/// 文字の区切りを (開始, 終了, コードポイント) で返す。不正な並びがあれば `None`。
pub fn chars(s: &[u8]) -> Option<Vec<(usize, usize, u32)>> {
    let mut out = Vec::with_capacity(s.len() / 2 + 1);
    let mut pos = 0;
    while pos < s.len() {
        let (cp, end) = next(s, pos)?;
        out.push((pos, end, cp));
        pos = end;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_rejects() {
        assert_eq!(next("あ".as_bytes(), 0), Some((0x3042, 3)));
        assert_eq!(next(b"a", 0), Some((0x61, 1)));
        // 途中で切れた列・冗長な表現・サロゲート・続きのバイトで始まる
        assert_eq!(next(&[0xe3, 0x81], 0), None);
        assert_eq!(next(&[0xc0, 0x80], 0), None);
        assert_eq!(next(&[0xed, 0xa0, 0x80], 0), None);
        assert_eq!(next(&[0x80], 0), None);
        assert!(is_ctrl(0x0a) && is_ctrl(0x7f) && is_ctrl(0x85) && !is_ctrl(0xa0));
    }
}

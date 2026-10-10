//! フィルタ効果。原作 plugins/filters の移植。
//!
//! 効果名は v0.3.0 で日本語にした（Trim → トリミング / Deform → 変形 / Align → 整列 / Transform → トランスフォーム）。
//! 設定項目の名前・範囲・初期値は原作と同じ。ラベルは自作の決まり（HexScript）に合わせた（原作は「テキスト」）。

pub mod align;
pub mod deform;
pub mod transform;
pub mod trim;

pub const LABEL: &str = "HexScript";

pub fn information(name: &str) -> String {
    format!("{name}@FlowType_H v{} by HexBrowns (fork of FlowType_K by Korarei)", env!("CARGO_PKG_VERSION"))
}

/// 原作の `return false`（以降のフィルタと出力を止める）に当たる戻し方。エラーのログは出さない。
pub fn stop<U: aviutl2::filter::FilterUserdata>(video: &mut aviutl2::filter::FilterProcVideo<U>) -> aviutl2::AnyResult<()> {
    video.prevent_post_effect();
    Ok(())
}

/// 本体の呼び出しが失敗したとき。原作と同じくログに出して止める。
pub fn fail<U: aviutl2::filter::FilterUserdata>(
    video: &mut aviutl2::filter::FilterProcVideo<U>,
    message: &str,
) -> aviutl2::AnyResult<()> {
    tracing::error!("{message}");
    stop(video)
}

/// 本体の画像の一辺の上限（`obj.getinfo("image_max")` と同じ。ルール `au2-conventions`「API 重要事項」）
pub const MAX_IMAGE_SIZE: u32 = 16384;

/// 0 で埋めた画素（大きさだけを変えたいときに渡す。原作は nullptr を渡していた）。
///
/// 一辺が `MAX_IMAGE_SIZE` を超えるか、確保に失敗したら `None`。`vec!` は確保に失敗すると abort して本体ごと落ちる
/// （変形のスケールや位置を大きくすると数十 GB を求めることがある）ので、失敗を返せる `try_reserve_exact` で確保する。
pub fn zeros(width: u32, height: u32) -> Option<Vec<u8>> {
    zeroed_bytes(width, height, 4)
}

/// 作る画像が上限を超えたことを、効果ごとに初回だけログへ出す（毎フレーム出すとログが埋まる）
pub fn too_large_once(warned: &std::sync::atomic::AtomicBool, effect: &str, w: i64, h: i64) {
    if !warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tracing::warn!("{effect}: 結果の大きさ {w}x{h} が上限 {MAX_IMAGE_SIZE} を超えるので描きません（以降は出しません）");
    }
}

/// `width * height * bytes_per_pixel` バイトを 0 で確保する。上限と失敗は `zeros` と同じ
pub fn zeroed_bytes(width: u32, height: u32, bytes_per_pixel: usize) -> Option<Vec<u8>> {
    if width > MAX_IMAGE_SIZE || height > MAX_IMAGE_SIZE {
        return None;
    }
    let len = (width as usize).checked_mul(height as usize)?.checked_mul(bytes_per_pixel)?;
    let mut v = Vec::new();
    v.try_reserve_exact(len).ok()?;
    v.resize(len, 0);
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_limits() {
        assert_eq!(zeros(2, 3).map(|v| v.len()), Some(24));
        assert_eq!(zeroed_bytes(2, 3, 8).map(|v| v.len()), Some(48));
        // 一辺が上限を超えると確保せずに None（変形でスケール 10000% を FHD に掛けた大きさ）
        assert!(zeros(192_000, 108_000).is_none());
        assert!(zeros(MAX_IMAGE_SIZE + 1, 1).is_none());
        assert!(zeros(1, MAX_IMAGE_SIZE + 1).is_none());
    }
}

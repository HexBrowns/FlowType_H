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

/// 0 で埋めた画素（大きさだけを変えたいときに渡す。原作は nullptr を渡していた）。
pub fn zeros(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; width as usize * height as usize * 4]
}

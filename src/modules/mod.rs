//! スクリプトモジュール（`obj.module("UTF8@FlowType_H")` などで読むもの）。原作 plugins/modules の移植。
//!
//! 名前は原作と同じ（スクリプト `@FlowType_H.anm2` がこの名前で読む）。原作の `Text@FlowType_H`（property）は
//! 原作でも登録されていない（text.cpp でコメントアウト）ので移植していない。

pub mod hash;
pub mod island;
pub mod kerning;
pub mod regex;
pub mod toml;
pub mod utf8;
pub mod vector;

/// スクリプトモジュールの型を 1 つ作る。`$name` は `@FlowType_H` の前の部分。
macro_rules! script_module {
    ($ty:ident, $name:literal, [$($fname:literal => $f:path),* $(,)?]) => {
        #[aviutl2::plugin(ScriptModule)]
        pub struct $ty;

        impl aviutl2::module::ScriptModuleFunctions for $ty {
            fn functions() -> Vec<aviutl2::module::ModuleFunction> {
                vec![$(aviutl2::module::ModuleFunction { name: $fname.to_string(), func: $f }),*]
            }
        }

        impl aviutl2::module::ScriptModule for $ty {
            fn new(_info: aviutl2::AviUtl2Info) -> aviutl2::AnyResult<Self> {
                Ok(Self)
            }

            fn plugin_info(&self) -> aviutl2::module::ScriptModuleTable {
                aviutl2::module::ScriptModuleTable {
                    information: format!(
                        "{}@FlowType_H v{} by HexBrowns (fork of FlowType_K by Korarei)",
                        $name,
                        env!("CARGO_PKG_VERSION")
                    ),
                    functions: <Self as aviutl2::module::ScriptModuleFunctions>::functions(),
                }
            }
        }
    };
}

pub(crate) use script_module;

pub const WRONG_COUNT: &str = "Function call has wrong argument count";
pub const BAD_ENCODING: &str = "Character encoding is unsupported in the text";

/// 「キャッシュを破棄」。
pub fn clear_cache() {
    regex::reset();
    island::reset();
}

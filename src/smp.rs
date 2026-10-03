//! スクリプトモジュールの引数と戻り値（`SCRIPT_MODULE_PARAM`）を、原作の C++ と同じ形で読み書きする。
//!
//! aviutl2-rs の `ScriptModuleCallHandle` は引数の型を確かめてから読む（違えば `Err`）。原作は型を確かめずに
//! 本体の `get_param_*` をそのまま呼び、足りない引数や違う型は本体の既定（0・空・false）で受けている。
//! 振る舞いを原作に合わせるため、ここでは本体の関数ポインタを直接呼ぶ。

use std::ffi::{c_void, CStr, CString};

use aviutl2::sys::module2::SCRIPT_MODULE_PARAM;

pub struct Smp {
    p: *mut SCRIPT_MODULE_PARAM,
}

impl Smp {
    /// 引数の数を返す。
    pub fn num(&self) -> i32 {
        unsafe { ((*self.p).get_param_num)() }
    }

    pub fn int(&self, index: i32) -> i32 {
        unsafe { ((*self.p).get_param_int)(index) }
    }

    pub fn double(&self, index: i32) -> f64 {
        unsafe { ((*self.p).get_param_double)(index) }
    }

    /// 文字列の引数をバイト列で返す。本体が null を返したら空（原作の `as_string_view`）。
    pub fn bytes(&self, index: i32) -> Vec<u8> {
        let ptr = unsafe { ((*self.p).get_param_string)(index) };
        if ptr.is_null() {
            return Vec::new();
        }
        unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec()
    }

    pub fn data(&self, index: i32) -> *mut c_void {
        unsafe { ((*self.p).get_param_data)(index) }
    }

    pub fn boolean(&self, index: i32) -> bool {
        unsafe { ((*self.p).get_param_boolean)(index) }
    }

    pub fn array_double(&self, index: i32, key: i32) -> f64 {
        unsafe { ((*self.p).get_param_array_double)(index, key) }
    }

    pub fn push_int(&mut self, value: i32) {
        unsafe { ((*self.p).push_result_int)(value) }
    }

    pub fn push_double(&mut self, value: f64) {
        unsafe { ((*self.p).push_result_double)(value) }
    }

    pub fn push_string(&mut self, value: &[u8]) {
        let c = c_string(value);
        unsafe { ((*self.p).push_result_string)(c.as_ptr()) }
    }

    pub fn push_array_double(&mut self, values: &[f64]) {
        unsafe { ((*self.p).push_result_array_double)(values.as_ptr(), values.len() as i32) }
    }

    pub fn push_array_boolean(&mut self, values: &[bool]) {
        unsafe { ((*self.p).push_result_array_boolean)(values.as_ptr(), values.len() as i32) }
    }

    /// 文字列の配列を返す。空なら要素 0 の配列（原作は空のときダミーを 1 つ指して数 0 で渡している）。
    pub fn push_array_string(&mut self, values: &[Vec<u8>]) {
        let owned: Vec<CString> = values.iter().map(|v| c_string(v)).collect();
        let dummy = c_string(b"");
        let mut ptrs: Vec<*const std::ffi::c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        if ptrs.is_empty() {
            ptrs.push(dummy.as_ptr());
            unsafe { ((*self.p).push_result_array_string)(ptrs.as_ptr(), 0) }
        } else {
            unsafe { ((*self.p).push_result_array_string)(ptrs.as_ptr(), ptrs.len() as i32) }
        }
    }

    /// 文字列の連想配列を返す。
    pub fn push_table_string(&mut self, keys: &[Vec<u8>], values: &[Vec<u8>]) {
        let k: Vec<CString> = keys.iter().map(|v| c_string(v)).collect();
        let v: Vec<CString> = values.iter().map(|v| c_string(v)).collect();
        let kp: Vec<*const std::ffi::c_char> = k.iter().map(|c| c.as_ptr()).collect();
        let vp: Vec<*const std::ffi::c_char> = v.iter().map(|c| c.as_ptr()).collect();
        unsafe { ((*self.p).push_result_table_string)(kp.as_ptr(), vp.as_ptr(), kp.len() as i32) }
    }

    pub fn set_error(&mut self, message: &str) {
        let c = c_string(message.as_bytes());
        unsafe { ((*self.p).set_error)(c.as_ptr()) }
    }
}

/// C 文字列にする。途中の NUL はそこで切る（C++ の `c_str()` を渡したときと同じに見える）。
fn c_string(bytes: &[u8]) -> CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).unwrap_or_default()
}

/// 本体から呼ばれる関数の入口。panic を本体へ伝えず、スクリプトのエラーにする。
pub fn call(p: *mut SCRIPT_MODULE_PARAM, f: impl FnOnce(&mut Smp)) {
    if p.is_null() {
        return;
    }
    let mut smp = Smp { p };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut smp)));
    if let Err(e) = result {
        let message = e
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        tracing::error!("Panic in script module: {message}");
        smp.set_error("Internal error occurred in FlowType_H");
    }
}

/// `ModuleFunction` に渡す関数を作る。
#[macro_export]
macro_rules! module_fn {
    ($name:ident, $body:path) => {
        extern "C" fn $name(p: *mut ::aviutl2::sys::module2::SCRIPT_MODULE_PARAM) {
            $crate::smp::call(p, $body);
        }
    };
}

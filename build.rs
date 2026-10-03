//! シェーダーを fxc でコンパイルし、OUT_DIR/*.cso に置く（src から include_bytes! で埋め込む）。
//!
//! - shaders/blit.hlsl → blit.cso（トリミング。原作 plugins/filters/intern/trim/intern/shaders/blit.hlsl の写し）
//!
//! フラグは原作の plugins/cmake/CompileShaders.cmake と同じ。fxc は Windows SDK のものを新しい版から探す
//! （AI/plugins/MotionBlur_H/build.rs と同じ探し方）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_fxc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FXC") {
        return Some(PathBuf::from(p));
    }
    let root = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&root)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("x64").join("fxc.exe").is_file())
        .collect();
    versions.sort();
    versions.pop().map(|p| p.join("x64").join("fxc.exe"))
}

fn compile(fxc: &Path, src: &Path, profile: &str, out: &Path) {
    println!("cargo:rerun-if-changed={}", src.display());
    let status = Command::new(fxc)
        .args(["/nologo", "/T", profile, "/E", "main", "/O3", "/WX", "/Qstrip_reflect", "/Qstrip_debug", "/Fo"])
        .arg(out)
        .arg(src)
        .status()
        .expect("fxc を起動できない");
    assert!(status.success(), "fxc がシェーダーのコンパイルに失敗した: {}", src.display());
}

fn main() {
    println!("cargo:rerun-if-env-changed=FXC");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let fxc = find_fxc().expect("fxc.exe が見つからない（Windows SDK を入れるか、環境変数 FXC で場所を指定する）");
    compile(&fxc, Path::new("shaders/blit.hlsl"), "ps_5_0", &out_dir.join("blit.cso"));
}

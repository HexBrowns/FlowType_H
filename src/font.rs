//! フォントを DirectWrite で探して HarfBuzz の face にする。原作 plugins/intern/font の移植。
//!
//! - 探す範囲はシステムのフォントと `ProgramData\aviutl2\Font`（とその 1 つ下のフォルダ）の .ttf/.otf/.ttc/.otc
//! - 探し方は原作と同じ順: フル名 → ファミリー名＋太さ＋斜体（条件を 3 → 1 個に減らす）→ Win32 のファミリー名で同じく
//! - 見つけた面のファイルが 1 つのときだけ使う（原作と同じ）
//!
//! 原作はフォントファイルの中身を DirectWrite のストリームのまま HarfBuzz に渡していた。こちらは読んで
//! メモリに持つ（面ごとに 1 回）。見つからなかった結果も「キャッシュを破棄」まで覚える
//! （原作もフォント一覧は起動時に作ったきりなので、途中で入れたフォントが見えないのは同じ）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use harfbuzz_sys as hb;
use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Graphics::DirectWrite::*;

/// HarfBuzz の face と、その元になったフォントファイルの中身。
pub struct FontData {
    face: *mut hb::hb_face_t,
    blob: *mut hb::hb_blob_t,
    _bytes: Vec<u8>,
}

// hb_face_t は作った後は読むだけで、HarfBuzz はスレッドをまたいで使ってよいとしている
unsafe impl Send for FontData {}
unsafe impl Sync for FontData {}

impl Drop for FontData {
    fn drop(&mut self) {
        unsafe {
            hb::hb_face_destroy(self.face);
            hb::hb_blob_destroy(self.blob);
        }
    }
}

impl FontData {
    fn new(bytes: Vec<u8>, index: u32) -> Option<Self> {
        let blob = unsafe {
            hb::hb_blob_create(
                bytes.as_ptr() as *const std::ffi::c_char,
                bytes.len() as u32,
                hb::HB_MEMORY_MODE_READONLY,
                std::ptr::null_mut(),
                None,
            )
        };
        if blob.is_null() {
            return None;
        }
        let face = unsafe { hb::hb_face_create(blob, index) };
        if face.is_null() {
            unsafe { hb::hb_blob_destroy(blob) };
            return None;
        }
        Some(Self { face, blob, _bytes: bytes })
    }

    /// 新しい hb_font_t を作る（呼び出しごとに作り、`Font` の Drop で捨てる）。
    pub fn create_font(self: &Arc<Self>) -> Font {
        let font = unsafe { hb::hb_font_create(self.face) };
        Font { font, _data: Arc::clone(self) }
    }
}

pub struct Font {
    pub font: *mut hb::hb_font_t,
    _data: Arc<FontData>,
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { hb::hb_font_destroy(self.font) };
    }
}

/// HarfBuzz のバッファ。
pub struct Buffer(pub *mut hb::hb_buffer_t);

impl Buffer {
    pub fn new() -> Self {
        Self(unsafe { hb::hb_buffer_create() })
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { hb::hb_buffer_destroy(self.0) };
    }
}

struct State {
    factory: Option<IDWriteFactory5>,
    fonts: Option<IDWriteFontSet>,
    cache: HashMap<String, Option<Arc<FontData>>>,
}

// DirectWrite の共有ファクトリーとフォントセットはスレッドをまたいで使える。触るのはこの Mutex の中だけ
unsafe impl Send for State {}

static STATE: LazyLock<Mutex<State>> =
    LazyLock::new(|| Mutex::new(State { factory: None, fonts: None, cache: HashMap::new() }));

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 原作の `FontCache::init`。システムのフォントと `dir` のフォントを合わせたセットを作る。
pub fn init(dir: &Path) {
    let mut state = STATE.lock();
    let factory: IDWriteFactory5 = match unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) } {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to load DWrite factory: {e}");
            return;
        }
    };
    state.fonts = build_font_set(&factory, dir);
    state.factory = Some(factory);
}

fn build_font_set(factory: &IDWriteFactory5, dir: &Path) -> Option<IDWriteFontSet> {
    unsafe {
        let builder = factory.CreateFontSetBuilder().ok()?;
        if let Ok(system) = factory.GetSystemFontSet() {
            let _ = builder.AddFontSet(&system);
        }
        for file in font_files(dir) {
            let path = wide(&file.to_string_lossy());
            let Ok(font) = factory.CreateFontFileReference(PCWSTR(path.as_ptr()), None) else {
                continue;
            };
            let _ = builder.AddFontFile(&font);
        }
        builder.CreateFontSet().ok()
    }
}

/// `dir` 直下と 1 つ下のフォルダのフォントファイル（原作の recursive_directory_iterator と同じ深さ）。
fn font_files(dir: &Path) -> Vec<std::path::PathBuf> {
    fn is_font(p: &Path) -> bool {
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("ttf") | Some("otf") | Some("ttc") | Some("otc")
        )
    }
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&p) {
                for s in sub.flatten() {
                    let sp = s.path();
                    if sp.is_file() && is_font(&sp) {
                        out.push(sp);
                    }
                }
            }
        } else if p.is_file() && is_font(&p) {
            out.push(p);
        }
    }
    out
}

/// 「キャッシュを破棄」。フォントセットは作り直さない（原作と同じ）。
pub fn reset() {
    STATE.lock().cache.clear();
}

/// プラグインが外れるとき。
pub fn deinit() {
    let mut state = STATE.lock();
    state.cache.clear();
    state.fonts = None;
    state.factory = None;
}

/// 原作の `FontCache::load`。見つからなければ `Ok(None)`、DirectWrite が失敗したら `Err`。
pub fn load(name: &str, is_bold: bool, is_italic: bool) -> Result<Option<Font>, String> {
    let flag = ((is_bold as u8) << 1) | is_italic as u8;
    let key = format!("{name}{}", (b'0' + flag) as char);
    let mut state = STATE.lock();
    if let Some(entry) = state.cache.get(&key) {
        return Ok(entry.as_ref().map(|d| d.create_font()));
    }
    let data = find(&mut state, name, is_bold, is_italic)?.map(Arc::new);
    let font = data.as_ref().map(|d| d.create_font());
    state.cache.insert(key, data);
    Ok(font)
}

fn find(state: &mut State, name: &str, is_bold: bool, is_italic: bool) -> Result<Option<FontData>, String> {
    if state.fonts.is_none() {
        let factory = state.factory.as_ref().ok_or("Failed to load DWrite factory")?;
        state.fonts = Some(unsafe { factory.GetSystemFontSet() }.map_err(|_| "Failed to get system font set")?);
    }
    let fonts = state.fonts.as_ref().unwrap();
    let Some(face) = search(fonts, name, is_bold, is_italic)? else {
        return Ok(None);
    };
    unsafe {
        let mut count = 0u32;
        face.GetFiles(&mut count, None).map_err(|_| "Failed to get font files count")?;
        if count == 0 {
            return Err("Failed to get font files count".into());
        }
        if count != 1 {
            return Ok(None);
        }
        let mut file: Option<IDWriteFontFile> = None;
        face.GetFiles(&mut count, Some(&mut file)).map_err(|_| "Failed to get font file")?;
        let file = file.ok_or("Failed to get font file")?;
        let mut key: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut size = 0u32;
        file.GetReferenceKey(&mut key, &mut size).map_err(|_| "Failed to get font file reference key")?;
        let loader = file.GetLoader().map_err(|_| "Failed to get font file loader")?;
        let stream = loader.CreateStreamFromKey(key, size).map_err(|_| "Failed to create font file stream")?;
        let file_size = stream.GetFileSize().map_err(|_| "Failed to get font file size")?;
        if file_size == 0 {
            return Err("Failed to get font file size".into());
        }
        let mut fragment: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut context: *mut std::ffi::c_void = std::ptr::null_mut();
        stream
            .ReadFileFragment(&mut fragment, 0, file_size, &mut context)
            .map_err(|_| "Failed to read font file fragment")?;
        if fragment.is_null() {
            return Err("Failed to read font file fragment".into());
        }
        let bytes = std::slice::from_raw_parts(fragment as *const u8, file_size as usize).to_vec();
        stream.ReleaseFileFragment(context);
        Ok(FontData::new(bytes, face.GetIndex()))
    }
}

fn search(fonts: &IDWriteFontSet, name: &str, is_bold: bool, is_italic: bool) -> Result<Option<IDWriteFontFace3>, String> {
    let name_w = wide(name);
    let weight = wide(if is_bold { "700" } else { "400" });
    let style = wide(if is_italic { "2" } else { "0" });
    let prop = |id: DWRITE_FONT_PROPERTY_ID, value: &Vec<u16>| DWRITE_FONT_PROPERTY {
        propertyId: id,
        propertyValue: PCWSTR(value.as_ptr()),
        localeName: PCWSTR::null(),
    };
    let first = |props: &[DWRITE_FONT_PROPERTY]| -> Result<Option<IDWriteFontFace3>, String> {
        let matched = unsafe { fonts.GetMatchingFonts2(props) }.map_err(|_| "Failed to get matching fonts")?;
        if unsafe { matched.GetFontCount() } == 0 {
            return Ok(None);
        }
        let reference = unsafe { matched.GetFontFaceReference(0) }.map_err(|_| "Failed to get font face reference")?;
        let face = unsafe { reference.CreateFontFace() }.map_err(|_| "Failed to create font face")?;
        Ok(Some(face))
    };

    if let Some(f) = first(&[prop(DWRITE_FONT_PROPERTY_ID_FULL_NAME, &name_w)])? {
        return Ok(Some(f));
    }
    for family in [DWRITE_FONT_PROPERTY_ID_FAMILY_NAME, DWRITE_FONT_PROPERTY_ID_WIN32_FAMILY_NAME] {
        let props = [
            prop(family, &name_w),
            prop(DWRITE_FONT_PROPERTY_ID_WEIGHT, &weight),
            prop(DWRITE_FONT_PROPERTY_ID_STYLE, &style),
        ];
        for n in (1..=3).rev() {
            if let Some(f) = first(&props[..n])? {
                return Ok(Some(f));
            }
        }
    }
    Ok(None)
}

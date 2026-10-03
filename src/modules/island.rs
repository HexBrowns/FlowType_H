//! `Island@FlowType_H`。原作 modules/intern/island/intern/island.cpp の移植。
//!
//! 画像の不透明な部分を、つながった塊（島）ごとに分けて並べ替え、各画素に島の番号を書き込む
//! （パーツ分解@FlowType_H が使う）。原作は行ごとの処理と並べ替えを並列にしていた。こちらは 1 本のスレッドで回す
//! （プラグインが裏で起こしたスレッドは外れる前に止める必要があり、作らないのがいちばん安全なため）。
//! 同じキーの島の並びは、原作（並列の不安定な並べ替え）では決まっていなかった。こちらは番号の小さい順で固定になる。

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Instant;

use parking_lot::Mutex;

use super::WRONG_COUNT;
use crate::smp::Smp;

crate::module_fn!(scan_fn, scan);
crate::module_fn!(fetch_fn, fetch);

super::script_module!(IslandModule, "Island", ["scan" => scan_fn, "fetch" => fetch_fn]);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Island {
    origin: [i32; 2],
    size: [i32; 2],
    delta: [f64; 2],
    key: [i32; 2],
    label: usize,
}

#[derive(Debug, Clone, Copy)]
struct Run {
    x0: i32,
    x1: i32,
    label: usize,
}

/// 原作の UnionFind（大きさで併合し、経路を縮める）。
struct UnionFind {
    data: Vec<i32>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { data: vec![-1; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.data[root] >= 0 {
            root = self.data[root] as usize;
        }
        let mut cur = x;
        while self.data[cur] >= 0 {
            let next = self.data[cur] as usize;
            self.data[cur] = root as i32;
            cur = next;
        }
        root
    }

    fn unite(&mut self, a: usize, b: usize) {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        if self.data[a] > self.data[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.data[a] += self.data[b];
        self.data[b] = a as i32;
    }

    fn ensure(&mut self, x: usize) {
        let old = self.data.len();
        if x < old {
            return;
        }
        let new = (x + 1).max(old * 2);
        self.data.resize(new, -1);
    }
}

static ISLANDS: LazyLock<Mutex<HashMap<i64, Vec<Island>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn reset() {
    ISLANDS.lock().clear();
}

const MAX_ISLANDS: usize = 262_144;

/// scan(id, threshold, connectivity, primary_axis, order_x, order_y, blocks_x, blocks_y, data, w, h) → 島の数
fn scan(p: &mut Smp) {
    if p.num() != 11 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let w = p.int(9);
    let h = p.int(10);
    let data = p.data(8) as *mut u8;
    if w <= 0 || h <= 0 || data.is_null() {
        p.set_error("Pixel data is null or corrupted");
        return;
    }
    let (w, h) = (w as usize, h as usize);
    // obj.getpixeldata の RGBA（1 画素 4 バイト）
    let pixels = unsafe { std::slice::from_raw_parts_mut(data, w * h * 4) };

    let id = p.double(0) as i64;
    let params = ScanParams {
        threshold: p.int(1),
        connectivity: p.int(2),
        primary_axis: p.int(3).clamp(0, 1) as usize,
        order: [p.int(4) != 1, p.int(5) != 1],
        blocks: [p.double(6), p.double(7)],
    };
    match scan_pixels(pixels, w, h, &params) {
        Ok(tmp) => {
            if tmp.is_empty() {
                tracing::warn!("No islands detected at this threshold");
            }
            p.push_int(tmp.len() as i32);
            ISLANDS.lock().insert(id, tmp);
        }
        Err(e) => p.set_error(e),
    }
}

#[derive(Clone, Copy)]
pub struct ScanParams {
    pub threshold: i32,
    pub connectivity: i32,
    pub primary_axis: usize,
    pub order: [bool; 2],
    pub blocks: [f64; 2],
}

/// 島を見つけて並べ替え、画素に番号を書き込む（原作の scan の本体）。
fn scan_pixels(pixels: &mut [u8], w: usize, h: usize, params: &ScanParams) -> Result<Vec<Island>, &'static str> {
    let ScanParams { threshold, connectivity, primary_axis, order, blocks } = *params;
    let texel = [
        if blocks[0] > 1.0 { blocks[0] / w as f64 } else { 1.0 },
        if blocks[1] > 1.0 { blocks[1] / h as f64 } else { 1.0 },
    ];

    let t0 = Instant::now();

    // 行ごとに、アルファがしきい値を超える画素の連なり（ラン）を集める
    let mut runs: Vec<Vec<Run>> = (0..h)
        .map(|y| {
            let row = &pixels[y * w * 4..(y + 1) * w * 4];
            let mut out = Vec::new();
            let mut x = 0;
            while x < w {
                if row[x * 4 + 3] as i32 <= threshold {
                    x += 1;
                    continue;
                }
                let x0 = x;
                while x < w && row[x * 4 + 3] as i32 > threshold {
                    x += 1;
                }
                out.push(Run { x0: x0 as i32, x1: x as i32 - 1, label: 0 });
            }
            out
        })
        .collect();

    let t1 = Instant::now();

    // 上の行のランと connectivity の幅で重なれば同じ島にする
    let mut uf = UnionFind::new(h * 16 + 256);
    let mut label = 1usize;
    for r in &mut runs[0] {
        uf.ensure(label);
        r.label = label;
        label += 1;
    }
    for y in 1..h {
        let (above, below) = runs.split_at_mut(y);
        let prev = &above[y - 1];
        let mut i = 0;
        for r in below[0].iter_mut() {
            while i < prev.len() && prev[i].x1 < r.x0 - connectivity {
                i += 1;
            }
            let mut j = i;
            while j < prev.len() && prev[j].x0 <= r.x1 + connectivity {
                if r.label == 0 {
                    r.label = prev[j].label;
                } else {
                    uf.unite(r.label, prev[j].label);
                }
                j += 1;
            }
            if r.label == 0 {
                uf.ensure(label);
                r.label = label;
                label += 1;
            }
        }
    }

    let t2 = Instant::now();

    // 島ごとの外接矩形
    let mut boxes: Vec<Option<([i32; 2], [i32; 2])>> = vec![None; label];
    for (y, row) in runs.iter_mut().enumerate() {
        for r in row.iter_mut() {
            r.label = uf.find(r.label);
            let b = boxes[r.label].get_or_insert(([r.x0, y as i32], [r.x0, y as i32]));
            for x in [r.x0, r.x1] {
                b.0[0] = b.0[0].min(x);
                b.0[1] = b.0[1].min(y as i32);
                b.1[0] = b.1[0].max(x);
                b.1[1] = b.1[1].max(y as i32);
            }
        }
    }

    let mut tmp = Vec::with_capacity(label);
    for (i, b) in boxes.iter().enumerate().skip(1) {
        let Some((min, max)) = *b else {
            continue;
        };
        // Eigen の AlignedBox::center()（double にしてから (min + max) / 2）
        let center = [(min[0] as f64 + max[0] as f64) / 2.0, (min[1] as f64 + max[1] as f64) / 2.0];
        tmp.push(Island {
            origin: min,
            size: [max[0] - min[0] + 1, max[1] - min[1] + 1],
            delta: [center[0] - w as f64 * 0.5 + 0.5, center[1] - h as f64 * 0.5 + 0.5],
            key: [(center[0] * texel[0]).floor() as i32, (center[1] * texel[1]).floor() as i32],
            label: i,
        });
    }

    if tmp.len() > MAX_ISLANDS {
        return Err("Too many islands detected and exceeds maximum limit");
    }

    let t3 = Instant::now();

    // 主軸のキー → もう一方のキーの順。order が false の軸は大きい順
    let s1 = primary_axis;
    let s2 = 1 - primary_axis;
    tmp.sort_by(|a, b| {
        let by = |s: usize| {
            let o = a.key[s].cmp(&b.key[s]);
            if order[s] {
                o
            } else {
                o.reverse()
            }
        };
        by(s1).then_with(|| by(s2))
    });

    let mut to_idx = vec![-1i64; label];
    for (i, isl) in tmp.iter().enumerate() {
        to_idx[isl.label] = i as i64;
    }

    let t4 = Instant::now();

    // 各画素に島の番号（idx * 4 を RGB に詰め、A = 255）を書く。島でない画素は透明
    for (y, row) in runs.iter().enumerate() {
        let line = &mut pixels[y * w * 4..(y + 1) * w * 4];
        let mut prev = 0usize;
        for r in row {
            let (x0, x1) = (r.x0 as usize, r.x1 as usize);
            if x0 > prev {
                line[prev * 4..x0 * 4].fill(0);
            }
            let idx = to_idx[r.label];
            if idx >= 0 {
                let v = (idx as u32).wrapping_mul(4) | 0xff00_0000;
                let px = v.to_le_bytes();
                for x in x0..=x1 {
                    line[x * 4..x * 4 + 4].copy_from_slice(&px);
                }
            } else {
                line[x0 * 4..(x1 + 1) * 4].fill(0);
            }
            prev = x1 + 1;
        }
        if prev < w {
            line[prev * 4..].fill(0);
        }
    }

    let t5 = Instant::now();
    let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
    tracing::debug!(
        "[island::scan] {}x{} n={} | runs={:.3} uf={:.3} bbox={:.3} sort={:.3} write={:.3} total={:.3} ms",
        w,
        h,
        tmp.len(),
        ms(t0, t1),
        ms(t1, t2),
        ms(t2, t3),
        ms(t3, t4),
        ms(t4, t5),
        ms(t0, t5)
    );
    Ok(tmp)
}

/// fetch(id, index) → origin_x, origin_y, w, h, delta_x, delta_y
fn fetch(p: &mut Smp) {
    let id = p.double(0) as i64;
    let idx = p.double(1);
    let islands = ISLANDS.lock();
    let list = islands.get(&id).map(Vec::as_slice).unwrap_or(&[]);
    if list.is_empty() {
        p.set_error("Cache is empty and scan must be performed first");
        return;
    }
    // static_cast<size_t>(double)。負は大きな数になって範囲外になる
    if !(idx >= 0.0) || idx as usize >= list.len() {
        p.set_error("Selected island index is out of bounds");
        return;
    }
    let isl = list[idx as usize];
    drop(islands);
    p.push_int(isl.origin[0]);
    p.push_int(isl.origin[1]);
    p.push_int(isl.size[0]);
    p.push_int(isl.size[1]);
    p.push_double(isl.delta[0]);
    p.push_double(isl.delta[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_find_merges() {
        let mut uf = UnionFind::new(4);
        uf.unite(1, 2);
        uf.unite(2, 3);
        assert_eq!(uf.find(1), uf.find(3));
        assert_ne!(uf.find(0), uf.find(1));
        uf.ensure(10);
        assert_eq!(uf.find(10), 10);
    }

    /// 8x4 の画像に島を 3 つ置く: 左上の 2x2、右の縦長 1x4、左下の 1 画素（左上とは 1 行あく）
    fn sample() -> Vec<u8> {
        let (w, h) = (8, 4);
        let mut px = vec![0u8; w * h * 4];
        let mut set = |x: usize, y: usize| px[(y * w + x) * 4 + 3] = 255;
        set(0, 0);
        set(1, 0);
        set(0, 1);
        set(1, 1);
        for y in 0..4 {
            set(6, y);
        }
        set(0, 3);
        px
    }

    fn params(primary_axis: usize, order: [bool; 2]) -> ScanParams {
        ScanParams { threshold: 0, connectivity: 0, primary_axis, order, blocks: [0.0, 0.0] }
    }

    #[test]
    fn finds_and_orders_islands() {
        let mut px = sample();
        let isl = scan_pixels(&mut px, 8, 4, &params(0, [true, true])).unwrap();
        assert_eq!(isl.len(), 3);
        // キーは中心を切り捨てた整数: 左上 (0, 0) / 左下 (0, 3) / 右 (6, 1)。X が主軸・昇順で、X が同じなら Y
        assert_eq!(isl[0].origin, [0, 0]);
        assert_eq!(isl[0].size, [2, 2]);
        assert_eq!(isl[1].origin, [0, 3]);
        assert_eq!(isl[2].origin, [6, 0]);
        assert_eq!(isl[2].size, [1, 4]);
        // 中心からのずれ: 左上の中心 (0.5, 0.5) − (4, 2) + 0.5
        assert_eq!(isl[0].delta, [-3.0, -1.0]);
        // 画素に番号（idx * 4）が R に入り、A = 255
        assert_eq!(&px[0..4], &[0, 0, 0, 255]);
        assert_eq!(&px[(3 * 8) * 4..(3 * 8) * 4 + 4], &[4, 0, 0, 255]);
        assert_eq!(&px[6 * 4..6 * 4 + 4], &[8, 0, 0, 255]);
        assert_eq!(&px[2 * 4..2 * 4 + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn connectivity_and_descending_order() {
        // connectivity 1 なら、1 行あいた左下は… 上の行のランと x が重なるだけでは足りず、行が隣り合わないとつながらない
        let mut px = sample();
        let isl = scan_pixels(&mut px, 8, 4, &ScanParams { connectivity: 1, ..params(0, [true, true]) }).unwrap();
        assert_eq!(isl.len(), 3);
        // X を降順にすると右の縦長が先頭
        let mut px = sample();
        let isl = scan_pixels(&mut px, 8, 4, &params(0, [false, true])).unwrap();
        assert_eq!(isl[0].origin, [6, 0]);
    }

    #[test]
    fn threshold_hides_everything() {
        let mut px = sample();
        let isl = scan_pixels(&mut px, 8, 4, &ScanParams { threshold: 255, ..params(0, [true, true]) }).unwrap();
        assert!(isl.is_empty());
        assert!(px.iter().all(|&b| b == 0));
    }
}

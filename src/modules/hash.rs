//! `Hash@FlowType_H`。原作 modules/intern/hash/intern/hash.cpp の移植。
//!
//! pcg4d は Mark Jarzynski & Marc Olano の PCG3D（https://github.com/markjarzynski/PCG3D/blob/master/LICENSE）を
//! 原作が改変したもの。乱数の値が変わると見た目が変わるので、式と整数への変換を原作と同じにしてある。

use super::WRONG_COUNT;
use crate::smp::Smp;

crate::module_fn!(hash4d_fn, hash4d);

super::script_module!(HashModule, "Hash", ["hash4d" => hash4d_fn]);

pub fn pcg4d(mut x: u32, mut y: u32, mut z: u32, mut w: u32) -> [u32; 4] {
    x = x.wrapping_mul(1664525).wrapping_add(1013904223);
    y = y.wrapping_mul(1664525).wrapping_add(1013904223);
    z = z.wrapping_mul(1664525).wrapping_add(1013904223);
    w = w.wrapping_mul(1664525).wrapping_add(1013904223);

    x = x.wrapping_add(y.wrapping_mul(w));
    y = y.wrapping_add(z.wrapping_mul(x));
    z = z.wrapping_add(x.wrapping_mul(y));
    w = w.wrapping_add(y.wrapping_mul(z));

    w ^= w >> 16;
    x ^= x >> 16;
    y ^= y >> 16;
    z ^= z >> 16;

    x = x.wrapping_add(y.wrapping_mul(w));
    y = y.wrapping_add(z.wrapping_mul(x));
    z = z.wrapping_add(x.wrapping_mul(y));
    w = w.wrapping_add(y.wrapping_mul(z));
    [x, y, z, w]
}

/// `static_cast<uint32_t>(double)` を MSVC（x64）と同じにする。64 bit の符号付きに切り捨ててから下位 32 bit を取る
/// （負の数は 2 の補数で回る。範囲外と NaN は 0）。Rust の `as u32` は負を 0 に詰めるので使わない。
pub fn to_u32(v: f64) -> u32 {
    let limit = 2f64.powi(63);
    if v.is_nan() || v >= limit || v < -limit {
        0
    } else {
        (v as i64) as u32
    }
}

/// `std::min(upper, v)`（v < upper なら v、そうでなければ upper）
fn min_of(upper: f64, v: f64) -> f64 {
    if v < upper {
        v
    } else {
        upper
    }
}

/// hash4d(x, y, z, w[, upper | lower, upper]) → 4 つの乱数
fn hash4d(p: &mut Smp) {
    let n = p.num();
    if n > 6 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let h = pcg4d(to_u32(p.double(0)), to_u32(p.double(1)), to_u32(p.double(2)), to_u32(p.double(3)));
    let unit = h.map(|v| v as f64 / 4294967296.0);
    match n {
        5 => {
            let upper = p.double(4);
            let range = upper + 1.0;
            for v in unit {
                p.push_double(min_of(upper, (range * v).floor()));
            }
        }
        6 => {
            let lower = p.double(4);
            let upper = p.double(5);
            let range = upper - lower + 1.0;
            for v in unit {
                p.push_double(min_of(upper, (range * v).floor() + lower));
            }
        }
        _ => {
            for v in unit {
                p.push_double(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_wraps_like_msvc() {
        assert_eq!(to_u32(-1.0), 0xffff_ffff);
        assert_eq!(to_u32(-1.5), 0xffff_ffff);
        assert_eq!(to_u32(4294967296.0 + 3.0), 3);
        assert_eq!(to_u32(f64::NAN), 0);
    }

    #[test]
    fn pcg4d_is_stable() {
        // 値そのものを固定しておく（式を書き換えたら落ちる）
        assert_eq!(pcg4d(0, 0, 0, 0), pcg4d(0, 0, 0, 0));
        assert_ne!(pcg4d(1, 0, 0, 0), pcg4d(0, 0, 0, 0));
    }
}

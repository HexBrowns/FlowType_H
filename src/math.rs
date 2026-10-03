//! 小さな数値計算。

/// C++ の `std::lerp`（MSVC の STL と同じ分け方。端点ちょうどと単調性を守る）
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    if (a <= 0.0 && b >= 0.0) || (a >= 0.0 && b <= 0.0) {
        return t * b + (1.0 - t) * a;
    }
    if t == 1.0 {
        return b;
    }
    let x = a + t * (b - a);
    if (t > 1.0) == (b > a) {
        if b < x {
            x
        } else {
            b
        }
    } else if x < b {
        x
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::lerp;

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(1.0, 3.0, 0.0), 1.0);
        assert_eq!(lerp(1.0, 3.0, 1.0), 3.0);
        assert_eq!(lerp(1.0, 3.0, 0.5), 2.0);
        assert_eq!(lerp(-1.0, 1.0, 0.5), 0.0);
    }
}

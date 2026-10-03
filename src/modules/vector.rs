//! `Vector@FlowType_H`。原作 modules/intern/vector/intern/vector.cpp の移植。

use super::WRONG_COUNT;
use crate::rotation::{self, Unit, Vec3};
use crate::smp::Smp;

crate::module_fn!(rotate_fn, rotate);

super::script_module!(VectorModule, "Vector", ["rotate" => rotate_fn]);

/// rotate(t, mode, w, x, y, z) → XYZ のオイラー角（度）
/// rotate(t, mode, w, x, y, z, {x, y, z}, …) → 回転したベクトルの配列
fn rotate(p: &mut Smp) {
    let n = p.num();
    if n < 6 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let t = p.double(0);
    let mode = p.int(1);
    let (w, x, y, z) = (p.double(2), p.double(3), p.double(4), p.double(5));
    if n == 6 {
        match rotation::to_euler(t, mode, Unit::Degree, w, x, y, z) {
            Ok(a) => {
                p.push_double(a[0]);
                p.push_double(a[1]);
                p.push_double(a[2]);
            }
            Err(e) => p.set_error(e),
        }
        return;
    }
    let mut vectors: Vec<Vec3> = (6..n).map(|j| [p.array_double(j, 0), p.array_double(j, 1), p.array_double(j, 2)]).collect();
    if let Err(e) = rotation::rotate(t, mode, Unit::Degree, w, x, y, z, &mut vectors) {
        p.set_error(e);
        return;
    }
    for v in &vectors {
        p.push_array_double(v);
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
// （Eigen 5.0.1 の式を写したファイルなので、このファイルだけ MPL-2.0。Eigen のソースは https://gitlab.com/libeigen/eigen ）

//! 回転の計算。原作 plugins/intern/vector（Eigen 5.0.1）の写し。
//!
//! Eigen の `Quaternion::slerp` / `AngleAxis::toRotationMatrix` / `Quaternion::toRotationMatrix` /
//! `MatrixBase::canonicalEulerAngles` / `_transformVector` を、vcpkg の Eigen 5.0.1 のソースと同じ式・同じ順で書いた。
//! 四元数の積だけは Eigen が SSE の特殊化を使うので、最後の桁が食い違うことがある（実害は無い）。

use std::f64::consts::PI;

pub type Vec3 = [f64; 3];
pub type Mat3 = [[f64; 3]; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Degree,
    #[allow(dead_code)]
    Radian,
}

pub fn to_rad(deg: f64) -> f64 {
    deg * (PI / 180.0)
}

pub fn to_deg(rad: f64) -> f64 {
    rad * (180.0 / PI)
}

#[derive(Debug, Clone, Copy)]
struct Quat {
    w: f64,
    x: f64,
    y: f64,
    z: f64,
}

impl Quat {
    const IDENTITY: Quat = Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    /// `Quaternion::normalized()`（係数を `MatrixBase::normalized()` で割る。長さ 0 ならそのまま）
    fn normalized(self) -> Quat {
        let z = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if z > 0.0 {
            let n = z.sqrt();
            Quat { w: self.w / n, x: self.x / n, y: self.y / n, z: self.z / n }
        } else {
            self
        }
    }

    fn dot(&self, o: &Quat) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }

    /// `QuaternionBase::slerp`（Eigen 5.0.1）
    fn slerp(&self, t: f64, other: &Quat) -> Quat {
        let one = 1.0 - f64::EPSILON;
        let d = self.dot(other);
        let abs_d = d.abs();
        let (scale0, mut scale1);
        if abs_d >= one {
            scale0 = 1.0 - t;
            scale1 = t;
        } else {
            let theta = abs_d.acos();
            let sin_theta = (1.0 - abs_d * abs_d).sqrt();
            scale0 = ((1.0 - t) * theta).sin() / sin_theta;
            scale1 = (t * theta).sin() / sin_theta;
        }
        if d < 0.0 {
            scale1 = -scale1;
        }
        Quat {
            w: scale0 * self.w + scale1 * other.w,
            x: scale0 * self.x + scale1 * other.x,
            y: scale0 * self.y + scale1 * other.y,
            z: scale0 * self.z + scale1 * other.z,
        }
    }

    /// `Quaternion(AngleAxis)`
    fn from_angle_axis(angle: f64, axis: Vec3) -> Quat {
        let ha = 0.5 * angle;
        let s = ha.sin();
        Quat { w: ha.cos(), x: s * axis[0], y: s * axis[1], z: s * axis[2] }
    }

    /// 四元数の積（Eigen の汎用の `quat_product`）
    fn mul(&self, b: &Quat) -> Quat {
        let a = self;
        Quat {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y + a.y * b.w + a.z * b.x - a.x * b.z,
            z: a.w * b.z + a.z * b.w + a.x * b.y - a.y * b.x,
        }
    }

    /// `QuaternionBase::toRotationMatrix`
    fn to_matrix(&self) -> Mat3 {
        let tx = 2.0 * self.x;
        let ty = 2.0 * self.y;
        let tz = 2.0 * self.z;
        let twx = tx * self.w;
        let twy = ty * self.w;
        let twz = tz * self.w;
        let txx = tx * self.x;
        let txy = ty * self.x;
        let txz = tz * self.x;
        let tyy = ty * self.y;
        let tyz = tz * self.y;
        let tzz = tz * self.z;
        [
            [1.0 - (tyy + tzz), txy - twz, txz + twy],
            [txy + twz, 1.0 - (txx + tzz), tyz - twx],
            [txz - twy, tyz + twx, 1.0 - (txx + tyy)],
        ]
    }

    /// `QuaternionBase::_transformVector`
    fn transform(&self, v: Vec3) -> Vec3 {
        let q = [self.x, self.y, self.z];
        let mut uv = cross(q, v);
        uv = [uv[0] + uv[0], uv[1] + uv[1], uv[2] + uv[2]];
        let c = cross(q, uv);
        [
            v[0] + self.w * uv[0] + c[0],
            v[1] + self.w * uv[1] + c[1],
            v[2] + self.w * uv[2] + c[2],
        ]
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `MatrixBase::normalized()`
fn normalized(v: Vec3) -> Vec3 {
    let z = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if z > 0.0 {
        let n = z.sqrt();
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        v
    }
}

/// `AngleAxis::toRotationMatrix`（Eigen 5.0.1）
fn angle_axis_matrix(angle: f64, axis: Vec3) -> Mat3 {
    let s = angle.sin();
    let sin_axis = [s * axis[0], s * axis[1], s * axis[2]];
    let c = angle.cos();
    let k = 1.0 - c;
    let cos1_axis = [k * axis[0], k * axis[1], k * axis[2]];
    let mut res = [[0.0; 3]; 3];
    let tmp = cos1_axis[0] * axis[1];
    res[0][1] = tmp - sin_axis[2];
    res[1][0] = tmp + sin_axis[2];
    let tmp = cos1_axis[0] * axis[2];
    res[0][2] = tmp + sin_axis[1];
    res[2][0] = tmp - sin_axis[1];
    let tmp = cos1_axis[1] * axis[2];
    res[1][2] = tmp - sin_axis[0];
    res[2][1] = tmp + sin_axis[0];
    res[0][0] = cos1_axis[0] * axis[0] + c;
    res[1][1] = cos1_axis[1] * axis[1] + c;
    res[2][2] = cos1_axis[2] * axis[2] + c;
    res
}

fn mat_vec(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// `MatrixBase::canonicalEulerAngles(0, 1, 2)`（Eigen 5.0.1。Tait-Bryan の枝）
fn canonical_euler_xyz(m: &Mat3) -> Vec3 {
    // a0 = 0, a1 = 1, a2 = 2 → odd = 0, i = 0, j = 1, k = 2
    let (i, j, k) = (0, 1, 2);
    let r0 = m[j][k].atan2(m[k][k]);
    let c2 = m[i][i].hypot(m[i][j]);
    let r1 = (-m[i][k]).atan2(c2);
    let s1 = r0.sin();
    let c1 = r0.cos();
    let r2 = (s1 * m[k][i] - c1 * m[j][i]).atan2(c1 * m[j][j] - s1 * m[k][j]);
    // odd == 0 なので符号を反転する
    [-r0, -r1, -r2]
}

const AXES: [Vec3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// 原作の `to_rotation_matrix`。mode をオイラー角の順序（mode / 9, (mode / 3) % 3, mode % 3）に読む。
fn euler_matrix(mode: i32, angle: Vec3) -> Mat3 {
    let order = [(mode / 9) as usize, ((mode / 3) % 3) as usize, (mode % 3) as usize];
    let q2 = Quat::from_angle_axis(angle[order[2]], AXES[order[2]]);
    let q1 = Quat::from_angle_axis(angle[order[1]], AXES[order[1]]);
    let q0 = Quat::from_angle_axis(angle[order[0]], AXES[order[0]]);
    q2.mul(&q1).mul(&q0).to_matrix()
}

pub const UNSUPPORTED: &str = "Selected rotation mode is unsupported";

fn valid_euler(mode: i32) -> bool {
    (5..=21).contains(&mode)
}

/// 原作の `vector::rotate`。
#[allow(clippy::too_many_arguments)]
pub fn rotate(
    t: f64,
    mode: i32,
    unit: Unit,
    w: f64,
    x: f64,
    y: f64,
    z: f64,
    vectors: &mut [Vec3],
) -> Result<(), &'static str> {
    if mode == 0 {
        let q = Quat::IDENTITY.slerp(t.abs(), &Quat { w, x, y, z }.normalized());
        for v in vectors.iter_mut() {
            *v = q.transform(*v);
        }
    } else if mode == 1 {
        let w = if unit == Unit::Degree { to_rad(w) } else { w };
        // AngleAxis * vector は回転行列にしてから掛ける（RotationBase の汎用の積）
        let m = angle_axis_matrix(w * t, normalized([x, y, z]));
        for v in vectors.iter_mut() {
            *v = mat_vec(&m, *v);
        }
    } else if valid_euler(mode) {
        let (x, y, z) = if unit == Unit::Degree { (to_rad(x), to_rad(y), to_rad(z)) } else { (x, y, z) };
        let m = euler_matrix(mode, [x * t, y * t, z * t]);
        for v in vectors.iter_mut() {
            *v = mat_vec(&m, *v);
        }
    } else {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

/// 原作の `vector::to_euler`。XYZ のオイラー角を返す。
pub fn to_euler(t: f64, mode: i32, unit: Unit, w: f64, x: f64, y: f64, z: f64) -> Result<Vec3, &'static str> {
    let mut result = if mode == 0 {
        let q = Quat::IDENTITY.slerp(t.abs(), &Quat { w, x, y, z }.normalized());
        canonical_euler_xyz(&q.to_matrix())
    } else if mode == 1 {
        let w = if unit == Unit::Degree { to_rad(w) } else { w };
        canonical_euler_xyz(&angle_axis_matrix(w * t, normalized([x, y, z])))
    } else if valid_euler(mode) {
        let (x, y, z) = if unit == Unit::Degree { (to_rad(x), to_rad(y), to_rad(z)) } else { (x, y, z) };
        canonical_euler_xyz(&euler_matrix(mode, [x * t, y * t, z * t]))
    } else {
        return Err(UNSUPPORTED);
    };
    if unit == Unit::Degree {
        result = [to_deg(result[0]), to_deg(result[1]), to_deg(result[2])];
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(b.iter()).all(|(p, q)| (p - q).abs() < 1e-9)
    }

    #[test]
    fn zyx_euler_roundtrips() {
        // ZYX（21）は X → Y → Z の順に回す行列 Rx * Ry * Rz。XYZ のオイラー角に戻すと同じ角度になる
        let e = to_euler(1.0, 21, Unit::Degree, 0.0, 10.0, 20.0, 30.0).unwrap();
        assert!(close(e, [10.0, 20.0, 30.0]), "{e:?}");
    }

    #[test]
    fn quaternion_and_axis_angle_agree() {
        // Z 軸まわり 90 度: 四元数 (cos45, 0, 0, sin45) と軸角 (90, 0, 0, 1) は同じ回転
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let mut a = [[1.0, 0.0, 0.0]];
        rotate(1.0, 0, Unit::Degree, h, 0.0, 0.0, h, &mut a).unwrap();
        let mut b = [[1.0, 0.0, 0.0]];
        rotate(1.0, 1, Unit::Degree, 90.0, 0.0, 0.0, 1.0, &mut b).unwrap();
        assert!(close(a[0], [0.0, 1.0, 0.0]) && close(b[0], [0.0, 1.0, 0.0]), "{a:?} {b:?}");
        // 影響度 0.5 で半分（45 度）
        let e = to_euler(0.5, 0, Unit::Degree, h, 0.0, 0.0, h).unwrap();
        assert!(close(e, [0.0, 0.0, 45.0]), "{e:?}");
    }

    #[test]
    fn rejects_unknown_mode() {
        assert!(to_euler(1.0, 3, Unit::Degree, 0.0, 0.0, 0.0, 0.0).is_err());
        assert!(rotate(1.0, 22, Unit::Degree, 0.0, 0.0, 0.0, 0.0, &mut []).is_err());
    }
}

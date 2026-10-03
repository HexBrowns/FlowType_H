//! `Toml@FlowType_H`。原作 modules/intern/toml/intern/toml.cpp の移植。
//!
//! モーションの「エフェクト::パラメータ」に書いた TOML を、表ごとに {効果名, {項目名 = 値}} の並びで返す。
//! 原作の toml++ の表はキーの辞書順に回る。toml クレートの表（BTreeMap）も同じ順になる。

use super::WRONG_COUNT;
use crate::smp::Smp;

crate::module_fn!(parse_fn, parse);

super::script_module!(TomlModule, "Toml", ["parse" => parse_fn]);

/// parse(tag, text)
fn parse(p: &mut Smp) {
    let tag = p.bytes(0);
    if tag == b"motion::effect" {
        parse_motion_effect(p);
    } else {
        p.set_error("Unknown tag");
    }
}

fn parse_motion_effect(p: &mut Smp) {
    if p.num() != 2 {
        p.set_error(WRONG_COUNT);
        return;
    }
    let text = p.bytes(1);
    if text.is_empty() {
        return;
    }
    let Some(table) = std::str::from_utf8(&text).ok().and_then(|s| s.parse::<toml::Table>().ok()) else {
        tracing::warn!("TOML syntax is incorrect");
        return;
    };
    for (section, node) in &table {
        let Some(inner) = node.as_table() else {
            continue;
        };
        let mut keys = Vec::new();
        let mut values = Vec::new();
        for (key, value) in inner {
            let v = match value {
                toml::Value::String(s) => s.clone(),
                toml::Value::Boolean(b) => if *b { "1" } else { "0" }.to_string(),
                toml::Value::Integer(i) => i.to_string(),
                // std::to_string(double) は "%f"（小数 6 桁）
                toml::Value::Float(f) => format_f(*f),
                _ => continue,
            };
            keys.push(key.as_bytes().to_vec());
            values.push(v.into_bytes());
        }
        p.push_string(section.as_bytes());
        p.push_table_string(&keys, &values);
    }
}

fn format_f(f: f64) -> String {
    if f.is_nan() {
        "nan".to_string()
    } else if f.is_infinite() {
        if f > 0.0 { "inf" } else { "-inf" }.to_string()
    } else {
        format!("{f:.6}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_like_to_string() {
        assert_eq!(format_f(1.5), "1.500000");
        assert_eq!(format_f(-0.25), "-0.250000");
    }

    #[test]
    fn tables_iterate_in_key_order() {
        let t: toml::Table = "[\"ぼかし\"]\n\"範囲\"=5\n[\"Aa\"]\nb=1\na=2".parse().unwrap();
        let names: Vec<&String> = t.keys().collect();
        assert_eq!(names, vec!["Aa", "ぼかし"]);
        let inner: Vec<&String> = t["Aa"].as_table().unwrap().keys().collect();
        assert_eq!(inner, vec!["a", "b"]);
    }
}

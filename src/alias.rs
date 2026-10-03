//! エイリアス（`[Object]` `[Object.0]` … の INI 形式）を読む。原作 plugins/intern/alias の移植。

pub struct Object {
    data: String,
}

#[derive(Clone, Copy)]
pub struct Effect<'a> {
    data: &'a str,
}

impl Object {
    pub fn new(alias: String) -> Self {
        Self { data: alias }
    }

    pub fn alias(&self) -> &str {
        &self.data
    }

    /// index < 0 なら `[Object]`（オブジェクトの情報）、それ以外は `[Object.index]` から次の効果の手前まで。
    pub fn get(&self, index: i32) -> Effect<'_> {
        let s = self.data.as_str();
        let (head, next) = if index < 0 {
            ("[Object]".to_string(), "[Object.0]".to_string())
        } else {
            (format!("[Object.{index}]"), format!("[Object.{}]", index + 1))
        };
        let Some(st) = s.find(&head) else {
            return Effect { data: "" };
        };
        // 原作は先頭から次の見出しを探している（見つからなければ末尾まで）
        let ed = s.find(&next).filter(|&e| e >= st).unwrap_or(s.len());
        Effect { data: &s[st..ed] }
    }
}

impl<'a> Effect<'a> {
    pub fn alias(&self) -> &'a str {
        self.data
    }

    /// `\nkey=` の後ろから行末まで。無ければ `def`。
    pub fn get(&self, key: &str, def: &'a str) -> &'a str {
        let target = format!("\n{key}=");
        let Some(st) = self.data.find(&target) else {
            return def;
        };
        let st = st + target.len();
        let rest = &self.data[st..];
        let ed = rest.find(['\r', '\n']).unwrap_or(rest.len());
        &rest[..ed]
    }

    /// 値の最初の `,` より前。値が空なら `def`。
    pub fn front(&self, key: &str, def: &'a str) -> &'a str {
        let v = self.get(key, "");
        if v.is_empty() {
            return def;
        }
        v.split(',').next().unwrap_or(v)
    }

    /// `\n` を改行に、`\\` を `\` に戻す。それ以外の `\x` はそのまま。
    pub fn unescape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                None => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIAS: &str = "[Object]\nlayer=1\nframe=0,29\n[Object.0]\neffect.name=テキスト\nサイズ=40.00\nフォント=Yu Gothic UI\nテキスト=a\\nb\\\\c\n[Object.1]\neffect.name=標準描画\nX=1.50,2.00,直線移動\n";

    #[test]
    fn reads_sections_and_values() {
        let o = Object::new(ALIAS.to_string());
        assert!(o.get(-1).alias().starts_with("[Object]"));
        assert_eq!(o.get(0).get("effect.name", ""), "テキスト");
        assert_eq!(o.get(1).get("X", ""), "1.50,2.00,直線移動");
        assert_eq!(o.get(1).front("X", "0"), "1.50");
        assert_eq!(o.get(1).front("Y", "0.00"), "0.00");
        assert_eq!(o.get(2).alias(), "");
        assert_eq!(Effect::unescape(o.get(0).get("テキスト", "")), "a\nb\\c");
    }
}

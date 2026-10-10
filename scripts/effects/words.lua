local word_groups

do
    local byte, max = string.byte, math.max

    -- 強化（句読点・文字種）= 1
    -- v0.1.0 の Motion の実装をそのまま移したもの。**区切りを変えない**（既存作品の見た目が変わるため）
    --   区切り: 空白・句読点表・ASCII の %p、CJK（UTF-8 先頭バイト 0xE3〜0xED の 3 バイト文字）と英字／数字の切り替わり
    --   utf8.split(content, true) の結果を使うので、改行などの制御文字は数えず区切りにもならない
    --   最初の語の番号は 1（語の番号 0 は先頭の区切りだけが使う）
    local punct = {
        ["、"] = true, ["。"] = true, ["！"] = true, ["？"] = true,
        ["!"] = true, ["?"] = true, ["…"] = true, ["・"] = true,
        [","] = true, ["."] = true, [";"] = true, [":"] = true,
        ["　"] = true, ["「"] = true, ["」"] = true, ["『"] = true, ["』"] = true,
    }

    local function classify(ch)
        if ch == "\n" or ch == "\r" or ch == "\v" or ch == "\t" or ch == " " then
            return "space"
        end
        if punct[ch] or (#ch == 1 and ch:match("%p")) then
            return "punct"
        end
        local b1 = ch:byte(1)
        if b1 < 128 then
            if ch:match("%d") then return "digit" end
            if ch:match("%a") then return "latn" end
            return "other"
        end
        if #ch >= 3 and b1 >= 0xE3 and b1 <= 0xED then
            return "cjk"
        end
        return "other"
    end

    local function enhanced(chars)
        local t = {}
        local g = 0
        local after_sep = true
        local prev = nil
        for _, ch in ipairs(chars) do
            local c = classify(ch)
            if c == "space" or c == "punct" then
                t[#t + 1] = max(g, 0)
                after_sep = true
                prev = nil
            else
                local boundary = after_sep
                if not boundary and prev and prev ~= c then
                    if (prev == "cjk" and (c == "latn" or c == "digit"))
                        or ((prev == "latn" or prev == "digit") and c == "cjk") then
                        boundary = true
                    end
                end
                if boundary then
                    g = g + 1
                end
                t[#t + 1] = g
                after_sep = false
                prev = c
            end
        end
        return t
    end

    -- 文節（簡易）= 2（v0.2.0）
    -- 形態素解析ではない。文字種の切り替わりだけで切る
    --   新しい語にする: かな→漢字、ひらがな／漢字→カタカナ、日本語→英字／数字、英字→漢字／カタカナ
    --   つなげる: 漢字→かな（送り仮名・助詞）、カタカナ→ひらがな、英字／数字→ひらがな、数字→漢字／カタカナ（助数詞）
    --   長音・小書きかな・濁点は直前の文字種に含める。英字に挟まれた ' ’ - は語の内側（don't / re-start）
    --   空白・句読点は直前の語に、開き括弧は次の語に付ける。改行などの制御文字も区切りになる（数には入れない）
    --   最初の語の番号は 0
    local function codepoint(ch)
        local b1, b2, b3, b4 = byte(ch, 1, 4)
        if b1 == nil then
            return -1
        end
        if b1 < 0x80 then
            return b1
        end
        b2, b3, b4 = (b2 or 0x80) - 0x80, (b3 or 0x80) - 0x80, (b4 or 0x80) - 0x80
        if b1 < 0xE0 then
            return (b1 - 0xC0) * 0x40 + b2
        end
        if b1 < 0xF0 then
            return (b1 - 0xE0) * 0x1000 + b2 * 0x40 + b3
        end
        return (b1 - 0xF0) * 0x40000 + b2 * 0x1000 + b3 * 0x40 + b4
    end

    local opens = {
        [0x28] = true, [0x5B] = true, [0x7B] = true, [0x2018] = true, [0x201C] = true,
        [0x3008] = true, [0x300A] = true, [0x300C] = true, [0x300E] = true, [0x3010] = true,
        [0x3014] = true, [0x3016] = true, [0x3018] = true, [0x301A] = true, [0x301D] = true,
        [0xFF08] = true, [0xFF3B] = true, [0xFF5B] = true, [0xFF5F] = true, [0xFF62] = true,
    }

    local links = { [0x27] = true, [0x2D] = true, [0x2010] = true, [0x2011] = true, [0x2019] = true }

    -- 長音・小書きかな・濁点（直前の文字種に含める）。値は先頭に来たときの文字種
    local joins = {
        [0x30FC] = "kata", [0xFF70] = "kata", [0xFF9E] = "kata", [0xFF9F] = "kata",
        [0x3099] = "hira", [0x309A] = "hira", [0x309B] = "hira", [0x309C] = "hira",
    }
    for _, cp in ipairs({ 0x3041, 0x3043, 0x3045, 0x3047, 0x3049, 0x3063, 0x3083, 0x3085, 0x3087, 0x308E, 0x3095, 0x3096 }) do
        joins[cp] = "hira"
        joins[cp + 0x60] = "kata"
    end
    for cp = 0x31F0, 0x31FF do
        joins[cp] = "kata"
    end
    for cp = 0xFF67, 0xFF6F do
        joins[cp] = "kata"
    end

    local function range(cp, lo, hi)
        return cp >= lo and cp <= hi
    end

    local function class_of(cp)
        if cp < 0 or cp <= 0x1F or cp == 0x7F or range(cp, 0x80, 0x9F) then
            return "ctrl"
        end
        if cp == 0x20 or cp == 0xA0 or cp == 0x1680 or range(cp, 0x2000, 0x200B)
            or cp == 0x2028 or cp == 0x2029 or cp == 0x202F or cp == 0x205F
            or cp == 0x3000 or cp == 0xFEFF then
            return "space"
        end
        if opens[cp] then
            return "open"
        end
        if links[cp] then
            return "link"
        end
        if joins[cp] then
            return "join"
        end
        if range(cp, 0x30, 0x39) or range(cp, 0xFF10, 0xFF19) then
            return "digit"
        end
        if range(cp, 0x41, 0x5A) or range(cp, 0x61, 0x7A)
            or (range(cp, 0xC0, 0x24F) and cp ~= 0xD7 and cp ~= 0xF7)
            or range(cp, 0x370, 0x4FF)
            or range(cp, 0xFF21, 0xFF3A) or range(cp, 0xFF41, 0xFF5A) then
            return "latn"
        end
        if range(cp, 0x3041, 0x309F) then
            return "hira"
        end
        if (range(cp, 0x30A1, 0x30FF) and cp ~= 0x30FB) or range(cp, 0x31F0, 0x31FF) or range(cp, 0xFF66, 0xFF9D) then
            return "kata"
        end
        if range(cp, 0x3400, 0x4DBF) or range(cp, 0x4E00, 0x9FFF) or range(cp, 0xF900, 0xFAFF)
            or range(cp, 0x20000, 0x3134F) or range(cp, 0x3005, 0x3007) or cp == 0x303B then
            return "kanji"
        end
        if range(cp, 0x21, 0x2F) or range(cp, 0x3A, 0x40) or range(cp, 0x5B, 0x60) or range(cp, 0x7B, 0x7E)
            or range(cp, 0xA1, 0xBF)
            or range(cp, 0x2010, 0x2027) or range(cp, 0x2030, 0x205E)
            or range(cp, 0x2190, 0x21FF) or range(cp, 0x2500, 0x27BF) or range(cp, 0x2B00, 0x2BFF)
            or range(cp, 0x2E00, 0x2E7F)
            or range(cp, 0x3001, 0x3004) or range(cp, 0x3008, 0x3020) or cp == 0x3030 or cp == 0x303D
            or cp == 0x30A0 or cp == 0x30FB
            or range(cp, 0xFE30, 0xFE4F)
            or range(cp, 0xFF01, 0xFF0F) or range(cp, 0xFF1A, 0xFF20) or range(cp, 0xFF3B, 0xFF40)
            or range(cp, 0xFF5B, 0xFF65)
            or range(cp, 0x1F300, 0x1FAFF) then
            return "punct"
        end
        return "other"
    end

    local function starts_new(prev, c)
        if c == "kanji" then
            return prev == "hira" or prev == "kata" or prev == "latn"
        elseif c == "kata" then
            return prev == "hira" or prev == "kanji" or prev == "latn"
        elseif c == "latn" or c == "digit" then
            return prev == "hira" or prev == "kata" or prev == "kanji"
        end
        return false
    end

    local function bunsetsu(chars)
        local cps, classes = {}, {}
        for k, ch in ipairs(chars) do
            cps[k] = codepoint(ch)
            classes[k] = class_of(cps[k])
        end

        local t = {}
        local g = -1
        local prev = nil
        local opened = false

        for k = 1, #chars do
            local c = classes[k]

            if c == "link" then
                c = (prev == "latn" and classes[k + 1] == "latn") and "latn" or "punct"
            elseif c == "join" then
                c = prev or joins[cps[k]]
            end

            if c == "ctrl" then
                prev = nil
            elseif c == "space" or c == "punct" then
                t[#t + 1] = max(g, 0)
                prev = nil
            elseif c == "open" then
                if not opened then
                    g = g + 1
                    opened = true
                end
                t[#t + 1] = g
                prev = nil
            else
                if not opened and (prev == nil or starts_new(prev, c)) then
                    g = g + 1
                end
                t[#t + 1] = g
                prev = c
                opened = false
            end
        end

        return t
    end

    -- 単語分割の共通関数（Motion と Blink が同じ区切りを返すように 1 か所にまとめる）
    -- 戻り値: 制御文字を除いた文字ごとの語の番号（utf8.count(content, true) と同じ長さ）
    -- 空白のみ（従来）= 0 は Regex モジュールで分けるので、ここでは扱わない
    word_groups = function(utf8, content, mode)
        local chars = utf8.split(content, mode ~= 2)
        if type(chars) ~= "table" then
            return {}
        end
        if mode == 2 then
            return bunsetsu(chars)
        end
        return enhanced(chars)
    end
end

return {
    word_groups = word_groups,
}

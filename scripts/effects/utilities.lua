local lerp, clamp, copy_xform, stop, to_color, to_style, to_number

do
    local max, min = math.max, math.min

    lerp = function(a, b, t)
        return a + (b - a) * t
    end

    clamp = function(v, lower, upper)
        return max(lower, min(upper, v))
    end

    copy_xform = function(dst, src)
        dst.ox, dst.oy, dst.oz = src.ox, src.oy, src.oz
        dst.cx, dst.cy, dst.cz = src.cx, src.cy, src.cz
        dst.rx, dst.ry, dst.rz = src.rx, src.ry, src.rz
        dst.sx, dst.sy, dst.sz = src.sx, src.sy, src.sz
        dst.alpha = src.alpha
    end

    stop = function(e)
        print("@error", e)
        obj.load("text", "")
    end

    -- getvalue の色は "f5f5f5" 等の文字列になり得る
    to_color = function(v, default)
        if type(v) == "number" then
            return v
        end
        if type(v) == "string" then
            local s = v:match("^%s*(.-)%s*$") or v
            s = s:gsub("^#", ""):gsub("^0[xX]", "")
            local n = tonumber(s, 16) or tonumber(s)
            if n ~= nil then
                return n
            end
        end
        return default or 0xffffff
    end

    -- obj.getvalue は対象が無いと nil ではなく「戻り値なし」を返す（lua.txt の「返却無し」）。
    -- tonumber(obj.getvalue(...)) と唯一の引数に直接渡すと引数なしの tonumber になって止まるので、
    -- to_number(obj.getvalue(...), 既定値) と後ろに引数を続けて受ける（ルール au2-lua-pitfalls）
    to_number = function(v, default)
        return tonumber(v) or default
    end

    -- 文字装飾は名称または数値
    to_style = function(v, styles)
        if type(v) == "number" then
            return v
        end
        if type(v) == "string" and styles ~= nil and styles[v] ~= nil then
            return styles[v]
        end
        return tonumber(v) or 0
    end
end

return {
    lerp = lerp,
    clamp = clamp,
    copy_xform = copy_xform,
    stop = stop,
    to_color = to_color,
    to_style = to_style,
    to_number = to_number,
}

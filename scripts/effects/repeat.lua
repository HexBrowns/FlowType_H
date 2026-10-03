--@リピート

--require:${PROJECT_REQUIRES_AVIUTL2}
--information:リピート@${SCRIPT_NAME} v${PROJECT_VERSION} by ${PROJECT_AUTHOR}
--label:${LABEL}

--group:レイアウト,true
--separator:個数
local layout_count_x = 1 --track@layout_count_x:レイアウト::個数::X,1,100,1,1
local layout_count_y = 1 --track@layout_count_y:レイアウト::個数::Y,1,100,1,1
--trackgroup@layout_count_x,layout_count_y:Group::レイアウト::個数
--separator:余白
local layout_padding_x = 0.0 --track@layout_padding_x:レイアウト::余白::X,-10000,10000,0,0.01
local layout_padding_y = 0.0 --track@layout_padding_y:レイアウト::余白::Y,-10000,10000,0,0.01
--trackgroup@layout_padding_x,layout_padding_y:Group::レイアウト::余白
--group:位置オフセット,false
local position_offset_angle = 0.0 --track@position_offset_angle:位置オフセット::角度,-89,89,0,0.01
local position_offset_axis = 0.0 --track@position_offset_axis:位置オフセット::軸,-3600,3600,0,0.01
--group:時間オフセット,false
local time_offset_interval = 0.0 --track@time_offset_interval:時間オフセット::間隔,-100,100,0,0.001
local time_offset_orientation = 0 --select@time_offset_orientation:時間オフセット::向き,列=0,行=1
local time_offset_order = 0 --select@time_offset_order:時間オフセット::順序,順方向=0,逆方向=1,ランダム=2,中央から=3,外側から=4
--group:追加オプション,false
local unit = 0 --select@unit:単位,フレーム=0,秒=1
local seed = 0 --track@seed:シード,-10000,10000,0,1
local should_highlight_order = false --check@should_highlight_order:順序を強調,false

do
    --#include "utilities.lua"
    local utils = require("utilities")
    local clamp, copy_xform, stop = utils.clamp, utils.copy_xform, utils.stop
    -- to_style / to_number は utilities.lua の #include 展開で定義される

    --#include "order.lua"
    local order_utils = require("order")
    local order_rank = order_utils.order_rank

    local buffer

    do
        buffer = require("string.buffer").new()
    end

    local ID = obj.effect_id
    local CACHE_IMAGE = "cache:24a8ba19-70d6-4ceb-ad75-b793c122a10b-" .. ID

    local max, floor, rad, random, randomseed = math.max, math.floor, math.rad, math.random, math.randomseed
    local getvalue, getinfo, copybuffer, pixelshader = obj.getvalue, obj.getinfo, obj.copybuffer, obj.pixelshader
    local INDEX, NUM, LAYER = obj.index, obj.num, obj.layer
    local FPS, TIME, TOTALTIME = obj.framerate, obj.time, obj.totaltime

    layout_count_x = floor(layout_count_x)
    layout_count_y = floor(layout_count_y)

    position_offset_angle = rad(position_offset_angle)
    position_offset_axis = rad(position_offset_axis)

    time_offset_interval = unit == 0 and time_offset_interval / obj.framerate or time_offset_interval

    if seed >= 0 then
        randomseed(seed)
        seed = obj.layer + random(2147483647) + 1
    else
        seed = -seed
    end

    local count = layout_count_x * layout_count_y

    if count <= 1 then
        return
    end

    local w, h = obj.w, obj.h

    if NUM > 1 then
        local text = getvalue(LAYER, "テキスト", "テキスト") --[[@as string | nil]]
        if text ~= nil then
            local KEY_SIZE = "0a5312f4-0ec0-430b-bbf8-2316b0fd3e20-" .. ID

            local size
            if INDEX == 0 then
                local styles = {
                    ["標準文字"] = 0,
                    ["影付き文字"] = 1,
                    ["影付き文字(薄)"] = 2,
                    ["縁取り文字"] = 3,
                    ["縁取り文字(細)"] = 4,
                    ["縁取り文字(太)"] = 5,
                    ["縁取り文字(角)"] = 6,
                }

                obj.setfont(
                    getvalue(LAYER, "テキスト", "フォント") --[[@as string]],
                    to_number(getvalue(LAYER, "テキスト", "サイズ"), 0),
                    to_style(getvalue(LAYER, "テキスト", "文字装飾"), styles),
                    0,
                    0,
                    getvalue(LAYER, "テキスト", "B") ~= "0",
                    getvalue(LAYER, "テキスト", "I") ~= "0",
                    to_number(getvalue(LAYER, "テキスト", "字間"), 0),
                    to_number(getvalue(LAYER, "テキスト", "行間"), 0)
                )

                size = { obj.load("text.layout", text:gsub("\\\\", "\\"):gsub("\\n", "\n")) }
                global[KEY_SIZE] = buffer:reset():encode(size):get()
            else
                size = buffer:set(global[KEY_SIZE]):decode()
            end

            if type(size) == "table" and #size == 2 then
                ---@cast size number[]
                w, h = size[1], size[2]
            else
                print("@warn", "Shared size table is missing or corrupted")
            end

            if INDEX == NUM - 1 then
                global[KEY_SIZE] = nil
            end
        end
    end

    local x, y = math.cos(position_offset_axis), math.sin(position_offset_axis)
    local s = math.tan(position_offset_angle)
    local c = s * x * y

    local dx, dy = w + layout_padding_x, h + layout_padding_y

    local ex, ey = dx * (1 - c), dx * s * x * x
    local fx, fy = -dy * s * y * y, dy * (1 + c)

    local ox = -0.5 * ((layout_count_x - 1) * ex + (layout_count_y - 1) * fx)
    local oy = -0.5 * ((layout_count_x - 1) * ey + (layout_count_y - 1) * fy)

    if not copybuffer(CACHE_IMAGE, "object") then
        stop("Failed to copy buffer")
        return
    end

    local xform = {}
    copy_xform(xform, obj)

    local order
    if time_offset_order == 2 then
        randomseed(seed)

        order = {}

        for i = 1, count do
            order[i] = i - 1
        end

        for i = count, 1, -1 do
            local j = random(1, i)
            order[i], order[j] = order[j], order[i]
        end
    elseif time_offset_order >= 3 then
        -- 中央から／外側から: 向き（列／行）で決まる並びの中で、順位の小さい順に並べる
        -- （同じ順位は並びの番号順）。格子の 2 次元の中心ではなく、並びの中央を基準にする
        order = {}

        for i = 1, count do
            order[i] = i - 1
        end

        table.sort(order, function(a, b)
            local ra = order_rank(a, count, time_offset_order)
            local rb = order_rank(b, count, time_offset_order)
            if ra ~= rb then
                return ra < rb
            end
            return a < b
        end)
    end

    local i = -1
    obj.multiobject(count, function()
        i = i + 1

        local j
        if time_offset_order == 0 then
            j = i
        elseif time_offset_order == 1 then
            j = count - i - 1
        elseif time_offset_order >= 2 then
            ---@cast order integer[]
            j = order[i + 1]
        end

        -- 時間オフセットと「順序を強調」に使う順位 k と段数 m（中央から／外側から以外は i, count）
        local k, m = i, count
        if time_offset_order >= 3 then
            k, m = order_rank(j, count, time_offset_order)
        end

        if not copybuffer("object", CACHE_IMAGE) then
            stop("Failed to copy buffer")
            return
        end

        copy_xform(obj, xform)

        local row, col
        if time_offset_orientation == 0 then
            row = floor(j / layout_count_x)
            col = j - row * layout_count_x
        else
            col = floor(j / layout_count_y)
            row = j - col * layout_count_y
        end

        obj.ox = obj.ox + ox + ex * col + fx * row
        obj.oy = obj.oy + oy + ey * col + fy * row

        if should_highlight_order and not getinfo("saving") then
            pixelshader(
                "tint@モーション@${SCRIPT_NAME}",
                "object",
                "object",
                { 1.0, 0.0, 0.0, 1.0, 1.0 - k / max(m - 1, 1) }
            )
        end

        return clamp(TIME + time_offset_interval * k, 0.0, TOTALTIME) - TIME
    end)
end

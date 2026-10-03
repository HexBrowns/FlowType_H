--@パーツ分解

--require:${PROJECT_REQUIRES_AVIUTL2}
--information:パーツ分解@${SCRIPT_NAME} v${PROJECT_VERSION} by ${PROJECT_AUTHOR}
--label:${LABEL}

local threshold = 0.0 --track@threshold:しきい値,0,100,0,0.01
local index = -1 --track@index:インデックス,-1,100,-1,1
--group:アンカー,true
local anchor_target = 0 --select@anchor_target:アンカー::対象=1,ピボット=0,位置=1
local anchor_should_overwrite = false --checksection@anchor_should_overwrite:アンカー::上書き,false,false
--group:並べ替え,false
--separator:順序
local sort_order_primary_axis = 1 --select@sort_order_primary_axis:並べ替え::順序::主軸=1,X=0,Y=1
local sort_order_x = 0 --select@sort_order_x:並べ替え::順序::X,左→右=0,右→左=1
local sort_order_y = 0 --select@sort_order_y:並べ替え::順序::Y,上→下=0,下→上=1
local sort_order_custom_order = {} --value@sort_order_custom_order:並べ替え::順序::カスタム,{}
--separator:ブロック
local sort_blocks_x = 0 --track@sort_blocks_x:並べ替え::ブロック::X,0,50,0,1,---
local sort_blocks_y = 0 --track@sort_blocks_y:並べ替え::ブロック::Y,0,50,0,1,---
--group:色調,false
local tint_source = 0 --select@tint_source:色調::ソース,画像=0,レイヤー=1
local tint_image = "" --file@tint_image:色調::画像,""
local tint_layer = 0 --track@tint_layer:色調::レイヤー,-100,100,0,1,---
local tint_order = 0 --select@tint_order:色調::順序,順方向=0,逆方向=1,ランダム=2,中央から=3,外側から=4
--group:時間オフセット,false
local time_offset_interval = 0.0 --track@time_offset_interval:時間オフセット::間隔,-100,100,0,0.001
local time_offset_order = 0 --select@time_offset_order:時間オフセット::順序,順方向=0,逆方向=1,ランダム=2,中央から=3,外側から=4
--group:追加オプション,false
local connectivity = 1 --select@connectivity:連結=1,4連結=0,8連結=1
local unit = 0 --select@unit:単位,フレーム=0,秒=1
local layer_reference = 0 --select@layer_reference:レイヤー参照,絶対=0,相対=1
local seed = 0 --track@seed:シード,-10000,10000,0,1
local should_highlight_order = false --check@should_highlight_order:順序を強調,false
--[[pixelshader@color_mask:
--#include <color_mask.hlsl>
]]

if obj.getoption("multi_object") then
    print("@error", "Disable Multi Object to use this script")
    return
end

do
    --#include "utilities.lua"
    local utils = require("utilities")
    local clamp, copy_xform, stop = utils.clamp, utils.copy_xform, utils.stop

    --#include "order.lua"
    local order_utils = require("order")
    local order_rank = order_utils.order_rank

    local island = obj.module("Island@${PROJECT_NAME}")
    local scan, fetch = island.scan, island.fetch

    local ID = obj.effect_id
    local CACHE_IMAGE = "cache:cf20d0bc-2ad4-4633-ab0f-2b349128aa13-" .. ID
    local CACHE_COLOR_MASK = "cache:dbd9e2b5-861a-4647-9bd7-cf8137f60c54-" .. ID
    local CACHE_ALPHA_MASK = "cache:9edded2f-c484-42b7-8bb8-032b0febed5e-" .. ID

    local max, random, randomseed = math.max, math.random, math.randomseed
    local getinfo, copybuffer, clearbuffer, pixelshader = obj.getinfo, obj.copybuffer, obj.clearbuffer, obj.pixelshader
    local LAYER, TIME, TOTALTIME = obj.layer, obj.time, obj.totaltime

    threshold = math.floor(threshold * 2.55)

    tint_layer = layer_reference == 0 and max(tint_layer, 0) or max(LAYER + tint_layer, 0)
    time_offset_interval = unit == 0 and time_offset_interval / obj.framerate or time_offset_interval

    local should_use_custom_order = #sort_order_custom_order > 0

    local should_load_lut = false
    if tint_source == 0 then
        should_load_lut = tint_image ~= ""
    elseif tint_source == 1 then
        if tint_layer > 0 and tint_layer ~= LAYER then
            if not obj.getvalue("layer" .. tint_layer) then
                local frame = obj.getvalue("frame_s") + obj.frame
                print("@warn", "No object found in Layer " .. tint_layer .. " at Frame " .. frame)
            else
                should_load_lut = true
            end
        end
    end

    if seed >= 0 then
        randomseed(seed)
        seed = LAYER + random(2147483647) + 1
    else
        seed = -seed
    end

    local set_anchor

    do
        local x, y, sign
        if anchor_target == 0 then
            x, y, sign = "cx", "cy", -1.0
        elseif anchor_target == 1 then
            x, y, sign = "ox", "oy", 1.0
        end

        set_anchor = function(dx, dy)
            obj[x] = obj[x] + dx * sign
            obj[y] = obj[y] + dy * sign
        end
    end

    local apply_tint

    if should_load_lut then
        local CACHE_LUT = "cache:007fc33f-4843-45ca-8ef0-84edcea977c9-" .. ID

        local rand1 = obj.rand1

        local ok, e = pcall(function()
            if not copybuffer("cache:tmp", "object") then
                error("Failed to copy buffer")
            end

            local xform = {}
            copy_xform(xform, obj)

            if tint_source == 0 then
                if not obj.load("image", tint_image) then
                    if not copybuffer("object", "cache:tmp") then
                        error("Failed to copy buffer")
                    end

                    error("Failed to load image file")
                end
            elseif tint_source == 1 then
                if not obj.load("layer", tint_layer, true) then
                    if not copybuffer("object", "cache:tmp") then
                        error("Failed to copy buffer")
                    end

                    error("Failed to load layer data")
                end
            end

            if not copybuffer(CACHE_LUT, "object") then
                error("Failed to copy buffer")
            end

            if not copybuffer("object", "cache:tmp") then
                error("Failed to copy buffer")
            end

            copy_xform(obj, xform)
        end)

        if not ok then
            stop(e)
            return
        end

        apply_tint = function(i, n)
            local t
            if tint_order == 0 then
                t = i / max(n - 1, 1)
            elseif tint_order == 1 then
                t = 1.0 - i / max(n - 1, 1)
            elseif tint_order >= 3 then
                -- 中央から／外側から: 順位 0 が色の始点、最後の段が終点
                local r, m = order_rank(i, n, tint_order)
                t = r / max(m - 1, 1)
            else
                t = rand1(-seed, i)
            end

            pixelshader("map@モーション@${SCRIPT_NAME}", "object", { "object", CACHE_LUT }, { t, 0.5 }, "copy", "clamp")
        end
    end

    if not copybuffer(CACHE_COLOR_MASK, "object") then
        stop("Failed to copy buffer")
        return
    end

    if not copybuffer(CACHE_IMAGE, "object") then
        stop("Failed to copy buffer")
        return
    end

    local data, W, H = obj.getpixeldata(CACHE_COLOR_MASK)

    local n = scan(
        ID,
        threshold,
        connectivity,
        sort_order_primary_axis,
        sort_order_x,
        sort_order_y,
        sort_blocks_x,
        sort_blocks_y,
        data,
        W,
        H
    )

    if n == 0 then
        return
    end

    obj.putpixeldata(CACHE_COLOR_MASK, data, W, H)

    if index >= 0 and index < n then
        local x, y, w, h, dx, dy = fetch(ID, index)
        clearbuffer("object", w, h)
        pixelshader("color_mask", "object", { CACHE_IMAGE, CACHE_COLOR_MASK }, { x, y, index })

        if anchor_target == 0 then
            if anchor_should_overwrite then
                obj.cx = -dx
                obj.cy = -dy
            else
                obj.cx = obj.cx - dx
                obj.cy = obj.cy - dy
            end
        elseif anchor_target == 1 then
            if anchor_should_overwrite then
                obj.ox = dx
                obj.oy = dy
            else
                obj.ox = obj.ox + dx
                obj.oy = obj.oy + dy
            end
        end

        if should_load_lut then
            apply_tint(index, n)
        end

        return
    end

    local xform = {}
    copy_xform(xform, obj)
    if anchor_should_overwrite then
        if anchor_target == 0 then
            xform.cx, xform.cy = 0.0, 0.0
        elseif anchor_target == 1 then
            xform.ox, xform.oy = 0.0, 0.0
        end
    end

    if should_use_custom_order then
        n = math.min(n, #sort_order_custom_order + 1)
        clearbuffer(CACHE_ALPHA_MASK, W, H)
        -- 最後の 1 個（残り全部）は、登録した島をこのマスクで抜いて描く。逆方向・ランダム・
        -- 中央から・外側からでは残りが登録した島より先に描かれることがあるので、
        -- 並べる前に登録した島を全部マスクへ描いておく（v0.2.0 までは描く順に足していた）
        for c = 1, n - 1 do
            pixelshader("color_mask", CACHE_ALPHA_MASK, { CACHE_IMAGE, CACHE_COLOR_MASK },
                { 0, 0, sort_order_custom_order[c] }, "draw")
        end
    end

    local order
    if time_offset_order == 2 then
        randomseed(seed)

        order = {}

        for i = 1, n do
            order[i] = i - 1
        end

        for i = n, 1, -1 do
            local j = random(1, i)
            order[i], order[j] = order[j], order[i]
        end
    elseif time_offset_order >= 3 then
        -- 中央から／外側から: 順位の小さい順に並べる（同じ順位は並べ替え後の番号順）
        -- 個別オブジェクトの番号の付け方は、ほかの順序と同じく時間の順になる
        order = {}

        for i = 1, n do
            order[i] = i - 1
        end

        table.sort(order, function(a, b)
            local ra = order_rank(a, n, time_offset_order)
            local rb = order_rank(b, n, time_offset_order)
            if ra ~= rb then
                return ra < rb
            end
            return a < b
        end)
    end

    local i = -1
    obj.multiobject(n, function()
        i = i + 1

        local j
        if time_offset_order == 0 then
            j = i
        elseif time_offset_order == 1 then
            j = n - i - 1
        elseif time_offset_order >= 2 then
            ---@cast order integer[]
            j = order[i + 1]
        end

        -- 時間オフセットと「順序を強調」に使う順位 k と段数 m（中央から／外側から以外は i, n）
        local k, m = i, n
        if time_offset_order >= 3 then
            k, m = order_rank(j, n, time_offset_order)
        end

        -- カスタム順序の最後の 1 個は「残り全部」。順方向以外は呼び出し順 i と並びの番号 j が
        -- 一致しないので、j で判定する（v0.2.0 までは逆方向・ランダムで i を使い、
        -- 表の外を読んで fetch に nil を渡していた）
        local is_listed = j < n - 1

        if should_use_custom_order then
            if is_listed then
                j = sort_order_custom_order[j + 1]
                local x, y, w, h, dx, dy = fetch(ID, j)
                clearbuffer("object", w, h)
                pixelshader("color_mask", "object", { CACHE_IMAGE, CACHE_COLOR_MASK }, { x, y, j })
                copy_xform(obj, xform)
                set_anchor(dx, dy)
            else
                if not copybuffer("object", CACHE_IMAGE) then
                    stop("Failed to copy buffer")
                    return
                end
                pixelshader("alpha_mask@モーション@${SCRIPT_NAME}", "object", CACHE_ALPHA_MASK, { 1.0 }, "mask")
                copy_xform(obj, xform)
            end
        else
            local x, y, w, h, dx, dy = fetch(ID, j)
            clearbuffer("object", w, h)
            pixelshader("color_mask", "object", { CACHE_IMAGE, CACHE_COLOR_MASK }, { x, y, j })
            copy_xform(obj, xform)
            set_anchor(dx, dy)
        end

        if should_load_lut then
            apply_tint(j, n)
        end

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

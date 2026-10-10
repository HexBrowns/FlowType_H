--@点滅

--require:${AVIUTL2_VERSION}
--information:点滅@${PROJECT_NAME} v${PROJECT_VERSION} by ${PROJECT_AUTHOR}
--label:${LABEL}

--#define BASED_ON 全体=-2,オブジェクト=-1,文字=0,文字（空白除く）=1,単語=2,行=3
local based_on = 0 --select@based_on:基準,${BASED_ON}
local duration = 8.0 --track@duration:時間,-10000,10000,8,0.001,---
--group:不透明度,true
local opacity_steps = 2 --track@opacity_steps:不透明度::段階,2,128,2,1
local opacity_min = 0.0 --track@opacity_min:不透明度::最小,0,100,0,0.01
local opacity_max = 100.0 --track@opacity_max:不透明度::最大,0,100,100,0.01
--group:スケール,false
local scale_steps = 2 --track@scale_steps:スケール::段階,2,128,2,1
local scale_min = 100.0 --track@scale_min:スケール::最小,-10000,10000,100,0.01
local scale_max = 100.0 --track@scale_max:スケール::最大,-10000,10000,100,0.01
--group:エッジ検出,false
local edge_intensity = 100.0 --track@edge_intensity:エッジ検出::強度,0,1000,100,0.01
local edge_threshold = -100.0 --track@edge_threshold:エッジ検出::しきい値,-100,100,-100,0.01
--group:文字,false
local characters_pool = "" --string@characters_pool:文字::候補,
local characters_font = "Noto Sans JP Black" --font@characters_font:文字::フォント,Noto Sans JP Black
local characters_scale = 100.0 --track@characters_scale:文字::スケール,0,1000,100,0.01
--group:色,false
local color_steps = 2 --track@color_steps:色::段階,2,128,2,1
local color_source = 0 --select@color_source:色::ソース,画像=0,レイヤー=1
local color_image = "" --file@color_image:色::画像,""
local color_layer = 0 --track@color_layer:色::レイヤー,-100,100,0,1,---
--group:追加オプション,false
local unit = 0 --select@unit:単位,フレーム=0,秒=1
local layer_reference = 0 --select@layer_reference:レイヤー参照,絶対=0,相対=1
local seed = 0 --track@seed:シード,-10000,10000,0,1
-- K互換のため末尾に追加（途中挿入すると旧aup2の位置ずれが起きる）
local word_mode = 1 --select@word_mode:単語分割=1,空白のみ（従来）=0,強化（句読点・文字種）=1,文節（簡易）=2

if obj.framerate == 0 then
    print("@error", "Framerate value must be greater than zero")
    return
end

do
    --#include "utilities.lua"
    local utils = require("utilities")
    local lerp, copy_xform, stop = utils.lerp, utils.copy_xform, utils.stop
    local to_color, to_style, to_number = utils.to_color, utils.to_style, utils.to_number

    --#include "words.lua"
    local words_utils = require("words")
    local word_groups = words_utils.word_groups

    local buffer

    do
        buffer = require("string.buffer").new()
    end

    local utf8 = obj.module("UTF8@${PROJECT_NAME}")
    local hash = obj.module("Hash@${PROJECT_NAME}")
    local hash4d = hash.hash4d

    local eps = 1.0e-4

    local max, floor = math.max, math.floor
    local copybuffer, pixelshader = obj.copybuffer, obj.pixelshader
    local ID, INDEX, NUM, LAYER = obj.effect_id, obj.index, obj.num, obj.layer
    local FPS, TIME = obj.framerate, obj.time

    duration = unit == 0 and duration / FPS or duration

    opacity_min = opacity_min * 0.01
    opacity_max = opacity_max * 0.01

    scale_min = scale_min * 0.01
    scale_max = scale_max * 0.01

    color_layer = layer_reference == 0 and max(color_layer, 0) or max(LAYER + color_layer, 0)

    local should_blink_opacity = opacity_max - opacity_min > eps
    local should_blink_scale = scale_max - scale_min > eps
    local should_edge_detect = edge_threshold > -100.0 + eps

    local should_load_lut = false
    if color_source == 0 then
        should_load_lut = color_image ~= ""
    elseif color_source == 1 then
        if color_layer > 0 and color_layer ~= LAYER then
            if not obj.getvalue("layer" .. color_layer) then
                local frame = obj.getvalue("frame_s") + obj.frame
                print("@warn", "No object found in Layer " .. color_layer .. " at Frame " .. frame)
            else
                should_load_lut = true
            end
        end
    end

    if seed >= 0 then
        math.randomseed(seed)
        seed = LAYER + math.random(2147483647) + 1
    else
        seed = -seed
    end

    if math.abs(duration) < eps then
        print("@error", "Duration value is too small for this animation")
        return
    end

    local text = obj.getvalue(LAYER, "テキスト", "テキスト") --[[@as string | nil]]

    local i, n = INDEX, NUM

    if NUM > 1 then
        local content

        if based_on >= 0 then
            local KEY_COUNT = "0fd2dd98-70e4-4c71-a1a5-042eecbfdbe0-" .. ID

            if text ~= nil then
                local c
                if INDEX == 0 then
                    content = text:gsub("\\\\", "\\"):gsub("\\n", "\n"):gsub("<.->", "")

                    c = utf8.count(content, true)
                    global[KEY_COUNT] = tostring(c)
                else
                    c = tonumber(global[KEY_COUNT])
                end

                if type(c) == "number" then
                    if n % c == 0 then
                        i = floor(i * c / n)
                        n = c
                    end
                else
                    print("@warn", "Shared count value is missing or corrupted")
                end

                if INDEX == NUM - 1 then
                    global[KEY_COUNT] = nil
                end
            end
        end

        if text ~= nil and based_on > 0 then
            local KEY_GROUP = "23ac879f-fa88-47fe-9047-98c62eb012d2-" .. ID

            local t
            if INDEX == 0 then
                local regex = obj.module("Regex@${PROJECT_NAME}")

                t = {}

                local j, id, pattern
                if based_on == 1 then
                    j = -1
                    id = -1
                    pattern = [=[[^\s\v\x85\pZ]]=]
                elseif based_on == 2 then
                    if word_mode >= 1 then
                        -- 強化（句読点・文字種）/ 文節（簡易）。Motion と共通の words.lua
                        -- v0.1.0 は存在しない utf8.codes を呼んでいて、この経路は nil 呼び出しで止まっていた
                        t = word_groups(utf8, content, word_mode)
                        pattern = nil
                    else
                        j = 0
                        id = -2
                        pattern = [=[[\s\v\x85\pZ]]=]
                    end
                else
                    j = 0
                    id = -3
                    pattern = "\\n"
                end

                if pattern ~= nil then
                    for _, m in ipairs({ regex.mark(id, content, pattern) }) do
                        if m[1] then
                            j = j + 1
                        end

                        if not m[2] then
                            t[#t + 1] = max(j, 0)
                        end
                    end
                end

                global[KEY_GROUP] = buffer:reset():encode(t):get()
            else
                t = buffer:set(global[KEY_GROUP]):decode()
            end

            if type(t) == "table" and #t == n then
                i = t[i + 1]
                n = t[n] + 1
            else
                print("@warn", "Shared motion group table is missing or corrupted")
            end

            if INDEX == NUM - 1 then
                global[KEY_GROUP] = nil
            end
        elseif based_on == -2 then
            i = 0
            n = 1
        end
    end

    if (TIME - (duration < 0.0 and obj.totaltime or 0.0)) / duration < 1.0 then
        if type(characters_pool) == "string" and characters_pool ~= "" then
            local alignment

            if text ~= nil then
                local styles = {
                    ["標準文字"] = 0,
                    ["影付き文字"] = 1,
                    ["影付き文字(薄)"] = 2,
                    ["縁取り文字"] = 3,
                    ["縁取り文字(細)"] = 4,
                    ["縁取り文字(太)"] = 5,
                    ["縁取り文字(角)"] = 6,
                }

                local alignments = {
                    ["左寄せ[上]"] = 0,
                    ["中央揃え[上]"] = 1,
                    ["右寄せ[上]"] = 2,
                    ["左寄せ[中]"] = 3,
                    ["中央揃え[中]"] = 4,
                    ["右寄せ[中]"] = 5,
                    ["左寄せ[下]"] = 6,
                    ["中央揃え[下]"] = 7,
                    ["右寄せ[下]"] = 8,
                    ["縦書 上寄[右]"] = 9,
                    ["縦書 中央[右]"] = 10,
                    ["縦書 下寄[右]"] = 11,
                    ["縦書 上寄[中]"] = 12,
                    ["縦書 中央[中]"] = 13,
                    ["縦書 下寄[中]"] = 14,
                    ["縦書 上寄[左]"] = 15,
                    ["縦書 中央[左]"] = 16,
                    ["縦書 下寄[左]"] = 17,
                }

                obj.setfont(
                    characters_font,
                    to_number(obj.getvalue(LAYER, "テキスト", "サイズ"), 0) * (tonumber(characters_scale) or 100) * 0.01,
                    to_style(obj.getvalue(LAYER, "テキスト", "文字装飾"), styles),
                    to_color(obj.getvalue(LAYER, "テキスト", "文字色"), 0xffffff),
                    to_color(obj.getvalue(LAYER, "テキスト", "影・縁色"), 0x000000),
                    obj.getvalue(LAYER, "テキスト", "B") ~= "0",
                    obj.getvalue(LAYER, "テキスト", "I") ~= "0",
                    to_number(obj.getvalue(LAYER, "テキスト", "字間"), 0),
                    to_number(obj.getvalue(LAYER, "テキスト", "行間"), 0)
                )

                alignment = alignments[obj.getvalue(LAYER, "テキスト", "文字揃え")]
            else
                obj.setfont(
                    characters_font,
                    max(obj.w, obj.h) * characters_scale * 0.01,
                    0,
                    0xffffff,
                    0x000000,
                    false,
                    false,
                    0,
                    0
                )

                alignment = 4
            end

            local chars = utf8.split(characters_pool, true)

            local hx, _, _, _ = hash4d(i, n, seed, FPS * TIME * 100.0, 1, #chars)

            local xform = {}
            copy_xform(xform, obj)

            obj.load("text", chars[hx], 0.0, 0.0, alignment)

            copy_xform(obj, xform)
        end

        local hx, hy, hz, hw = hash4d(i, n, seed + 1, FPS * TIME * 100.0)

        if should_blink_opacity then
            obj.alpha = lerp(opacity_min, opacity_max, floor(hx * opacity_steps) / (opacity_steps - 1))
        end

        if should_blink_scale then
            local r = floor(hy * scale_steps) / (scale_steps - 1)
            local scale = lerp(scale_min, scale_max, r)
            obj.sx = scale
            obj.sy = scale
            obj.sz = scale
        end

        if should_edge_detect and hz < 0.5 then
            local ok, e = pcall(function()
                if not copybuffer("cache:tmp", "object") then
                    error("Failed to copy buffer")
                end

                obj.effect(
                    "エッジ抽出",
                    "強さ",
                    edge_intensity,
                    "しきい値",
                    edge_threshold,
                    "輝度エッジを抽出",
                    0,
                    "透明度エッジを抽出",
                    1
                )

                pixelshader("alpha_mask@モーション@${PROJECT_NAME}", "cache:tmp", "object", { 0.0 }, "mask")

                if not copybuffer("object", "cache:tmp") then
                    error("Failed to copy buffer")
                end
            end)

            if not ok then
                stop(e)
                return
            end
        end

        if should_load_lut then
            local ok, e = pcall(function()
                local r = floor(hw * color_steps) / (color_steps - 1)

                if not copybuffer("cache:tmp", "object") then
                    error("Failed to copy buffer")
                end

                local xform = {}
                copy_xform(xform, obj)

                if color_source == 0 then
                    if not obj.load("image", color_image) then
                        if not copybuffer("object", "cache:tmp") then
                            error("Failed to copy buffer")
                        end

                        error("Failed to load image file")
                    end
                elseif color_source == 1 then
                    if not obj.load("layer", color_layer, true) then
                        if not copybuffer("object", "cache:tmp") then
                            error("Failed to copy buffer")
                        end

                        error("Failed to load layer data")
                    end
                end

                pixelshader(
                    "map@モーション@${PROJECT_NAME}",
                    "cache:tmp",
                    { "cache:tmp", "object" },
                    { r, 0.5 },
                    "copy",
                    "clamp"
                )

                if not copybuffer("object", "cache:tmp") then
                    error("Failed to copy buffer")
                end

                copy_xform(obj, xform)
            end)

            if not ok then
                stop(e)
                return
            end
        end
    end
end

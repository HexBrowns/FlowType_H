--@カーニング

--require:${AVIUTL2_VERSION}
--information:カーニング@${PROJECT_NAME} v${PROJECT_VERSION} by ${PROJECT_AUTHOR}
--label:${LABEL}

local kerning_mode = 1 --select@kerning_mode:カーニング方式=1,なし=0,メトリクス=1
--group:フィルタ,true
local filter_regex_pattern = "" --string@filter_regex_pattern:フィルタ::正規表現,
local filter_capture_group = 0 --track@filter_capture_group:フィルタ::キャプチャグループ,0,20,0,1
local should_limit_fx = false --checksection@should_limit_fx:フィルタ::下位エフェクトを制限,false,false
--group:変形,false
--separator:ピボット
local xform_pivot_x = 0.0 --track@xform_pivot_x:変形::ピボット::X,-100000,100000,0,0.01
local xform_pivot_y = 0.0 --track@xform_pivot_y:変形::ピボット::Y,-100000,100000,0,0.01
local xform_pivot_z = 0.0 --track@xform_pivot_z:変形::ピボット::Z,-100000,100000,0,0.01
--trackgroup@xform_pivot_x,xform_pivot_y,xform_pivot_z:Group::変形::ピボット
--separator:位置
local xform_position_x = 0.0 --track@xform_position_x:変形::位置::X,-100000,100000,0,0.01
local xform_position_y = 0.0 --track@xform_position_y:変形::位置::Y,-100000,100000,0,0.01
local xform_position_z = 0.0 --track@xform_position_z:変形::位置::Z,-100000,100000,0,0.01
--trackgroup@xform_position_x,xform_position_y,xform_position_z:Group::変形::位置
--separator:回転
local xform_rotation_w = 0.0 --track@xform_rotation_w:変形::回転::W,-3600,3600,0,0.01
local xform_rotation_x = 0.0 --track@xform_rotation_x:変形::回転::X,-3600,3600,0,0.01
local xform_rotation_y = 0.0 --track@xform_rotation_y:変形::回転::Y,-3600,3600,0,0.01
local xform_rotation_z = 0.0 --track@xform_rotation_z:変形::回転::Z,-3600,3600,0,0.01
--#define EULER XYZオイラー=5,XZYオイラー=7,YXZオイラー=11,YZXオイラー=15,ZXYオイラー=19,ZYXオイラー=21
--#define ROTATION_MODES クォータニオン=0,軸角=1,${EULER}
local xform_rotation_mode = 21 --select@xform_rotation_mode:変形::回転::モード=21,${ROTATION_MODES}
--trackgroup@xform_rotation_x,xform_rotation_y,xform_rotation_z:Group::変形::回転
--separator:スケール
local xform_scale_x = 100.0 --track@xform_scale_x:変形::スケール::X,-10000,10000,100,0.01
local xform_scale_y = 100.0 --track@xform_scale_y:変形::スケール::Y,-10000,10000,100,0.01
local xform_scale_z = 100.0 --track@xform_scale_z:変形::スケール::Z,-10000,10000,100,0.01
--trackgroup@xform_scale_x,xform_scale_y,xform_scale_z:Group::変形::スケール
--separator:合成
--#define DARKEN 比較（暗）=7,乗算=3,焼き込みリニア=10
--#define LIGHTEN 比較（明）=6,スクリーン=4,覆い焼きリニア（加算）=1
--#define CONTRAST オーバーレイ=5,リニアライト=11
--#define COMPARATIVE 差の絶対値=12,減算=2
--#define HSL カラー=9,輝度=8
--#define BLEND_MODES 通常=0,${DARKEN},${LIGHTEN},${CONTRAST},${COMPARATIVE},${HSL}
local xform_blend_mode = 0 --select@xform_blend_mode:変形::合成::ブレンドモード,${BLEND_MODES}
local xform_opacity = 100.0 --track@xform_opacity:変形::合成::不透明度,0,100,100,0.01
--separator:対象
local xform_target_local_space = true --checksection@xform_target_local_space:変形::対象::ローカル空間,true,false
local xform_target_world_space = false --checksection@xform_target_world_space:変形::対象::ワールド空間,false,false
--group:色調,false
local tint_color = nil --color@tint_color:色調::色,nil
local tint_opacity = 100.0 --track@tint_opacity:色調::不透明度,0,100,100,0.01
--group:追加オプション,false
local influence = 100.0 --track@influence:影響度,0,100,100,0.01

if obj.num < 2 then
    print("@error", "Enable Multi Object to use this script")
    return
end

do
    local _ = obj.setanchor("xform_position_x,xform_position_y,xform_position_z", 0, "line", "xyz")

    --#include "utilities.lua"
    local utils = require("utilities")
    local lerp = utils.lerp

    local buffer

    do
        buffer = require("string.buffer").new()
    end

    local ID = obj.effect_id
    local KEY_COUNT = "8973f111-5db5-4890-907d-52539fe55570-" .. ID

    local getvalue = obj.getvalue
    local INDEX, NUM, LAYER, FPS, TIME = obj.index, obj.num, obj.layer, obj.framerate, obj.time

    xform_scale_x = xform_scale_x * 0.01
    xform_scale_y = xform_scale_y * 0.01
    xform_scale_z = xform_scale_z * 0.01
    xform_opacity = xform_opacity * 0.01

    tint_opacity = tint_opacity * 0.01

    influence = influence * 0.01

    local text = getvalue(LAYER, "テキスト", "テキスト") --[[@as string | nil]]
    if text == nil then
        print("@error", "'テキスト' effect was not found in the source")
        return
    end

    local content = INDEX == 0 and text:gsub("\\\\", "\\"):gsub("\\n", "\n"):gsub("<.->", "") or nil

    local i, n = INDEX, NUM

    local c
    if INDEX == 0 then
        local utf8 = obj.module("UTF8@${PROJECT_NAME}")

        c = utf8.count(content, true)
        global[KEY_COUNT] = tostring(c)
    else
        c = tonumber(global[KEY_COUNT])
    end

    if type(c) == "number" then
        if n % c == 0 then
            i = math.floor(i * c / n)
            n = c
        end
    else
        print("@warn", "Shared count value is missing or corrupted")
    end

    if INDEX == NUM - 1 then
        global[KEY_COUNT] = nil
    end

    local offset = { 0.0, 0.0 }
    if kerning_mode == 1 then
        local KEY_KERNING = "308494ea-bd57-4fbf-9573-458df515646c-" .. ID

        local t
        if INDEX == 0 then
            local kerning = obj.module("Kerning@${PROJECT_NAME}")

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

            t = {
                kerning.shift(
                    obj.id,
                    content,
                    getvalue(LAYER, "テキスト", "サイズ"),
                    getvalue(LAYER, "テキスト", "フォント"),
                    alignments[getvalue(LAYER, "テキスト", "文字揃え")],
                    getvalue(LAYER, "テキスト", "B") ~= "0",
                    getvalue(LAYER, "テキスト", "I") ~= "0"
                ),
            }
            global[KEY_KERNING] = buffer:reset():encode(t):get()
        else
            t = buffer:set(global[KEY_KERNING]):decode()
        end

        if type(t) == "table" and #t == n then
            offset = t[i + 1]
        else
            print("@warn", "Shared kerning offset table is missing or corrupted")
        end

        if INDEX == NUM - 1 then
            global[KEY_KERNING] = nil
        end
    end

    local is_matched = true
    if filter_regex_pattern ~= "" then
        local KEY_REGEX = "db5638b7-59ab-4c53-8471-772fc3b03366-" .. ID

        local t = {}
        if INDEX == 0 then
            local regex = obj.module("Regex@${PROJECT_NAME}")

            for _, m in ipairs({ regex.mark(ID, content, filter_regex_pattern, filter_capture_group) }) do
                if not m[2] then
                    t[#t + 1] = m[1]
                end
            end
            global[KEY_REGEX] = buffer:reset():encode(t):get()
        else
            t = buffer:set(global[KEY_REGEX]):decode()
        end

        if type(t) == "table" and #t == n then
            is_matched = t[i + 1]
        else
            print("@warn", "Shared regex filter table is missing or corrupted")
        end

        if INDEX == NUM - 1 then
            global[KEY_REGEX] = nil
        end
    end

    if is_matched then
        local vector = obj.module("Vector@${PROJECT_NAME}")
        local rotate = vector.rotate

        if xform_target_world_space then
            local v = {
                (obj.ox - xform_pivot_x * influence) * (1.0 + (xform_scale_x - 1.0) * influence),
                (obj.oy - xform_pivot_y * influence) * (1.0 + (xform_scale_y - 1.0) * influence),
                (obj.oz - xform_pivot_z * influence) * (1.0 + (xform_scale_z - 1.0) * influence),
            }

            v = rotate(
                influence,
                xform_rotation_mode,
                xform_rotation_w,
                xform_rotation_x,
                xform_rotation_y,
                xform_rotation_z,
                v
            )

            obj.ox = v[1]
            obj.oy = v[2]
            obj.oz = v[3]
        end

        if xform_target_local_space then
            local rx, ry, rz = rotate(
                influence,
                xform_rotation_mode,
                xform_rotation_w,
                xform_rotation_x,
                xform_rotation_y,
                xform_rotation_z
            )

            obj.cx = obj.cx + xform_pivot_x * influence
            obj.cy = obj.cy + xform_pivot_y * influence
            obj.cz = obj.cz + xform_pivot_z * influence
            obj.rx = obj.rx + rx
            obj.ry = obj.ry + ry
            obj.rz = obj.rz + rz
            obj.sx = lerp(obj.sx, obj.sx * xform_scale_x, influence)
            obj.sy = lerp(obj.sy, obj.sy * xform_scale_y, influence)
            obj.sz = lerp(obj.sz, obj.sz * xform_scale_z, influence)
        end

        if xform_target_local_space or xform_target_world_space then
            obj.ox = obj.ox + (xform_position_x + offset[1]) * influence
            obj.oy = obj.oy + (xform_position_y + offset[2]) * influence
            obj.oz = obj.oz + xform_position_z * influence
        end

        obj.alpha = lerp(obj.alpha, obj.alpha * xform_opacity, influence)

        ---@diagnostic disable-next-line: param-type-mismatch
        obj.setoption("blend", xform_blend_mode)

        if tint_color ~= nil then
            local r, g, b = RGB(tint_color)
            obj.pixelshader(
                "tint@モーション@${PROJECT_NAME}",
                "object",
                "object",
                { r / 255.0, g / 255.0, b / 255.0, 1.0, tint_opacity * influence }
            )
        end
    elseif should_limit_fx then
        obj.draw()
    end
end

local order_rank

do
    local abs, floor = math.abs, math.floor

    -- 順序の選択肢「中央から=3」「外側から=4」を、時刻用の順位と段数に写す（v0.2.0）
    -- それ以外（順方向=0 / 逆方向=1 / ランダム=2）は i, n をそのまま返す
    --
    --   中央から: 順位 = floor(|i - (n - 1) / 2|)、段数 = ceil(n / 2)
    --   外側から: 順位 = 段数 - 1 - 中央からの順位
    --
    -- 奇数個は中央の 1 個、偶数個は中央の 2 個が順位 0。左右対称の位置は同じ順位（同時に動く）
    -- 並びは読み順（文字・オブジェクト・セルの番号順）で、画面上の 2 次元の中心ではない
    -- 乱数・レイアウトには元の i, n を使い続けること（左右対称の 2 個が同じ乱数になるため）
    order_rank = function(i, n, mode)
        if mode ~= 3 and mode ~= 4 then
            return i, n
        end

        if n <= 1 then
            return 0, 1
        end

        local m = floor((n + 1) / 2)
        local r = floor(abs(2 * i - (n - 1)) / 2)

        if mode == 4 then
            r = m - 1 - r
        end

        return r, m
    end
end

return {
    order_rank = order_rank,
}

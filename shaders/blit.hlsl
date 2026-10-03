// トリミング@FlowType_H。原作 plugins/filters/intern/trim/intern/shaders/blit.hlsl の写し（変えていない）
Texture2D tex : register(t0);
cbuffer params : register(b0) {
    int2 origin;
}

float4
main(float4 pos : SV_Position) : SV_Target {
    return tex.Load(int3(int2(pos.xy) + origin, 0));
}

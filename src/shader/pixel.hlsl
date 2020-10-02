struct PS_INPUT {
    float4 pos: SV_POSITION;
    float4 color: COLOR0;
    float2 tex_pos: TEXCOORD0;
};

sampler sampler0;
Texture2D texture0;

float4 main(PS_INPUT input): SV_Target {
    float alpha = texture0.Sample(sampler0, input.tex_pos).r;

    if (alpha <= 0.0f) { discard; }

    float4 target0 = input.color;
    target0.a *= alpha;
    return target0;
}

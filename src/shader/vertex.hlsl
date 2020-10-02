cbuffer vertexBuffer: register(b0) {
    float4x4 ProjectionMatrix;
};

struct VS_INPUT {
    uint vertex_id: SV_VertexID;
    float3 left_top: POSITION0;
    float2 right_bottom: POSITION1;
    float2 tex_left_top: TEXCOORD0;
    float2 tex_right_bottom: TEXCOORD1;
    float4 col: COLOR0;
};

struct PS_INPUT {
    float4 pos: SV_POSITION;
    float4 color: COLOR0;
    float2 tex_pos: TEXCOORD0;
};

PS_INPUT main(VS_INPUT input) {
    PS_INPUT o;

    float left = input.left_top.x;
    float right = input.right_bottom.x;
    float top = input.left_top.y;
    float bottom = input.right_bottom.y;
    float2 pos = float2(0.0f, 0.0f);
    
    switch (input.vertex_id) {
        case 0:
            pos = float2(left, top);
            o.tex_pos = input.tex_left_top;
            break;

        case 1:
            pos = float2(right, top);
            o.tex_pos = float2(input.tex_right_bottom.x, input.tex_left_top.y);
            break;

        case 2:
            pos = float2(left, bottom);
            o.tex_pos = float2(input.tex_left_top.x, input.tex_right_bottom.y);
            break;

        case 3:
            pos = float2(right, bottom);
            o.tex_pos = input.tex_right_bottom;
            break;
    }

    o.pos = mul(ProjectionMatrix, float4(pos, input.left_top.z, 1.0f));
    o.color = input.col;
    return o;
}

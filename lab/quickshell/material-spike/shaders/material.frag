#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 itemSize;
    vec2 bodySize;
    float bodyMargin;
    vec4 radii;
    vec4 relief;
    vec4 lighting;
    vec2 interaction;
    vec4 bodyColor;
    vec4 highlightColor;
    vec4 sideColor;
    vec4 edgeColor;
    vec4 shadow;
};

float roundedBox(vec2 c, vec2 halfSize, vec4 r)
{
    float radius = c.x > 0.0 ? (c.y < 0.0 ? r.y : r.z) : (c.y < 0.0 ? r.x : r.w);
    vec2 q = abs(c) - halfSize + vec2(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

float swept(vec2 c, vec2 offset, vec2 halfSize, vec4 r)
{
    if (dot(offset, offset) == 0.0)
        return roundedBox(c, halfSize, r);
    float lo = 0.0;
    float hi = 1.0;
    for (int i = 0; i < 18; ++i) {
        float a = lo + (hi - lo) / 3.0;
        float b = hi - (hi - lo) / 3.0;
        if (roundedBox(c - a * offset, halfSize, r) < roundedBox(c - b * offset, halfSize, r))
            hi = b;
        else
            lo = a;
    }
    return roundedBox(c - 0.5 * (lo + hi) * offset, halfSize, r);
}

vec2 quintic(float t)
{
    if (t <= 0.0)
        return vec2(0.0);
    if (t >= 1.0)
        return vec2(1.0, 0.0);
    return vec2(t * t * t * (10.0 - 15.0 * t + 6.0 * t * t), 30.0 * t * t * (1.0 - t) * (1.0 - t));
}

vec3 reliefNormal(vec2 p)
{
    float amplitude = relief.x;
    float w = relief.y;
    if (amplitude == 0.0)
        return vec3(0.0, 0.0, 1.0);
    float g[8];
    vec2 d[8];
    float dist[4] = float[4](p.x, bodySize.x - p.x, p.y, bodySize.y - p.y);
    vec2 slope[4] = vec2[4](vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0));
    for (int i = 0; i < 4; ++i) {
        vec2 s = quintic(dist[i] / w);
        g[i] = s.x;
        d[i] = s.y * slope[i];
    }
    vec2 local[4] = vec2[4](p, vec2(bodySize.x - p.x, p.y), bodySize - p, vec2(p.x, bodySize.y - p.y));
    vec2 sign4[4] = vec2[4](vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, -1.0));
    float radius[4] = float[4](radii.x, radii.y, radii.z, radii.w);
    for (int i = 0; i < 4; ++i) {
        g[4 + i] = 1.0;
        d[4 + i] = vec2(0.0);
        float r = radius[i];
        if (r > 0.0) {
            float ratio = w / r;
            float tau = 4.0 * ratio;
            vec2 uv = clamp((vec2(r) - local[i]) / r, 0.0, 1.0);
            vec2 s = quintic((1.0 - dot(uv, uv)) / tau);
            g[4 + i] = s.x;
            d[4 + i] = s.y * 2.0 * uv * ratio / tau * sign4[i];
        }
    }
    vec2 dz = vec2(0.0);
    for (int j = 0; j < 8; ++j) {
        float others = 1.0;
        for (int k = 0; k < 8; ++k)
            if (k != j)
                others *= g[k];
        dz += d[j] * others;
    }
    dz *= amplitude / w;
    return vec3(-dz, 1.0) / sqrt(1.0 + dot(dz, dz));
}

vec3 faceColor(vec2 p)
{
    vec3 n = reliefNormal(p);
    float k = clamp(dot(n.xy, lighting.xy), -1.0, 1.0);
    if (lighting.w > 0.0) {
        float j = clamp(dot(n.xy, interaction), -1.0, 1.0);
        k = (1.0 - lighting.w) * k + lighting.w * j;
    }
    float weight = clamp((1.0 - lighting.z) * k, -1.0, 1.0);
    vec3 tip = weight > 0.0 ? highlightColor.rgb : sideColor.rgb;
    return bodyColor.rgb + abs(weight) * (tip - bodyColor.rgb);
}

vec3 toLinear(vec3 c)
{
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}

vec3 toSrgb(vec3 c)
{
    c = clamp(c, 0.0, 1.0);
    return mix(12.92 * c, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}

float coverage(float distance, float pixel)
{
    return clamp(0.5 - distance / pixel, 0.0, 1.0);
}

void main()
{
    vec2 p = qt_TexCoord0 * itemSize - vec2(bodyMargin);
    vec2 halfSize = 0.5 * bodySize;
    vec2 c = p - halfSize;
    float edge = relief.w;
    vec2 offset = -lighting.xy * relief.z;
    vec4 innerRadii = max(radii - vec4(edge), vec4(0.0));
    float pixel = sqrt(abs(dFdx(p.x) * dFdy(p.y) - dFdy(p.x) * dFdx(p.y)));

    float front = roundedBox(c, halfSize, radii);
    float silhouette = swept(c, offset, halfSize, radii);
    float interior = swept(c, offset, halfSize - vec2(edge), innerRadii);

    float aSilhouette = coverage(silhouette, pixel);
    float aInterior = coverage(interior, pixel);
    float aFace = coverage(max(interior, front), pixel);
    float fFace = aFace;
    float fSide = max(aInterior - aFace, 0.0);
    float fEdge = max(aSilhouette - aInterior, 0.0);
    float alpha = fFace + fSide + fEdge;

    vec3 linear = vec3(0.0);
    if (alpha > 0.0) {
        vec3 mixed = fFace * toLinear(faceColor(p)) + fSide * toLinear(sideColor.rgb) + fEdge * toLinear(edgeColor.rgb);
        linear = mixed / alpha;
    }
    vec4 result = vec4(toSrgb(linear) * alpha, alpha);

    if (shadow.x > 0.0) {
        float spread = swept(c - shadow.zw, offset, halfSize, radii);
        float umbra = shadow.x * (1.0 - smoothstep(0.0, shadow.y, spread));
        result += vec4(0.0, 0.0, 0.0, umbra) * (1.0 - result.a);
    }
    fragColor = result * qt_Opacity;
}

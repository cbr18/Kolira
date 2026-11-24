#version 440
layout(location = 0) out vec4 fragColor;

// Quick Switch closing/reveal wave. Progress runs 0 → 1 in both directions:
// u_reveal = 0 paints the screen from the edges inward (closing),
// u_reveal = 1 opens a hole from the origin outward (reveal).
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float u_progress;
    float u_reveal;
    vec2 u_resolution;
    vec2 u_origin;
    vec3 u_accent;
    vec3 u_c0;
    vec3 u_c2;
};

void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution;
    uv.y = 1.0 - uv.y;                       // Qt origin is top-left

    vec2 p = uv - u_origin;
    p.x *= u_resolution.x / max(u_resolution.y, 1.0);

    float maxD = length(vec2(u_resolution.x / max(u_resolution.y, 1.0), 1.0)) * 0.75;
    float radius = u_progress * maxD;
    float edge = 0.055;

    float d = length(p);
    float mask = smoothstep(radius + edge, radius - edge, d);
    float alpha = mix(mask, 1.0 - mask, u_reveal);

    // Light riding the wave front, plus a faint palette tint behind it.
    float ring = smoothstep(edge * 1.8, 0.0, abs(d - radius));
    vec3 colour = mix(vec3(0.0), mix(u_c0, u_accent, 0.65), ring * 0.9);
    colour += mix(u_c0, u_c2, 0.4) * (1.0 - mask) * 0.06;

    fragColor = vec4(colour, clamp(alpha, 0.0, 1.0) * qt_Opacity);
}
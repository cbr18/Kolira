#version 440
layout(location = 0) out vec4 fragColor;

// Qt 6 RHI: the uniforms live in one std140 block, in the order the QML
// properties are declared.
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float u_time;
    vec2 u_resolution;
    vec3 u_accent;
    vec3 u_c0;
    vec3 u_c1;
    vec3 u_c2;
    float u_intensity;
};

// Soft liquid bands: three offset sine layers, no textures, no particles.
float bands(vec2 p, float t) {
    float total = 0.0;
    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float wave = sin(p.x * (1.4 + fi * 0.55) + t * (0.9 + fi * 0.35) + fi * 1.7);
        wave += 0.5 * sin(p.y * (1.1 + fi * 0.4) - t * (0.6 + fi * 0.2) + fi * 2.3);
        total += smoothstep(0.45, 1.10, wave) * (0.55 - fi * 0.12);
    }
    return clamp(total, 0.0, 1.0);
}

void main() {
    // gl_FragCoord avoids the qt_TexCoord0 / qt_Texcoord0 naming trap between
    // the vertex and fragment stages.
    vec2 uv = gl_FragCoord.xy / max(u_resolution, vec2(1.0));
    float aspect = u_resolution.x / max(u_resolution.y, 1.0);
    vec2 p = (uv - 0.5) * vec2(aspect, 1.0);
    float t = u_time * 0.16;

    float band = bands(p, t);

    // Slow colour drift between the palette entries.
    vec3 colour = mix(u_c0, u_c1, smoothstep(-0.45, 0.45, p.x + 0.22 * sin(t * 0.7)));
    colour = mix(colour, u_c2, smoothstep(-0.5, 0.5, p.y + 0.18 * cos(t * 0.9)));
    colour = mix(colour, u_accent, band);

    // Keep the frame dark: the light lives in the bands.
    float vignette = smoothstep(1.15, 0.15, length(p));
    float glow = 0.10 + band * 0.85;
    vec3 result = colour * glow * vignette * u_intensity;
    result += colour * 0.05 * vignette;
    result += u_accent * 0.02;

    fragColor = vec4(result, qt_Opacity);
}
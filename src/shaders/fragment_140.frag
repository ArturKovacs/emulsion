#version 140
uniform sampler2D tex;
uniform sampler2D tex_nearest;
uniform float bright_shade;
uniform float lod_level;
uniform vec2 texel_size;
in vec2 v_tex_coords;
out vec4 f_color;

// Do not modify the following comment. It's meant for the application to parse
//DEFINE_HERE SHARP_MAGNIFY

vec3 srgb2linear(vec3 color) {
    return pow(color, vec3(1.0/2.2));
}
vec3 linear2srgb(vec3 color) {
    return pow(color, vec3(2.2));
}

float luminance(vec3 col) {
    // return col.r * 0.2126 + col.g * 0.7152 + col.b * 0.0722;
    return col.r * 0.4 + col.g * 0.4 + col.b * 0.2;
}

vec3 levels(vec3 color, float min_v, float max_v) {
    float diff = max_v - min_v;
    return min(max(color - vec3(min_v), vec3(0.0)) / vec3(diff), vec3(1.0));
}

vec3 colorLevels(vec3 color) {
    // return levels(color, 0.1, 0.9);
    return color;
}

vec3 edgeLevels(vec3 color) {
    return levels(color, 0.2, 0.8);
    // return levels(color, 0.0, 1.0);
}

vec3 edgeSample(vec2 uv) {
    return edgeLevels(srgb2linear(texture2D(tex, uv).rgb));
}

vec2 getEdgeNormal(vec2 uv) {
    vec2 step_hor = vec2(texel_size.x * 0.45, 0.0);
    vec3 right = edgeSample(uv + step_hor);
    vec3 left = edgeSample(uv - step_hor);
    float hor = luminance(right) - luminance(left);

    vec2 step_vert = vec2(0.0, texel_size.y * 0.45);
    vec3 above = edgeSample(uv + step_vert);
    vec3 below = edgeSample(uv - step_vert);
    float vert = luminance(above) - luminance(below);

    return vec2(hor, vert);
}

// vec2 vecCurve(vec2 v) {
//     vec2 s = sign(v);
//     return pow(abs(v), vec2(0.5)) * s;
// }
vec2 vecCurve(vec2 v) {
    return (v / (1.0 + length(v))) * 2.0;
}

vec3 comicSample(vec2 uv) {
    // vec2 centralEdge = getEdge(uv);
    vec2 centralNormal = vec2(0.0);
    vec2 step_hor = vec2(texel_size.x * 0.25, 0.0);
    vec2 step_vert = vec2(0.0, texel_size.y * 0.25);
    centralNormal += getEdgeNormal(uv + step_hor);
    centralNormal += getEdgeNormal(uv - step_hor);
    centralNormal += getEdgeNormal(uv + step_vert);
    centralNormal += getEdgeNormal(uv - step_vert);
    centralNormal *= 0.25;
    centralNormal = vecCurve(centralNormal);
    // return vec3(0.0, (centralNormal * 0.5) + vec2(0.5));
    vec3 centralColor = texture2D(tex, uv).rgb;

    vec2 posA = uv + centralNormal * texel_size * 0.85;
    vec2 posB = uv - centralNormal * texel_size * 0.85;

    vec3 colorA = colorLevels(texture2D(tex, posA).rgb);
    vec3 colorB = colorLevels(texture2D(tex, posB).rgb);
    
    vec3 color = distance(centralColor, colorA) < distance(centralColor, colorB) ? colorA : colorB;
    return color;
}

vec3 heightSample(vec2 uv) {
    float lum = luminance(srgb2linear(texture2D(tex, uv).rgb));
    // float value = smoothstep(0.45, 0.55, );
    return linear2srgb(vec3(step(0.7, lum)));
}

void main() {
    vec4 color = textureLod(tex, v_tex_coords, lod_level);

#if SHARP_MAGNIFY == 1
    vec3 comicColor = comicSample(v_tex_coords);
#else
    vec3 comicColor = color.rgb;
#endif

    const float grid_size = 12.0;
    vec4 grid_color;
    if ((mod(gl_FragCoord.x, grid_size * 2.0) < grid_size)
        ^^ (mod(gl_FragCoord.y, grid_size * 2.0) < grid_size)
    ) {
        grid_color = vec4(bright_shade);
    } else {
        grid_color = vec4(bright_shade * 0.55);
    }
    f_color = mix(grid_color, vec4(comicColor, 1.0), color.a);
}

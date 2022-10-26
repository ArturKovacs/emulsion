#version 140
uniform sampler2D tex;
uniform sampler2D tex_nearest;
uniform float bright_shade;
uniform float lod_level;
uniform vec2 texel_size;
in vec2 v_tex_coords;
out vec4 f_color;

vec3 srgb2linear(vec3 color) {
    return pow(color, vec3(1.0/2.2));
}
vec3 linear2srgb(vec3 color) {
    return pow(color, vec3(2.2));
}

float luminance(vec3 col) {
    return col.r * 0.2126 + col.g * 0.7152 + col.b * 0.0722;
}

vec3 levels(vec3 color, float min_v, float max_v) {
    float diff = max_v - min_v;
    return max(color - vec3(min_v), vec3(0.0)) / vec3(diff);
}

vec3 colorLevels(vec3 color) {
    // return levels(color, 0.1, 0.9);
    return color;
}

vec3 edgeLevels(vec3 color) {
    return levels(color, 0.2, 0.8);
}

vec3 edgeSample(vec2 uv) {
    return edgeLevels(srgb2linear(texture2D(tex, uv).rgb));
}

vec2 getEdge(vec2 uv) {
    vec2 step_hor = vec2(texel_size.x * 0.4, 0.0);
    vec3 right = edgeSample(uv + step_hor);
    vec3 left = edgeSample(uv - step_hor);
    float hor = luminance(right) - luminance(left);

    vec2 step_vert = vec2(0.0, texel_size.y * 0.4);
    vec3 above = edgeSample(uv + step_vert);
    vec3 below = edgeSample(uv - step_vert);
    float vert = luminance(below) - luminance(above);

    return vec2(vert, hor);
}

vec3 edgeBlur(vec2 pos, vec2 edge) {
    vec2 nextPos = pos + edge * texel_size;
    vec2 prevPos = pos - edge * texel_size;

    vec3 curr_col = texture2D(tex_nearest, pos).rgb;
    vec3 next_col = texture2D(tex_nearest, nextPos).rgb;
    vec3 prev_col = texture2D(tex_nearest, prevPos).rgb;
    return (curr_col + next_col + prev_col) * 0.333;
}

vec3 comicSample(vec2 uv) {
    vec2 centralEdge = getEdge(uv);
    vec2 centralNormal = vec2(centralEdge.y, -centralEdge.x);

    vec2 posA = uv + centralNormal * texel_size;
    vec2 posB = uv - centralNormal * texel_size;
    vec2 edgeA = getEdge(posA);
    vec2 edgeB = getEdge(posB);

    // vec3 colorA = colorLevels(texture2D(tex_nearest, posA).rgb);
    // vec3 colorB = colorLevels(texture2D(tex_nearest, posB).rgb);
    vec3 colorA = colorLevels(edgeBlur(posA, edgeA).rgb);
    vec3 colorB = colorLevels(edgeBlur(posB, edgeB).rgb);
    
    vec3 color = dot(centralEdge, edgeA) > dot(centralEdge, edgeB) ? colorB : colorA;
    return color;
}

vec3 heightSample(vec2 uv) {
    float lum = luminance(srgb2linear(texture2D(tex, uv).rgb));
    // float value = smoothstep(0.45, 0.55, );
    return linear2srgb(vec3(step(0.7, lum)));
}

void main() {
    vec4 color = textureLod(tex, v_tex_coords, lod_level);

    vec3 comicColor = comicSample(v_tex_coords);

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

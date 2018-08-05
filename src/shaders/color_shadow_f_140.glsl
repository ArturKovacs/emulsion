#version 140
uniform vec4 color;
uniform vec2 size;
uniform float shadow_offset;
in vec2 v_tex_coords;
out vec4 f_color;

void main() {
    const float shadow_size = 12.0;
    float shadow_pixel_offset = shadow_size * shadow_offset;
    vec2 tex_cood_from_edge = vec2(0.5) - abs(v_tex_coords - vec2(0.5));
    vec2 shadow_along_axes = 
        max(vec2(0.0), vec2(1.0) - (tex_cood_from_edge * size + shadow_pixel_offset) / shadow_size);

    float shadow = shadow_along_axes.x + shadow_along_axes.y;
    f_color = mix(color, vec4(vec3(0.0), 1.0), shadow);
}

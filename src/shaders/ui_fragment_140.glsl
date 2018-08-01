#version 140
uniform sampler2D tex;
uniform float brighten;
in vec2 v_tex_coords;
out vec4 f_color;
void main() {
    vec4 color = texture(tex, v_tex_coords);
    color = vec4(mix(color.rgb, vec3(1.0), max(0.0, brighten)), color.a);
    color = vec4(mix(color.rgb, vec3(0.0), -min(0.0, brighten)), color.a);
    f_color = color;
}

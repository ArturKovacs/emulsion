#version 140
uniform vec4 color;
in vec2 v_tex_coords;
out vec4 f_color;

void main() {
    f_color = color;
}

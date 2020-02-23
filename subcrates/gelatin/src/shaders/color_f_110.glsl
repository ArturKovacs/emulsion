#version 110
uniform vec4 color;
varying vec2 v_tex_coords;

void main() {
    gl_FragColor = color;
}

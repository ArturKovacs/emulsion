#version 110
uniform sampler2D tex;
uniform float brighten;
varying vec2 v_tex_coords;
void main() {
    vec4 color = texture(tex, v_tex_coords);
    color = vec4(mix(color.rgb, vec3(1.0), max(0.0, brighten)), color.a);
    color = vec4(mix(color.rgb, vec3(0.0), -min(0.0, brighten)), color.a);
    gl_FragColor = color;
}

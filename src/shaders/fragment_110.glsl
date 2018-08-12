#version 110
uniform sampler2D tex;
uniform float bright_shade;
varying vec2 v_tex_coords;
void main() {
    vec4 color = texture2D(tex, v_tex_coords);
    const float grid_size = 12.0;
    vec4 grid_color;
    if ((mod(gl_FragCoord.x, grid_size * 2.0) < grid_size)
        ^^ (mod(gl_FragCoord.y, grid_size * 2.0) < grid_size)
    ) {
        grid_color = vec4(bright_shade);
    } else {
        grid_color = vec4(bright_shade * 0.55);
    }
    gl_FragColor = mix(grid_color, color, color.a);
}

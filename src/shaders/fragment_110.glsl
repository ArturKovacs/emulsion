#version 110
uniform sampler2D tex;
varying vec2 v_tex_coords;
void main() {
    vec4 color = texture2D(tex, v_tex_coords);
    const float grid_size = 8.0;
    vec4 grid_color;
    if ((mod(gl_FragCoord.x, grid_size * 2.0) < grid_size)
        ^^ (mod(gl_FragCoord.y, grid_size * 2.0) < grid_size)
    ) {
        grid_color = vec4(0.9);
    } else {
        grid_color = vec4(0.5);
    }
    gl_FragColor = mix(grid_color, color, color.a);
    //gl_FragColor = texture2D(tex, v_tex_coords);
}
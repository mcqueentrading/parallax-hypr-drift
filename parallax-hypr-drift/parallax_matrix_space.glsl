// parrlax-hypr-drift background.
// Conservative DriftWM GLSL ES 1.00 shader: stock dot-grid style, but with
// several camera-speed layers to create parallax depth while panning.
precision mediump float;

varying vec2 v_coords;
uniform vec2 size;
uniform vec2 u_camera;

const vec4 BG_COLOR = vec4(0.0, 0.0, 0.0, 1.0);
const vec3 FAR_COLOR = vec3(0.24, 0.55, 0.90);
const vec3 MID_COLOR = vec3(0.0, 0.95, 0.75);
const vec3 NEAR_COLOR = vec3(1.0, 1.0, 1.0);

float dots(vec2 p, float spacing, float radius) {
    vec2 grid = mod(p, spacing);
    vec2 dist_to_dot = min(grid, spacing - grid);
    float d = length(dist_to_dot);
    return 1.0 - smoothstep(radius - 0.5, radius + 0.5, d);
}

float lines(vec2 p, float spacing, float width) {
    vec2 grid = mod(p, spacing);
    vec2 dist_to_line = min(grid, spacing - grid);
    float d = min(dist_to_line.x, dist_to_line.y);
    return 1.0 - smoothstep(width, width + 1.0, d);
}

void main() {
    vec2 screen = v_coords * size;
    vec2 center = screen - size * 0.5;

    vec2 far_pos = screen + mod(u_camera * 0.18, 180.0);
    vec2 mid_pos = screen + mod(u_camera * 0.42, 110.0);
    vec2 near_pos = screen + mod(u_camera * 0.78, 80.0);

    float far_dots = dots(far_pos, 180.0, 1.2);
    float mid_dots = dots(mid_pos + vec2(40.0, 20.0), 110.0, 1.0);
    float near_dots = dots(near_pos, 80.0, 0.9);
    float grid = lines(near_pos, 160.0, 0.7);

    float vignette = smoothstep(0.95, 0.20, length(center / size));
    vec3 bg = mix(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.035, 0.055), vignette);
    vec3 col = bg;
    col += FAR_COLOR * far_dots * 0.30;
    col += MID_COLOR * mid_dots * 0.40;
    col += NEAR_COLOR * near_dots * 0.75;
    col += MID_COLOR * grid * 0.08;

    gl_FragColor = vec4(col, 1.0);
}

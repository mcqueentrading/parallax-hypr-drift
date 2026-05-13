// border.glsl — uniform-thickness stroke around a rounded rectangle.
// The stroke sits OUTSIDE the inner rect; the outer arc radius equals
// u_inner_radius + u_border_width, which keeps stroke thickness constant
// along the rounded corners (treating the single radius as the outer arc
// would pinch the stroke at corners).
precision mediump float;
varying vec2 v_coords;
uniform float alpha;
uniform vec2 size;          // element size in pixels
uniform vec4 u_inner_rect;  // (x, y, w, h) of inner content rect within element
uniform float u_inner_radius;
uniform float u_border_width;
uniform vec4 u_color;

float sd_rounded_box(vec2 p, vec2 half_size, float r) {
    vec2 q = abs(p) - half_size + vec2(r);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    vec2 pixel = v_coords * size;
    vec2 inner_center = u_inner_rect.xy + u_inner_rect.zw * 0.5;
    vec2 inner_half = u_inner_rect.zw * 0.5;
    vec2 p = pixel - inner_center;

    float outer_radius = u_inner_radius + u_border_width;
    vec2 outer_half = inner_half + vec2(u_border_width);

    float sd_inner = sd_rounded_box(p, inner_half, u_inner_radius);
    float sd_outer = sd_rounded_box(p, outer_half, outer_radius);

    // Stroke region: outside the inner rect AND inside the outer rect.
    // Smoothstep over a 1-pixel band gives anti-aliasing at both edges.
    float a_outer = clamp(0.5 - sd_outer, 0.0, 1.0);
    float a_inner = clamp(0.5 + sd_inner, 0.0, 1.0);
    float coverage = a_outer * a_inner;

    gl_FragColor = u_color * coverage * alpha;
}

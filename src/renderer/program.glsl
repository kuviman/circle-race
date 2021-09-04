varying vec2 v_vt;

#ifdef VERTEX_SHADER
attribute vec2 a_pos;

uniform mat4 u_projection_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_model_matrix;

void main() {
    v_vt = vec2(a_pos.x, 1.0 - a_pos.y);
    gl_Position = u_projection_matrix * u_view_matrix * u_model_matrix * vec4(a_pos, 0.0, 1.0);
}
#endif

#ifdef FRAGMENT_SHADER
uniform vec4 u_color;
uniform float u_inner;
void main() {
    float len = length(v_vt - vec2(0.5, 0.5)) * 2.0;
    if (len > 1.0 || len < u_inner) {
        discard;
    }
    gl_FragColor = u_color;
}
#endif
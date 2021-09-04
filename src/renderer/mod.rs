use super::*;

#[derive(ugli::Vertex, Clone)]
pub struct Vertex {
    pub a_pos: Vec2<f32>,
}

pub struct Renderer {
    quad: ugli::VertexBuffer<Vertex>,
    program: ugli::Program,
}

impl Renderer {
    pub fn new(geng: &Rc<Geng>) -> Self {
        Self {
            quad: ugli::VertexBuffer::new_static(
                geng.ugli(),
                vec![
                    Vertex {
                        a_pos: vec2(0.0, 0.0),
                    },
                    Vertex {
                        a_pos: vec2(1.0, 0.0),
                    },
                    Vertex {
                        a_pos: vec2(1.0, 1.0),
                    },
                    Vertex {
                        a_pos: vec2(0.0, 1.0),
                    },
                ],
            ),
            program: geng
                .shader_lib()
                .compile(include_str!("program.glsl"))
                .unwrap(),
        }
    }
    pub fn draw(
        &self,
        framebuffer: &mut ugli::Framebuffer,
        camera: &Camera,
        position: Vec2<f32>,
        inner_radius: f32,
        outer_radius: f32,
        color: Color<f32>,
    ) {
        let camera_uniforms = camera.uniforms(framebuffer.size().map(|x| x as f32));
        let uniforms = (
            camera_uniforms,
            ugli::uniforms! {
                u_model_matrix: Mat4::translate(position.extend(0.0)) * Mat4::scale_uniform(outer_radius * 2.0) * Mat4::translate(vec3(-0.5, -0.5, 0.0)),
                u_color: color,
                u_inner: inner_radius / outer_radius,
            },
        );
        ugli::draw(
            framebuffer,
            &self.program,
            ugli::DrawMode::TriangleFan,
            &self.quad,
            uniforms,
            ugli::DrawParameters {
                blend_mode: Some(default()),
                ..default()
            },
        );
    }
}

use geng::prelude::*;

mod camera;
mod renderer;

use camera::*;
use renderer::*;

#[derive(geng::Assets)]
struct Assets {
    #[asset(path = "thruster.mp3")]
    thruster: geng::Sound,
    #[asset(path = "bump.mp3")]
    bump: geng::Sound,
    #[asset(path = "music.ogg")]
    music: geng::Sound,
}

struct Circle {
    pub pos: Vec2<f32>,
    pub r: f32,
}

struct Collision {
    pub pos: Vec2<f32>,
    pub normal: Vec2<f32>,
    pub penetration: f32,
}

impl Circle {
    pub fn collide(&self, other: &Self) -> Option<Collision> {
        let delta_pos = other.pos - self.pos;
        let dist = delta_pos.len();
        let penetration = self.r + other.r - dist;
        if penetration > 0.0 {
            Some(Collision {
                pos: self.pos + delta_pos.normalize() * self.r,
                normal: delta_pos.normalize(),
                penetration,
            })
        } else {
            None
        }
    }
}

struct Player {
    pub pos: Vec2<f32>,
    pub vel: Vec2<f32>,
    pub rotation: f32,
    pub w: f32,
}

impl Player {
    pub fn new(pos: Vec2<f32>) -> Self {
        Self {
            pos,
            vel: vec2(0.0, 0.0),
            rotation: f32::PI / 2.0,
            w: 0.0,
        }
    }
    pub fn update(&mut self, delta_time: f32) {
        const DAMP: f32 = 0.9;
        self.vel -= self.vel * DAMP * delta_time.min(1.0);
        self.w -= self.w * DAMP * delta_time.min(1.0);
        self.pos += self.vel * delta_time;
        self.rotation += self.w * delta_time;
    }
    fn left_thruster_tube(&self) -> Vec2<f32> {
        self.pos + Vec2::rotated(vec2(1.0 - 0.6, 1.0), self.rotation)
    }
    fn right_thruster_tube(&self) -> Vec2<f32> {
        self.pos + Vec2::rotated(vec2(1.0 - 0.6, -1.0), self.rotation)
    }
    fn left_thruster(&self) -> Circle {
        Circle {
            pos: self.pos + Vec2::rotated(vec2(1.0, 1.0), self.rotation),
            r: 0.6,
        }
    }
    fn right_thruster(&self) -> Circle {
        Circle {
            pos: self.pos + Vec2::rotated(vec2(1.0, -1.0), self.rotation),
            r: 0.6,
        }
    }
    fn head(&self) -> Circle {
        Circle {
            pos: self.pos + Vec2::rotated(vec2(-1.0, 0.0), self.rotation),
            r: 0.3,
        }
    }
    pub fn collide(&self, circle: &Circle) -> Option<Collision> {
        if let Some(collision) = self.head().collide(circle) {
            return Some(collision);
        }
        if let Some(collision) = self.left_thruster().collide(circle) {
            return Some(collision);
        }
        if let Some(collision) = self.right_thruster().collide(circle) {
            return Some(collision);
        }
        None
    }
    pub fn apply_impulse(&mut self, impulse: Vec2<f32>, pos: Vec2<f32>) {
        self.vel += impulse;
        self.w += Vec2::skew(pos - self.pos, impulse);
    }
}

struct Particle {
    pub pos: Vec2<f32>,
    pub r: f32,
    pub color: Color<f32>,
    pub vel: Vec2<f32>,
    pub life: f32,
}

impl Particle {
    pub fn update(&mut self, delta_time: f32) {
        self.pos += self.vel * delta_time;
        self.life -= delta_time;
    }
}

struct Game {
    t: f32,
    assets: Assets,
    next_thruster_particle: f32,
    geng: Rc<Geng>,
    renderer: Rc<Renderer>,
    camera: Camera,
    obstacles: Vec<Circle>,
    player: Player,
    particles: Vec<Particle>,
    background: Vec<Vec2<f32>>,
    font: geng::Font,
    laps_done: i32,
    current_lap_timer: Timer,
    best_lap_time: Option<f32>,
    thruster_effect: Option<geng::SoundEffect>,
    music_effect: Option<geng::SoundEffect>,
}

const INNER: f32 = 55.0;
const OUTER: f32 = 70.0;

impl Game {
    pub fn new(geng: &Rc<Geng>, mut assets: Assets) -> Self {
        assets.thruster.looped = true;
        assets.music.looped = true;
        Self {
            music_effect: None,
            t: 0.0,
            assets,
            geng: geng.clone(),
            renderer: Rc::new(Renderer::new(geng)),
            camera: Camera::new(20.0),
            obstacles: {
                let mut result = Vec::new();
                let tire_size = 1.0;
                let noise = noise::OpenSimplex::new();
                let mut add_circle = |r: f32| {
                    let mut angle = 0.0;
                    while angle < 2.0 * f32::PI {
                        let r = r
                            * (1.0
                                + noise::NoiseFn::get(&noise, [angle as f64 * 10.0, 0.0]) as f32
                                    * 0.1);
                        result.push(Circle {
                            pos: Vec2::rotated(vec2(r, 0.0), angle),
                            r: tire_size,
                        });
                        angle += 2.0 * tire_size / r;
                    }
                };
                add_circle(INNER);
                add_circle(OUTER);
                result
            },
            player: Player::new(vec2((INNER + OUTER) / 2.0, 0.0)),
            particles: Vec::new(),
            next_thruster_particle: 0.0,
            background: {
                let mut result = Vec::new();
                let r = (INNER + OUTER) / 2.0;
                let mut angle = 0.0;
                while angle < 2.0 * f32::PI {
                    const RANDOM: f32 = 5.0;
                    result.push(
                        Vec2::rotated(vec2(r, 0.0), angle)
                            + vec2(
                                global_rng().gen_range(-RANDOM..=RANDOM),
                                global_rng().gen_range(-RANDOM..=RANDOM),
                            ),
                    );
                    angle += 2.0 * 3.0 / r;
                }
                result
            },
            font: geng::Font::new(geng, include_bytes!("PixelEmulator-xq08.ttf").to_vec()).unwrap(),
            laps_done: 0,
            best_lap_time: None,
            current_lap_timer: Timer::new(),
            thruster_effect: None,
        }
    }
    fn draw_impl(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Color::WHITE), None);
        for &pos in &self.background {
            self.renderer.draw(
                framebuffer,
                &self.camera,
                pos,
                3.0,
                10.0,
                Color::rgba(0.8, 0.8, 0.8, 0.6),
            );
        }
        const N: usize = 10;
        for i in 0..=N {
            self.renderer.draw(
                framebuffer,
                &self.camera,
                vec2(INNER + (OUTER - INNER) * i as f32 / N as f32, 0.0),
                0.2,
                0.4,
                Color::rgba(0.5, 0.5, 0.5, 1.0),
            );
        }
        for obstacle in &self.obstacles {
            let inner_r = obstacle.r / 3.0;
            self.renderer.draw(
                framebuffer,
                &self.camera,
                obstacle.pos,
                inner_r,
                obstacle.r,
                Color::GRAY,
            );
            self.renderer.draw(
                framebuffer,
                &self.camera,
                obstacle.pos,
                inner_r - 0.1,
                inner_r + 0.1,
                Color::BLACK,
            );
            self.renderer.draw(
                framebuffer,
                &self.camera,
                obstacle.pos,
                obstacle.r - 0.1,
                obstacle.r + 0.1,
                Color::BLACK,
            );
        }

        let head = self.player.head();
        let left_thruster = self.player.left_thruster();
        let right_thruster = self.player.right_thruster();

        {
            const N: usize = 10;
            for i in 0..N / 2 {
                let pos = left_thruster.pos
                    + (right_thruster.pos - left_thruster.pos)
                        * (i as f32 + (self.t * 10.0).fract())
                        / N as f32;
                self.renderer.draw(
                    framebuffer,
                    &self.camera,
                    pos,
                    0.0,
                    0.1,
                    Color::rgba(1.0, 0.0, 0.0, 0.5),
                );
            }
        }

        {
            const N: usize = 10;
            for i in 0..N / 2 {
                let pos = right_thruster.pos
                    + (left_thruster.pos - right_thruster.pos)
                        * (i as f32 + (self.t * 10.0).fract())
                        / N as f32;
                self.renderer.draw(
                    framebuffer,
                    &self.camera,
                    pos,
                    0.0,
                    0.1,
                    Color::rgba(1.0, 0.0, 0.0, 0.5),
                );
            }
        }

        self.renderer.draw(
            framebuffer,
            &self.camera,
            head.pos,
            0.0,
            head.r,
            Color::BLUE,
        );
        self.renderer.draw(
            framebuffer,
            &self.camera,
            head.pos,
            head.r - 0.1,
            head.r + 0.1,
            Color::BLACK,
        );

        for particle in &self.particles {
            self.renderer.draw(
                framebuffer,
                &self.camera,
                particle.pos,
                0.0,
                particle.r,
                particle.color,
            );
        }

        self.renderer.draw(
            framebuffer,
            &self.camera,
            self.player.left_thruster_tube(),
            0.0,
            0.4,
            Color::BLACK,
        );
        self.renderer.draw(
            framebuffer,
            &self.camera,
            self.player.left_thruster_tube(),
            0.0,
            0.25,
            Color::rgb(0.3, 0.3, 0.0),
        );

        self.renderer.draw(
            framebuffer,
            &self.camera,
            self.player.right_thruster_tube(),
            0.0,
            0.4,
            Color::BLACK,
        );
        self.renderer.draw(
            framebuffer,
            &self.camera,
            self.player.right_thruster_tube(),
            0.0,
            0.25,
            Color::rgb(0.3, 0.3, 0.0),
        );

        let mut draw_thruster = |thruster: &Circle| {
            const N: usize = 10;
            for i in 0..N {
                let pos = head.pos
                    + (thruster.pos - head.pos) * (i as f32 + (self.t * 10.0).fract()) / N as f32;
                self.renderer.draw(
                    framebuffer,
                    &self.camera,
                    pos,
                    0.0,
                    0.1,
                    Color::rgba(1.0, 0.0, 0.0, 0.5),
                );
            }
            self.renderer.draw(
                framebuffer,
                &self.camera,
                thruster.pos,
                0.0,
                thruster.r,
                Color::rgb(0.7, 0.7, 0.3),
            );
            self.renderer.draw(
                framebuffer,
                &self.camera,
                thruster.pos,
                thruster.r - 0.1,
                thruster.r + 0.1,
                Color::BLACK,
            );
        };

        draw_thruster(&left_thruster);
        draw_thruster(&right_thruster);
    }
}

const FORCE: f32 = 10.0;

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;
        self.t += delta_time;
        self.camera.target_position = self.player.pos + self.player.vel * 0.7;
        self.camera.target_fov = 20.0 + self.player.vel.len() * 0.3;
        self.camera.update(delta_time * 0.8);
        let left_thruster = self.player.left_thruster();
        let mut left_thruster_force = vec2(0.0, 0.0);
        if self.geng.window().is_key_pressed(geng::Key::Left) {
            left_thruster_force = Vec2::rotated(vec2(FORCE, 0.0), self.player.rotation);
        }
        self.player
            .apply_impulse(left_thruster_force * delta_time, left_thruster.pos);
        let mut right_thruster_force = vec2(0.0, 0.0);
        let right_thruster = self.player.right_thruster();
        if self.geng.window().is_key_pressed(geng::Key::Right) {
            right_thruster_force = Vec2::rotated(vec2(FORCE, 0.0), self.player.rotation);
        }
        if left_thruster_force.len() + right_thruster_force.len() > 1.0 {
            if self.thruster_effect.is_none() {
                let mut effect = self.assets.thruster.effect();
                effect.set_volume(0.3);
                effect.play();
                self.thruster_effect = Some(effect);
            }
            if self.music_effect.is_none() {
                let mut effect = self.assets.music.effect();
                effect.set_volume(0.3);
                effect.play();
                self.music_effect = Some(effect);
            }
        } else {
            if let Some(mut effect) = self.thruster_effect.take() {
                effect.pause();
            }
        }
        self.player
            .apply_impulse(right_thruster_force * delta_time, right_thruster.pos);
        let last_arg = self.player.pos.arg();
        self.player.update(delta_time);
        let now_arg = self.player.pos.arg();
        if now_arg.abs() < 1.0 {
            if last_arg < 0.0 && now_arg >= 0.0 {
                self.laps_done += 1;
                if self.best_lap_time.is_none()
                    || self.best_lap_time.unwrap() > self.current_lap_timer.elapsed() as f32
                {
                    self.best_lap_time = Some(self.current_lap_timer.elapsed() as f32);
                }
                self.current_lap_timer = Timer::new();
            }
            if last_arg >= 0.0 && now_arg < 0.0 {
                self.laps_done -= 1;
            }
        }
        for obstacle in &self.obstacles {
            if let Some(collision) = self.player.collide(obstacle) {
                self.player.pos -= collision.normal * collision.penetration;
                let impulse = -collision.normal * Vec2::dot(collision.normal, self.player.vel);
                let volume = (impulse.len() * 0.3).min(1.0);
                if volume > 0.1 {
                    let mut effect = self.assets.bump.effect();
                    effect.set_volume(volume as f64 * 0.3);
                    effect.play();
                }
                self.player.apply_impulse(impulse, collision.pos);
            }
        }
        self.next_thruster_particle -= delta_time;
        while self.next_thruster_particle < 0.0 {
            self.next_thruster_particle += 1.0 / 100.0;
            if left_thruster_force.len() > 0.1 {
                self.particles.push(Particle {
                    pos: self.player.left_thruster_tube(),
                    vel: self.player.vel * 0.5 - left_thruster_force * 0.1
                        + vec2(
                            global_rng().gen_range(-1.0..=1.0),
                            global_rng().gen_range(-1.0..=1.0),
                        ) * 0.6,
                    r: 0.2,
                    color: Color::rgba(1.0, 0.5, 0.0, 0.5),
                    life: 1.0,
                });
            }
            if right_thruster_force.len() > 0.1 {
                self.particles.push(Particle {
                    pos: self.player.right_thruster_tube(),
                    vel: self.player.vel * 0.5 - right_thruster_force * 0.1
                        + vec2(
                            global_rng().gen_range(-1.0..=1.0),
                            global_rng().gen_range(-1.0..=1.0),
                        ) * 0.6,
                    r: 0.2,
                    color: Color::rgba(1.0, 0.5, 0.0, 0.5),
                    life: 1.0,
                });
            }
        }
        for particle in &mut self.particles {
            particle.update(delta_time);
        }
        self.particles.retain(|particle| particle.life > 0.0);
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        if true {
            let texture_height = 200;
            let mut texture = ugli::Texture2d::new_uninitialized(
                self.geng.ugli(),
                vec2(
                    texture_height * framebuffer.size().x / framebuffer.size().y,
                    texture_height,
                ),
            );
            {
                let mut framebuffer = ugli::Framebuffer::new_color(
                    self.geng.ugli(),
                    ugli::ColorAttachment::Texture(&mut texture),
                );
                self.draw_impl(&mut framebuffer);
            }
            texture.set_filter(ugli::Filter::Nearest);
            let framebuffer_size = framebuffer.size();
            ugli::clear(framebuffer, Some(Color::WHITE), None);
            self.geng.draw_2d().textured_quad(
                framebuffer,
                AABB::pos_size(
                    vec2(0.0, framebuffer_size.y as f32),
                    vec2(framebuffer_size.x as f32, -(framebuffer_size.y as f32)),
                ),
                &texture,
                Color::WHITE,
            );
        } else {
            self.draw_impl(framebuffer);
        }

        let framebuffer_size = framebuffer.size();
        let font_size = (framebuffer.size().y / 20) as f32;

        self.geng.draw_2d().quad(
            framebuffer,
            AABB::pos_size(
                vec2(0.0, 0.0),
                vec2(framebuffer_size.x as f32, font_size * 1.1),
            ),
            Color::rgba(1.0, 1.0, 1.0, 0.5),
        );

        self.geng.draw_2d().quad(
            framebuffer,
            AABB::pos_size(
                vec2(0.0, framebuffer_size.y as f32 - font_size * 1.1),
                vec2(framebuffer_size.x as f32, font_size * 1.1),
            ),
            Color::rgba(1.0, 1.0, 1.0, 0.5),
        );

        self.font.draw_aligned(
            framebuffer,
            "LEFT for",
            self.camera.world_to_screen(
                framebuffer_size.map(|x| x as f32),
                vec2((INNER + OUTER) / 2.0, 3.0),
            ) + vec2(0.0, font_size * 0.7),
            0.5,
            font_size * 0.7,
            Color::rgba(0.5, 0.5, 0.5, 1.0),
        );
        self.font.draw_aligned(
            framebuffer,
            "left thruster",
            self.camera.world_to_screen(
                framebuffer_size.map(|x| x as f32),
                vec2((INNER + OUTER) / 2.0, 3.0),
            ),
            0.5,
            font_size * 0.7,
            Color::rgba(0.5, 0.5, 0.5, 1.0),
        );
        self.font.draw_aligned(
            framebuffer,
            "RIGHT for",
            self.camera.world_to_screen(
                framebuffer_size.map(|x| x as f32),
                vec2((INNER + OUTER) / 2.0, -3.0),
            ),
            0.5,
            font_size * 0.7,
            Color::rgba(0.5, 0.5, 0.5, 1.0),
        );
        self.font.draw_aligned(
            framebuffer,
            "right thruster",
            self.camera.world_to_screen(
                framebuffer_size.map(|x| x as f32),
                vec2((INNER + OUTER) / 2.0, -3.0),
            ) + vec2(0.0, -font_size * 0.7),
            0.5,
            font_size * 0.7,
            Color::rgba(0.5, 0.5, 0.5, 1.0),
        );

        self.font.draw(
            framebuffer,
            &format!(
                "PLAY TIME: {}:{}",
                (self.t as i32) / 60,
                (self.t as i32) % 60
            ),
            vec2(5.0, 5.0),
            font_size,
            Color::BLACK,
        );

        self.font.draw_aligned(
            framebuffer,
            &format!("LAPS DONE: {}", self.laps_done),
            vec2(framebuffer_size.x as f32 - 5.0, 5.0),
            1.0,
            font_size,
            Color::BLACK,
        );

        self.font.draw(
            framebuffer,
            &format!(
                "CURRENT LAP: {}:{}",
                (self.current_lap_timer.elapsed() as i32) / 60,
                (self.current_lap_timer.elapsed() as i32) % 60
            ),
            vec2(5.0, framebuffer_size.y as f32 - font_size - 5.0),
            font_size,
            Color::BLACK,
        );

        match self.best_lap_time {
            Some(time) => self.font.draw_aligned(
                framebuffer,
                &format!("BEST LAP: {}:{}", (time as i32) / 60, (time as i32) % 60),
                vec2(
                    framebuffer_size.x as f32 - 5.0,
                    framebuffer_size.y as f32 - font_size - 5.0,
                ),
                1.0,
                font_size,
                Color::BLACK,
            ),
            None => self.font.draw_aligned(
                framebuffer,
                "BEST LAP: N/A",
                vec2(
                    framebuffer_size.x as f32 - 5.0,
                    framebuffer_size.y as f32 - font_size - 5.0,
                ),
                1.0,
                font_size,
                Color::BLACK,
            ),
        }
    }
}

fn main() {
    if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        std::env::set_current_dir(std::path::Path::new(&dir).join("static")).unwrap();
    } else {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = std::env::current_exe().unwrap().parent() {
                std::env::set_current_dir(path).unwrap();
            }
        }
    }
    let geng = Rc::new(Geng::new(geng::ContextOptions {
        title: "TriJam 135".to_owned(),
        ..default()
    }));
    let geng_clone = geng.clone();
    geng::run(
        geng.clone(),
        geng::LoadingScreen::new(
            &geng,
            geng::EmptyLoadingScreen,
            geng::LoadAsset::load(&geng, "."),
            move |assets| Game::new(&geng_clone, assets.unwrap()),
        ),
    );
}

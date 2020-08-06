use crate::graph::NodeGraph;
use glam::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Method {
    Linear,
    Spline,
    Step,
}

#[derive(Debug, Clone)]
pub struct Sampler {
    pub method: Method,
    pub key_frames: Vec<f32>,
    pub float_frames: Vec<f32>,
    pub vec_frames: Vec<Vec3>,
    pub rot_frames: Vec<Quat>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            method: Method::Linear,
            key_frames: Vec::new(),
            float_frames: Vec::new(),
            vec_frames: Vec::new(),
            rot_frames: Vec::new(),
        }
    }
}

impl Sampler {
    pub fn sample_vec3(&self, time: f32, k: usize) -> Vec3 {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        if f <= 0.0 {
            self.vec_frames[0]
        } else {
            match self.method {
                Method::Linear => (1.0 - f) * self.vec_frames[k] + f * self.vec_frames[k + 1],
                Method::Spline => {
                    let t = f;
                    let t2 = t * t;
                    let t3 = t2 * t;
                    let p0 = self.vec_frames[k * 3 + 1];
                    let m0 = (t1 - t0) * self.vec_frames[k * 3 + 2];
                    let p1 = self.vec_frames[(k + 1) * 3 + 1];
                    let m1 = (t1 - t0) * self.vec_frames[(k + 1) * 3];
                    m0 * (t3 - 2.0 * t2 + t)
                        + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                        + p1 * (-2.0 * t3 + 3.0 * t2)
                        + m1 * (t3 - t2)
                }
                Method::Step => self.vec_frames[k],
            }
        }
    }

    pub fn sample_float(&self, time: f32, k: usize, i: usize, count: usize) -> f32 {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        if f <= 0.0 {
            self.float_frames[0]
        } else {
            match self.method {
                Method::Linear => {
                    (1.0 - f) * self.float_frames[k * count + i]
                        + f * self.float_frames[(k + 1) * count + i]
                }
                Method::Spline => {
                    let t = f;
                    let t2 = t * t;
                    let t3 = t2 * t;
                    let p0 = self.float_frames[(k * count + i) * 3 + 1];
                    let m0 = (t1 - t0) * self.float_frames[(k * count + i) * 3 + 2];
                    let p1 = self.float_frames[((k + 1) * count + i) * 3 + 1];
                    let m1 = (t1 - t0) * self.float_frames[((k + 1) * count + i) * 3];
                    m0 * (t3 - 2.0 * t2 + t)
                        + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                        + p1 * (-2.0 * t3 + 3.0 * t2)
                        + m1 * (t3 - t2)
                }
                Method::Step => self.float_frames[k],
            }
        }
    }

    pub fn sample_rotation(&self, time: f32, k: usize) -> Quat {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        if f <= 0.0 {
            self.rot_frames[0]
        } else {
            match self.method {
                Method::Linear => Quat::from(
                    (Vec4::from(self.rot_frames[k]) * (1.0 - f))
                        + (Vec4::from(self.rot_frames[k + 1]) * f),
                ),
                Method::Spline => {
                    let t = f;
                    let t2 = t * t;
                    let t3 = t2 * t;

                    let p0 = Vec4::from(self.rot_frames[k * 3 + 1]);
                    let m0 = Vec4::from(self.rot_frames[k * 3 + 2]) * (t1 - t0);
                    let p1 = Vec4::from(self.rot_frames[(k + 1) * 3 + 1]);
                    let m1 = Vec4::from(self.rot_frames[(k + 1) * 3]) * (t1 - t0);
                    Quat::from(
                        m0 * (t3 - 2.0 * t2 + t)
                            + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                            + p1 * (-2.0 * t3 + 3.0 * t2)
                            + m1 * (t3 - t2),
                    )
                }
                Method::Step => self.rot_frames[k],
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Target {
    Translation,
    Rotation,
    Scale,
    MorphWeights,
}

#[derive(Debug, Clone)]
pub struct Channel {
    pub node_id: u32,
    pub sampler_ids: Vec<u32>,
    pub targets: Vec<Target>,
    pub key_frames: Vec<f32>,

    pub sampler: Method,
    pub vec3s: Vec<Vec3A>,
    pub rotations: Vec<Quat>,
    pub weights: Vec<f32>,

    pub duration: f32,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            sampler_ids: Vec::new(),
            targets: Vec::new(),
            key_frames: Vec::new(),
            node_id: std::u32::MAX,

            sampler: Method::Linear,
            vec3s: Vec::new(),
            rotations: Vec::new(),
            weights: Vec::new(),

            duration: 0.0,
        }
    }
}

impl Channel {
    pub fn sample_translation(&self, time: f32, k: usize) -> Vec3A {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        match self.sampler {
            Method::Linear => (1.0 - f) * self.vec3s[k] + f * self.vec3s[k + 1],
            Method::Spline => {
                let t = f;
                let t2 = t * t;
                let t3 = t2 * t;
                let p0 = self.vec3s[k * 3 + 1];
                let m0 = (t1 - t0) * self.vec3s[k * 3 + 2];
                let p1 = self.vec3s[(k + 1) * 3 + 1];
                let m1 = (t1 - t0) * self.vec3s[(k + 1) * 3];
                m0 * (t3 - 2.0 * t2 + t)
                    + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                    + p1 * (-2.0 * t3 + 3.0 * t2)
                    + m1 * (t3 - t2)
            }
            Method::Step => self.vec3s[k],
        }
    }

    pub fn sample_scale(&self, time: f32, k: usize) -> Vec3A {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        match self.sampler {
            Method::Linear => (1.0 - f) * self.vec3s[k] + f * self.vec3s[k + 1],
            Method::Spline => {
                let t = f;
                let t2 = t * t;
                let t3 = t2 * t;
                let p0 = self.vec3s[k * 3 + 1];
                let m0 = (t1 - t0) * self.vec3s[k * 3 + 2];
                let p1 = self.vec3s[(k + 1) * 3 + 1];
                let m1 = (t1 - t0) * self.vec3s[(k + 1) * 3];
                m0 * (t3 - 2.0 * t2 + t)
                    + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                    + p1 * (-2.0 * t3 + 3.0 * t2)
                    + m1 * (t3 - t2)
            }
            Method::Step => self.vec3s[k],
        }
    }

    pub fn sample_weight(&self, time: f32, k: usize, i: usize, count: usize) -> f32 {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        match self.sampler {
            Method::Linear => {
                (1.0 - f) * self.weights[k * count + i] + f * self.weights[(k + 1) * count + i]
            }
            Method::Spline => {
                let t = f;
                let t2 = t * t;
                let t3 = t2 * t;
                let p0 = self.weights[(k * count + i) * 3 + 1];
                let m0 = (t1 - t0) * self.weights[(k * count + i) * 3 + 2];
                let p1 = self.weights[((k + 1) * count + i) * 3 + 1];
                let m1 = (t1 - t0) * self.weights[((k + 1) * count + i) * 3];
                m0 * (t3 - 2.0 * t2 + t)
                    + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                    + p1 * (-2.0 * t3 + 3.0 * t2)
                    + m1 * (t3 - t2)
            }
            Method::Step => self.weights[k],
        }
    }

    pub fn sample_rotation(&self, time: f32, k: usize) -> Quat {
        let t0 = self.key_frames[k];
        let t1 = self.key_frames[k + 1];
        let f = (time - t0) / (t1 - t0);

        match self.sampler {
            Method::Linear => Quat::from(
                (Vec4::from(self.rotations[k]) * (1.0 - f))
                    + (Vec4::from(self.rotations[k + 1]) * f),
            ),
            Method::Spline => {
                let t = f;
                let t2 = t * t;
                let t3 = t2 * t;

                let p0 = Vec4::from(self.rotations[k * 3 + 1]);
                let m0 = Vec4::from(self.rotations[k * 3 + 2]) * (t1 - t0);
                let p1 = Vec4::from(self.rotations[(k + 1) * 3 + 1]);
                let m1 = Vec4::from(self.rotations[(k + 1) * 3]) * (t1 - t0);
                Quat::from(
                    m0 * (t3 - 2.0 * t2 + t)
                        + p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
                        + p1 * (-2.0 * t3 + 3.0 * t2)
                        + m1 * (t3 - t2),
                )
            }
            Method::Step => self.rotations[k],
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Animation {
    pub name: String,
    pub affected_roots: Vec<u32>,
    pub channels: Vec<Channel>,
    pub time: f32,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: String::new(),
            affected_roots: Vec::new(),
            channels: Vec::new(),
            time: 0.0,
        }
    }
}

#[allow(dead_code)]
impl Animation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, dt: f32, nodes: &mut NodeGraph) {
        self.time += dt;
        self.set_time(self.time, nodes);
    }

    pub fn set_time(&mut self, time: f32, nodes: &mut NodeGraph) {
        self.affected_roots.iter().for_each(|id| {
            let id = *id as usize;
            nodes.trigger_changed(id);
        });

        self.time = time;
        let channels = &mut self.channels;

        channels.iter_mut().for_each(|c| {
            let current_time = time % c.duration;
            let node_id = c.node_id as usize;
            c.targets.iter().for_each(|t| {
                let mut key = 0;
                while current_time > c.key_frames[key as usize + 1] {
                    key += 1;
                }

                match t {
                    Target::Translation => {
                        nodes[node_id].set_translation(c.sample_translation(current_time, key));
                    }
                    Target::Rotation => {
                        nodes[node_id].set_rotation(c.sample_rotation(current_time, key));
                    }
                    Target::Scale => {
                        nodes[node_id].set_scale(c.sample_scale(current_time, key));
                    }
                    Target::MorphWeights => {
                        let node = &mut nodes[node_id];
                        let weights = node.weights.len();
                        for i in 0..weights {
                            node.weights[i] = c.sample_weight(current_time, key, i, weights);
                        }
                        node.morhped = true;
                    }
                }
            });
            nodes[node_id].update_matrix();
        });
    }
}

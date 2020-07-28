use crate::graph::{Node, NodeFlags};
use glam::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Method {
    Linear,
    Spline,
    Step,
}

pub struct Sampler {
    pub method: Method,
    pub key_frames: Vec<f32>,
    pub float_frames: Vec<f32>,
    pub vec_frames: Vec<Vec3>,
    pub rot_frames: Vec<Quat>,
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
    Weights,
}

pub struct Channel {
    pub sampler_ids: Vec<u32>,
    pub targets: Vec<Target>,

    pub node_id: u32,
    pub key: u32,
    pub time: f32,
}

#[allow(dead_code)]
pub struct Animation {
    samplers: Vec<Sampler>,
    channels: Vec<Channel>,

    ticks_per_second: f32,
    duration: f32,
    time: f32,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            samplers: Vec::new(),
            channels: Vec::new(),

            ticks_per_second: 0.0,
            duration: 0.0,
            time: 0.0,
        }
    }
}

#[allow(dead_code)]
impl Animation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, dt: f32, nodes: &mut [Node]) {
        self.time += dt;
        self.set_time(self.time, nodes);
    }

    pub fn set_time(&mut self, time: f32, nodes: &mut [Node]) {
        self.time = time % self.duration;
        let samplers = &mut self.samplers;
        let channels = &mut self.channels;

        channels.iter_mut().for_each(|c| {
            for i in 0..c.sampler_ids.len() {
                let sampler = &mut samplers[c.sampler_ids[i] as usize];
                if sampler.key_frames.is_empty() {
                    // Nothing to update
                    return;
                }

                let target = &c.targets[i];
                let anim_duration = *sampler.key_frames.last().unwrap();
                if time > anim_duration {
                    c.key = 0;
                    c.time = c.time % anim_duration;
                }

                while c.time > sampler.key_frames[c.key as usize + 1] {
                    c.key += 1;
                }

                let node = &mut nodes[c.node_id as usize];
                let key = c.key as usize;

                match target {
                    Target::Translation => {
                        node.set_translation(sampler.sample_vec3(c.time, key));
                    }
                    Target::Rotation => {
                        node.set_rotation(sampler.sample_rotation(c.time, key));
                    }
                    Target::Scale => {
                        node.set_scale(sampler.sample_vec3(c.time, key));
                    }
                    Target::Weights => {
                        let weights = node.weights.len();
                        for i in 0..weights {
                            node.weights[i] = sampler.sample_float(time, key, i, weights);
                        }
                        node.flags.set_flag(NodeFlags::Morphed);
                    }
                }
            }
        });
    }
}

pub mod backend;
pub mod scene;
pub mod system;
pub mod utils;

pub mod prelude {
    pub use crate::backend::*;
    pub use crate::backend::*;
    pub use crate::scene::*;
    pub use crate::system::*;
    pub use crate::utils::collections::*;
    pub use crate::utils::task::*;
    pub use crate::utils::*;
    pub use l3d::prelude::*;
}

pub mod math {
    pub use glam::*;
}

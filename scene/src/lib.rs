pub mod material;
pub mod scene;
pub mod triangle_scene;
pub mod objects;
pub mod constants;
mod utils;

pub use material::*;
pub use scene::*;
pub use triangle_scene::*;
pub use objects::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

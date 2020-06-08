use num::*;

#[derive(Debug, Copy, Clone)]
pub struct Tile {
    pub x_start: usize,
    pub x_end: usize,
    pub y_start: usize,
    pub y_end: usize,
}

pub struct Surface<T: Sized + Copy + Send + Sync> {
    ptr: *mut T,
    width: usize,
    height: usize,
    tile_width: usize,
    tile_height: usize,
    tiles: Vec<Tile>,
}

impl<T: Sized + Copy + Send + Sync> Surface<T> {
    pub fn new(
        slice: &mut [T],
        width: usize,
        height: usize,
        tile_width: usize,
        tile_height: usize,
    ) -> Self {
        let mut surface = Self {
            ptr: slice.as_mut_ptr(),
            width,
            height,
            tile_width,
            tile_height,
            tiles: Vec::new(),
        };

        surface.setup_tiles();
        surface
    }

    fn setup_tiles(&mut self) {
        let width_in_tiles = (self.width as f32 / self.tile_width as f32).ceil() as usize;
        let height_in_tiles = (self.height as f32 / self.tile_height as f32).ceil() as usize;
        let num_tiles = width_in_tiles * height_in_tiles;
        self.tiles = Vec::with_capacity(num_tiles);
        for y in 0..height_in_tiles {
            let start_pixel_y = y * self.tile_height;
            for w in 0..width_in_tiles {
                let start_pixel_x = w * self.tile_width;
                self.tiles.push(Tile {
                    x_start: start_pixel_x,
                    x_end: start_pixel_x + self.tile_width,
                    y_start: start_pixel_y,
                    y_end: start_pixel_y + self.tile_height,
                });
            }
        }
    }

    pub fn get<B: Sized + Integer + Unsigned + Num + NumCast>(&self, x: B, y: B) -> Option<&T> {
        let x: usize = x.to_usize().unwrap();
        let y: usize = y.to_usize().unwrap();

        if x < self.width && y <= self.height {
            unsafe { self.ptr.add(x + y * self.width).as_ref() }
        } else {
            None
        }
    }

    pub fn get_unchecked<B: Sized + Integer + Unsigned + Num + NumCast>(&self, x: B, y: B) -> &T {
        let x: usize = x.to_usize().unwrap();
        let y: usize = y.to_usize().unwrap();

        unsafe { self.ptr.add(x + y * self.width).as_ref().unwrap() }
    }

    pub fn get_mut<B: Sized + Integer + Unsigned + Num + NumCast>(
        &self,
        x: B,
        y: B,
    ) -> Option<&mut T> {
        let x: usize = x.to_usize().unwrap();
        let y: usize = y.to_usize().unwrap();

        if x < self.width && y <= self.height {
            unsafe { self.ptr.add(x + y * self.width).as_mut() }
        } else {
            None
        }
    }

    pub fn get_mut_unchecked<B: Sized + Integer + Unsigned + Num + NumCast>(
        &self,
        x: B,
        y: B,
    ) -> &mut T {
        let x: usize = x.to_usize().unwrap();
        let y: usize = y.to_usize().unwrap();

        unsafe { self.ptr.add(x + y * self.width).as_mut().unwrap() }
    }

    pub fn draw<B: Sized + Integer + Unsigned + Num + NumCast>(&self, x: B, y: B, color: T) {
        let x: usize = x.to_usize().unwrap();
        let y: usize = y.to_usize().unwrap();

        if x < self.width && y <= self.height {
            unsafe {
                (*self.ptr.add(x + y * self.width)) = color;
            }
        }
    }

    pub fn as_tiles(&self) -> &[Tile] {
        self.tiles.as_slice()
    }

    pub unsafe fn as_mut(&self) -> &mut [T] {
        std::slice::from_raw_parts_mut(self.ptr, self.width * self.height)
    }

    pub fn clear(&self) {
        unsafe {
            std::ptr::write_bytes(self.ptr, 0, self.width * self.height);
        }
    }
}

unsafe impl<T: Sized + Copy + Send + Sync> Send for Surface<T> {}

unsafe impl<T: Sized + Copy + Send + Sync> Sync for Surface<T> {}

pub use glam::*;

#[inline(always)]
pub fn vec4_sqrt(vec: Vec4) -> Vec4 {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    unsafe {
        use std::arch::x86_64::_mm_sqrt_ps;
        _mm_sqrt_ps(vec.into()).into()
    }
    #[cfg(any(
        all(not(target_arch = "x86_64"), not(target_arch = "x86")),
        target_arch = "wasm32-unknown-unknown"
    ))]
    {
        Vec4::new(vec[0].sqrt(), vec[1].sqrt(), vec[2].sqrt(), vec[3].sqrt())
    }
}

#[inline(always)]
pub fn vec4_rsqrt(vec: Vec4) -> Vec4 {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    unsafe {
        use std::arch::x86_64::_mm_rsqrt_ps;
        _mm_rsqrt_ps(vec.into()).into()
    }
    #[cfg(any(
        all(not(target_arch = "x86_64"), not(target_arch = "x86")),
        target_arch = "wasm32-unknown-unknown"
    ))]
    {
        Vec4::new(vec[0].sqrt(), vec[1].sqrt(), vec[2].sqrt(), vec[3].sqrt())
    }
}

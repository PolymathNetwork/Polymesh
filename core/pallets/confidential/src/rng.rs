use rand_core::{CryptoRng, Error, RngCore};
use sp_runtime_interface::runtime_interface;

#[cfg(not(feature = "std"))]
use core::num::NonZeroU32;

#[cfg(feature = "std")]
use rand::rngs::OsRng;

/// Host functions to allow native access to OS RNG from Wasm.
///
/// # Change Log
/// - Runtime Spec version 1005. It is incompatible with previous versions, and it requires a
/// binary update.
///
/// # TODO
///  - Use lazy static RNG object to mitigate performance impact during the `OsRng` object
///  creation. We should double-check the security implications of having a global RNG in a
///  multi-threading environment.
#[runtime_interface(wasm_only)]
pub trait NativeRng {
    fn next_u32() -> u32 {
        OsRng::default().next_u32()
    }

    fn next_u64() -> u64 {
        OsRng::default().next_u64()
    }

    fn fill_bytes(dest: &mut [u8]) {
        OsRng::default().fill_bytes(dest)
    }

    fn try_fill_bytes(dest: &mut [u8]) -> i32 {
        match OsRng::default().try_fill_bytes(dest) {
            Ok(..) => 0,
            Err(err) => err.raw_os_error().unwrap_or(1),
        }
    }
}

#[derive(Clone)]
pub struct Rng {
    #[cfg(feature = "std")]
    inner: OsRng,
}

impl Default for Rng {
    #[cfg(feature = "std")]
    fn default() -> Self {
        Rng {
            inner: OsRng::default(),
        }
    }

    #[cfg(not(feature = "std"))]
    fn default() -> Self {
        Rng {}
    }
}

impl RngCore for Rng {
    #[inline]
    #[cfg(feature = "std")]
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    #[inline]
    #[cfg(not(feature = "std"))]
    fn next_u32(&mut self) -> u32 {
        native_rng::next_u32()
    }

    #[inline]
    #[cfg(feature = "std")]
    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    #[inline]
    #[cfg(not(feature = "std"))]
    fn next_u64(&mut self) -> u64 {
        native_rng::next_u64()
    }

    #[inline]
    #[cfg(feature = "std")]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    #[inline]
    #[cfg(not(feature = "std"))]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        native_rng::fill_bytes(dest);
    }

    #[inline]
    #[cfg(feature = "std")]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.inner.try_fill_bytes(dest)
    }

    #[inline]
    #[cfg(not(feature = "std"))]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        match native_rng::try_fill_bytes(dest) {
            0 => Ok(()),
            code => {
                let non_zero_code = unsafe { NonZeroU32::new_unchecked(code as u32) };
                Err(Error::from(non_zero_code))
            }
        }
    }
}

impl CryptoRng for Rng {}

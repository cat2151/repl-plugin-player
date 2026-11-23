//! Asynchronous plugin switching for evolve: requests return at once and are
//! settled by polling, with a generation counter that turns folded cycles stale.
pub mod activity;
#[cfg(test)]
mod alloc_count;
pub mod builder;
pub mod ffi;
pub mod host;
pub mod layout;
pub mod level;
pub mod loader;
pub mod ownership;
mod pad_gain;
pub mod playback;
pub mod pool;
pub mod preload;
#[cfg(test)]
mod test_host;

#[macro_use]
pub mod macros;

/// Constants like memory locations
pub mod consts;

/// Debugging support
pub mod debug;

/// Devices
pub mod device;

/// Interrupt instructions
pub mod interrupt;

/// Inter-processor interrupts
pub mod ipi;

/// Paging
pub mod paging;

pub mod rmm;

/// Initialization and start function
pub mod start;

/// Stop function
pub mod stop;

// Interrupt vectors
pub mod vectors;

/// Early init support
pub mod init;

pub mod time;

pub use ::rmm::AArch64Arch as CurrentRmmArch;

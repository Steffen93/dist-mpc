pub const PERFORM_MEASUREMENTS: bool = true;
pub const HOST_ENV_KEY: &str = "DIST_MPC_HOST";
pub const DEFAULT_HOST: &str = "localhost";
pub const THREADS: usize = 128;
pub static mut TOTAL_BYTES: u64 = 0;
pub static mut TOTAL_GAS: u64 = 0;
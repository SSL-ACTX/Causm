//! `causm-kernel-sys`: Safe Rust FFI declarations for verified Causm Pulse microkernel.

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellT {
    pub payload: u64,
    pub tag: u32,
    pub expire: u64,
}

extern "C" {
    pub fn Pulse_Causm_Arena_read_valid_cell(c: *const CellT) -> u64;
    pub fn Pulse_Causm_Arena_consume_cell(c: *mut CellT);
    pub fn Pulse_Causm_Arena_tick_cell_decay(c: *mut CellT, current_clk: u64);
    pub fn Pulse_Causm_Arena_lease_cell(c: *mut CellT, current_clk: u64, duration: u64);
    pub fn Pulse_Causm_Arena_init_valid_cell(c: *mut CellT, v: u64);
}

impl CellT {
    #[inline]
    pub fn read_valid(&self) -> u64 {
        unsafe { Pulse_Causm_Arena_read_valid_cell(self) }
    }

    #[inline]
    pub fn consume(&mut self) {
        unsafe { Pulse_Causm_Arena_consume_cell(self) }
    }

    #[inline]
    pub fn tick_decay(&mut self, current_clk: u64) {
        unsafe { Pulse_Causm_Arena_tick_cell_decay(self, current_clk) }
    }

    #[inline]
    pub fn lease(&mut self, current_clk: u64, duration: u64) {
        unsafe { Pulse_Causm_Arena_lease_cell(self, current_clk, duration) }
    }

    #[inline]
    pub fn init_valid(&mut self, v: u64) {
        unsafe { Pulse_Causm_Arena_init_valid_cell(self, v) }
    }
}

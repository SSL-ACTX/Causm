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

    pub fn Pulse_Causm_Arena_arena_read(a: *const CellT, idx: usize) -> u64;
    pub fn Pulse_Causm_Arena_arena_consume(a: *mut CellT, idx: usize);
    pub fn Pulse_Causm_Arena_arena_lease(a: *mut CellT, idx: usize, current_clk: u64, duration: u64);
    pub fn Pulse_Causm_Arena_arena_init_valid(a: *mut CellT, idx: usize, v: u64);
    pub fn Pulse_Causm_Arena_arena_tick_decay(a: *mut CellT, idx: usize, current_clk: u64);
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

/// A verified multi-register arena backed by the Pulse microkernel.
pub struct Arena {
    cells: Vec<CellT>,
}

impl Arena {
    pub fn new(capacity: usize) -> Self {
        Self {
            cells: vec![CellT { payload: 0, tag: 3, expire: 0 }; capacity],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[inline]
    pub fn get_cell(&self, idx: usize) -> Option<&CellT> {
        self.cells.get(idx)
    }

    #[inline]
    pub fn read(&self, idx: usize) -> Option<u64> {
        let cell = self.cells.get(idx)?;
        if cell.tag == 0 {
            Some(unsafe { Pulse_Causm_Arena_arena_read(self.cells.as_ptr(), idx) })
        } else {
            None
        }
    }

    #[inline]
    pub fn consume(&mut self, idx: usize) -> bool {
        if let Some(cell) = self.cells.get(idx) {
            if cell.tag == 0 {
                unsafe { Pulse_Causm_Arena_arena_consume(self.cells.as_mut_ptr(), idx) };
                return true;
            }
        }
        false
    }

    #[inline]
    pub fn lease(&mut self, idx: usize, current_clk: u64, duration: u64) -> bool {
        if let Some(cell) = self.cells.get(idx) {
            if cell.tag == 0 && current_clk.checked_add(duration).is_some() {
                unsafe { Pulse_Causm_Arena_arena_lease(self.cells.as_mut_ptr(), idx, current_clk, duration) };
                return true;
            }
        }
        false
    }

    #[inline]
    pub fn init_valid(&mut self, idx: usize, v: u64) -> bool {
        if idx < self.cells.len() {
            unsafe { Pulse_Causm_Arena_arena_init_valid(self.cells.as_mut_ptr(), idx, v) };
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn tick_decay(&mut self, idx: usize, current_clk: u64) -> bool {
        if let Some(cell) = self.cells.get(idx) {
            if cell.tag == 1 && current_clk >= cell.expire {
                unsafe { Pulse_Causm_Arena_arena_tick_decay(self.cells.as_mut_ptr(), idx, current_clk) };
                return true;
            }
        }
        false
    }

    /// Advance global clock and decay all expired leased slots in the arena.
    pub fn tick_sweep(&mut self, current_clk: u64) {
        for idx in 0..self.cells.len() {
            if self.cells[idx].tag == 1 && current_clk >= self.cells[idx].expire {
                unsafe { Pulse_Causm_Arena_arena_tick_decay(self.cells.as_mut_ptr(), idx, current_clk) };
            }
        }
    }
}

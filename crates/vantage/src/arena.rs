//! A scratch arena for terrain regions.
//!
//! The host hands a region over as hundreds of small allocations (every block type's name,
//! state and tags). With the normal allocator, receiving and freeing them costs most of a
//! frame's fuel. While a region is being received, allocations come from a bump arena instead,
//! and freeing them does nothing; the arena is emptied once that region has been dropped.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;

const SIZE: usize = 512 * 1024;

#[repr(C, align(16))]
struct Buffer(UnsafeCell<[u8; SIZE]>);

struct State {
    on: UnsafeCell<bool>,
    top: UnsafeCell<usize>,
}

// SAFETY: the mod runs on a single wasm thread.
unsafe impl Sync for Buffer {}
unsafe impl Sync for State {}

static BUFFER: Buffer = Buffer(UnsafeCell::new([0; SIZE]));
static STATE: State = State { on: UnsafeCell::new(false), top: UnsafeCell::new(0) };

pub struct Arena;

fn base() -> usize {
    BUFFER.0.get() as usize
}

fn owns(ptr: *mut u8) -> bool {
    let at = ptr as usize;
    at >= base() && at < base() + SIZE
}

unsafe impl GlobalAlloc for Arena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: single-threaded; see above.
        unsafe {
            if *STATE.on.get() {
                let top = *STATE.top.get();
                let start = (base() + top).next_multiple_of(layout.align()) - base();
                if start + layout.size() <= SIZE {
                    *STATE.top.get() = start + layout.size();
                    return (base() + start) as *mut u8;
                }
            }
            System.alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !owns(ptr) {
            // SAFETY: anything outside the arena came from the system allocator.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if !owns(ptr) {
            // SAFETY: as above.
            return unsafe { System.realloc(ptr, layout, new_size) };
        }
        // SAFETY: the new block is at least as large as the copied part.
        unsafe {
            let new = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
            if !new.is_null() {
                std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
            }
            new
        }
    }
}

/// Runs `receive` with allocations going to the arena.
pub fn receiving<T>(receive: impl FnOnce() -> T) -> T {
    // SAFETY: single-threaded.
    unsafe { *STATE.on.get() = true };
    let value = receive();
    unsafe { *STATE.on.get() = false };
    value
}

/// Empties the arena. Only call when nothing allocated in it is still alive.
pub fn reset() {
    // SAFETY: single-threaded; callers guarantee nothing in the arena is still in use.
    unsafe { *STATE.top.get() = 0 };
}

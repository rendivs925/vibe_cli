use bumpalo::Bump;
use std::cell::RefCell;

/// Ultra-fast memory arena for zero-latency allocations
/// Uses bump allocation strategy for maximum performance
pub struct UltraFastArena {
    bump: Bump,
}

impl UltraFastArena {
    /// Create a new arena with pre-allocated capacity
    pub fn new() -> Self {
        let bump = Bump::with_capacity(1024 * 1024); // 1MB pre-allocation
        Self { bump }
    }

    /// Allocate a string in the arena (zero-copy, ultra-fast)
    pub fn alloc_str<'a>(&'a self, s: &str) -> &'a str {
        self.bump.alloc_str(s)
    }

    /// Allocate a slice in the arena
    pub fn alloc_slice<'a, T: Copy>(&'a self, slice: &[T]) -> &'a [T] {
        self.bump.alloc_slice_copy(slice)
    }

    /// Reset the arena for reuse (ultra-fast deallocation)
    pub fn reset(&mut self) {
        self.bump.reset();
    }

    /// Get current memory usage
    pub fn allocated_bytes(&self) -> usize {
        self.bump.allocated_bytes()
    }

    /// Execute a function with arena allocation scope
    pub fn with_scope<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&UltraFastArena) -> R,
    {
        let result = f(self);
        self.reset(); // Reset after use for memory efficiency
        result
    }
}

impl Default for UltraFastArena {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    /// Thread-local arena for maximum performance
    static THREAD_ARENA: RefCell<UltraFastArena> = RefCell::new(UltraFastArena::new());
}

/// Get the thread-local arena for ultra-fast allocations
pub fn with_thread_local_arena<F, R>(f: F) -> R
where
    F: FnOnce(&UltraFastArena) -> R,
{
    THREAD_ARENA.with(|arena| {
        let arena_ref = arena.borrow();
        f(&arena_ref)
    })
}

/// Ultra-fast string interning for repeated strings
pub struct StringInterner<'a> {
    strings: std::collections::HashMap<&'a str, &'a str, fxhash::FxBuildHasher>,
}

impl<'a> StringInterner<'a> {
    pub fn new() -> Self {
        Self {
            strings: std::collections::HashMap::with_hasher(fxhash::FxBuildHasher::default()),
        }
    }

    /// Intern a string within the arena lifetime
    pub fn intern(&mut self, arena: &'a UltraFastArena, s: &str) -> &'a str {
        if let Some(&interned) = self.strings.get(s) {
            return interned;
        }

        let interned = arena.alloc_str(s);
        self.strings.insert(interned, interned);
        interned
    }
}

impl<'a> Default for StringInterner<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ultra_fast_arena() {
        let arena = UltraFastArena::new();
        let s1 = arena.alloc_str("hello");
        let s2 = arena.alloc_str("world");
        assert_eq!(s1, "hello");
        assert_eq!(s2, "world");

        let slice = arena.alloc_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::new();
        let arena = UltraFastArena::new();

        let s1 = interner.intern(&arena, "test");
        let s2 = interner.intern(&arena, "test");
        assert_eq!(s1, s2);
        assert_eq!(s1 as *const str, s2 as *const str); // Same pointer
    }
}
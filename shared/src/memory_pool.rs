/// Memory pool and object reuse for frequent allocations
///
/// Provides:
/// - Generic object pooling
/// - SmallVec and ArrayVec alternatives
/// - Buffer pooling for I/O operations
/// - Reusable allocation patterns
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Generic object pool for reusing allocations
pub struct ObjectPool<T> {
    objects: Arc<Mutex<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
}

impl<T> ObjectPool<T>
where
    T: Send + 'static,
{
    /// Create a new object pool
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            objects: Arc::new(Mutex::new(VecDeque::new())),
            factory: Arc::new(factory),
            max_size,
        }
    }

    /// Get an object from the pool or create a new one
    pub fn acquire(&self) -> PooledObject<T> {
        let obj = {
            let mut objects = self.objects.lock().unwrap();
            objects.pop_front()
        };

        let object = obj.unwrap_or_else(|| (self.factory)());

        PooledObject {
            object: Some(object),
            pool: Arc::clone(&self.objects),
            max_size: self.max_size,
        }
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.objects.lock().unwrap().len()
    }

    /// Clear all pooled objects
    pub fn clear(&self) {
        self.objects.lock().unwrap().clear();
    }
}

/// RAII wrapper for pooled objects
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
    max_size: usize,
}

impl<T> PooledObject<T> {
    /// Get a reference to the object
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get a mutable reference to the object
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.object.take() {
            let mut pool = self.pool.lock().unwrap();
            if pool.len() < self.max_size {
                pool.push_back(obj);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// Buffer pool for I/O operations
pub struct BufferPool {
    pool: ObjectPool<Vec<u8>>,
    buffer_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool with specified buffer size
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        let pool = ObjectPool::new(move || Vec::with_capacity(buffer_size), max_buffers);

        Self { pool, buffer_size }
    }

    /// Acquire a buffer from the pool
    pub fn acquire(&self) -> PooledObject<Vec<u8>> {
        let mut buffer = self.pool.acquire();
        buffer.clear(); // Clear previous contents
        buffer.reserve(self.buffer_size);
        buffer
    }

    /// Get pool statistics
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            buffer_size: self.buffer_size,
            pooled_buffers: self.pool.size(),
        }
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    pub buffer_size: usize,
    pub pooled_buffers: usize,
}

/// Stack-allocated small vector (simplified implementation)
///
/// Similar to smallvec but integrated into our codebase
pub enum SmallVec<T, const N: usize> {
    Stack([Option<T>; N], usize), // Data + length
    Heap(Vec<T>),
}

impl<T, const N: usize> SmallVec<T, N> {
    /// Create a new empty SmallVec
    pub fn new() -> Self {
        Self::Stack([const { None }; N], 0)
    }

    /// Push an element
    pub fn push(&mut self, value: T) {
        match self {
            Self::Stack(arr, len) => {
                if *len < N {
                    arr[*len] = Some(value);
                    *len += 1;
                } else {
                    // Spill to heap
                    let mut vec = Vec::with_capacity(N * 2);
                    for item in arr.iter_mut() {
                        if let Some(v) = item.take() {
                            vec.push(v);
                        }
                    }
                    vec.push(value);
                    *self = Self::Heap(vec);
                }
            }
            Self::Heap(vec) => vec.push(value),
        }
    }

    /// Get length
    pub fn len(&self) -> usize {
        match self {
            Self::Stack(_, len) => *len,
            Self::Heap(vec) => vec.len(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get element by index
    pub fn get(&self, index: usize) -> Option<&T> {
        match self {
            Self::Stack(arr, len) => {
                if index < *len {
                    arr[index].as_ref()
                } else {
                    None
                }
            }
            Self::Heap(vec) => vec.get(index),
        }
    }

    /// Iterate over elements
    pub fn iter(&self) -> SmallVecIter<T, N> {
        SmallVecIter {
            small_vec: self,
            index: 0,
        }
    }
}

impl<T, const N: usize> Default for SmallVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator for SmallVec
pub struct SmallVecIter<'a, T, const N: usize> {
    small_vec: &'a SmallVec<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for SmallVecIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.small_vec.get(self.index);
        if result.is_some() {
            self.index += 1;
        }
        result
    }
}

/// Reusable string buffer for building strings
pub struct StringBuffer {
    pool: ObjectPool<String>,
}

impl StringBuffer {
    /// Create a new string buffer pool
    pub fn new(initial_capacity: usize, max_buffers: usize) -> Self {
        let pool = ObjectPool::new(move || String::with_capacity(initial_capacity), max_buffers);

        Self { pool }
    }

    /// Acquire a string buffer
    pub fn acquire(&self) -> PooledObject<String> {
        let mut buffer = self.pool.acquire();
        buffer.clear();
        buffer
    }
}

/// Memory-efficient collection builder
pub struct CollectionBuilder<T> {
    items: Vec<T>,
    capacity: usize,
}

impl<T> CollectionBuilder<T> {
    /// Create with estimated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Add an item
    pub fn add(&mut self, item: T) -> &mut Self {
        self.items.push(item);
        self
    }

    /// Add multiple items
    pub fn add_all<I>(&mut self, items: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
    {
        self.items.extend(items);
        self
    }

    /// Get current size
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Build and return the collection
    pub fn build(self) -> Vec<T> {
        self.items
    }

    /// Build into a boxed slice
    pub fn build_boxed(self) -> Box<[T]> {
        self.items.into_boxed_slice()
    }
}

/// Macro for creating SmallVec
#[macro_export]
macro_rules! small_vec {
    ($($x:expr),* $(,)?) => {{
        let mut sv = $crate::memory_pool::SmallVec::<_, 8>::new();
        $(sv.push($x);)*
        sv
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(|| Vec::<i32>::new(), 10);

        let mut obj1 = pool.acquire();
        obj1.push(1);
        obj1.push(2);

        drop(obj1); // Return to pool

        let mut obj2 = pool.acquire();
        // Object is reused as-is - not automatically cleared
        assert_eq!(obj2.len(), 2);
        // User must clear if needed
        obj2.clear();
        assert_eq!(obj2.len(), 0);

        assert_eq!(pool.size(), 0); // obj2 still in use
    }

    #[test]
    fn test_object_pool_max_size() {
        let pool = ObjectPool::new(|| Vec::<i32>::new(), 2);

        let obj1 = pool.acquire();
        let obj2 = pool.acquire();
        let obj3 = pool.acquire();

        drop(obj1);
        drop(obj2);
        drop(obj3);

        // Only 2 should be pooled (max_size = 2)
        assert_eq!(pool.size(), 2);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024, 5);

        let mut buffer = pool.acquire();
        buffer.extend_from_slice(b"Hello, World!");

        assert_eq!(buffer.len(), 13);

        drop(buffer);

        let buffer2 = pool.acquire();
        assert_eq!(buffer2.len(), 0); // Should be cleared
    }

    #[test]
    fn test_small_vec_stack() {
        let mut sv: SmallVec<i32, 4> = SmallVec::new();

        sv.push(1);
        sv.push(2);
        sv.push(3);

        assert_eq!(sv.len(), 3);
        assert_eq!(sv.get(0), Some(&1));
        assert_eq!(sv.get(1), Some(&2));
        assert_eq!(sv.get(2), Some(&3));
    }

    #[test]
    fn test_small_vec_spill_to_heap() {
        let mut sv: SmallVec<i32, 2> = SmallVec::new();

        sv.push(1);
        sv.push(2);
        sv.push(3); // This should spill to heap

        assert_eq!(sv.len(), 3);
        assert_eq!(sv.get(2), Some(&3));
    }

    #[test]
    fn test_small_vec_iter() {
        let mut sv: SmallVec<i32, 4> = SmallVec::new();
        sv.push(1);
        sv.push(2);
        sv.push(3);

        let collected: Vec<&i32> = sv.iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }

    #[test]
    fn test_string_buffer() {
        let pool = StringBuffer::new(256, 10);

        let mut s1 = pool.acquire();
        s1.push_str("Hello");

        drop(s1);

        let s2 = pool.acquire();
        assert_eq!(s2.len(), 0);
    }

    #[test]
    fn test_collection_builder() {
        let mut builder = CollectionBuilder::with_capacity(10);

        builder.add(1).add(2).add(3).add_all(vec![4, 5, 6]);

        let vec = builder.build();
        assert_eq!(vec, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_pooled_object_deref() {
        let pool = ObjectPool::new(|| vec![0, 1, 2], 5);

        let obj = pool.acquire();
        assert_eq!(obj[0], 0);
        assert_eq!(obj.len(), 3);
    }
}

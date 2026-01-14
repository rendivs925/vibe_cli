use anyhow::Result;
/// Batch processing optimization with parallel execution
///
/// Provides:
/// - Parallel batch operations with rayon
/// - Chunk-based processing for large datasets
/// - Memory-efficient streaming operations
/// - Concurrent I/O operations
use rayon::prelude::*;
use std::path::Path;

/// Batch processor for parallel operations
pub struct BatchProcessor {
    chunk_size: usize,
    max_parallelism: usize,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunk_size,
            max_parallelism: num_cpus::get(),
        }
    }

    /// Create with custom parallelism level
    pub fn with_parallelism(chunk_size: usize, max_parallelism: usize) -> Self {
        Self {
            chunk_size,
            max_parallelism,
        }
    }

    /// Process items in parallel batches
    pub fn process<T, R, F>(&self, items: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Send + Sync,
    {
        rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_parallelism)
            .build()
            .unwrap()
            .install(|| items.into_par_iter().map(processor).collect())
    }

    /// Process items in chunks
    pub fn process_chunks<T, R, F>(&self, items: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send + Sync + Clone,
        R: Send,
        F: Fn(&[T]) -> Vec<R> + Send + Sync,
    {
        let chunks: Vec<Vec<T>> = items
            .chunks(self.chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        chunks
            .into_par_iter()
            .flat_map(|chunk| processor(&chunk))
            .collect()
    }

    /// Process with map-reduce pattern
    pub fn map_reduce<T, R, MapF, ReduceF>(
        &self,
        items: Vec<T>,
        map_fn: MapF,
        reduce_fn: ReduceF,
        initial: R,
    ) -> R
    where
        T: Send,
        R: Send + Sync + Clone,
        MapF: Fn(T) -> R + Send + Sync,
        ReduceF: Fn(R, R) -> R + Send + Sync,
    {
        items
            .into_par_iter()
            .map(map_fn)
            .reduce(|| initial.clone(), reduce_fn)
    }

    /// Filter and process in parallel
    pub fn filter_process<T, F, P>(&self, items: Vec<T>, filter: F, processor: P) -> Vec<T>
    where
        T: Send,
        F: Fn(&T) -> bool + Send + Sync,
        P: Fn(T) -> T + Send + Sync,
    {
        items
            .into_par_iter()
            .filter(filter)
            .map(processor)
            .collect()
    }
}

/// File batch processor for concurrent I/O
pub struct FileBatchProcessor {
    batch_size: usize,
}

impl FileBatchProcessor {
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// Read multiple files in parallel
    pub fn read_files<P: AsRef<Path> + Send + Sync>(&self, paths: Vec<P>) -> Vec<Result<String>> {
        paths
            .par_iter()
            .map(|path| std::fs::read_to_string(path.as_ref()).map_err(Into::into))
            .collect()
    }

    /// Read files and process content in parallel
    pub fn read_and_process<P, F, R>(&self, paths: Vec<P>, processor: F) -> Vec<Result<R>>
    where
        P: AsRef<Path> + Send + Sync,
        F: Fn(String) -> Result<R> + Send + Sync,
        R: Send,
    {
        paths
            .par_iter()
            .map(|path| {
                std::fs::read_to_string(path.as_ref())
                    .map_err(Into::into)
                    .and_then(&processor)
            })
            .collect()
    }
}

/// Vector batch operations
pub struct VectorBatchOps;

impl VectorBatchOps {
    /// Parallel vector transformation
    pub fn transform<T, R, F>(items: Vec<T>, transform_fn: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Send + Sync,
    {
        items.into_par_iter().map(transform_fn).collect()
    }

    /// Parallel filtering
    pub fn filter<T, F>(items: Vec<T>, predicate: F) -> Vec<T>
    where
        T: Send,
        F: Fn(&T) -> bool + Send + Sync,
    {
        items.into_par_iter().filter(predicate).collect()
    }

    /// Parallel sum
    pub fn sum<T>(items: Vec<T>) -> T
    where
        T: Send + std::ops::Add<Output = T> + Default + Clone,
    {
        items
            .into_par_iter()
            .fold(|| T::default(), |a, b| a + b)
            .reduce(|| T::default(), |a, b| a + b)
    }

    /// Parallel aggregation with custom operation
    pub fn aggregate<T, F>(items: Vec<T>, initial: T, op: F) -> T
    where
        T: Send + Sync + Clone,
        F: Fn(T, T) -> T + Send + Sync + Clone,
    {
        items
            .into_par_iter()
            .fold(|| initial.clone(), |a, b| op(a, b))
            .reduce(|| initial.clone(), |a, b| op(a, b))
    }

    /// Parallel partition
    pub fn partition<T, F>(items: Vec<T>, predicate: F) -> (Vec<T>, Vec<T>)
    where
        T: Send,
        F: Fn(&T) -> bool + Send + Sync,
    {
        items.into_par_iter().partition(predicate)
    }

    /// Parallel flat map
    pub fn flat_map<T, R, F>(items: Vec<T>, map_fn: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> Vec<R> + Send + Sync,
    {
        items.into_par_iter().flat_map(map_fn).collect()
    }
}

/// Stream processor for memory-efficient large dataset processing
pub struct StreamProcessor<T> {
    items: Vec<T>,
    chunk_size: usize,
}

impl<T> StreamProcessor<T>
where
    T: Send + Clone,
{
    pub fn new(items: Vec<T>, chunk_size: usize) -> Self {
        Self { items, chunk_size }
    }

    /// Process stream in chunks with given function
    pub fn process_stream<R, F>(self, processor: F) -> Vec<R>
    where
        R: Send,
        F: Fn(Vec<T>) -> Vec<R> + Send + Sync,
    {
        let chunks: Vec<Vec<T>> = self
            .items
            .chunks(self.chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        chunks
            .into_par_iter()
            .flat_map(|chunk| processor(chunk))
            .collect()
    }

    /// Process with side effects
    pub fn for_each<F>(self, action: F)
    where
        T: Sync,
        F: Fn(&T) + Send + Sync,
    {
        self.items.par_iter().for_each(action);
    }
}

/// Concurrent map for key-value processing
pub struct ConcurrentMapOps;

impl ConcurrentMapOps {
    /// Process map entries in parallel
    pub fn transform_values<K, V, R, F>(
        map: std::collections::HashMap<K, V>,
        transform: F,
    ) -> std::collections::HashMap<K, R>
    where
        K: Send + Eq + std::hash::Hash,
        V: Send,
        R: Send,
        F: Fn(V) -> R + Send + Sync,
    {
        map.into_par_iter()
            .map(|(k, v)| (k, transform(v)))
            .collect()
    }

    /// Filter map entries in parallel
    pub fn filter<K, V, F>(
        map: std::collections::HashMap<K, V>,
        predicate: F,
    ) -> std::collections::HashMap<K, V>
    where
        K: Send + Eq + std::hash::Hash,
        V: Send,
        F: Fn(&K, &V) -> bool + Send + Sync,
    {
        map.into_par_iter()
            .filter(|(k, v)| predicate(k, v))
            .collect()
    }
}

/// Parallel sorting utilities
pub struct ParallelSort;

impl ParallelSort {
    /// Parallel sort (unstable, faster)
    pub fn sort_unstable<T>(mut items: Vec<T>) -> Vec<T>
    where
        T: Send + Ord,
    {
        items.par_sort_unstable();
        items
    }

    /// Parallel sort with custom comparator
    pub fn sort_by<T, F>(mut items: Vec<T>, compare: F) -> Vec<T>
    where
        T: Send,
        F: Fn(&T, &T) -> std::cmp::Ordering + Send + Sync,
    {
        items.par_sort_by(compare);
        items
    }

    /// Parallel sort with key extraction
    pub fn sort_by_key<T, K, F>(mut items: Vec<T>, key_fn: F) -> Vec<T>
    where
        T: Send,
        K: Ord,
        F: Fn(&T) -> K + Send + Sync,
    {
        items.par_sort_by_key(key_fn);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_processor() {
        let processor = BatchProcessor::new(100);

        let items: Vec<i32> = (0..1000).collect();
        let results = processor.process(items, |x| x * 2);

        assert_eq!(results.len(), 1000);
        assert_eq!(results[0], 0);
        assert_eq!(results[999], 1998);
    }

    #[test]
    fn test_process_chunks() {
        let processor = BatchProcessor::new(10);

        let items: Vec<i32> = (0..100).collect();
        let results =
            processor.process_chunks(items, |chunk| chunk.iter().map(|x| x * 2).collect());

        assert_eq!(results.len(), 100);
    }

    #[test]
    fn test_map_reduce() {
        let processor = BatchProcessor::new(10);

        let items: Vec<i32> = (1..=100).collect();
        let sum = processor.map_reduce(items, |x| x, |a, b| a + b, 0);

        assert_eq!(sum, 5050); // Sum of 1 to 100
    }

    #[test]
    fn test_filter_process() {
        let processor = BatchProcessor::new(10);

        let items: Vec<i32> = (0..100).collect();
        let results = processor.filter_process(items, |x| x % 2 == 0, |x| x * 2);

        assert_eq!(results.len(), 50);
        assert_eq!(results[0], 0);
        assert_eq!(results[1], 4);
    }

    #[test]
    fn test_vector_batch_ops_transform() {
        let items: Vec<i32> = (0..100).collect();
        let results = VectorBatchOps::transform(items, |x| x * 2);

        assert_eq!(results.len(), 100);
        assert_eq!(results[50], 100);
    }

    #[test]
    fn test_vector_batch_ops_filter() {
        let items: Vec<i32> = (0..100).collect();
        let results = VectorBatchOps::filter(items, |x| x % 2 == 0);

        assert_eq!(results.len(), 50);
    }

    #[test]
    fn test_vector_batch_ops_sum() {
        let items: Vec<i32> = vec![1, 2, 3, 4, 5];
        let sum = VectorBatchOps::sum(items);

        assert_eq!(sum, 15);
    }

    #[test]
    fn test_vector_batch_ops_partition() {
        let items: Vec<i32> = (0..100).collect();
        let (even, odd) = VectorBatchOps::partition(items, |x| x % 2 == 0);

        assert_eq!(even.len(), 50);
        assert_eq!(odd.len(), 50);
    }

    #[test]
    fn test_vector_batch_ops_flat_map() {
        let items: Vec<i32> = vec![1, 2, 3];
        let results = VectorBatchOps::flat_map(items, |x| vec![x, x * 2]);

        assert_eq!(results, vec![1, 2, 2, 4, 3, 6]);
    }

    #[test]
    fn test_stream_processor() {
        let items: Vec<i32> = (0..100).collect();
        let processor = StreamProcessor::new(items, 10);

        let results = processor.process_stream(|chunk| chunk.iter().map(|x| x * 2).collect());

        assert_eq!(results.len(), 100);
    }

    #[test]
    fn test_parallel_sort() {
        let items: Vec<i32> = vec![5, 2, 8, 1, 9, 3];
        let sorted = ParallelSort::sort_unstable(items);

        assert_eq!(sorted, vec![1, 2, 3, 5, 8, 9]);
    }

    #[test]
    fn test_parallel_sort_by_key() {
        let items: Vec<(i32, &str)> = vec![(3, "three"), (1, "one"), (2, "two")];

        let sorted = ParallelSort::sort_by_key(items, |x| x.0);

        assert_eq!(sorted[0].0, 1);
        assert_eq!(sorted[1].0, 2);
        assert_eq!(sorted[2].0, 3);
    }

    #[test]
    fn test_concurrent_map_ops_transform() {
        let mut map = std::collections::HashMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);

        let transformed = ConcurrentMapOps::transform_values(map, |v| v * 2);

        assert_eq!(transformed.get("a"), Some(&2));
        assert_eq!(transformed.get("b"), Some(&4));
        assert_eq!(transformed.get("c"), Some(&6));
    }

    #[test]
    fn test_concurrent_map_ops_filter() {
        let mut map = std::collections::HashMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);
        map.insert("d", 4);

        let filtered = ConcurrentMapOps::filter(map, |_k, v| v % 2 == 0);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.get("b"), Some(&2));
        assert_eq!(filtered.get("d"), Some(&4));
    }
}

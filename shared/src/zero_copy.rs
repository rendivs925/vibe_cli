/// Zero-copy utilities and string optimization
///
/// Provides tools for eliminating unnecessary allocations:
/// - Cow (Clone-on-Write) helpers
/// - String interning for repeated strings
/// - Borrow vs owned optimization utilities
/// - Memory-efficient string operations
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// String interner for deduplicating repeated strings
///
/// Useful for strings that appear multiple times (file paths, identifiers)
pub struct StringInterner {
    strings: Arc<Mutex<HashMap<String, Arc<str>>>>,
}

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            strings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Intern a string, returning an Arc to the canonical version
    pub fn intern(&self, s: impl AsRef<str>) -> Arc<str> {
        let s = s.as_ref();
        let mut map = self.strings.lock().unwrap();

        if let Some(interned) = map.get(s) {
            Arc::clone(interned)
        } else {
            let arc: Arc<str> = Arc::from(s);
            map.insert(s.to_string(), Arc::clone(&arc));
            arc
        }
    }

    /// Get number of interned strings
    pub fn len(&self) -> usize {
        self.strings.lock().unwrap().len()
    }

    /// Check if interner is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all interned strings
    pub fn clear(&self) {
        self.strings.lock().unwrap().clear();
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to efficiently concatenate strings with known capacity
pub fn concat_strings(parts: &[&str]) -> String {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    let mut result = String::with_capacity(total_len);

    for part in parts {
        result.push_str(part);
    }

    result
}

/// Join strings with a separator, using pre-allocated capacity
pub fn join_with_separator<'a, I>(parts: I, separator: &str) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let parts_vec: Vec<&str> = parts.into_iter().collect();
    let len = parts_vec.len();

    if len == 0 {
        return String::new();
    }

    let total_len: usize = parts_vec.iter().map(|s| s.len()).sum();
    let sep_len = separator.len() * (len.saturating_sub(1));
    let mut result = String::with_capacity(total_len + sep_len);

    for (i, part) in parts_vec.iter().enumerate() {
        if i > 0 {
            result.push_str(separator);
        }
        result.push_str(part);
    }

    result
}

/// Convert to Cow, avoiding allocation if already borrowed
pub fn to_cow<'a>(s: &'a str) -> Cow<'a, str> {
    Cow::Borrowed(s)
}

/// Convert owned String to Cow
pub fn string_to_cow(s: String) -> Cow<'static, str> {
    Cow::Owned(s)
}

/// Efficient string builder with pre-allocated capacity
pub struct StringBuilder {
    buffer: String,
}

impl StringBuilder {
    /// Create with estimated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
        }
    }

    /// Append a string slice
    pub fn push(&mut self, s: &str) -> &mut Self {
        self.buffer.push_str(s);
        self
    }

    /// Append a character
    pub fn push_char(&mut self, c: char) -> &mut Self {
        self.buffer.push(c);
        self
    }

    /// Append with separator if buffer is not empty
    pub fn push_with_separator(&mut self, s: &str, separator: &str) -> &mut Self {
        if !self.buffer.is_empty() {
            self.buffer.push_str(separator);
        }
        self.buffer.push_str(s);
        self
    }

    /// Get current length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Build and return the string
    pub fn build(self) -> String {
        self.buffer
    }

    /// Get a reference to the buffer
    pub fn as_str(&self) -> &str {
        &self.buffer
    }
}

/// Macro for efficient string concatenation at compile time
#[macro_export]
macro_rules! concat_str {
    ($($s:expr),* $(,)?) => {{
        let parts = &[$($s),*];
        $crate::zero_copy::concat_strings(parts)
    }};
}

/// Zero-copy buffer for reading without allocation
pub struct ZeroCopyBuffer<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ZeroCopyBuffer<'a> {
    /// Create a new buffer from byte slice
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Read a slice without copying
    pub fn read_slice(&mut self, len: usize) -> Option<&'a [u8]> {
        if self.position + len > self.data.len() {
            return None;
        }

        let slice = &self.data[self.position..self.position + len];
        self.position += len;
        Some(slice)
    }

    /// Read until a delimiter without copying
    pub fn read_until(&mut self, delimiter: u8) -> Option<&'a [u8]> {
        let start = self.position;
        while self.position < self.data.len() {
            if self.data[self.position] == delimiter {
                let slice = &self.data[start..self.position];
                self.position += 1; // Skip delimiter
                return Some(slice);
            }
            self.position += 1;
        }
        None
    }

    /// Get remaining bytes without copying
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.position..]
    }

    /// Check if all data has been read
    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    /// Reset position to beginning
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

/// Trait for types that can be efficiently converted to string slices
pub trait AsStrRef {
    fn as_str_ref(&self) -> &str;
}

impl AsStrRef for String {
    fn as_str_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsStrRef for &str {
    fn as_str_ref(&self) -> &str {
        self
    }
}

impl AsStrRef for Arc<str> {
    fn as_str_ref(&self) -> &str {
        self.as_ref()
    }
}

impl<'a> AsStrRef for Cow<'a, str> {
    fn as_str_ref(&self) -> &str {
        self.as_ref()
    }
}

/// Helper to avoid unnecessary String clones
pub fn clone_if_owned<'a>(s: Cow<'a, str>) -> String {
    match s {
        Cow::Borrowed(b) => b.to_string(),
        Cow::Owned(o) => o,
    }
}

/// Memory-efficient key-value store using string interning
pub struct InternedMap {
    interner: StringInterner,
    map: HashMap<Arc<str>, Arc<str>>,
}

impl InternedMap {
    pub fn new() -> Self {
        Self {
            interner: StringInterner::new(),
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let key = self.interner.intern(key);
        let value = self.interner.intern(value);
        self.map.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&Arc<str>> {
        let key_arc = self.interner.intern(key);
        self.map.get(&key_arc)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Default for InternedMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interner() {
        let interner = StringInterner::new();

        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        let s3 = interner.intern("world");

        // Same string should point to same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_concat_strings() {
        let parts = &["Hello", " ", "World", "!"];
        let result = concat_strings(parts);
        assert_eq!(result, "Hello World!");
        assert_eq!(result.capacity(), result.len()); // No extra capacity
    }

    #[test]
    fn test_join_with_separator() {
        let parts = vec!["one", "two", "three"];
        let result = join_with_separator(parts.iter().copied(), ", ");
        assert_eq!(result, "one, two, three");
    }

    #[test]
    fn test_string_builder() {
        let mut builder = StringBuilder::with_capacity(20);
        builder.push("Hello").push_char(' ').push("World");

        assert_eq!(builder.as_str(), "Hello World");
        assert_eq!(builder.build(), "Hello World");
    }

    #[test]
    fn test_string_builder_with_separator() {
        let mut builder = StringBuilder::with_capacity(30);
        builder
            .push_with_separator("one", ", ")
            .push_with_separator("two", ", ")
            .push_with_separator("three", ", ");

        assert_eq!(builder.build(), "one, two, three");
    }

    #[test]
    fn test_zero_copy_buffer() {
        let data = b"Hello World!";
        let mut buffer = ZeroCopyBuffer::new(data);

        let slice = buffer.read_slice(5).unwrap();
        assert_eq!(slice, b"Hello");

        let remaining = buffer.remaining();
        assert_eq!(remaining, b" World!");
    }

    #[test]
    fn test_zero_copy_buffer_read_until() {
        let data = b"line1\nline2\nline3";
        let mut buffer = ZeroCopyBuffer::new(data);

        let line1 = buffer.read_until(b'\n').unwrap();
        assert_eq!(line1, b"line1");

        let line2 = buffer.read_until(b'\n').unwrap();
        assert_eq!(line2, b"line2");
    }

    #[test]
    fn test_interned_map() {
        let mut map = InternedMap::new();

        map.insert("key1", "value1");
        map.insert("key2", "value2");
        map.insert("key1", "value1_updated");

        assert_eq!(map.len(), 2);
        assert_eq!(map.get("key1").unwrap().as_ref(), "value1_updated");
    }

    #[test]
    fn test_as_str_ref() {
        fn process<T: AsStrRef>(s: T) -> usize {
            s.as_str_ref().len()
        }

        assert_eq!(process("test"), 4);
        assert_eq!(process(String::from("hello")), 5);
        assert_eq!(process(Arc::from("world")), 5);
    }
}

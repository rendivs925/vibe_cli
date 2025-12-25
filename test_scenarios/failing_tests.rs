#[cfg(test)]
mod tests {
    #[test]
    fn test_assertion_failure() {
        let expected = 5;
        let actual = 3;
        assert_eq!(expected, actual, "Values should be equal");
    }

    #[test]
    fn test_panic_condition() {
        let data: Option<i32> = None;
        // This will panic when unwrapping None
        let value = data.unwrap();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_index_out_of_bounds() {
        let arr = [1, 2, 3];
        // This will panic at runtime
        let value = arr[10];
        assert_eq!(value, 0);
    }

    #[test]
    fn test_type_error() {
        let x: i32 = "not a number";
        assert_eq!(x, 42);
    }
}
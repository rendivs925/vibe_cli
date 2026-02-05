use crate::cache::CacheManager;
use shared::types::Result;
use std::path::PathBuf;

#[test]
fn test_validate_command_syntax() {
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command_syntax("ls -la"));
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command_syntax("echo hello"));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true).validate_command_syntax("rm -rf /"));
    assert!(
        !CacheManager::new(PathBuf::from("/tmp"), true).validate_command_syntax("dd if=/dev/zero")
    );
    assert!(
        !CacheManager::new(PathBuf::from("/tmp"), true).validate_command_syntax("echo; rm -rf /")
    );
}

#[test]
fn test_validate_command_exists() {
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command_exists("ls"));
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command_exists("echo"));
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command_exists("cat"));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true)
        .validate_command_exists("nonexistent_command_xyz123"));
}

#[test]
fn test_validate_command() {
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command("ls -la"));
    assert!(CacheManager::new(PathBuf::from("/tmp"), true).validate_command("echo hello world"));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true).validate_command("rm -rf /"));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true)
        .validate_command("nonexistent_command_xyz123"));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true).validate_command(""));
    assert!(!CacheManager::new(PathBuf::from("/tmp"), true).validate_command("   "));
}

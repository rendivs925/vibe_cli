use super::CliHandlers;
use shared::types::Result;

impl CliHandlers {
    pub fn handle_clear_cache(&self) -> Result<()> {
        let cache_paths = vec![
            self.cache_manager.cache_path("commands"),
            self.cache_manager.cache_path("explain"),
            self.cache_manager.cache_path("rag"),
        ];

        let mut cleared = 0;
        let mut failed = 0;

        for path in cache_paths {
            if path.exists() {
                match std::fs::remove_file(&path) {
                    Ok(_) => {
                        println!("Cleared: {}", path.display());
                        cleared += 1;
                    }
                    Err(e) => {
                        println!("Failed to clear {}: {:?}", path.display(), e);
                        failed += 1;
                    }
                }
            }
        }

        if cleared == 0 && failed == 0 {
            println!("No cache files found.");
        } else {
            println!("\nCleared {} cache file(s), {} failed", cleared, failed);
        }

        Ok(())
    }
}

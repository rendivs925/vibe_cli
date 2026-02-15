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

    pub fn handle_clear_rag_cache(&self) -> Result<()> {
        let cache_path = self.cache_manager.cache_path("rag");
        if !cache_path.exists() {
            println!("No RAG cache file found.");
            return Ok(());
        }

        match std::fs::remove_file(&cache_path) {
            Ok(_) => println!("Cleared: {}", cache_path.display()),
            Err(e) => println!("Failed to clear {}: {:?}", cache_path.display(), e),
        }

        Ok(())
    }

    pub fn handle_clear_embeddings(&self) -> Result<()> {
        let db_path = std::path::Path::new(&self.config.db_path);
        if !db_path.exists() {
            println!("No embeddings database found.");
            return Ok(());
        }

        match std::fs::remove_file(db_path) {
            Ok(_) => println!("Cleared: {}", db_path.display()),
            Err(e) => println!("Failed to clear {}: {:?}", db_path.display(), e),
        }

        Ok(())
    }
}

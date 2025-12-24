use anyhow::{Result, Context};
use glob::glob;
use std::path::PathBuf;

/// 데이터 파일 로딩을 담당하는 유틸리티 (Single Responsibility Principle)
pub struct DataLoader;

impl DataLoader {
    /// Glob 패턴으로 파일 목록 가져오기
    pub fn load_files(pattern: &str) -> Result<Vec<PathBuf>> {
        let mut data_files = Vec::new();
        
        for entry in glob(pattern)
            .context("Failed to read glob pattern")? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        data_files.push(path);
                    }
                },
                Err(e) => eprintln!("Warning: Error reading path: {}", e),
            }
        }
        
        if data_files.is_empty() {
            anyhow::bail!("No files found matching pattern: {}", pattern);
        }
        
        // Sort files for consistent ordering
        data_files.sort();
        
        println!("Found {} file(s) matching pattern '{}'", data_files.len(), pattern);
        for (i, file) in data_files.iter().enumerate() {
            println!("  [{}] {}", i + 1, file.display());
        }
        println!();
        
        Ok(data_files)
    }
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::SystemTime,
};

use anyhow::{Context, Result};
use time::Date;
use time_tracking_parser::TimeTrackingData;
use tokio::{fs, sync::Mutex};

use crate::{Config, DATE_FORMAT, file_utils::create_template_content, get_time_tracking_dir};

static LOADED: OnceLock<bool> = OnceLock::new();
static DATA_SVC: OnceLock<DataService> = OnceLock::new();

/// Cache entry for a date's data
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The parsed time tracking data
    data: Option<String>,
    /// File modification time when cached
    file_mod_time: Option<SystemTime>,
    /// When this entry was cached
    cached_at: SystemTime,
}

/// Centralized data service for time tracking files
#[derive(Debug, Clone)]
pub struct DataService {
    /// Cache of file contents by date
    cache: Arc<Mutex<HashMap<Date, CacheEntry>>>,
    /// Cache timeout in seconds (default: 30)
    cache_timeout: u64,
}

impl DataService {
    pub fn get() -> &'static Self {
        LOADED.get_or_init(|| {
            DATA_SVC.get_or_init(|| Self::new(30));
            true
        });

        DATA_SVC.get().unwrap()
    }
    /// Create a new data service with specified cache timeout
    fn new(cache_timeout_seconds: u64) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_timeout: cache_timeout_seconds,
        }
    }

    /// Clear the cache entry for a specific date
    /// This should be called when we know a file has been edited
    pub async fn invalidate_date(&self, date: &Date) {
        let mut cache = self.cache.lock().await;
        cache.remove(date);
    }

    /// Clear all cache entries
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.lock().await;
        cache.clear();
    }

    /// Get the file path for a given date and config
    pub async fn get_file_path(&self, date: Date) -> Result<PathBuf> {
        let time_tracking_dir = get_time_tracking_dir()?;

        // Create directory if it doesn't exist
        if !time_tracking_dir.exists() {
            fs::create_dir_all(&time_tracking_dir).await?;
        }

        let date_str = date.format(&DATE_FORMAT).context("could not format date")?;
        let filename = format!("{}.md", date_str);

        Ok(time_tracking_dir.join(filename))
    }

    /// Read a day's content from file, using cache when possible
    pub async fn read_day(&self, date: &Date) -> Result<Option<String>> {
        let file_path = self.get_file_path(*date).await?;

        if !file_path.exists() {
            return Ok(None);
        }

        // Check cache first
        if let Some(content) = self.get_cached_content(date, &file_path).await? {
            return Ok(Some(content));
        }

        // Read from file and cache
        let content = fs::read_to_string(&file_path).await?;
        self.cache_content(*date, &file_path, &content).await?;

        Ok(Some(content))
    }

    /// Parse a day's time tracking data
    pub async fn parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>> {
        let config = Config::get();
        if let Some(content) = self.read_day(date).await? {
            let data = time_tracking_parser::parse_time_tracking_data(
                &content,
                config.get_prefix(),
                config.get_suffix(),
            );
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// Create a new day file with template content if it doesn't exist
    pub async fn create_day_file_if_not_exists(&self, date: &Date) -> Result<PathBuf> {
        let config = Config::get();
        let file_path = self.get_file_path(*date).await?;

        if !file_path.exists() {
            let template_content = create_template_content(date, config.get_template_file())?;
            fs::write(&file_path, template_content).await?;

            // Invalidate cache since we just created the file
            self.invalidate_date(date).await;
        }

        Ok(file_path)
    }

    /// Check if a date has data (contains projects with time > 0)
    pub async fn check_date_has_data(&self, date: &Date) -> Result<bool> {
        if let Some(data) = self.parse_day(date).await? {
            Ok(!data.projects.is_empty() && data.total_minutes > 0)
        } else {
            Ok(false)
        }
    }

    /// Find all populated dates within a date range
    pub async fn find_populated_dates(
        &self,
        start_date: Date,
        end_date: Date,
    ) -> Result<Vec<Date>> {
        let mut populated_dates = Vec::new();
        let mut current_date = start_date;

        while current_date <= end_date {
            if self.check_date_has_data(&current_date).await? {
                populated_dates.push(current_date);
            }

            // Move to next day
            current_date = current_date.next_day().unwrap_or(current_date);
            if current_date == start_date {
                // Prevent infinite loop in case of date arithmetic issues
                break;
            }
        }

        Ok(populated_dates)
    }

    /// Get weekly data for a range of dates
    pub async fn get_weekly_data(&self, dates: &[Date]) -> Result<HashMap<Date, u32>> {
        let mut weekly_data = HashMap::new();

        for &date in dates {
            let total_minutes = if let Some(data) = self.parse_day(&date).await? {
                data.total_minutes
            } else {
                0
            };
            weekly_data.insert(date, total_minutes);
        }

        Ok(weekly_data)
    }

    /// Get cached content if valid, None otherwise
    async fn get_cached_content(&self, date: &Date, file_path: &Path) -> Result<Option<String>> {
        let cache = self.cache.lock().await;

        if let Some(entry) = cache.get(date) {
            let now = SystemTime::now();

            // Check if cache entry is still valid (within timeout)
            if let Ok(duration) = now.duration_since(entry.cached_at)
                && duration.as_secs() < self.cache_timeout
                && let Ok(metadata) = tokio::fs::metadata(file_path).await
                && let Ok(file_mod_time) = metadata.modified()
                && let Some(cached_mod_time) = entry.file_mod_time
                && file_mod_time <= cached_mod_time
            {
                // File hasn't been modified, use cached content
                return Ok(entry.data.clone());
            }
        }

        Ok(None)
    }

    /// Cache content for a date
    async fn cache_content(&self, date: Date, file_path: &Path, content: &str) -> Result<()> {
        let mut cache = self.cache.lock().await;

        let file_mod_time = if let Ok(metadata) = tokio::fs::metadata(file_path).await {
            metadata.modified().ok()
        } else {
            None
        };

        let entry = CacheEntry {
            data: Some(content.to_string()),
            file_mod_time,
            cached_at: SystemTime::now(),
        };

        cache.insert(date, entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[tokio::test]
    async fn test_data_service_creation() {
        let service = DataService::new(60);
        assert_eq!(service.cache_timeout, 60);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let service = DataService::new(60);
        let test_date = date!(2023 - 10 - 15);

        // Add something to cache manually for testing
        {
            let mut cache = service.cache.lock().await;
            cache.insert(
                test_date,
                CacheEntry {
                    data: Some("test content".to_string()),
                    file_mod_time: None,
                    cached_at: SystemTime::now(),
                },
            );
        }

        // Verify it's in cache
        {
            let cache = service.cache.lock().await;
            assert!(cache.contains_key(&test_date));
        }

        // Invalidate
        service.invalidate_date(&test_date).await;

        // Verify it's removed
        {
            let cache = service.cache.lock().await;
            assert!(!cache.contains_key(&test_date));
        }
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let service = DataService::new(60);
        let test_date1 = date!(2023 - 10 - 15);
        let test_date2 = date!(2023 - 10 - 16);

        // Add entries to cache
        {
            let mut cache = service.cache.lock().await;
            cache.insert(
                test_date1,
                CacheEntry {
                    data: Some("test1".to_string()),
                    file_mod_time: None,
                    cached_at: SystemTime::now(),
                },
            );
            cache.insert(
                test_date2,
                CacheEntry {
                    data: Some("test2".to_string()),
                    file_mod_time: None,
                    cached_at: SystemTime::now(),
                },
            );
        }

        // Clear cache
        service.clear_cache().await;

        // Verify empty
        {
            let cache = service.cache.lock().await;
            assert!(cache.is_empty());
        }
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let service = DataService::new(60);

        let test_date = date!(2001 - 10 - 15);
        let result = service.read_day(&test_date).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_create_and_read_file() {
        let service = DataService::new(60);

        let test_date = date!(2023 - 10 - 15);

        // Create file
        let file_path = service
            .create_day_file_if_not_exists(&test_date)
            .await
            .unwrap();
        assert!(file_path.exists());

        // Read file
        let content = service.read_day(&test_date).await.unwrap();
        assert!(content.is_some());
    }
}


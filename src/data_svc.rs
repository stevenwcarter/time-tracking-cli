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

/// Where a [`DataService`] looks for day files.
#[derive(Debug, Clone)]
enum DataDir {
    /// Resolve from the global [`Config`] on each use. The CLI and web paths
    /// run after `Config::get()` has parsed the real argv, so this is what they
    /// have always done.
    FromConfig,
    /// A directory supplied by the caller, so nothing touches the config
    /// singleton. Used by the TUI and by tests.
    Fixed(PathBuf),
}

impl DataDir {
    fn resolve(&self) -> Result<PathBuf> {
        match self {
            Self::FromConfig => get_time_tracking_dir(),
            Self::Fixed(dir) => Ok(dir.clone()),
        }
    }
}

/// Centralized data service for time tracking files
#[derive(Debug, Clone)]
pub struct DataService {
    /// Cache of file contents by date
    cache: Arc<Mutex<HashMap<Date, CacheEntry>>>,
    /// Cache timeout in seconds (default: 30)
    cache_timeout: u64,
    /// Directory the day files are read from
    data_dir: DataDir,
}

impl DataService {
    /// Cache lifetime of the process-wide service.
    pub const DEFAULT_CACHE_TIMEOUT_SECONDS: u64 = 30;

    pub fn get() -> &'static Self {
        DATA_SVC.get_or_init(|| Self::new(Self::DEFAULT_CACHE_TIMEOUT_SECONDS))
    }

    /// Create a new data service that resolves its directory from the global
    /// configuration.
    fn new(cache_timeout_seconds: u64) -> Self {
        Self::with_data_dir(cache_timeout_seconds, DataDir::FromConfig)
    }

    /// Create a new data service that reads from `data_dir`, so callers that
    /// already know their directory never reach for the config singleton.
    pub fn new_with_dir(cache_timeout_seconds: u64, data_dir: PathBuf) -> Self {
        Self::with_data_dir(cache_timeout_seconds, DataDir::Fixed(data_dir))
    }

    fn with_data_dir(cache_timeout_seconds: u64, data_dir: DataDir) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_timeout: cache_timeout_seconds,
            data_dir,
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
        let time_tracking_dir = self.data_dir.resolve()?;

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

        // Stat and read in one step so cache_content doesn't need to re-stat
        let metadata = tokio::fs::metadata(&file_path).await.ok();
        let file_mod_time = metadata.and_then(|m| m.modified().ok());
        let content = fs::read_to_string(&file_path).await?;
        self.cache_content(*date, file_mod_time, &content).await;

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
            let template_content =
                create_template_content(date, config.get_template_file()).await?;
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
        // Collect all dates in range first
        let mut dates = Vec::new();
        let mut current_date = start_date;
        while current_date <= end_date {
            dates.push(current_date);
            match current_date.next_day() {
                Some(next) => current_date = next,
                None => break,
            }
        }

        // Check all dates in parallel
        let mut set = tokio::task::JoinSet::new();
        for date in dates {
            let svc = self.clone();
            set.spawn(async move {
                let has_data = svc.check_date_has_data(&date).await?;
                Ok::<(Date, bool), anyhow::Error>((date, has_data))
            });
        }

        let mut populated_dates = Vec::new();
        while let Some(result) = set.join_next().await {
            let (date, has_data) = result??;
            if has_data {
                populated_dates.push(date);
            }
        }
        populated_dates.sort();
        Ok(populated_dates)
    }

    /// Get weekly data for a range of dates
    pub async fn get_weekly_data(&self, dates: &[Date]) -> Result<HashMap<Date, u32>> {
        let mut set = tokio::task::JoinSet::new();
        for &date in dates {
            let svc = self.clone();
            set.spawn(async move {
                let total_minutes = if let Some(data) = svc.parse_day(&date).await? {
                    data.total_minutes
                } else {
                    0
                };
                Ok::<(Date, u32), anyhow::Error>((date, total_minutes))
            });
        }

        let mut weekly_data = HashMap::new();
        while let Some(result) = set.join_next().await {
            let (date, total_minutes) = result??;
            weekly_data.insert(date, total_minutes);
        }
        Ok(weekly_data)
    }

    /// Get cached content if valid, None otherwise
    async fn get_cached_content(&self, date: &Date, file_path: &Path) -> Result<Option<String>> {
        // Clone the entry so we can release the lock before doing I/O
        let cached_entry = {
            let cache = self.cache.lock().await;
            cache.get(date).cloned()
        };

        if let Some(entry) = cached_entry {
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
    async fn cache_content(&self, date: Date, file_mod_time: Option<SystemTime>, content: &str) {
        let mut cache = self.cache.lock().await;

        let entry = CacheEntry {
            data: Some(content.to_string()),
            file_mod_time,
            cached_at: SystemTime::now(),
        };

        cache.insert(date, entry);
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
    async fn test_new_with_dir_reads_from_the_injected_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let service = DataService::new_with_dir(60, dir.path().to_path_buf());
        let test_date = date!(2023 - 10 - 15);

        let file_path = service
            .create_day_file_if_not_exists(&test_date)
            .await
            .unwrap();

        assert_eq!(file_path, dir.path().join("2023-10-15.md"));
        assert!(file_path.exists());
        assert!(service.read_day(&test_date).await.unwrap().is_some());
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

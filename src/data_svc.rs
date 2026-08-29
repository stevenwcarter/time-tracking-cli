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

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    Config, DATE_FORMAT, file_utils::create_template_content, format_day_with_date,
    get_time_tracking_dir,
};

static DATA_SVC: OnceLock<DataService> = OnceLock::new();

/// Cache entry for a date's data
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The raw file content
    data: Option<String>,
    /// The parsed time tracking data, memoized alongside `data` so a full
    /// cache hit never re-runs the markdown parse.
    parsed: Option<TimeTrackingData>,
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

/// How day files are parsed and created: the markers that bound the time
/// entries within a file, and the template a new day file starts from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseSettings {
    /// Line before which parsing does not start, e.g. ```` ```timetracking ````.
    pub prefix: Option<String>,
    /// Line at which parsing stops, e.g. ```` ``` ````.
    pub suffix: Option<String>,
    /// Template file a newly created day file is seeded from.
    pub template_file: Option<String>,
}

impl ParseSettings {
    /// Read the markers and the template out of a loaded configuration.
    pub fn from_config(config: &Config) -> Self {
        Self {
            prefix: config.get_prefix().map(str::to_owned),
            suffix: config.get_suffix().map(str::to_owned),
            template_file: config.get_template_file().map(str::to_owned),
        }
    }
}

/// How a [`DataService`] parses the day files it reads.
#[derive(Debug, Clone)]
enum ParseOpts {
    /// Read from the global [`Config`] on each use. The CLI and web paths run
    /// after `Config::get()` has parsed the real argv, so this is what they
    /// have always done.
    FromConfig,
    /// Settings supplied by the caller, so nothing touches the config
    /// singleton. Used by the TUI and by tests.
    Fixed(ParseSettings),
}

impl ParseOpts {
    fn prefix(&self) -> Option<&str> {
        match self {
            Self::FromConfig => Config::get().get_prefix(),
            Self::Fixed(settings) => settings.prefix.as_deref(),
        }
    }

    fn suffix(&self) -> Option<&str> {
        match self {
            Self::FromConfig => Config::get().get_suffix(),
            Self::Fixed(settings) => settings.suffix.as_deref(),
        }
    }

    fn template_file(&self) -> Option<&str> {
        match self {
            Self::FromConfig => Config::get().get_template_file(),
            Self::Fixed(settings) => settings.template_file.as_deref(),
        }
    }
}

/// One project's rollup across a week.
///
/// `notes` are already prefixed with the day they came from, in day order,
/// because [`DataService::get_weekly_summary`] visits the week in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyProject {
    /// Project code exactly as it appears in the day files.
    pub name: String,
    /// Minutes booked against this project across the whole week.
    pub total_minutes: u32,
    /// Notes for this project, each rendered as `"<Weekday YYYY-MM-DD>: <note>"`.
    pub notes: Vec<String>,
}

/// Everything a week of day files adds up to.
///
/// This is the one weekly aggregation in the codebase: the stdout printer and
/// the TUI both render it rather than each recomputing the rollup.
#[derive(Debug, Clone, Default)]
pub struct WeeklySummary {
    /// Working minutes across the week.
    pub total_minutes: u32,
    /// Dead (gap) minutes across the week.
    pub dead_time_minutes: u32,
    /// Per-project rollup, ordered by minutes descending then name ascending.
    pub projects: Vec<WeeklyProject>,
    /// Parser warnings, each prefixed with the day that produced it.
    pub warnings: Vec<String>,
    /// Working minutes per day, zero for a day with no file.
    pub per_day: HashMap<Date, u32>,
    /// Every requested date, in the order given, with its raw content and its
    /// parse. A day with no file on disk contributes `(date, String::new(), None)`.
    pub days: Vec<(Date, String, Option<TimeTrackingData>)>,
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
    /// How those files are parsed and created
    parse_opts: ParseOpts,
    /// Real (non-cached) parse count. Test-only seam: a second `parse_day`
    /// call returns the same data whether or not it reparsed, so equal
    /// results alone can't prove memoization — this counter, incremented
    /// only where `parse_time_tracking_data` actually runs, can.
    #[cfg(test)]
    parse_count: Arc<AtomicUsize>,
}

impl DataService {
    /// Cache lifetime of the process-wide service.
    pub const DEFAULT_CACHE_TIMEOUT_SECONDS: u64 = 30;

    pub fn get() -> &'static Self {
        DATA_SVC.get_or_init(|| Self::new(Self::DEFAULT_CACHE_TIMEOUT_SECONDS))
    }

    /// Create a new data service that resolves both its directory and its
    /// parse settings from the global configuration.
    fn new(cache_timeout_seconds: u64) -> Self {
        Self::with_sources(
            cache_timeout_seconds,
            DataDir::FromConfig,
            ParseOpts::FromConfig,
        )
    }

    /// Create a new data service that reads `data_dir` with `parse_settings`.
    ///
    /// Nothing about such a service reads — or, on a fresh machine, writes —
    /// the global configuration, so callers that already know their inputs
    /// (the TUI) and tests that must stay hermetic use this.
    pub fn new_with_dir(
        cache_timeout_seconds: u64,
        data_dir: PathBuf,
        parse_settings: ParseSettings,
    ) -> Self {
        Self::with_sources(
            cache_timeout_seconds,
            DataDir::Fixed(data_dir),
            ParseOpts::Fixed(parse_settings),
        )
    }

    fn with_sources(cache_timeout_seconds: u64, data_dir: DataDir, parse_opts: ParseOpts) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_timeout: cache_timeout_seconds,
            data_dir,
            parse_opts,
            #[cfg(test)]
            parse_count: Arc::new(AtomicUsize::new(0)),
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

    /// Parse a day's time tracking data, using the cached parse when the
    /// backing file hasn't changed. This is the hot path for the TUI: a
    /// single navigation key can call this ~97 times, and on a full cache
    /// hit none of those calls should re-run the markdown parser.
    pub async fn parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>> {
        let file_path = self.get_file_path(*date).await?;

        if !file_path.exists() {
            return Ok(None);
        }

        if let Some(parsed) = self.get_cached_parsed(date, &file_path).await? {
            return Ok(Some(parsed));
        }

        let Some(content) = self.read_day(date).await? else {
            return Ok(None);
        };

        let parsed = time_tracking_parser::parse_time_tracking_data(
            &content,
            self.parse_opts.prefix(),
            self.parse_opts.suffix(),
        );

        #[cfg(test)]
        self.parse_count.fetch_add(1, Ordering::Relaxed);

        self.cache_parsed(*date, parsed.clone()).await;

        Ok(Some(parsed))
    }

    /// Real parses performed since the service was created. Test-only:
    /// proves memoization by making the counter, not just the returned
    /// value, the thing under test.
    #[cfg(test)]
    fn parse_count(&self) -> usize {
        self.parse_count.load(Ordering::Relaxed)
    }

    /// Create a new day file with template content if it doesn't exist
    pub async fn create_day_file_if_not_exists(&self, date: &Date) -> Result<PathBuf> {
        let file_path = self.get_file_path(*date).await?;

        if !file_path.exists() {
            let template_content =
                create_template_content(date, self.parse_opts.template_file()).await?;
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

    /// Aggregate `dates` into a single [`WeeklySummary`].
    ///
    /// Days are visited in the order given, which is what keeps the per-day
    /// prefixes on warnings and project notes in day order. Both the raw
    /// content and the parse come from the per-date cache, so a week that is
    /// already cached costs no file reads and no reparses.
    pub async fn get_weekly_summary(&self, dates: &[Date]) -> Result<WeeklySummary> {
        let mut summary = WeeklySummary::default();
        let mut week_projects: HashMap<String, (u32, Vec<String>)> = HashMap::new();

        for day_date in dates {
            let (Some(content), Some(data)) = (
                self.read_day(day_date).await?,
                self.parse_day(day_date).await?,
            ) else {
                summary.per_day.insert(*day_date, 0);
                summary.days.push((*day_date, String::new(), None));
                continue;
            };

            summary.total_minutes += data.total_minutes;
            summary.dead_time_minutes += data.dead_time_minutes;
            summary.per_day.insert(*day_date, data.total_minutes);

            for warning in &data.warnings {
                if !warning.contains("Error parsing time range '#'") {
                    // Skip markdown header warnings
                    summary.warnings.push(format!(
                        "{}: {}",
                        format_day_with_date(day_date),
                        warning
                    ));
                }
            }

            for project in &data.projects {
                let entry = week_projects
                    .entry(project.name.clone())
                    .or_insert((0, Vec::new()));
                entry.0 += project.total_minutes;
                for note in &project.notes {
                    entry
                        .1
                        .push(format!("{}: {}", format_day_with_date(day_date), note));
                }
            }

            summary.days.push((*day_date, content, Some(data)));
        }

        summary.projects = week_projects
            .into_iter()
            .map(|(name, (total_minutes, notes))| WeeklyProject {
                name,
                total_minutes,
                notes,
            })
            .collect();
        // Ties on minutes fall back to the name so the ordering is stable from
        // run to run; iterating the `HashMap` alone is not.
        summary.projects.sort_by(|a, b| {
            b.total_minutes
                .cmp(&a.total_minutes)
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(summary)
    }

    /// Working minutes per day for `dates`.
    ///
    /// A projection of [`Self::get_weekly_summary`] rather than a second
    /// implementation of the same walk.
    pub async fn get_weekly_data(&self, dates: &[Date]) -> Result<HashMap<Date, u32>> {
        Ok(self.get_weekly_summary(dates).await?.per_day)
    }

    /// Return the cache entry for `date` if it is still valid for
    /// `file_path`: within the cache timeout and not modified on disk since
    /// it was cached. Both `get_cached_content` and `get_cached_parsed` are
    /// built on this so the raw content and the parsed value share exactly
    /// one validity check and always expire together.
    async fn get_valid_entry(&self, date: &Date, file_path: &Path) -> Result<Option<CacheEntry>> {
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
                // File hasn't been modified, the entry is still good
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }

    /// Get cached content if valid, None otherwise
    async fn get_cached_content(&self, date: &Date, file_path: &Path) -> Result<Option<String>> {
        Ok(self
            .get_valid_entry(date, file_path)
            .await?
            .and_then(|entry| entry.data))
    }

    /// Get the cached parse for `date` if it is still valid, None otherwise
    async fn get_cached_parsed(
        &self,
        date: &Date,
        file_path: &Path,
    ) -> Result<Option<TimeTrackingData>> {
        Ok(self
            .get_valid_entry(date, file_path)
            .await?
            .and_then(|entry| entry.parsed))
    }

    /// Cache content for a date. Freshly read content has no known parse
    /// yet, so `parsed` starts `None` and is filled in by `cache_parsed`
    /// once `parse_day` actually runs the parser.
    async fn cache_content(&self, date: Date, file_mod_time: Option<SystemTime>, content: &str) {
        let mut cache = self.cache.lock().await;

        let entry = CacheEntry {
            data: Some(content.to_string()),
            parsed: None,
            file_mod_time,
            cached_at: SystemTime::now(),
        };

        cache.insert(date, entry);
    }

    /// Store a freshly computed parse alongside whatever `cache_content`
    /// already recorded for `date` (the raw content and its mod time). If
    /// the entry was invalidated out from under us between the parse and
    /// this call, there's nothing to attach the parse to and it is simply
    /// dropped — the next `parse_day` call will parse again, which is
    /// correct, just not maximally memoized.
    async fn cache_parsed(&self, date: Date, parsed: TimeTrackingData) {
        let mut cache = self.cache.lock().await;
        if let Some(entry) = cache.get_mut(&date) {
            entry.parsed = Some(parsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use time::macros::date;

    /// A service that touches nothing outside `dir`: it neither reads nor
    /// creates the user's config file, so the test harness's own argv is never
    /// handed to clap. The returned `TempDir` must be held for the test's
    /// lifetime — dropping it deletes the directory.
    fn hermetic_service(cache_timeout_seconds: u64) -> (DataService, TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let service = DataService::new_with_dir(
            cache_timeout_seconds,
            dir.path().to_path_buf(),
            ParseSettings::default(),
        );
        (service, dir)
    }

    #[tokio::test]
    async fn test_data_service_creation() {
        let (service, _dir) = hermetic_service(60);
        assert_eq!(service.cache_timeout, 60);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2023 - 10 - 15);

        // Add something to cache manually for testing
        {
            let mut cache = service.cache.lock().await;
            cache.insert(
                test_date,
                CacheEntry {
                    data: Some("test content".to_string()),
                    parsed: None,
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
        let (service, _dir) = hermetic_service(60);
        let test_date1 = date!(2023 - 10 - 15);
        let test_date2 = date!(2023 - 10 - 16);

        // Add entries to cache
        {
            let mut cache = service.cache.lock().await;
            cache.insert(
                test_date1,
                CacheEntry {
                    data: Some("test1".to_string()),
                    parsed: None,
                    file_mod_time: None,
                    cached_at: SystemTime::now(),
                },
            );
            cache.insert(
                test_date2,
                CacheEntry {
                    data: Some("test2".to_string()),
                    parsed: None,
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
        let (service, _dir) = hermetic_service(60);

        let test_date = date!(2001 - 10 - 15);
        let result = service.read_day(&test_date).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_new_with_dir_reads_from_the_injected_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let service =
            DataService::new_with_dir(60, dir.path().to_path_buf(), ParseSettings::default());
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
    async fn test_injected_parse_settings_bound_the_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let service = DataService::new_with_dir(
            60,
            dir.path().to_path_buf(),
            ParseSettings {
                prefix: Some("```timetracking".to_string()),
                suffix: Some("```".to_string()),
                template_file: None,
            },
        );
        let test_date = date!(2023 - 10 - 15);
        tokio::fs::write(
            dir.path().join("2023-10-15.md"),
            "9:00-10:00 ignored-before-the-fence\n\
             ```timetracking\n\
             10:00-11:30 admin\n\
             ```\n\
             11:30-12:00 ignored-after-the-fence\n",
        )
        .await
        .unwrap();

        let data = service.parse_day(&test_date).await.unwrap().unwrap();

        assert_eq!(data.total_minutes, 90);
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "admin");
    }

    #[tokio::test]
    async fn test_create_and_read_file() {
        let (service, _dir) = hermetic_service(60);

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

    #[tokio::test]
    async fn parse_day_is_memoized_between_calls() {
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 24);
        let path = service.get_file_path(test_date).await.unwrap();
        tokio::fs::write(&path, "8-10 admin\n  - note\n")
            .await
            .unwrap();

        let first = service.parse_day(&test_date).await.unwrap().unwrap();
        let second = service.parse_day(&test_date).await.unwrap().unwrap();

        assert_eq!(first.total_minutes, second.total_minutes);
        assert_eq!(
            service.parse_count(),
            1,
            "second call must be served from cache"
        );
    }

    /// The week of 2026-08-22..2026-08-28, which every weekly test uses.
    fn week_of_2026_08_22() -> Vec<Date> {
        (22..=28)
            .map(|d| date!(2026 - 08 - 01).replace_day(d).expect("valid day"))
            .collect()
    }

    #[tokio::test]
    async fn weekly_projects_sort_by_minutes_desc_then_name_asc() {
        let (service, dir) = hermetic_service(60);
        // zulu and alpha tie at 120 minutes; beta is larger.
        tokio::fs::write(
            dir.path().join("2026-08-24.md"),
            "8-10 zulu\n10-12 alpha\n1-4 beta\n",
        )
        .await
        .unwrap();

        let summary = service
            .get_weekly_summary(&week_of_2026_08_22())
            .await
            .unwrap();

        let names: Vec<&str> = summary.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["beta", "alpha", "zulu"],
            "minutes desc, then name asc"
        );
    }

    #[tokio::test]
    async fn get_weekly_data_is_a_projection_of_the_summary() {
        let (service, dir) = hermetic_service(60);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        let week = week_of_2026_08_22();

        let summary = service.get_weekly_summary(&week).await.unwrap();
        let per_day = service.get_weekly_data(&week).await.unwrap();

        assert_eq!(summary.per_day, per_day);
    }

    #[tokio::test]
    async fn weekly_summary_keeps_missing_days_and_orders_notes_by_day() {
        let (service, dir) = hermetic_service(60);
        tokio::fs::write(
            dir.path().join("2026-08-24.md"),
            "8-10 admin\n  - monday note\n",
        )
        .await
        .unwrap();
        tokio::fs::write(
            dir.path().join("2026-08-26.md"),
            "8-10 admin\n  - wednesday note\n",
        )
        .await
        .unwrap();

        let summary = service
            .get_weekly_summary(&week_of_2026_08_22())
            .await
            .unwrap();

        assert_eq!(summary.total_minutes, 240);
        assert_eq!(summary.days.len(), 7, "every requested date is represented");
        assert_eq!(
            summary
                .days
                .iter()
                .filter(|(_, _, data)| data.is_none())
                .count(),
            5,
            "days with no file keep their slot"
        );
        assert_eq!(
            summary.projects[0].notes,
            vec![
                "Monday 2026-08-24: monday note".to_string(),
                "Wednesday 2026-08-26: wednesday note".to_string(),
            ],
            "notes follow the order the week is walked in"
        );
    }

    #[tokio::test]
    async fn touching_the_file_invalidates_the_parsed_cache() {
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 24);
        let path = service.get_file_path(test_date).await.unwrap();

        tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
        assert_eq!(
            service
                .parse_day(&test_date)
                .await
                .unwrap()
                .unwrap()
                .total_minutes,
            120
        );

        // Sleep past filesystem mtime granularity, then rewrite.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        tokio::fs::write(&path, "8-12 admin\n").await.unwrap();

        assert_eq!(
            service
                .parse_day(&test_date)
                .await
                .unwrap()
                .unwrap()
                .total_minutes,
            240
        );
        assert_eq!(
            service.parse_count(),
            2,
            "the rewrite must trigger a real reparse, not reuse the stale cache"
        );
    }
}

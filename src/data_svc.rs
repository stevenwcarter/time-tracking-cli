use std::{
    collections::HashMap,
    ffi::OsStr,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::SystemTime,
};

use anyhow::{Context, Result};
use time::Date;
use time_tracking_parser::TimeTrackingData;
use tokio::{fs, sync::Mutex};
use tracing::warn;

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

/// One day as the load phase of [`DataService::get_weekly_summary`] hands it
/// to the fold phase: its index in the caller's slice, its date, and its raw
/// content and parse (both `None` when the day has no file).
type DayLoad = (usize, Date, Option<String>, Option<TimeTrackingData>);

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

    /// The process-wide data service, built on first use with
    /// [`Self::DEFAULT_CACHE_TIMEOUT_SECONDS`] and taking both its directory
    /// and its parse settings from the global configuration.
    ///
    /// The CLI and web paths share this one instance, and so share its
    /// cache. The TUI and the tests build their own with
    /// [`Self::new_with_dir`] instead, which never reads the global config.
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

    /// Create the data directory if it is not there yet.
    ///
    /// The one place a [`DataService`] creates anything on disk that is not a
    /// day file. Creation used to happen inside [`Self::get_file_path`], which
    /// sits on *every* read path in the CLI, the web server and the TUI — so a
    /// report that only ever looked at files brought the directory into being
    /// as a side effect, and paid a resolve plus a stat plus a create for each
    /// of the ninety dates the calendar asks about. Callers that are about to
    /// write call this first; callers that only read no longer touch it.
    pub async fn ensure_data_dir(&self) -> Result<()> {
        let dir = self.data_dir.resolve()?;
        // `create_dir_all` is already a no-op on an existing directory, so
        // there is nothing for an `exists()` check to save.
        fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("could not create data directory {}", dir.display()))
    }

    /// The path `date`'s day file has, whether or not anything is there.
    ///
    /// Pure: it resolves the directory and joins a name, and creates and stats
    /// nothing. A caller that needs the directory to exist calls
    /// [`Self::ensure_data_dir`] itself.
    pub async fn get_file_path(&self, date: Date) -> Result<PathBuf> {
        let time_tracking_dir = self.data_dir.resolve()?;
        let date_str = date.format(&DATE_FORMAT).context("could not format date")?;

        Ok(time_tracking_dir.join(format!("{date_str}.md")))
    }

    /// Read a day's content from file, using cache when possible
    pub async fn read_day(&self, date: &Date) -> Result<Option<String>> {
        let file_path = self.get_file_path(*date).await?;

        // One stat answers everything the read needs to know up front: whether
        // the file is there, whether it is a day file at all, and when it was
        // last written — so `cache_content` does not have to stat again.
        let metadata = match tokio::fs::metadata(&file_path).await {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e).with_context(|| format!("could not stat {}", file_path.display()));
            }
        };
        // A directory or a FIFO can be named `YYYY-MM-DD.md` as easily as a
        // day file can. Refusing them here is not pedantry: `read_to_string`
        // on a directory fails with `EISDIR`, and on a FIFO it blocks forever
        // with nothing to time it out — a hang is strictly worse than an
        // error. "There is no day file here" is both true and non-fatal, and
        // it is the same answer [`Self::existing_dates`] gives.
        if !metadata.is_file() {
            return Ok(None);
        }

        // Check cache first
        if let Some(content) = self.get_cached_content(date, &file_path).await? {
            return Ok(Some(content));
        }

        let file_mod_time = metadata.modified().ok();
        let content = match fs::read_to_string(&file_path).await {
            Ok(content) => content,
            // The file was there at the stat above and is gone now: deleting
            // the day file while a load is in flight is enough to land here.
            // "No file" is the honest answer and the same one the stat would
            // have given a moment earlier.
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e).with_context(|| format!("could not read {}", file_path.display()));
            }
        };
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

        self.cache_parsed(*date, &content, parsed.clone()).await;

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
        // This is a write, so it is one of the two places that materialises
        // the directory; `get_file_path` no longer does it on the way past.
        self.ensure_data_dir().await?;
        let file_path = self.get_file_path(*date).await?;

        // create_new is atomic: it either creates the file or fails with
        // AlreadyExists. An exists()-then-write pair leaves a window in
        // which another writer (a second ttcli, the TUI, the web server)
        // creates and fills the file, and our template write then truncates
        // their content back to empty.
        //
        // `create_template_content` can do real I/O (reading a configured
        // template file) and can fail, so it stays inside the success arm:
        // an existing file must never pay for it, and a broken template
        // path must never turn an already-created day into an error.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt as _;
                let template_content =
                    create_template_content(date, self.parse_opts.template_file()).await?;
                file.write_all(template_content.as_bytes()).await?;

                // Invalidate cache since we just created the file
                self.invalidate_date(date).await;
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                // Someone else got there first; their content stands.
            }
            Err(e) => return Err(e.into()),
        }

        Ok(file_path)
    }

    /// The markers and template this service parses and creates files with.
    ///
    /// Callers that render a day themselves must parse it with *these*
    /// settings rather than reaching for [`Config::get`], or the aggregate and
    /// the per-day views of the same file can disagree about where the entries
    /// begin and end. See `show_weekly_summary_with`.
    pub fn parse_settings(&self) -> ParseSettings {
        match &self.parse_opts {
            ParseOpts::FromConfig => ParseSettings::from_config(Config::get()),
            ParseOpts::Fixed(settings) => settings.clone(),
        }
    }

    /// Check if a date has data (contains projects with time > 0)
    pub async fn check_date_has_data(&self, date: &Date) -> Result<bool> {
        if let Some(data) = self.parse_day(date).await? {
            Ok(!data.projects.is_empty() && data.total_minutes > 0)
        } else {
            Ok(false)
        }
    }

    /// The dates in `start..=end` that have a day file on disk, ascending.
    ///
    /// One `read_dir` of the whole directory rather than a `stat` per date:
    /// the TUI's calendar asks about ninety consecutive days on every arrow
    /// key, and all ninety live in the same directory, so listing it once is
    /// both fewer syscalls now and a cost that stops growing with the window.
    ///
    /// A directory that is not there yields an empty `Vec` rather than an
    /// error — it holds no day files, which is the answer the caller wants,
    /// and reads no longer create it (see [`Self::ensure_data_dir`]).
    ///
    /// Only regular files are candidates. A directory or a FIFO can be named
    /// `YYYY-MM-DD.md` just as easily, and admitting one is how a scan ends up
    /// failing with `EISDIR` or blocking forever on an open FIFO.
    ///
    /// **Existing is not the same as populated.** A day file with nothing
    /// logged in it still exists; see [`Self::find_populated_dates`].
    pub async fn existing_dates(&self, start: Date, end: Date) -> Result<Vec<Date>> {
        let dir = self.data_dir.resolve()?;
        let mut entries = match fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(e).with_context(|| format!("could not list {}", dir.display()));
            }
        };

        let mut dates = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("could not read an entry of {}", dir.display()))?
        {
            let Some(date) = day_file_date(&entry.file_name()) else {
                continue;
            };
            if !(start..=end).contains(&date) {
                continue;
            }
            // Checked last, and only for entries already known to be in-range
            // day files, so the ninety-probes win stands: on Linux this reads
            // the `d_type` `read_dir` already returned and costs no syscall.
            match entry.file_type().await {
                Ok(ft) if ft.is_file() => {}
                // A symlinked day file is a real use (a day file living in a
                // synced folder), and resolving the target here would cost a
                // stat per candidate, so let it through and let `read_day`
                // and the per-day containment in the walkers judge it.
                Ok(ft) if ft.is_symlink() => {}
                // A directory, a FIFO, a socket — or an entry we could not
                // even classify. None of them is a day file, and none of them
                // is worth failing the whole listing over.
                _ => continue,
            }
            dates.push(date);
        }
        // `read_dir` hands entries back in whatever order the filesystem keeps
        // them in, and every caller wants them in day order.
        dates.sort_unstable();

        Ok(dates)
    }

    /// Find all populated dates within a date range.
    ///
    /// Two stages, because they answer two different questions:
    /// [`Self::existing_dates`] narrows the range to the days that have a file
    /// at all with a single directory listing, and only those are parsed.
    /// "Populated" still means exactly what it always did — projects, and time
    /// on the clock — so a day file with nothing logged in it is not one.
    pub async fn find_populated_dates(
        &self,
        start_date: Date,
        end_date: Date,
    ) -> Result<Vec<Date>> {
        // The fan-out is deliberately a `JoinSet` and must stay one. Dropping
        // a `JoinSet` aborts the tasks it owns, so these children are reachable
        // from the single `JoinHandle` the TUI holds for the load that spawned
        // them: `App::spawn_load` aborts that handle when a newer load
        // supersedes it, and the abort cascades here. Detached `tokio::spawn`s
        // would look identical and pass every test while bounding nothing — a
        // held-down date key would pile the children up unabortably.
        let mut set = tokio::task::JoinSet::new();
        for date in self.existing_dates(start_date, end_date).await? {
            let svc = self.clone();
            set.spawn(async move {
                let has_data =
                    day_or_skip(date, svc.check_date_has_data(&date).await).unwrap_or(false);
                (date, has_data)
            });
        }

        let mut populated_dates = Vec::new();
        while let Some(result) = set.join_next().await {
            let (date, has_data) = result?;
            if has_data {
                populated_dates.push(date);
            }
        }
        populated_dates.sort_unstable();
        Ok(populated_dates)
    }

    /// Aggregate `dates` into a single [`WeeklySummary`].
    ///
    /// The days are loaded concurrently and then folded in the caller's order,
    /// which is what keeps the per-day prefixes on warnings and project notes
    /// in day order. Both the raw content and the parse come from the per-date
    /// cache, so a week that is already cached costs no file reads and no
    /// reparses; a cold week pays for its slowest day rather than all seven.
    pub async fn get_weekly_summary(&self, dates: &[Date]) -> Result<WeeklySummary> {
        // Load phase: one task per date. The dates are distinct, so no two
        // tasks can race to parse the same day.
        let mut set = tokio::task::JoinSet::new();
        for (idx, &date) in dates.iter().enumerate() {
            let svc = self.clone();
            set.spawn(async move {
                let loaded = async {
                    let content = svc.read_day(&date).await?;
                    let parsed = svc.parse_day(&date).await?;
                    Ok::<_, anyhow::Error>((content, parsed))
                }
                .await;
                // An unreadable day renders as an empty day, exactly like one
                // with no file, rather than taking the other six down with it.
                let (content, parsed) = day_or_skip(date, loaded).unwrap_or((None, None));
                (idx, date, content, parsed)
            });
        }

        let mut loaded: Vec<DayLoad> = Vec::with_capacity(dates.len());
        while let Some(result) = set.join_next().await {
            loaded.push(result?);
        }
        // Restore the caller's order before folding: the fold below is where
        // the day prefixes on notes and warnings get their sequence.
        loaded.sort_unstable_by_key(|(idx, ..)| *idx);

        let mut summary = WeeklySummary::default();
        let mut week_projects: HashMap<String, (u32, Vec<String>)> = HashMap::new();

        // Fold phase: sequential, in date order.
        for (_, day_date, content, parsed) in loaded {
            let (Some(content), Some(data)) = (content, parsed) else {
                summary.per_day.insert(day_date, 0);
                summary.days.push((day_date, String::new(), None));
                continue;
            };

            summary.total_minutes += data.total_minutes;
            summary.dead_time_minutes += data.dead_time_minutes;
            summary.per_day.insert(day_date, data.total_minutes);

            for warning in &data.warnings {
                if !warning.contains("Error parsing time range '#'") {
                    // Skip markdown header warnings
                    summary.warnings.push(format!(
                        "{}: {}",
                        format_day_with_date(&day_date),
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
                        .push(format!("{}: {}", format_day_with_date(&day_date), note));
                }
            }

            summary.days.push((day_date, content, Some(data)));
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
                // Inequality, not `<=`. The question is "has the file changed
                // since we cached it", not "is it newer than what we cached":
                // a restore from backup, a `git checkout`, a `cp -p` or clock
                // skew on a network mount all move the mtime *backwards*, and
                // `<=` called every one of those unmodified.
                && file_mod_time == cached_mod_time
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
    /// already recorded for `date` (the raw content and its mod time).
    ///
    /// `content` is the text the parse was actually computed from, and the
    /// attach happens **only** if the entry still holds exactly that text.
    /// Both ways of losing that race drop the parse: the entry may have been
    /// invalidated out from under us, or a concurrent `read_day` may have
    /// replaced it with newer content. Attaching regardless left the new
    /// content sitting next to the old parse, and `get_valid_entry` then
    /// certified that pairing as fresh for the rest of the TTL — the raw-file
    /// pane showing the new text while the totals, the calendar marker and the
    /// bar chart showed the old numbers. Dropping costs only a reparse on the
    /// next call, which is correct, just not maximally memoized.
    async fn cache_parsed(&self, date: Date, content: &str, parsed: TimeTrackingData) {
        let mut cache = self.cache.lock().await;
        if let Some(entry) = cache.get_mut(&date)
            && entry.data.as_deref() == Some(content)
        {
            entry.parsed = Some(parsed);
        }
    }
}

/// Contain a per-day failure inside a multi-day walk: log it and drop the day.
///
/// One unreadable entry must never be fatal to the whole scan. A day file the
/// user cannot read, one deleted between the directory listing and the read,
/// or anything else that only `stat` and `open` can discover would otherwise
/// propagate out of the `JoinSet` and fail the entire week or the entire
/// ninety-day calendar sweep — hiding every other day along with it. The
/// single-day paths still surface their errors; it is only the walkers, where
/// one day is a detail of a larger answer, that degrade.
fn day_or_skip<T>(date: Date, result: Result<T>) -> Option<T> {
    result
        .inspect_err(|e| warn!("skipping the day file for {date}: {e:#}"))
        .ok()
}

/// The date a day file's name encodes, or `None` for anything else that
/// shares the directory — a template, a README, an editor's swap file.
///
/// `Date::parse` is the whole pattern check: it rejects any stem that is not
/// exactly `YYYY-MM-DD`, trailing characters included, so matching
/// `^\d{4}-\d{2}-\d{2}\.md$` needs no regex crate.
fn day_file_date(file_name: &OsStr) -> Option<Date> {
    let stem = file_name.to_str()?.strip_suffix(".md")?;
    Date::parse(stem, DATE_FORMAT).ok()
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
    /// A write that lands while a parse is in flight must not leave the new
    /// content sitting next to the old parse.
    ///
    /// `cache_parsed` used to attach its parse to whatever entry happened to
    /// be in the map when it finally took the lock, so this interleaving left
    /// `{data: C2, parsed: parse(C1), mod: M2}` — which `get_valid_entry` then
    /// certifies as fresh for the whole TTL. It is not exotic: `App::load`
    /// issues three racing accesses to the same date on every load, the TUI
    /// auto-reloads the instant the file changes on disk, and `r` does not
    /// invalidate, so the only escape was to wait the TTL out.
    ///
    /// The window is opened deliberately rather than slept at. `C1` is large
    /// enough that parsing it takes tens of milliseconds, and the test waits
    /// on the cache entry `read_day` writes — the exact moment the window
    /// opens — instead of guessing with a fixed delay. The reviewer's original
    /// formulation slept 1100ms *before* writing, by which time the parse it
    /// meant to race had long since finished; it passed 50/50 against the bug.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_write_racing_an_in_flight_parse_does_not_poison_the_cache() {
        let (service, dir) = hermetic_service(60);
        let d = date!(2026 - 08 - 24);
        let path = dir.path().join("2026-08-24.md");

        // C1: slow to parse, and its total is nowhere near C2's.
        let c1: String = (0..20_000).map(|_| "8-10 admin\n").collect();
        tokio::fs::write(&path, &c1).await.unwrap();

        let parser = {
            let svc = service.clone();
            tokio::spawn(async move { svc.parse_day(&d).await })
        };

        // Wait for the parser's `read_day` to publish C1, then it is parsing.
        //
        // Bounded for the same reason `reading_a_fifo_named_like_a_day_file_
        // does_not_block` is: the loop's exit condition is something the code
        // under test has to make true, so a regression that stops it — a
        // `read_day` that errors before publishing, say — would spin here
        // forever and wedge the suite instead of failing it.
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let seen = {
                    let cache = service.cache.lock().await;
                    cache.get(&d).and_then(|e| e.data.clone())
                };
                if seen.as_deref() == Some(c1.as_str()) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the parser's read_day must publish C1 to the cache");

        // C2 lands mid-parse and a concurrent reader republishes the entry.
        tokio::fs::write(&path, "8-12 admin\n").await.unwrap();
        assert_eq!(
            service.read_day(&d).await.unwrap().as_deref(),
            Some("8-12 admin\n"),
            "the racing reader must see the new content"
        );

        parser.await.unwrap().unwrap();

        assert_eq!(
            service.parse_day(&d).await.unwrap().unwrap().total_minutes,
            240,
            "the cache must not be holding the pre-write parse"
        );
    }

    /// A day file replaced by an *older* copy must invalidate the cache.
    ///
    /// The validity check asked `file_mod_time <= cached_mod_time`, which is an
    /// ordering question nobody was asking: a `git checkout`, a `cp -p`, an
    /// `rsync --times`, a restore from backup, an editor that preserves mtime,
    /// or clock skew on a network mount all move the mtime backwards, and the
    /// cache then declared itself still valid and served the pre-restore
    /// content for the rest of the TTL. `!=` is the question actually being
    /// asked: has the file changed since we cached it.
    #[tokio::test]
    async fn a_day_file_restored_to_an_older_copy_is_not_served_from_cache() {
        let (service, dir) = hermetic_service(60);
        let d = date!(2026 - 08 - 24);
        let path = dir.path().join("2026-08-24.md");

        tokio::fs::write(&path, "8-12 admin\n").await.unwrap();
        assert_eq!(
            service.parse_day(&d).await.unwrap().unwrap().total_minutes,
            240
        );

        tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
        let an_hour_ago = SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(an_hour_ago)
            .unwrap();

        assert_eq!(
            service.parse_day(&d).await.unwrap().unwrap().total_minutes,
            120,
            "a file whose mtime went backwards has still changed"
        );
    }

    /// A day file the caller cannot read must cost that one day, not the week.
    ///
    /// `read_day`'s error used to propagate through the `JoinSet`'s `result??`
    /// and fail `get_weekly_summary` outright, so a single `chmod 000` file —
    /// or a directory, or a FIFO, named `YYYY-MM-DD.md` — hid all seven days
    /// and exited non-zero. Verified before the fix from the CLI: EISDIR and
    /// EACCES both printed the header and then `Error: ...`, and the FIFO hung
    /// until it was killed at 15s.
    #[tokio::test]
    async fn one_unreadable_entry_does_not_fail_the_whole_week() {
        let (service, dir) = hermetic_service(60);
        let good = date!(2026 - 08 - 24);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        // A directory named exactly like a day file: `read_to_string` gives
        // EISDIR. Nothing rejects it by name, so it reaches the read.
        std::fs::create_dir(dir.path().join("2026-08-25.md")).unwrap();

        let summary = service
            .get_weekly_summary(&week_of_2026_08_22())
            .await
            .expect("one bad entry must not fail the week");

        assert_eq!(
            summary.total_minutes, 120,
            "the readable day must still be counted"
        );
        assert_eq!(summary.per_day.get(&good), Some(&120));
        assert_eq!(
            summary.per_day.get(&date!(2026 - 08 - 25)),
            Some(&0),
            "the unreadable entry renders as an empty day"
        );
    }

    /// The same containment for the ninety-day calendar sweep.
    #[tokio::test]
    async fn one_unreadable_entry_does_not_fail_the_calendar_scan() {
        let (service, dir) = hermetic_service(60);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        std::fs::create_dir(dir.path().join("2026-08-25.md")).unwrap();

        let found = service
            .find_populated_dates(date!(2026 - 08 - 22), date!(2026 - 08 - 28))
            .await
            .expect("one bad entry must not fail the scan");

        assert_eq!(found, vec![date!(2026 - 08 - 24)]);
    }

    /// A FIFO named like a day file used to block the scan forever, which is
    /// worse than an error: nothing times out and nothing reports. It is not a
    /// regular file, so `existing_dates` must never offer it as a candidate.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_fifo_named_like_a_day_file_is_not_a_candidate() {
        use std::ffi::CString;

        let (service, dir) = hermetic_service(60);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        let fifo = CString::new(dir.path().join("2026-08-27.md").to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path and a valid mode; `mkfifo` only
        // reads the pointer for the duration of the call.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);

        let dates = service
            .existing_dates(date!(2026 - 08 - 22), date!(2026 - 08 - 28))
            .await
            .unwrap();

        assert_eq!(
            dates,
            vec![date!(2026 - 08 - 24)],
            "a FIFO is not a day file and must never reach `read_day`"
        );
    }

    /// Reading a FIFO must return "no day file", not block forever.
    ///
    /// The weekly path does not go through `existing_dates` — it hands
    /// `get_weekly_summary` all seven dates of the week directly — so the
    /// candidate-set filter does not protect it and `read_day` has to refuse
    /// non-regular files itself. Verified before the fix from the CLI: a FIFO
    /// named `2026-08-27.md` hung `ttcli --week` until it was killed at 15s.
    /// The timeout is deliberate: a regression here must fail the suite, never
    /// wedge it.
    #[cfg(unix)]
    #[tokio::test]
    async fn reading_a_fifo_named_like_a_day_file_does_not_block() {
        use std::ffi::CString;

        let (service, dir) = hermetic_service(60);
        let d = date!(2026 - 08 - 27);
        let fifo = CString::new(dir.path().join("2026-08-27.md").to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path and a valid mode; `mkfifo` only
        // reads the pointer for the duration of the call.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);

        let got = tokio::time::timeout(std::time::Duration::from_secs(5), service.read_day(&d))
            .await
            .expect("reading a FIFO must not block");

        assert_eq!(got.unwrap(), None, "a FIFO is not a day file");
    }

    /// A day file the user cannot read costs that day, not the week.
    ///
    /// This is the case the candidate-set filter cannot catch — the entry is a
    /// perfectly ordinary regular file — so it is what pins `day_or_skip`.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_unreadable_day_file_does_not_fail_the_whole_week() {
        use std::os::unix::fs::PermissionsExt;

        // SAFETY: `geteuid` reads a process attribute and cannot fail.
        if unsafe { libc::geteuid() } == 0 {
            // Root reads a 0o000 file happily, so there is nothing to contain.
            return;
        }

        let (service, dir) = hermetic_service(60);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        let locked = dir.path().join("2026-08-26.md");
        tokio::fs::write(&locked, "9-11 ops\n").await.unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

        let summary = service
            .get_weekly_summary(&week_of_2026_08_22())
            .await
            .expect("an unreadable day must not fail the week");

        assert_eq!(
            summary.total_minutes, 120,
            "the readable day must still be counted"
        );
        assert_eq!(summary.per_day.get(&date!(2026 - 08 - 26)), Some(&0));
    }

    /// A directory is likewise never a candidate, so it does not even reach
    /// the read that the containment above would have to catch.
    #[tokio::test]
    async fn a_directory_named_like_a_day_file_is_not_a_candidate() {
        let (service, dir) = hermetic_service(60);
        tokio::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n")
            .await
            .unwrap();
        std::fs::create_dir(dir.path().join("2026-08-25.md")).unwrap();

        assert_eq!(
            service
                .existing_dates(date!(2026 - 08 - 22), date!(2026 - 08 - 28))
                .await
                .unwrap(),
            vec![date!(2026 - 08 - 24)]
        );
    }

    /// A symlinked day file must keep working: pointing a day file at one in a
    /// synced folder is a real use, and the non-regular-file skip above is the
    /// kind of change that quietly breaks it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlinked_day_file_is_still_a_candidate() {
        let (service, dir) = hermetic_service(60);
        let target = dir.path().join("elsewhere.md");
        tokio::fs::write(&target, "8-10 admin\n").await.unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("2026-08-24.md")).unwrap();

        assert_eq!(
            service
                .existing_dates(date!(2026 - 08 - 22), date!(2026 - 08 - 28))
                .await
                .unwrap(),
            vec![date!(2026 - 08 - 24)]
        );
        assert_eq!(
            service
                .parse_day(&date!(2026 - 08 - 24))
                .await
                .unwrap()
                .unwrap()
                .total_minutes,
            120
        );
    }

    /// A day file deleted between `read_day`'s `exists()` check and its read
    /// reads as absent, not as an error. Reachable by deleting today's file
    /// while the TUI is loading.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_day_file_deleted_mid_read_reads_as_absent() {
        let (service, dir) = hermetic_service(0);
        let d = date!(2026 - 08 - 24);
        let path = dir.path().join("2026-08-24.md");

        // Racing a delete against the read is inherently timing-dependent, so
        // run it until the window is hit rather than once and hopefully.
        for _ in 0..2_000 {
            tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
            let svc = service.clone();
            let reader = tokio::spawn(async move { svc.read_day(&d).await });
            let _ = tokio::fs::remove_file(&path).await;
            let got = reader.await.unwrap();
            assert!(
                got.is_ok(),
                "a deletion racing the read must read as absent, not error: {:?}",
                got.err()
            );
        }
    }

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
    async fn create_day_file_does_not_clobber_content_written_after_the_exists_check() {
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 29);

        // Simulate the racing writer having already won: the file exists
        // with real content by the time the template write would land.
        let path = service.get_file_path(test_date).await.unwrap();
        tokio::fs::write(&path, "real user content\n")
            .await
            .unwrap();

        service
            .create_day_file_if_not_exists(&test_date)
            .await
            .unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(
            after, "real user content\n",
            "template write clobbered real content"
        );
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

    /// The listing is the candidate set for every calendar scan, so it has to
    /// recognise a day file and nothing else: a stray note, and a date-named
    /// file with the wrong extension, both live in the same directory.
    #[tokio::test]
    async fn existing_dates_lists_only_files_that_are_there() {
        let (service, dir) = hermetic_service(60);
        std::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n").unwrap();
        std::fs::write(dir.path().join("2026-08-26.md"), "8-10 admin\n").unwrap();
        std::fs::write(dir.path().join("notes.md"), "not a date\n").unwrap();
        std::fs::write(dir.path().join("2026-08-25.txt"), "wrong extension\n").unwrap();

        let got = service
            .existing_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31))
            .await
            .unwrap();

        assert_eq!(got, vec![date!(2026 - 08 - 24), date!(2026 - 08 - 26)]);
    }

    /// One listing covers the whole directory, so the range has to be applied
    /// afterwards — the calendar asks for a ninety-day window out of however
    /// many years of files have accumulated.
    #[tokio::test]
    async fn existing_dates_keeps_to_the_requested_range() {
        let (service, dir) = hermetic_service(60);
        for name in [
            "2026-07-31.md",
            "2026-08-01.md",
            "2026-08-31.md",
            "2026-09-01.md",
        ] {
            std::fs::write(dir.path().join(name), "8-10 admin\n").unwrap();
        }

        let got = service
            .existing_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31))
            .await
            .unwrap();

        assert_eq!(got, vec![date!(2026 - 08 - 01), date!(2026 - 08 - 31)]);
    }

    /// A directory that is not there holds no day files. Erroring instead
    /// would turn a fresh install into a TUI that cannot draw a calendar.
    #[tokio::test]
    async fn existing_dates_of_a_missing_directory_is_empty_rather_than_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let service = DataService::new_with_dir(
            60,
            dir.path().join("does-not-exist"),
            ParseSettings::default(),
        );

        let got = service
            .existing_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31))
            .await
            .unwrap();

        assert!(got.is_empty());
    }

    /// The listing narrows the candidates; it must not become the definition.
    /// `read_dir` says the file *exists* — "populated" still means projects
    /// with time on the clock.
    #[tokio::test]
    async fn an_existing_but_empty_file_is_not_populated() {
        let (service, dir) = hermetic_service(60);
        std::fs::write(dir.path().join("2026-08-24.md"), "# just a header\n").unwrap();

        let got = service
            .find_populated_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31))
            .await
            .unwrap();

        assert!(
            got.is_empty(),
            "a file with no logged time is not a populated date"
        );
    }

    /// Guards the test above from passing vacuously: a day that *does* have
    /// time on it still has to survive the listing and reach the parse.
    #[tokio::test]
    async fn a_populated_file_is_still_found_through_the_listing() {
        let (service, dir) = hermetic_service(60);
        std::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n").unwrap();

        let got = service
            .find_populated_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31))
            .await
            .unwrap();

        assert_eq!(got, vec![date!(2026 - 08 - 24)]);
    }

    /// `get_file_path` used to `create_dir_all` on the way past, so every read
    /// in the CLI, the web server and the TUI materialised the data directory
    /// as a side effect.
    #[tokio::test]
    async fn reading_does_not_create_the_data_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("does-not-exist");
        let service = DataService::new_with_dir(60, missing.clone(), ParseSettings::default());

        assert!(
            service
                .read_day(&date!(2026 - 08 - 24))
                .await
                .unwrap()
                .is_none()
        );

        assert!(
            !missing.exists(),
            "a read must not create the data directory"
        );
    }

    /// The other half of that move: creation now belongs to the write path,
    /// which has to keep working on a machine that has never run the tool.
    #[tokio::test]
    async fn creating_a_day_file_creates_the_data_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("does-not-exist");
        let service = DataService::new_with_dir(60, missing.clone(), ParseSettings::default());

        let path = service
            .create_day_file_if_not_exists(&date!(2026 - 08 - 24))
            .await
            .unwrap();

        assert!(path.exists());
        assert!(
            missing.is_dir(),
            "the write path is what materialises the directory"
        );
    }
}

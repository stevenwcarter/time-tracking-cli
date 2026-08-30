use crate::{
    Config, DATE_FORMAT, DataService,
    context::GraphQLContext,
    graphql::{Schema, create_schema},
};
use axum::{
    Extension, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Json, Response},
    routing::{MethodFilter, get, on},
};
use axum_embed::{FallbackBehavior, ServeEmbed};
use juniper::GraphQLObject;
use juniper_axum::{extract::JuniperRequest, graphiql, playground, response::JuniperResponse};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use time::{Date, OffsetDateTime};
use tokio::{select, sync::oneshot::Receiver};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, cors::CorsLayer};
use tracing::{debug, info};

use crate::{get_week_dates, parse_weekday};

#[derive(RustEmbed, Clone)]
#[folder = "site/build/"]
struct SiteAssets;

#[derive(Clone, Default)]
pub struct AppState {
    pub config: Config,
}

#[derive(Serialize, Deserialize)]
struct DateQuery {
    date: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct WeekQuery {
    date: Option<String>,
    week_start_day: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, GraphQLObject)]
pub struct DayData {
    pub date: String,
    pub total_hours: f64,
    pub dead_time_hours: f64,
    pub projects: Vec<ProjectData>,
    pub warnings: Vec<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

impl DayData {
    pub fn empty(date: Date) -> Self {
        DayData {
            date: date.format(&DATE_FORMAT).unwrap_or_default(),
            total_hours: 0.0,
            dead_time_hours: 0.0,
            projects: vec![],
            warnings: vec![],
            start_time: None,
            end_time: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, GraphQLObject)]
pub struct ProjectData {
    pub name: String,
    pub total_hours: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, GraphQLObject)]
pub struct ProjectSummary {
    pub name: String,
    pub total_hours: f64,
}

#[derive(Debug, Serialize, Deserialize, GraphQLObject)]
pub struct WeekData {
    pub start_date: String,
    pub end_date: String,
    pub total_hours: f64,
    pub dead_time_hours: f64,
    pub days: Vec<DayData>,
    pub project_summaries: Vec<ProjectSummary>,
}

/// Serve the web app on `port` of localhost: the `/api` REST endpoints, the
/// Juniper schema under `/graphql`, and the embedded React build for
/// everything else.
///
/// This is the crate's webapp entry point, re-exported from the crate root.
/// It runs until ctrl-c or a message on `rx` — which is how the TUI exiting
/// takes the server down with it — then drains the in-flight requests.
pub async fn run_server(port: u16, config: Config, rx: Receiver<()>) -> anyhow::Result<()> {
    let state = AppState { config };
    let context = GraphQLContext::new(state.clone());
    let qm_schema = create_schema();

    let middleware = ServiceBuilder::new().layer(CompressionLayer::new());
    let graphql_routes = Router::new()
        .route(
            "/",
            on(MethodFilter::GET.or(MethodFilter::POST), custom_graphql),
        )
        .route(
            "/graphiql",
            get(graphiql("/graphql", "/graphql/subscriptions")),
        )
        .route(
            "/playground",
            get(playground("/graphql", "/graphql/subscriptions")),
        )
        .layer(Extension(context.clone()))
        .layer(Extension(Arc::new(qm_schema)))
        .layer(middleware.clone());

    let serve_assets = ServeEmbed::<SiteAssets>::with_parameters(
        Some("/index.html".to_string()),
        FallbackBehavior::Ok,
        None,
    );

    let fallback_serve_assets = serve_assets.clone();

    let app = Router::new()
        .route_service("/assets/{*uri}", serve_assets)
        .layer(middleware::from_fn(set_static_cache_control))
        .route("/api/day", get(get_day_data))
        .route("/api/day/{date}", get(get_day_data_by_date))
        .route("/api/week", get(get_week_data))
        .route("/api/week/{date}", get(get_week_data_by_date))
        .nest("/graphql", graphql_routes)
        .fallback_service(fallback_serve_assets)
        .layer(CorsLayer::permissive())
        .layer(Extension(context))
        .layer(middleware)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    info!(
        "Time Tracking Web Server running on http://localhost:{}",
        port
    );
    info!("Access your time tracking data via the web interface");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(rx))
        .await?;

    Ok(())
}

async fn shutdown_signal(rx: Receiver<()>) {
    select!(
        _ = tokio::signal::ctrl_c() => {}, _ = rx => {});
    debug!("Signal received, starting graceful shutdown");
}

async fn custom_graphql(
    Extension(schema): Extension<Arc<Schema>>,
    Extension(context): Extension<GraphQLContext>,
    JuniperRequest(request): JuniperRequest,
) -> JuniperResponse {
    JuniperResponse(request.execute(&schema, &context).await)
}

async fn get_day_data(
    State(state): State<AppState>,
    Query(params): Query<DateQuery>,
) -> Result<Json<DayData>, StatusCode> {
    let date = match params.date {
        Some(date_str) => {
            Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        None => OffsetDateTime::now_local()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .date(),
    };

    let data = get_day_data_impl(date, &state).await?;
    Ok(Json(data))
}

async fn get_day_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<DayData>, StatusCode> {
    let date = Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?;

    let data = get_day_data_impl(date, &state).await?;
    Ok(Json(data))
}

/// Build one day's [`DayData`] for the REST and GraphQL endpoints.
///
/// Delegates to [`get_day_data_impl_with`] against the process-wide
/// [`DataService`], which is what makes concurrent requests share its
/// 30-second cache. The service is a parameter there and not here so tests
/// can hand in a hermetic one instead of the global singleton.
pub async fn get_day_data_impl(date: Date, _state: &AppState) -> Result<DayData, StatusCode> {
    // `_state`: the endpoint's parse markers now come from the DataService,
    // which resolves them from the same `Config::get()` this state was cloned
    // from. The parameter stays because this is a public signature other
    // crates may name.
    get_day_data_impl_with(DataService::get(), date).await
}

pub(crate) async fn get_day_data_impl_with(
    svc: &DataService,
    date: Date,
) -> Result<DayData, StatusCode> {
    let date_str = date
        .format(&DATE_FORMAT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // `parse_day`, not `read_day` + a fresh parse: the service memoizes the
    // parse alongside the raw content, and going around it meant every REST
    // and GraphQL call reparsed from scratch — fanned out ×7 per week
    // request, and re-run on both queries per 500ms editor autosave.
    let data = svc.parse_day(&date).await.map_err(|e| {
        tracing::error!(%date_str, error = %e, "failed to read day data");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // `None` is "no file on disk", the same case the old `read_day` -> `None`
    // arm answered with an empty day.
    let Some(data) = data else {
        return Ok(DayData::empty(date));
    };

    let start_time = data.formatted_start_time();
    let end_time = data.formatted_end_time();
    let total_hours = data.total_minutes as f64 / 60.0;
    let dead_time_hours = data.dead_time_minutes as f64 / 60.0;
    let warnings = data.warnings;

    let projects: Vec<ProjectData> = data
        .projects
        .into_iter()
        .map(|p| ProjectData {
            name: p.name,
            total_hours: p.total_minutes as f64 / 60.0,
            notes: p.notes,
        })
        .collect();

    Ok(DayData {
        date: date_str,
        total_hours,
        dead_time_hours,
        projects,
        warnings,
        start_time: Some(start_time),
        end_time: Some(end_time),
    })
}

async fn get_week_data(
    State(state): State<AppState>,
    Query(params): Query<WeekQuery>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = match params.date {
        Some(date_str) => {
            Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        None => OffsetDateTime::now_local()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .date(),
    };

    let week_start_day = params
        .week_start_day
        .unwrap_or_else(|| state.config.get_week_start_day().to_string());

    get_week_data_impl(date, week_start_day, &state).await
}

async fn get_week_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?;

    get_week_data_impl(date, state.config.get_week_start_day().to_string(), &state).await
}

pub async fn aggregate_week_days(
    week_dates: &[Date],
    state: &AppState,
) -> (Vec<DayData>, Vec<ProjectSummary>, f64, f64) {
    let mut set = tokio::task::JoinSet::new();
    for (idx, &day_date) in week_dates.iter().enumerate() {
        let state = state.clone();
        set.spawn(async move { (idx, get_day_data_impl(day_date, &state).await) });
    }

    let mut results: Vec<(usize, DayData)> = Vec::with_capacity(week_dates.len());
    while let Some(outcome) = set.join_next().await {
        match outcome {
            Ok((idx, Ok(day_data))) => results.push((idx, day_data)),
            // Dropped rather than logged: `e` here is only the StatusCode.
            // `get_day_data_impl_with` already logged the underlying error
            // with its date and its I/O detail, so re-logging the status adds
            // a line and no information.
            Ok((_, Err(_))) => {}
            Err(e) => tracing::warn!("Task panicked loading day data: {}", e),
        }
    }
    // Restore original date order
    results.sort_unstable_by_key(|(idx, _)| *idx);

    let mut total_week_hours = 0.0;
    let mut total_dead_hours = 0.0;
    let mut week_projects: HashMap<String, f64> = HashMap::new();

    // Build a complete days vec (including empties for missing dates)
    let mut days: Vec<DayData> = week_dates.iter().map(|&d| DayData::empty(d)).collect();
    for (idx, day_data) in results {
        total_week_hours += day_data.total_hours;
        total_dead_hours += day_data.dead_time_hours;
        for project in &day_data.projects {
            *week_projects.entry(project.name.clone()).or_insert(0.0) += project.total_hours;
        }
        days[idx] = day_data;
    }

    let mut project_summaries: Vec<ProjectSummary> = week_projects
        .into_iter()
        .map(|(name, total_hours)| ProjectSummary { name, total_hours })
        .collect();
    project_summaries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    (days, project_summaries, total_week_hours, total_dead_hours)
}

async fn get_week_data_impl(
    date: Date,
    week_start_day: String,
    state: &AppState,
) -> Result<Json<WeekData>, StatusCode> {
    let week_start_weekday = parse_weekday(&week_start_day).map_err(|_| StatusCode::BAD_REQUEST)?;

    let week_dates = get_week_dates(&date, week_start_weekday);

    let (days, project_summaries, total_week_hours, total_dead_hours) =
        aggregate_week_days(&week_dates, state).await;

    Ok(Json(WeekData {
        start_date: week_dates[0]
            .format(DATE_FORMAT)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        end_date: week_dates[6]
            .format(DATE_FORMAT)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        total_hours: total_week_hours,
        dead_time_hours: total_dead_hours,
        days,
        project_summaries,
    }))
}

async fn set_static_cache_control(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_svc::ParseSettings;
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use time::macros::date;

    /// A `tracing` writer that keeps everything written to it in memory.
    #[derive(Clone, Default)]
    struct LogCapture(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for LogCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("log buffer").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
        type Writer = Self;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Run `body` with a capturing subscriber installed, returning its value
    /// and everything it logged.
    ///
    /// A plain `#[test]` with its own current-thread runtime rather than
    /// `#[tokio::test]`: `tracing::subscriber::with_default` sets a
    /// thread-local, and only a current-thread runtime keeps the async work
    /// on the thread that has it.
    fn capture_logs<T>(body: impl FnOnce() -> T) -> (T, String) {
        let capture = LogCapture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        let value = tracing::subscriber::with_default(subscriber, body);
        let logged = String::from_utf8(capture.0.lock().expect("log buffer").clone())
            .expect("log output is utf-8");
        (value, logged)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
    }

    /// A service whose data directory sits *below a regular file*, so every
    /// stat under it fails with ENOTDIR — an I/O error that is emphatically
    /// not `NotFound`, which is the only kind `read_day` swallows.
    fn unreadable_service() -> (DataService, TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let blocker = dir.path().join("not-a-directory");
        std::fs::File::create(&blocker)
            .expect("blocker file")
            .write_all(b"x")
            .expect("blocker contents");
        let svc = DataService::new_with_dir(60, blocker.join("days"), ParseSettings::default());
        (svc, dir)
    }

    /// A hermetic service whose parse markers differ from `Config::default()`'s
    /// (which has none), so a test can tell which of the two the endpoint
    /// honoured.
    fn service_with_markers(prefix: &str, suffix: &str) -> (DataService, TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let svc = DataService::new_with_dir(
            60,
            dir.path().to_path_buf(),
            ParseSettings {
                prefix: Some(prefix.to_owned()),
                suffix: Some(suffix.to_owned()),
                template_file: None,
            },
        );
        (svc, dir)
    }

    #[test]
    fn repeated_day_requests_reuse_the_memoized_parse() {
        let dir = tempfile::tempdir().expect("temp dir");
        let svc = DataService::new_with_dir(60, dir.path().to_path_buf(), ParseSettings::default());
        let day = date!(2026 - 08 - 24);
        let rt = runtime();

        rt.block_on(async {
            let path = svc.get_file_path(day).await.unwrap();
            tokio::fs::write(&path, "8-10 admin\n  - note\n")
                .await
                .unwrap();

            for _ in 0..5 {
                get_day_data_impl_with(&svc, day).await.expect("day data");
            }
        });

        assert_eq!(
            svc.parse_count(),
            1,
            "five requests for an unchanged day must run the parser once, \
             not once per request"
        );
    }

    #[test]
    fn day_data_is_parsed_with_the_services_markers() {
        // The endpoint used to parse with `state.config`'s markers; it now
        // parses with the service's. Production keeps the two in step —
        // `run_server` is handed a clone of the same `Config::get()` the
        // process-wide `DataService` reads — so this pins which one actually
        // governs the parse, and a future divergence fails here instead of
        // silently changing endpoint output.
        let (svc, _dir) = service_with_markers("```timetracking", "```");
        let day = date!(2026 - 08 - 24);
        let rt = runtime();

        let data = rt.block_on(async {
            let path = svc.get_file_path(day).await.unwrap();
            tokio::fs::write(
                &path,
                "8-9 outside-the-fence\n```timetracking\n9-11 admin\n```\n",
            )
            .await
            .unwrap();
            get_day_data_impl_with(&svc, day).await.expect("day data")
        });

        let names: Vec<&str> = data.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"admin"),
            "the fenced entry must be parsed: {names:?}"
        );
        assert!(
            !names.contains(&"outside-the-fence"),
            "the service's markers must bound the parse: {names:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_day_file_logs_its_cause_before_becoming_a_500() {
        let (svc, _dir) = unreadable_service();
        let rt = runtime();

        let (result, logged) =
            capture_logs(|| rt.block_on(get_day_data_impl_with(&svc, date!(2026 - 08 - 24))));

        assert_eq!(
            result.unwrap_err(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "an unreadable day file must still surface as a 500"
        );
        assert!(
            logged.contains("failed to read day data"),
            "the failure must be logged, not silently collapsed: {logged}"
        );
        assert!(
            logged.contains("2026-08-24"),
            "the log must name the date that failed: {logged}"
        );
        assert!(
            logged.contains("could not stat"),
            "the log must carry read_day's own context, not just a status: {logged}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_parse_failure_still_logs_its_cause_before_becoming_a_500() {
        // Guards the read-failure logging across this rewrite of the same
        // `map_err`.
        let (svc, _dir) = unreadable_service();
        let rt = runtime();

        let (result, logged) =
            capture_logs(|| rt.block_on(get_day_data_impl_with(&svc, date!(2026 - 08 - 24))));

        assert_eq!(result.unwrap_err(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            logged.contains("failed to read day data") && logged.contains("2026-08-24"),
            "the read-failure log must survive the parse_day rewrite: {logged}"
        );
    }
}

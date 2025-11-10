use crate::{
    Config, DATE_FORMAT,
    context::GraphQLContext,
    get_time_tracking_dir,
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
use tracing::debug;

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
            date: date.format(&DATE_FORMAT).unwrap(),
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

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    println!(
        "🌐 Time Tracking Web Server running on http://localhost:{}",
        port
    );
    println!("📊 Access your time tracking data via the web interface");

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
            .expect("Failed to get local time")
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

pub async fn get_day_data_impl(date: Date, state: &AppState) -> Result<DayData, StatusCode> {
    let time_tracking_dir =
        get_time_tracking_dir().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_path = time_tracking_dir.join(format!("{}.md", date.format(&DATE_FORMAT).unwrap()));

    if !file_path.exists() {
        return Ok(DayData {
            date: date.format(&DATE_FORMAT).unwrap().to_string(),
            total_hours: 0.0,
            dead_time_hours: 0.0,
            projects: vec![],
            warnings: vec![],
            start_time: None,
            end_time: None,
        });
    }

    let content =
        std::fs::read_to_string(&file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data = time_tracking_parser::parse_time_tracking_data(
        &content,
        state.config.get_prefix(),
        state.config.get_suffix(),
    );

    let projects: Vec<ProjectData> = data
        .projects
        .iter()
        .map(|p| ProjectData {
            name: p.name.clone(),
            total_hours: p.total_minutes as f64 / 60.0,
            notes: p.notes.clone(),
        })
        .collect();

    let start_time = data.formatted_start_time();
    let end_time = data.formatted_end_time();
    let warnings = data.warnings.clone();

    Ok(DayData {
        date: date.format(&DATE_FORMAT).unwrap().to_string(),
        total_hours: data.total_minutes as f64 / 60.0,
        dead_time_hours: data.dead_time_minutes as f64 / 60.0,
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
            //
        }
        None => OffsetDateTime::now_local().unwrap().date(),
    };

    let week_start_day = params
        .week_start_day
        .unwrap_or_else(|| "Saturday".to_string());

    get_week_data_impl(date, week_start_day, &state).await
}

async fn get_week_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?;

    get_week_data_impl(date, "Saturday".to_string(), &state).await
}

async fn get_week_data_impl(
    date: Date,
    week_start_day: String,
    state: &AppState,
) -> Result<Json<WeekData>, StatusCode> {
    let week_start_weekday = parse_weekday(&week_start_day).map_err(|_| StatusCode::BAD_REQUEST)?;

    let week_dates = get_week_dates(&date, week_start_weekday);

    let mut total_week_hours = 0.0;
    let mut total_dead_hours = 0.0;
    let mut week_projects: HashMap<String, f64> = HashMap::new();
    let mut days = Vec::new();

    for day_date in &week_dates {
        match get_day_data_impl(*day_date, state).await {
            Ok(day_data) => {
                total_week_hours += day_data.total_hours;
                total_dead_hours += day_data.dead_time_hours;

                for project in &day_data.projects {
                    *week_projects.entry(project.name.clone()).or_insert(0.0) +=
                        project.total_hours;
                }

                days.push(day_data);
            }
            Err(_) => {
                // If we can't get day data, add an empty day
                days.push(DayData::empty(*day_date));
            }
        }
    }

    let project_summaries: Vec<ProjectSummary> = week_projects
        .into_iter()
        .map(|(name, total_hours)| ProjectSummary { name, total_hours })
        .collect();

    Ok(Json(WeekData {
        start_date: week_dates[0]
            .format(DATE_FORMAT)
            .expect("could not parse date"),
        end_date: week_dates[6]
            .format(DATE_FORMAT)
            .expect("could not parse date"),
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

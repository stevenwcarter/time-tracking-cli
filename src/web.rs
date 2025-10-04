use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::get,
};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::{get_time_tracking_dir_with_override, get_week_dates, parse_weekday};

#[derive(Clone)]
struct AppState {
    data_directory: Option<String>,
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

#[derive(Serialize)]
struct DayData {
    date: String,
    total_hours: f64,
    dead_time_hours: f64,
    projects: Vec<ProjectData>,
    warnings: Vec<String>,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[derive(Serialize)]
struct ProjectData {
    name: String,
    total_hours: f64,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct WeekData {
    start_date: String,
    end_date: String,
    total_hours: f64,
    dead_time_hours: f64,
    days: Vec<DayData>,
    projects: HashMap<String, f64>,
}

pub async fn run_server(
    port: u16,
    data_directory: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState { data_directory };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/api/day", get(get_day_data))
        .route("/api/day/:date", get(get_day_data_by_date))
        .route("/api/week", get(get_week_data))
        .route("/api/week/:date", get(get_week_data_by_date))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    println!(
        "🌐 Time Tracking Web Server running on http://localhost:{}",
        port
    );
    println!("📊 Access your time tracking data via the web interface");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn get_day_data(
    State(state): State<AppState>,
    Query(params): Query<DateQuery>,
) -> Result<Json<DayData>, StatusCode> {
    let date = match params.date {
        Some(date_str) => {
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|_| StatusCode::BAD_REQUEST)?
        }
        None => Local::now().date_naive(),
    };

    get_day_data_impl(date, &state).await
}

async fn get_day_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<DayData>, StatusCode> {
    let date =
        NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|_| StatusCode::BAD_REQUEST)?;

    get_day_data_impl(date, &state).await
}

async fn get_day_data_impl(date: NaiveDate, state: &AppState) -> Result<Json<DayData>, StatusCode> {
    let time_tracking_dir = get_time_tracking_dir_with_override(state.data_directory.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_path = time_tracking_dir.join(format!("{}.md", date.format("%Y-%m-%d")));

    if !file_path.exists() {
        return Ok(Json(DayData {
            date: date.format("%Y-%m-%d").to_string(),
            total_hours: 0.0,
            dead_time_hours: 0.0,
            projects: vec![],
            warnings: vec![],
            start_time: None,
            end_time: None,
        }));
    }

    let content =
        std::fs::read_to_string(&file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data = time_tracking_parser::parse_time_tracking_data(&content);

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

    Ok(Json(DayData {
        date: date.format("%Y-%m-%d").to_string(),
        total_hours: data.total_minutes as f64 / 60.0,
        dead_time_hours: data.dead_time_minutes as f64 / 60.0,
        projects,
        warnings,
        start_time: Some(start_time),
        end_time: Some(end_time),
    }))
}

async fn get_week_data(
    State(state): State<AppState>,
    Query(params): Query<WeekQuery>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = match params.date {
        Some(date_str) => {
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|_| StatusCode::BAD_REQUEST)?
        }
        None => Local::now().date_naive(),
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
    let date =
        NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|_| StatusCode::BAD_REQUEST)?;

    get_week_data_impl(date, "Saturday".to_string(), &state).await
}

async fn get_week_data_impl(
    date: NaiveDate,
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
            Ok(Json(day_data)) => {
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
                days.push(DayData {
                    date: day_date.format("%Y-%m-%d").to_string(),
                    total_hours: 0.0,
                    dead_time_hours: 0.0,
                    projects: vec![],
                    warnings: vec![],
                    start_time: None,
                    end_time: None,
                });
            }
        }
    }

    Ok(Json(WeekData {
        start_date: week_dates[0].format("%Y-%m-%d").to_string(),
        end_date: week_dates[6].format("%Y-%m-%d").to_string(),
        total_hours: total_week_hours,
        dead_time_hours: total_dead_hours,
        days,
        projects: week_projects,
    }))
}

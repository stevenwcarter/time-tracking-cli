use juniper::{EmptySubscription, FieldResult, RootNode};
use std::fs;
use time::Date;

use crate::{
    DATE_FORMAT,
    context::GraphQLContext,
    create_template_content, get_time_tracking_dir_with_override, get_week_dates, parse_weekday,
    web::{DayData, ProjectSummary, WeekData, get_day_data_impl},
};

pub struct Query;

#[juniper::graphql_object(Context = GraphQLContext)]
impl Query {
    #[graphql(name = "test")]
    pub async fn test(_context: &GraphQLContext) -> FieldResult<String> {
        Ok("Hello, GraphQL!".to_string())
    }

    #[graphql(name = "dataForDate")]
    pub async fn data_for_date(context: &GraphQLContext, date: String) -> FieldResult<DayData> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        get_day_data_impl(date, state).await.map_err(|e| e.into())
    }

    #[graphql(name = "fileContentForDate")]
    pub async fn file_content_for_date(
        context: &GraphQLContext,
        date: String,
    ) -> FieldResult<String> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        let time_tracking_dir =
            get_time_tracking_dir_with_override(state.config.data_directory.as_deref())
                .map_err(|e| format!("Failed to get time tracking directory: {}", e))?;
        let file_path = time_tracking_dir.join(format!("{}.md", date.format(DATE_FORMAT).unwrap()));

        if !file_path.exists() {
            // Create file with template content if it doesn't exist
            let template_content =
                create_template_content(&date, state.config.template_file.as_deref())
                    .map_err(|e| format!("Failed to create template content: {}", e))?;
            fs::write(&file_path, &template_content)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            return Ok(template_content);
        }

        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e).into())
    }

    #[graphql(name = "weekDataForDate")]
    pub async fn week_data_for_date(
        context: &GraphQLContext,
        date: String,
        week_start_day: Option<String>,
    ) -> FieldResult<WeekData> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        let week_start_day = week_start_day
            .or_else(|| state.config.week_start_day.clone())
            .unwrap_or_else(|| "Saturday".to_string());

        let week_start_weekday =
            parse_weekday(&week_start_day).map_err(|e| format!("Invalid week start day: {}", e))?;

        let week_dates = get_week_dates(&date, week_start_weekday);

        let mut total_week_hours = 0.0;
        let mut total_dead_hours = 0.0;
        let mut week_projects: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
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

        Ok(WeekData {
            start_date: week_dates[0].format(DATE_FORMAT).unwrap(),
            end_date: week_dates[6].format(DATE_FORMAT).unwrap(),
            total_hours: total_week_hours,
            dead_time_hours: total_dead_hours,
            days,
            project_summaries,
        })
    }
}

pub struct Mutation;

#[juniper::graphql_object(Context = GraphQLContext)]
impl Mutation {
    #[graphql(name = "testMutation")]
    pub async fn test_mutation(_context: &GraphQLContext) -> FieldResult<String> {
        Ok("Hello from Mutation!".to_string())
    }

    #[graphql(name = "updateFileContent")]
    pub async fn update_file_content(
        context: &GraphQLContext,
        date: String,
        content: String,
    ) -> FieldResult<String> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        let time_tracking_dir =
            get_time_tracking_dir_with_override(state.config.data_directory.as_deref())
                .map_err(|e| format!("Failed to get time tracking directory: {}", e))?;

        // Create directory if it doesn't exist
        fs::create_dir_all(&time_tracking_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let file_path = time_tracking_dir.join(format!("{}.md", date.format(DATE_FORMAT).unwrap()));

        fs::write(&file_path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(format!(
            "Successfully updated file for date {}",
            date.format(DATE_FORMAT).unwrap()
        ))
    }
}

pub type Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

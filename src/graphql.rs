use juniper::{EmptySubscription, FieldResult, RootNode};
use time::Date;
use tokio::fs;

use crate::{
    DATE_FORMAT,
    context::GraphQLContext,
    create_template_content, get_time_tracking_dir, get_week_dates, parse_weekday,
    web::{DayData, WeekData, aggregate_week_days, get_day_data_impl},
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

        let time_tracking_dir = get_time_tracking_dir()
            .map_err(|e| format!("Failed to get time tracking directory: {}", e))?;
        let file_path = time_tracking_dir.join(format!("{}.md", date.format(DATE_FORMAT).unwrap()));

        if !file_path.exists() {
            // Create file with template content if it doesn't exist
            let template_content =
                create_template_content(&date, state.config.template_file.as_deref())
                    .await
                    .map_err(|e| format!("Failed to create template content: {}", e))?;
            fs::write(&file_path, &template_content)
                .await
                .map_err(|e| format!("Failed to create file: {}", e))?;
            return Ok(template_content);
        }

        fs::read_to_string(&file_path)
            .await
            .map_err(|e| format!("Failed to read file: {}", e).into())
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

        let (days, project_summaries, total_week_hours, total_dead_hours) =
            aggregate_week_days(&week_dates, state).await;

        let start_date = week_dates
            .first()
            .ok_or("week_dates is empty")?
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format start date: {e}"))?;
        let end_date = week_dates
            .last()
            .ok_or("week_dates is empty")?
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format end date: {e}"))?;

        Ok(WeekData {
            start_date,
            end_date,
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
        _context: &GraphQLContext,
        date: String,
        content: String,
    ) -> FieldResult<String> {
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        let time_tracking_dir = get_time_tracking_dir()
            .map_err(|e| format!("Failed to get time tracking directory: {}", e))?;

        // Create directory if it doesn't exist
        fs::create_dir_all(&time_tracking_dir)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let date_str = date
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format date: {e}"))?;
        let file_path = time_tracking_dir.join(format!("{}.md", date_str));

        fs::write(&file_path, &content)
            .await
            .map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(format!("Successfully updated file for date {}", date_str))
    }
}

pub type Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

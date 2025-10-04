use chrono::NaiveDate;
use juniper::{EmptySubscription, FieldResult, RootNode};

use crate::{
    context::GraphQLContext,
    web::{DayData, get_day_data_impl},
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
        let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| "Invalid date format, expected YYYY-MM-DD")?;

        get_day_data_impl(date, state).await.map_err(|e| e.into())
    }
}

pub struct Mutation;

#[juniper::graphql_object(Context = GraphQLContext)]
impl Mutation {
    #[graphql(name = "testMutation")]
    pub async fn test_mutation(_context: &GraphQLContext) -> FieldResult<String> {
        Ok("Hello from Mutation!".to_string())
    }
}

pub type Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

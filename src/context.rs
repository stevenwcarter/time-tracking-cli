use crate::web::AppState;

#[derive(Clone, Default)]
pub struct GraphQLContext {
    pub app_state: AppState,
}

impl GraphQLContext {
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }
}

impl juniper::Context for GraphQLContext {}

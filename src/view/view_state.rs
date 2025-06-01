use super::main_view::Route;

pub struct ViewState {
    pub route: Route,
    pub plugin_selected: Option<String>,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
            route: Route::Main,
            plugin_selected: None,
        }
    }
}

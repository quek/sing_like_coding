pub struct ViewState {
    pub plugin_selected: Option<String>,
}

impl ViewState {
    pub fn new() -> Self {
        Self {
            plugin_selected: None,
        }
    }
}

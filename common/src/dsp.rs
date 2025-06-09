pub fn linear_to_db(val: f32) -> f32 {
    20.0 * val.max(1e-20).log10()
}

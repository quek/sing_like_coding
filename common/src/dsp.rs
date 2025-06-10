const DB_CURVE_EXPONENT: f32 = 2.0;

pub fn db_to_norm(db: f32, min_db: f32, max_db: f32) -> f32 {
    let db = db.clamp(min_db, max_db);
    let t = (db - min_db) / (max_db - min_db);
    t.powf(DB_CURVE_EXPONENT)
}

pub fn db_from_norm(norm: f32, min_db: f32, max_db: f32) -> f32 {
    let norm = norm.clamp(0.0, 1.0);
    let t = norm.powf(1.0 / DB_CURVE_EXPONENT);
    min_db + t * (max_db - min_db)
}

pub fn linear_to_db(val: f32) -> f32 {
    20.0 * val.max(1e-20).log10()
}

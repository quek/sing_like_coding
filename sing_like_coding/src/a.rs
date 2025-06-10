fn main() {
    let min_db = -60.0;
    let max_db = 6.0;
    for &db in &[-60.0, -30.0, -18.0, -9.0, 0.0, 3.0, 6.0] {
        let norm = normalize_db(db, min_db, max_db);
        println!("db: {:>5} -> norm: {:.3}", db, norm);
    }
}

fn normalize_db(db: f32, min_db: f32, max_db: f32) -> f32 {
    if db >= 0.0 {
        // +6〜0dB → 0.8〜1.0
        0.8 + 0.2 * (db / (max_db))
    } else if db >= -18.0 {
        // 0〜-18dB → 0.4〜0.8
        0.8 + 0.4 * (db / 18.0) // db = -18 → 0.8 - 0.4 = 0.4
    } else {
        // -18〜-60dB → 0.0〜0.4
        0.4 * ((db - min_db) / (-18.0 - min_db)) // db = -60 → 0.0, db = -18 → 0.4
    }
}

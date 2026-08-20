//! Seed app data loader.
//!
//! Loads the pre-seeded sample apps from `seed_data.json` (embedded in the binary
//! via `include_str!`). The JSON file is the single source of truth for seed apps;
//! no data is hardcoded in Rust code.
//!
//! Seed apps are registered into the database on first startup (or when new seed apps
//! are added to the JSON file). They serve as demo apps in the marketplace.

use crate::MarketApp;

/// Seed app entry from the JSON file (minimal fields for registration).
#[derive(serde::Deserialize)]
struct SeedEntry {
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    platforms: Vec<String>,
    template: String,
    version: String,
    owner: String,
    visibility: String,
    category: String,
    net: String,
}

/// Generate a stable pseudo-random launch count for seed apps.
fn rand_launches(min: u64, max: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seed = (now % 7919) as u64;
    min + (seed % (max - min + 1))
}

/// Load seed apps from the embedded `seed_data.json` file.
pub(crate) fn load_seed_apps() -> Vec<MarketApp> {
    let data: Vec<SeedEntry> =
        serde_json::from_str(include_str!("seed_data.json"))
            .expect("Failed to parse seed_data.json");

    data.into_iter().map(|e| MarketApp {
        id: e.id,
        name: e.name,
        description: e.description,
        tags: e.tags,
        platforms: e.platforms,
        template: e.template,
        source: String::new(),
        created_at: "2026-08-18 10:00".into(),
        version: e.version,
        owner: e.owner,
        visibility: e.visibility.clone(),
        status: if e.visibility == "public" { "published".into() } else { "draft".into() },
        launches: rand_launches(1000, 50000),
        report: String::new(),
        review_note: String::new(),
        wasm: None,
        tier: "open".into(),
        category: e.category,
        share: "public".into(),
        net: e.net,
        kind: "app".into(),
        hide_branding: false,
    })
    .collect()
}
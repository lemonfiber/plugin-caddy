//! The manifest, as the core at the pinned commit reads it.

use toml::{Table, Value};

fn manifest() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/plugin.toml")).unwrap_or_default()
}

/// The digest of the image each service the manifest's adapters front runs.
fn fronted_digests(manifest: &Table) -> Vec<String> {
    let services: Vec<&Table> = manifest
        .get("service")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
        .collect();
    services
        .iter()
        .filter_map(|service| service.get("fronts").and_then(Value::as_str))
        .filter_map(|fronted| {
            services
                .iter()
                .find(|service| service.get("id").and_then(Value::as_str) == Some(fronted))
        })
        .filter_map(|service| service.get("digest").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_pinned_core_refuses_nothing_in_the_manifest() {
    let read: Result<lemonfiber_plugin::Manifest, _> = toml::from_str(&manifest());
    let refused: Vec<String> = match read {
        Ok(manifest) => lemonfiber_plugin::refusals(&manifest, &[])
            .into_iter()
            .map(|violation| format!("{}: {}", violation.location, violation.message))
            .collect(),
        Err(why) => vec![why.to_string()],
    };
    assert!(refused.is_empty(), "{refused:#?}");
}

#[test]
fn the_adapter_names_the_image_of_the_caddy_it_fronts() {
    let named: Vec<String> = caddy_adapter::adapter::about()
        .releases
        .into_iter()
        .map(|release| release.digest)
        .collect();
    let fronted = fronted_digests(&manifest().parse().unwrap_or_default());
    assert_eq!(fronted, vec![caddy_adapter::adapter::DIGEST.to_owned()]);
    assert_eq!(named, fronted);
}

/// The service of the manifest named `id`.
fn service(manifest: &Table, id: &str) -> Table {
    manifest
        .get("service")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
        .find(|service| service.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
        .unwrap_or_default()
}

#[test]
fn caddy_reads_its_configuration_and_writes_none_of_it() {
    let caddy = service(&manifest().parse().unwrap_or_default(), "caddy");

    assert_eq!(caddy.get("config_ro").and_then(Value::as_bool), Some(true));
}

#[test]
fn caddys_admin_api_is_off_in_every_configuration_it_can_start_from() {
    let starting: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/config/caddy/caddy.json"
        ))
        .unwrap_or_default(),
    )
    .unwrap_or_default();
    let written = caddy_adapter::caddy::config::rendered(&lemonfiber_ports::service::Routing {
        tls: lemonfiber_ports::service::Tls::Automatic,
        routes: Vec::new(),
    })
    .unwrap_or_default();

    for configuration in [starting, written] {
        assert_eq!(
            configuration.get("admin"),
            Some(&serde_json::json!({ "disabled": true })),
            "{configuration}"
        );
    }
}

#[test]
fn caddy_starts_from_no_caddyfile() {
    assert!(!std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/config/caddy/Caddyfile"
    ))
    .exists());
}

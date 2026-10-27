use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::json;

use super::{loaded, served, Refused, ADMIN_LISTEN};

fn route(name: &str, host: &str, port: u16) -> Route {
    Route {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
    }
}

fn table(tls: Tls) -> Routing {
    Routing {
        tls,
        routes: vec![
            route("watch.home.example", "door", 8096),
            route("requests.home.example", "seerr", 5055),
        ],
    }
}

fn none() -> Routing {
    Routing {
        tls: Tls::Automatic,
        routes: Vec::new(),
    }
}

#[test]
fn a_table_reads_back_from_the_configuration_it_is_loaded_as() {
    for tls in [
        Tls::Automatic,
        Tls::Files {
            certificate: "certificates/house.crt".to_owned(),
            key: "certificates/house.key".to_owned(),
        },
    ] {
        let table = table(tls);
        let config = loaded(&table);
        assert!(config.is_ok());
        let Ok(config) = config else { return };

        assert_eq!(served(&config), Ok(table));
    }
}

#[test]
fn the_configuration_keeps_the_admin_api_on_its_socket_and_names_each_route_once() {
    let config = loaded(&table(Tls::Automatic));
    assert!(config.is_ok());
    let Ok(config) = config else { return };

    assert_eq!(config.pointer("/admin/listen"), Some(&json!(ADMIN_LISTEN)));
    assert_eq!(
        config.pointer("/apps/http/servers/lemonfiber/routes/0/match/0/host"),
        Some(&json!(["watch.home.example"]))
    );
    assert_eq!(
        config.pointer("/apps/http/servers/lemonfiber/routes/1/handle/0/upstreams/0/dial"),
        Some(&json!("seerr:5055"))
    );
    assert_eq!(config.pointer("/apps/tls"), None);
}

#[test]
fn files_are_loaded_from_the_proxy_configuration_directory_and_nothing_is_fetched() {
    let config = loaded(&table(Tls::Files {
        certificate: "house.crt".to_owned(),
        key: "house.key".to_owned(),
    }));
    assert!(config.is_ok());
    let Ok(config) = config else { return };

    assert_eq!(
        config.pointer("/apps/tls/certificates/load_files/0/certificate"),
        Some(&json!("/etc/caddy/house.crt"))
    );
    assert_eq!(
        config.pointer("/apps/http/servers/lemonfiber/automatic_https/disable_certificates"),
        Some(&json!(true))
    );
}

#[test]
fn a_file_outside_the_configuration_directory_is_refused() {
    for file in ["", "/etc/passwd", "../secrets", "a/../b", "a//b", "./a"] {
        let refused = loaded(&table(Tls::Files {
            certificate: file.to_owned(),
            key: "house.key".to_owned(),
        }));
        assert_eq!(refused, Err(Refused::Outside(file.to_owned())), "{file:?}");
    }
}

#[test]
fn a_name_or_host_that_is_not_a_plain_name_is_refused() {
    for name in ["", "-a.example", ".a.example", "a b", "a/b", "a:80", "a\"b"] {
        let mut named = table(Tls::Automatic);
        named.routes = vec![route(name, "door", 8096)];
        assert_eq!(
            loaded(&named),
            Err(Refused::Unnamed(name.to_owned())),
            "{name:?}"
        );

        let mut hosted = table(Tls::Automatic);
        hosted.routes = vec![route("a.example", name, 8096)];
        assert_eq!(
            loaded(&hosted),
            Err(Refused::Unnamed(name.to_owned())),
            "{name:?}"
        );
    }
}

#[test]
fn a_caddy_running_no_table_serves_none() {
    assert_eq!(served(&json!(null)), Ok(none()));
    assert_eq!(
        served(&json!({"apps": {"http": {"servers": {"lemonfiber": {}}}}})),
        Ok(none())
    );
}

#[test]
fn a_configuration_in_no_shape_a_table_is_loaded_as_is_unread() {
    for routes in [
        json!([{}]),
        json!([{"match": [{"host": ["a.example"]}]}]),
        json!([{"match": [{"host": ["a.example"]}], "handle": [{"upstreams": [{"dial": "door"}]}]}]),
        json!([{"match": [{"host": ["a.example"]}], "handle": [{"upstreams": [{"dial": "door:http"}]}]}]),
    ] {
        let config = json!({"apps": {"http": {"servers": {"lemonfiber": {"routes": routes}}}}});
        assert!(
            matches!(served(&config), Err(Refused::Unread(_))),
            "{config}"
        );
    }
    for files in [
        json!({"certificate": "/elsewhere/a.crt", "key": "/etc/caddy/a.key"}),
        json!({"certificate": "/etc/caddy/a.crt"}),
    ] {
        let config = json!({"apps": {
            "http": {"servers": {"lemonfiber": {"routes": []}}},
            "tls": {"certificates": {"load_files": [files]}},
        }});
        assert!(
            matches!(served(&config), Err(Refused::Unread(_))),
            "{config}"
        );
    }
}

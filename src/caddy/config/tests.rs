use lemonfiber_ports::service::{Route, Routing, Tls};
use serde_json::json;

use super::{rendered, Refused};

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

fn files(certificate: &str, key: &str) -> Tls {
    Tls::Files {
        certificate: certificate.to_owned(),
        key: key.to_owned(),
    }
}

/// Every attempt to carry a directive, a placeholder or a block of its own through a
/// value, each of which has to be refused before anything is written.
const INJECTIONS: [&str; 16] = [
    "",
    "a b",
    "a\nb",
    "a\r\nb",
    "a\tb",
    "a}\n:80 {\n\tfile_server",
    "a { admin localhost:2019 }",
    "{$DOMAIN}",
    "{env.HOME}",
    "home.{$DOMAIN:home.local}",
    "import /etc/passwd",
    "# a comment",
    "a\"b",
    "a'b",
    "a;b",
    "a\u{0}b",
];

#[test]
fn every_route_is_matched_on_its_name_and_handed_to_its_service_with_admin_off() {
    let config = rendered(&table(Tls::Automatic));
    assert!(config.is_ok());
    let config = config.unwrap_or_default();

    assert_eq!(config.pointer("/admin"), Some(&json!({ "disabled": true })));
    let server = config.pointer("/apps/http/servers/lemonfiber");
    assert_eq!(
        server.and_then(|server| server.pointer("/listen")),
        Some(&json!([":8443"]))
    );
    assert_eq!(
        server.and_then(|server| server.pointer("/routes/0/match/0/host")),
        Some(&json!(["watch.home.example"]))
    );
    assert_eq!(
        server.and_then(|server| server.pointer("/routes/1/handle/0/upstreams/0/dial")),
        Some(&json!("seerr:5055"))
    );
    assert_eq!(config.pointer("/apps/tls"), None);
}

#[test]
fn certificates_load_from_files_beneath_the_configuration_with_issuing_off() {
    let config = rendered(&table(files(
        "certificates/house.crt",
        "certificates/house.key",
    )))
    .unwrap_or_default();

    assert_eq!(
        config.pointer("/apps/tls/certificates/load_files"),
        Some(&json!([{
            "certificate": "/etc/caddy/certificates/house.crt",
            "key": "/etc/caddy/certificates/house.key",
        }]))
    );
    assert_eq!(
        config.pointer("/apps/http/servers/lemonfiber/automatic_https"),
        Some(&json!({ "disable_certificates": true }))
    );
}

#[test]
fn a_host_may_be_an_ipv4_address() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![route("watch.home.example", "192.0.2.7", 8096)],
    };

    assert!(rendered(&routing).is_ok());
}

#[test]
fn a_name_two_routes_answer_on_is_refused_rather_than_the_last_taking_it() {
    for second in ["watch.home.example", "WATCH.home.example"] {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![
                route("watch.home.example", "door", 8096),
                route(second, "elsewhere", 80),
            ],
        };
        assert_eq!(rendered(&routing), Err(Refused::Twice(second.to_owned())));
    }
}

#[test]
fn two_names_may_reach_one_service() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![
            route("watch.home.example", "door", 8096),
            route("tv.home.example", "door", 8096),
        ],
    };

    assert!(rendered(&routing).is_ok());
}

#[test]
fn a_wildcard_that_would_answer_for_another_routes_name_is_refused() {
    for wildcard in ["*.home.example", "*.example", "*"] {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![
                route(wildcard, "elsewhere", 80),
                route("watch.home.example", "door", 8096),
            ],
        };
        assert_eq!(
            rendered(&routing),
            Err(Refused::Unnamed(wildcard.to_owned()))
        );
    }
}

#[test]
fn every_route_answers_on_its_own_name_alone() {
    let config = rendered(&table(Tls::Automatic)).unwrap_or_default();
    let routes = config
        .pointer("/apps/http/servers/lemonfiber/routes")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    let hosts: Vec<serde_json::Value> = routes
        .iter()
        .filter_map(|route| route.pointer("/match/0/host").cloned())
        .collect();
    assert_eq!(
        hosts,
        [
            json!(["watch.home.example"]),
            json!(["requests.home.example"])
        ]
    );
    assert!(routes.iter().all(|route| route
        .pointer("/match")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        == Some(1)));
}

#[test]
fn a_name_carrying_anything_but_a_dns_name_is_refused() {
    let mut names: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    names.extend(
        [
            ".lead",
            "trail.",
            "-a",
            "a-",
            "a..b",
            "*",
            "a.*.b",
            "**.a",
            "1.2.3.4:80",
        ]
        .map(str::to_owned),
    );
    names.push("a".repeat(64));
    names.push(format!("{}.a", "a.".repeat(127)));
    for name in names {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![route(&name, "door", 8096)],
        };
        assert_eq!(rendered(&routing), Err(Refused::Unnamed(name)));
    }
}

#[test]
fn a_host_carrying_anything_but_a_dns_name_or_an_ipv4_address_is_refused() {
    let mut hosts: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    hosts.extend(["*.door", "door:80", "[::1]", "::1", "256.0.0.1", "door/x"].map(str::to_owned));
    for host in hosts {
        let routing = Routing {
            tls: Tls::Automatic,
            routes: vec![route("watch.home.example", &host, 8096)],
        };
        assert_eq!(rendered(&routing), Err(Refused::Unhosted(host)));
    }
}

#[test]
fn port_zero_is_refused() {
    let routing = Routing {
        tls: Tls::Automatic,
        routes: vec![route("watch.home.example", "door", 0)],
    };

    assert_eq!(rendered(&routing), Err(Refused::Unported(0)));
}

#[test]
fn a_certificate_that_is_not_a_plain_file_beneath_the_configuration_is_refused() {
    let mut paths: Vec<String> = INJECTIONS.iter().map(|one| (*one).to_owned()).collect();
    paths.extend(
        [
            "/etc/passwd",
            "../key",
            "certificates//key",
            "./key",
            ".hidden",
            "a/../../b",
            "caddy.json",
            "routing.json",
            "a/b/c/d/e",
            "a{b}",
            "a$b",
        ]
        .map(str::to_owned),
    );
    paths.push("a".repeat(65));
    for path in paths {
        assert_eq!(
            rendered(&table(files(&path, "house.key"))),
            Err(Refused::Outside(path.clone()))
        );
        assert_eq!(
            rendered(&table(files("house.crt", &path))),
            Err(Refused::Outside(path))
        );
    }
}

#[test]
fn whatever_is_accepted_holds_no_character_a_directive_or_placeholder_needs() {
    let alphabet: Vec<char> = "ab-.*{}$ \n#\"/:_".chars().collect();
    let mut accepted = 0;
    for seed in 0..20_000_u32 {
        let mut value = String::new();
        let mut at = seed;
        for _ in 0..6 {
            let index = usize::try_from(at % 16).unwrap_or_default();
            value.push(alphabet.get(index).copied().unwrap_or('a'));
            at /= 16;
        }
        let routing = Routing {
            tls: files(&value, "house.key"),
            routes: vec![route(&value, &value, 8096)],
        };
        if let Ok(config) = rendered(&routing) {
            accepted += 1;
            let text = config.to_string();
            assert!(
                !value.contains(['{', '}', '$', ' ', '\n', '#', '"', ':', '/']),
                "{value:?} reached {text}"
            );
        }
    }
    assert!(accepted > 0);
}

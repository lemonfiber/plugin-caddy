use lemonfiber_ports::service::{Route, Routing, Tls};

use super::{rendered, Refused, WRITTEN};

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

#[test]
fn every_route_is_a_site_handed_to_its_service_under_the_admin_api_kept_off() {
    assert_eq!(
        rendered(&table(Tls::Automatic)),
        Ok(format!(
            "{WRITTEN}\n\n{{\n\tadmin off\n\tpersist_config off\n\thttp_port 8080\n\thttps_port 8443\n}}\n\
             \nwatch.home.example {{\n\treverse_proxy door:8096\n}}\n\
             \nrequests.home.example {{\n\treverse_proxy seerr:5055\n}}\n"
        ))
    );
}

#[test]
fn each_site_serves_the_certificate_files_beneath_the_proxys_configuration() {
    let text = rendered(&table(Tls::Files {
        certificate: "certificates/house.crt".to_owned(),
        key: "certificates/house.key".to_owned(),
    }));
    assert!(text.is_ok());
    let text = text.unwrap_or_default();

    assert_eq!(
        text.matches("\ttls /etc/caddy/certificates/house.crt /etc/caddy/certificates/house.key\n")
            .count(),
        2
    );
}

#[test]
fn a_table_with_no_route_keeps_the_admin_api_off_and_serves_nothing() {
    let none = Routing {
        tls: Tls::Automatic,
        routes: Vec::new(),
    };

    let text = rendered(&none).unwrap_or_default();

    assert!(text.contains("\tadmin off\n"));
    assert!(!text.contains("reverse_proxy"));
}

#[test]
fn a_certificate_outside_the_proxys_configuration_is_refused() {
    for file in ["", "/etc/passwd", "../key", "certificates//key", "./key"] {
        let table = table(Tls::Files {
            certificate: file.to_owned(),
            key: "house.key".to_owned(),
        });
        assert_eq!(rendered(&table), Err(Refused::Outside(file.to_owned())));
    }
}

#[test]
fn a_name_or_host_that_is_not_plain_is_refused() {
    for bad in ["", "a b", ".hidden", "-dash", "home{$DOMAIN}", "a\n}"] {
        let named = Routing {
            tls: Tls::Automatic,
            routes: vec![route(bad, "door", 8096)],
        };
        assert_eq!(rendered(&named), Err(Refused::Unnamed(bad.to_owned())));
        let hosted = Routing {
            tls: Tls::Automatic,
            routes: vec![route("watch.home.example", bad, 8096)],
        };
        assert_eq!(rendered(&hosted), Err(Refused::Unnamed(bad.to_owned())));
    }
}

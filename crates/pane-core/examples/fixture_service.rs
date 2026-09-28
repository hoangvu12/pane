//! Serves the fixture service, the made-up package registry the package
//! search samples query, on 127.0.0.1 until stopped, printing each request
//! it receives. It never reaches beyond this computer.
//!
//! Usage: `cargo run -p pane-core --example fixture_service [-- --port N]`
//! (default port 8740, the samples' default address).

#[path = "../tests/support/service.rs"]
mod service;

use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut port = 8740;
    while let Some(arg) = args.next() {
        match (
            arg.as_str(),
            args.next().and_then(|value| value.parse().ok()),
        ) {
            ("--port", Some(value)) => port = value,
            _ => {
                eprintln!("usage: fixture_service [--port N]");
                std::process::exit(2);
            }
        }
    }
    let service = match service::Service::start_on(port) {
        Ok(service) => service,
        Err(error) => {
            eprintln!("fixture service: cannot listen on 127.0.0.1:{port}: {error}");
            std::process::exit(1);
        }
    };
    println!("fixture service listening on {}", service.url());
    let mut printed = 0;
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let requests = service.requests();
        for request in &requests[printed..] {
            println!("GET {request}");
        }
        printed = requests.len();
    }
}

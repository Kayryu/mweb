mod web;

use web::{Handler, WebServer};

fn main() {
    println!("Hello, world!");
    env_logger::init();

    let handler = Handler {};
    let server = WebServer::new(handler);
    server.launch();
}

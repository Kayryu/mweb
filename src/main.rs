mod web;

use web::{Handler, WebServer};

fn main() {
    println!("Hello, world!");

    let handler = Handler {};
    let server = WebServer::new(handler);
    server.launch();
}

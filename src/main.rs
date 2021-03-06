mod web;

use web::{Handler, WebServer};

fn main() {
    println!("Hello, world!");
    env_logger::init();

    let handler = Handler {};
    let server = WebServer::new(handler);
    server.launch();
}

#[cfg(test)]
mod tests {
    use http_req::{request::{RequestBuilder, Method}, tls, uri::Uri, request};
    use std::{convert::TryFrom, net::TcpStream};

    #[test]
    #[ignore = "manual"]
    fn client() {
        let mut writer = Vec::new(); //container for body of a response
        const BODY: &[u8; 27] = b"field1=value1&field2=value2";
        let res = request::post("https://localhost:8443/command", BODY, &mut writer).unwrap();

        println!("Status: {} {}", res.status_code(), res.reason());
        println!("Headers {}", res.headers());
    }

    #[test]
    fn tls_client() {
        //Parse uri and assign it to variable `addr`
        let addr: Uri = Uri::try_from("https://localhost:8443/command").unwrap();

        //Connect to remote host
        let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();

        let mut tls = tls::Config::default();
        tls.add_root_cert_file_pem(std::path::Path::new("ca_cert.pem")).unwrap();
        println!("tls load success");
        let mut stream = tls.connect(addr.host().unwrap_or(""), stream)
            .unwrap();

        println!("tls bind success");
        //Container for response's body
        let mut writer = Vec::new();

        //Add header `Connection: Close`
        let response = RequestBuilder::new(&addr)
            .method(Method::POST)
            .header("Connection", "Close")
            .send(&mut stream, &mut writer)
            .unwrap();

        println!("Status: {} {}", response.status_code(), response.reason());
        //println!("{}", String::from_utf8_lossy(&writer));
    }
}
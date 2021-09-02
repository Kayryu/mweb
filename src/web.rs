use http::{Method, Request, Response, Version};
use log::{debug, error, info};
use std::io::{Read, Write};
use std::net::{TcpListener};
use std::str;

trait Flat {
    fn flat(&self) -> Vec<u8>;
}

impl<T> Flat for Response<T>
where
    T: AsRef<[u8]>,
{
    fn flat(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        let status = self.status();
        let s = format!(
            "HTTP/1.1 {} {}\r\n",
            status.as_str(),
            status.canonical_reason().unwrap_or("Unsupported Status")
        );
        data.extend_from_slice(&s.as_bytes());
        for (key, value) in self.headers().iter() {
            data.extend_from_slice(key.as_str().as_bytes());
            data.extend_from_slice(b": ");
            data.extend_from_slice(value.as_bytes());
            data.extend_from_slice(b"\r\n");
        }

        data.extend_from_slice(b"\r\n");
        data.extend_from_slice(self.body().as_ref());
        return data;
    }
}

trait Wrapper<T> {
    fn json(data: T) -> Self;
}

impl<T> Wrapper<T> for Response<T>
where
    T: AsRef<[u8]>,
{
    fn json(data: T) -> Self {
        let response = Response::builder()
            .header("Connection", "close")
            .status(200)
            .header("content-type", "application/json")
            .header("content-length", data.as_ref().len())
            .body(data)
            .unwrap();
        return response;
    }
}

pub struct Handler {}

const NOT_FOUND: &[u8] = b"html";

impl Handler {
    pub fn process(&self, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
        debug!("Request Method {}, uri {}", req.method(), req.uri());
        match *req.method() {
            Method::POST => {
                match req.uri().path() {
                    "/command" => {
                        // response
                        let message = b"hello world from server\r\n";
                        let response: Response<Vec<u8>> = Response::builder()
                            .header("Connection", "close")
                            .body(message.to_vec())
                            .unwrap();
                        return response;
                    }
                    _ => {
                        return Response::builder()
                            .status(400)
                            .body(NOT_FOUND.to_vec())
                            .unwrap();
                    }
                }
            }
            _ => {
                return Response::builder()
                    .status(404)
                    .body(NOT_FOUND.to_vec())
                    .unwrap();
            }
        };
    }
}

pub struct WebServer {
    handler: Handler,
}

impl WebServer {
    pub fn new(handler: Handler) -> Self {
        WebServer { handler }
    }

    pub fn parse(&self, plaintext: &Vec<u8>) -> Result<Request<Vec<u8>>, ()> {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut parse_req = httparse::Request::new(&mut headers);

        match parse_req.parse(&plaintext) {
            Ok(httparse::Status::Complete(parsed_len)) => {
                debug!("Request.parse Complete({})", parsed_len);

                // content-type | content-length

                // copy to http:Request
                let mut rb = Request::builder()
                    .method(parse_req.method.unwrap())
                    .version(Version::HTTP_11)
                    .uri(parse_req.path.unwrap());

                for header in parse_req.headers {
                    rb = rb.header(header.name.clone(), header.value.clone());
                }
                let (_headers, body) = plaintext.split_at(parsed_len);

                debug!("body {}", str::from_utf8(body).unwrap());
                let response = rb.body(body.to_vec()).unwrap();
                return Ok(response);
            }
            Ok(httparse::Status::Partial) => return Ok(Request::default()),
            Err(e) => {
                error!("e : {}", e.to_string());
                return Err(());
            }
        };
    }

    pub fn launch(&self) {
        let listener = TcpListener::bind("0.0.0.0:8443").unwrap();
        loop {
            match listener.accept() {
                Ok((mut socket, addr)) => {
                    info!("new client from {:?}", addr);

                    let mut plaintext = [0u8; 1024]; //Vec::new();
                    match socket.read(&mut plaintext) {
                        Ok(_) => {
                            let request = self.parse(&plaintext.to_vec()).unwrap();
                            debug!("request :{:?}", request);

                            let response = self.handler.process(request);

                            let data = response.flat();
                            // response to vec.
                            socket.write(&data).unwrap();
                        }
                        Err(e) => {
                            error!("Error in read_to_end: {:?}", e);
                        }
                    }
                }
                Err(e) => error!("couldn't get client: {:?}", e),
            }
        }
    }
}

use http::{Method, Request, Response, Version};
use log::{debug, error, info, warn, trace};
use rustls::ServerConfig;
use std::io::{Read, Write, BufReader};
use std::net::TcpListener;
use std::str;
use std::sync::Arc;
use std::fs;

fn header_flat<T>(res: &Response<T>) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();
    let status = res.status();
    let s = format!(
        "HTTP/1.1 {} {}\r\n",
        status.as_str(),
        status.canonical_reason().unwrap_or("Unsupported Status")
    );
    data.extend_from_slice(&s.as_bytes());
    for (key, value) in res.headers().iter() {
        data.extend_from_slice(key.as_str().as_bytes());
        data.extend_from_slice(b": ");
        data.extend_from_slice(value.as_bytes());
        data.extend_from_slice(b"\r\n");
    }

    data.extend_from_slice(b"\r\n");
    data
}

trait Flat {
    fn flat(&self) -> Vec<u8>;
}

impl<T> Flat for Response<T>
where
    T: AsRef<[u8]>,
{
    fn flat(&self) -> Vec<u8> {
        let mut data = header_flat(&self);
        data.extend_from_slice(self.body().as_ref());
        return data;
    }
}

const NOT_FOUND: &[u8] = b"html";

trait ResponseExt<T> {
    fn e100(data: T) -> Self;
    fn e404(data: T) -> Self;
    fn json(data: T) -> Self;
    fn html(content: T) -> Self;
}

impl<T> ResponseExt<T> for Response<T>
where
    T: AsRef<[u8]>,
{
    fn e100(e: T) -> Self {
        let response = Response::builder().status(100).body(e).unwrap();
        return response;
    }

    fn e404(e: T) -> Self {
        let response = Response::builder().status(404).body(e).unwrap();
        return response;
    }
    fn json(content: T) -> Self {
        let response = Response::builder()
            .status(200)
            .version(Version::HTTP_11)
            // .header("connection", "close")
            .header("content-type", "application/json")
            .header("content-length", content.as_ref().len())
            .body(content)
            .unwrap();
        return response;
    }

    fn html(content: T) -> Self {
        let response = Response::builder()
            .status(200)
            .header("content-type", "text/html; charset=utf-8")
            .header("content-length", content.as_ref().len())
            .body(content)
            .unwrap();
        return response;
    }
}

pub struct Handler {}

impl Handler {
    pub fn process(&self, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
        debug!("Request Method {}, uri {}", req.method(), req.uri());
        match *req.method() {
            Method::POST => {
                match req.uri().path() {
                    "/command" => {
                        // response
                        let message = b"hello world from server\r\n";
                        let response: Response<Vec<u8>> = Response::json(message.to_vec());
                        debug!("response :{:?}", response);
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
                error!("No matching routes for {} {}", req.method(), req.uri());
                return Response::builder()
                    .status(200)
                    .body(b"Hello mweb".to_vec())
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

    pub fn parse(&self, plaintext: &Vec<u8>) -> Result<(Request<Vec<u8>>, bool, usize), String> {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut parse_req = httparse::Request::new(&mut headers);

        match parse_req.parse(&plaintext) {
            Ok(httparse::Status::Complete(parsed_len)) => {
                debug!("Request.parse Complete({})", parsed_len);

                // content-type | content-length
                let mut content_length = None;
                let header = parse_req
                    .headers
                    .iter()
                    .find(|h| h.name.to_lowercase() == "content-length");
                if let Some(&h) = header {
                    content_length =
                        usize::from_str_radix(str::from_utf8(h.value).unwrap(), 10).ok();
                }

                // true if the client sent a `Expect: 100-continue` header
                let expects_continue: bool = match parse_req
                    .headers
                    .iter()
                    .find(|h| h.name.to_lowercase() == "expect")
                {
                    Some(header) => {
                        str::from_utf8(header.value).unwrap().to_lowercase() == "100-continue"
                    }
                    None => false,
                };

                // copy to http:Request
                let mut rb = Request::builder()
                    .method(parse_req.method.unwrap())
                    .version(Version::HTTP_11)
                    .uri(parse_req.path.unwrap());

                for header in parse_req.headers {
                    rb = rb.header(header.name.clone(), header.value.clone());
                }

                // find pos of body
                let (_headers, mut body) = plaintext.split_at(parsed_len);
                if let Some(len) = content_length {
                    if !expects_continue {
                        let (b, _) = body.split_at(len);
                        body = b;
                    } else {
                        let (a, _) = body.split_at(0);
                        body = a;
                    }
                }
                debug!("body {}", str::from_utf8(body).unwrap());

                let response = rb.body(body.to_vec()).unwrap();
                return Ok((response, expects_continue, content_length.unwrap_or_default()));
            }
            Ok(httparse::Status::Partial) => {
                warn!("httparse Status in Partial");
                return Ok((Request::default(), false, 0));
            }
            Err(e) => {
                error!("e : {}", e.to_string());
                return Err(e.to_string());
            }
        };
    }

    pub fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
        let certfile = fs::File::open(filename).expect("cannot open certificate file");
        let mut reader = BufReader::new(certfile);
        rustls::internal::pemfile::certs(&mut reader).unwrap()
    }

    pub fn load_private_key(filename: &str) -> rustls::PrivateKey {
        let rsa_keys = {
            let keyfile = fs::File::open(filename)
                .expect("cannot open private key file");
            let mut reader = BufReader::new(keyfile);
            rustls::internal::pemfile::rsa_private_keys(&mut reader)
                .expect("file contains invalid rsa private key")
        };
    
        let pkcs8_keys = {
            let keyfile = fs::File::open(filename)
                .expect("cannot open private key file");
            let mut reader = BufReader::new(keyfile);
            rustls::internal::pemfile::pkcs8_private_keys(&mut reader)
                .expect("file contains invalid pkcs8 private key (encrypted keys not supported)")
        };
    
        // prefer to load pkcs8 keys
        if !pkcs8_keys.is_empty() {
            pkcs8_keys[0].clone()
        } else {
            assert!(!rsa_keys.is_empty());
            rsa_keys[0].clone()
        }
    }

    pub fn make_config(&self) -> Arc<ServerConfig> {
        let mut cfg = rustls::ServerConfig::new(rustls::NoClientAuth::new());
        let mut versions = Vec::new();
        versions.push(rustls::ProtocolVersion::TLSv1_2);
        cfg.versions = versions;
        let certs = WebServer::load_certs("rsa_sha256_cert.pem");
        let privkey = WebServer::load_private_key("rsa_sha256_key.pem");
        cfg.set_single_cert_with_ocsp_and_sct(certs, privkey, vec![], vec![]).unwrap();
        let a_cfg = Arc::new(cfg);
        a_cfg
    }    

    pub fn launch(&self) {
        let listener = TcpListener::bind("0.0.0.0:8443").unwrap();

        let cfg = self.make_config();

        loop {
            match listener.accept() {
                Ok((mut socket, addr)) => {
                    info!("new client from {:?}", addr);

                    // add tls here.
                    let mut session = rustls::ServerSession::new(&cfg);
                    let mut socket = rustls::Stream::new(&mut session, &mut socket);

                    let mut plaintext = [0u8; 2048]; //Vec::new();
                    match socket.read(&mut plaintext) {
                        Ok(len) => {
                            debug!("receive data length: {}", len);
                            let data = match self.parse(&plaintext.to_vec()) {
                                Ok((mut request, expects, content_len)) => {
                                    if expects || len < content_len {
                                        let data = Response::e100(Vec::new()).flat();
                                        // response to continue.
                                        socket.write(&data).unwrap();
                                        // read all data;
                                        let mut body = vec![0u8; content_len];
                                        socket.read(&mut body).unwrap();
                                        let (parts, _) = request.into_parts();
                                        request = Request::from_parts(parts, body);
                                    }
                                    trace!("request :{:?}", request);
                                    self.handler.process(request).flat()
                                },
                                Err(e) => {
                                    e.as_bytes().to_vec()
                                }
                            };
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

# Mini HTTP Server

## Play

create self cert.
```
bash cert.sh rsa_sha256
```

Run server:
```
cargo run
```

Client call with `curl`:
```
curl -v --cacert ca_cert.pem -H "Accept: application/json" -H "Content-type: application/json" https://localhost:8443/command -X POST -d '{name:"alice"}'
```

Client call with test:
```
cargo test
```
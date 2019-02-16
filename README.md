
# hyper-reverse-proxy

[![Build Status](https://travis-ci.org/brendanzab/hyper-reverse-proxy.svg?branch=master)](https://travis-ci.org/brendanzab/hyper-reverse-proxy)
[![Documentation](https://docs.rs/hyper-reverse-proxy/badge.svg)](https://docs.rs/hyper-reverse-proxy)
[![Version](https://img.shields.io/crates/v/hyper-reverse-proxy.svg)](https://crates.io/crates/hyper-reverse-proxy)
[![License](https://img.shields.io/crates/l/hyper-reverse-proxy.svg)](https://github.com/brendanzab/hyper-reverse-proxy/blob/master/LICENSE)

A simple reverse proxy, to be used with [Hyper].

The implementation ensures that [Hop-by-hop headers] are stripped correctly in both directions,
and adds the client's IP address to a comma-space-separated list of forwarding addresses in the
`X-Forwarded-For` header.

The implementation is based on Go's [`httputil.ReverseProxy`].

[Hyper]: http://hyper.rs/
[Hop-by-hop headers]: http://www.w3.org/Protocols/rfc2616/rfc2616-sec13.html
[`httputil.ReverseProxy`]: https://golang.org/pkg/net/http/httputil/#ReverseProxy

# Example

Add these dependencies to your `Cargo.toml` file.

```
[dependencies]
hyper-reverse-proxy = "0.3.0"
hyper = "0.12.24"
futures = "0.1"
```

The following example will set up a reverse proxy listening on `127.0.0.1:13900`,
and will proxy these calls:

* `"/target/first"` will be proxied to `http://127.0.0.1:13901`

* `"/target/second"` will be proxied to `http://127.0.0.1:13902`

* All other URLs will be handled by `debug_request` function, that will display request information.

```rust,no_run
extern crate hyper;
extern crate hyper_reverse_proxy;
extern crate futures;

use hyper::server::conn::AddrStream;
use hyper::{Body, Request, Response, Server};
use hyper::service::{service_fn, make_service_fn};
use futures::future::{self, Future};

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

fn debug_request(req: Request<Body>) -> BoxFut {
    let body_str = format!("{:?}", req);
    let response = Response::new(Body::from(body_str));
    Box::new(future::ok(response))
}

fn main() {

    // This is our socket address...
    let addr = ([127, 0, 0, 1], 13900).into();

    // A `Service` is needed for every connection, so this
    // creates one from our `hello_world` function.
    let make_svc = make_service_fn(|socket: &AddrStream| {
        let remote_addr = socket.remote_addr();
        service_fn(move |req: Request<Body>| { // returns BoxFut

            // Auth
            if req.uri().path().starts_with("/target/first") {
                return hyper_reverse_proxy::call(remote_addr.ip(), "http://127.0.0.1:13901", req)
            } else if req.uri().path().starts_with("/target/second") {
                return hyper_reverse_proxy::call(remote_addr.ip(), "http://127.0.0.1:13902", req)
            } else {
                debug_request(req)
            }
        })
    });

    let server = Server::bind(&addr)
        .serve(make_svc)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Running server on {:?}", addr);

    // Run this server for... forever!
    hyper::rt::run(server);
}
```

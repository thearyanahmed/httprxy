use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use hyper_trust_dns::{TrustDnsHttpConnector, TrustDnsResolver};
use std::net::IpAddr;
use std::{convert::Infallible, net::SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use log::{error, info, trace};
use httprxy::ReverseProxy;

lazy_static::lazy_static! {
     static ref PROXY_CLIENT: ReverseProxy<TrustDnsHttpConnector> = {
        ReverseProxy::new(
            hyper::Client::builder().build::<_, Body>(TrustDnsResolver::default().into_http_connector()),
        )
    };
}

fn debug_request(req: &Request<Body>) -> Result<Response<Body>, Infallible> {
    trace!("target path did not match");

    let body_str = format!("{:?}", req);
    Ok(Response::new(Body::from(body_str)))
}

async fn handle(map: HashMap<String, String>, client_ip: IpAddr, req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();

    match map.get(path) {
        None => { debug_request(&req) } // just trace
        Some(forward_uri) => {
            trace!("forwarding to {}",forward_uri);

            match PROXY_CLIENT
                .call(client_ip, forward_uri, req)
                .await
            {
                Ok(response) => Ok(response),
                Err(_error) => Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let bind_addr = "127.0.0.1:8000";
    let addr: SocketAddr = bind_addr.parse().expect("Could not parse ip:port.");

    // Shared, thread-safe map with initial values
    let shared_map: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::from([
        ("/server1".to_string(), "http://127.0.0.1:1223".to_string()),
        ("/server2".to_string(), "http://127.0.0.1:1224".to_string()),
    ])));

    info!("registering service");

    let make_svc = make_service_fn(|conn: &AddrStream| {
        let remote_addr = conn.remote_addr().ip();
        let cloned_map = shared_map.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let map = cloned_map.lock().unwrap().clone();
                handle(map, remote_addr, req) // Use dereferenced map
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    info!("running server on {:?}",addr);

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }
}

use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use hyper_trust_dns::{TrustDnsHttpConnector, TrustDnsResolver};
use std::net::{IpAddr, TcpStream};
use std::{convert::Infallible, net::SocketAddr};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use futures::StreamExt;
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

async fn proxy(_forward_uri: &str, req: Request<Body>) -> Result<Response<Body>, Infallible>  {
    // Parse the destination address and port from the forward URI
    // let mut parts = forward_uri.split(':');

    // println!("parts : {:#?}", parts);

    // let _dest_ip = parts.next().expect("Invalid forward URI");
    // let _dest_port: u16 = parts.next().expect("Invalid forward URI").parse().expect("Invalid port");


    // Connect to the destination TCP server
    let mut socket = TcpStream::connect("127.0.0.1:1234").unwrap();
    let req_str = format!(
        "{} {} {:?}\r\n{:?}\r\n",
        req.method(),
        req.uri(),
        req.version(),
        req.headers()
    );

    trace!("req_str {:#?}" , req_str);

    socket.write_all(req_str.as_bytes()).unwrap();

    // Forward the request body
    let mut req_body = req.into_body();
    while let Some(chunk) = req_body.next().await {
        let chunk = chunk.unwrap();
        socket.write_all(&chunk).unwrap();
    }

    // Read the response from the TCP server
    let mut response = Vec::new();
    socket.read_to_end(&mut response).unwrap();

    // Create a response with the received data
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(response))
        .unwrap())
}


async fn handle(map: HashMap<String, String>, _client_ip: IpAddr, req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();

    match map.get(path) {
        None => { debug_request(&req) } // just trace
        Some(forward_uri) => {
            trace!("forwarding to {}",forward_uri);

            match proxy(forward_uri,req).await {
                Ok(res) => {Ok(res)}
                Err(error) => {
                    error!("error : {:#?}",error);

                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap())
                }
            }

            // match PROXY_CLIENT
            //     .call(client_ip, forward_uri, req)
            //     .await
            // {
            //     Ok(response) => Ok(response),
            //     Err(_error) => Ok(Response::builder()
            //         .status(StatusCode::INTERNAL_SERVER_ERROR)
            //         .body(Body::empty())
            //         .unwrap()),
            // }
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
        ("/server1".to_string(), "http://127.0.0.1:1234".to_string()),
        ("/server2".to_string(), "http://127.0.0.1:1235".to_string()),
    ])));

    info!("registering make service");

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

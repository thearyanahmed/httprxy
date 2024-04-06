use httprxy::ReverseProxy;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use hyper_trust_dns::{TrustDnsHttpConnector, TrustDnsResolver};
use log::{error, info, trace};
use std::collections::HashMap;
use std::net::{IpAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::{convert::Infallible, net::SocketAddr};

use std::io;
use std::net::TcpListener;
use std::thread;

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

fn try_or_continue<T, E>(result: Result<T, E>) -> Option<T> {
    match result {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}

async fn handle(
    map: HashMap<String, String>,
    client_ip: IpAddr,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();

    let proxy_addr = "127.0.0.1:8000";
    let to_addr = "http://127.0.0.1:1223";

    let listener = TcpListener::bind(proxy_addr).expect("Unable to bind proxy addr");

    println!("Proxing TCP packets from {} to {}", proxy_addr, to_addr);

    for incoming_stream in listener.incoming() {
        let proxy_stream = match try_or_continue(incoming_stream) {
            Some(val) => val,
            None => {
                // Handle the error or continue with your logic
                // For example:
                println!("Failed to get incoming stream, continuing...");
                continue;
            }
        };

        let conn_thread = TcpStream::connect(to_addr)
            .map(|to_stream| thread::spawn(move || handle_conn(proxy_stream, to_stream)));

        match conn_thread {
            Ok(_) => {
                println!("Successfully established a connection with client");
            }
            Err(err) => {
                println!("Unable to establish a connection with client {}", err);
            }
        }
    }
    Ok(())
}

fn handle_conn(lhs_stream: TcpStream, rhs_stream: TcpStream) {
    let lhs_arc = Arc::new(lhs_stream);
    let rhs_arc = Arc::new(rhs_stream);

    let (mut lhs_tx, mut lhs_rx) = (lhs_arc.try_clone().unwrap(), lhs_arc.try_clone().unwrap());
    let (mut rhs_tx, mut rhs_rx) = (rhs_arc.try_clone().unwrap(), rhs_arc.try_clone().unwrap());

    let connections = vec![
        thread::spawn(move || io::copy(&mut lhs_tx, &mut rhs_rx).unwrap()),
        thread::spawn(move || io::copy(&mut rhs_tx, &mut lhs_rx).unwrap()),
    ];

    for t in connections {
        t.join().unwrap();
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

    info!("running server on {:?}", addr);

    if let Err(e) = server.await {
        error!("server error: {}", e);
    }
}

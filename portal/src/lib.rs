use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use log::info;
use std::convert::Infallible;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use tokio::sync::oneshot::Sender;

async fn file_service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
	info!("File requested: {}", req.uri().path());

	let file = if req.uri().path() == "/" { "/index.html" } else { req.uri().path() };
	let mut file = File::open(format!("portal/site/dist{}", file)).expect("file not found");
	let mut contents = Vec::new();
	file.read_to_end(&mut contents).expect("error reading from file");
	let response = Response::new(contents.into());
	Ok(response)
}

pub fn start() -> Sender<()> {
	let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

	let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(file_service)) });

	let (tx, rx) = tokio::sync::oneshot::channel::<()>();

	let server = Server::bind(&addr).serve(make_svc).with_graceful_shutdown(async {
		rx.await.ok();
	});

	tokio::spawn(async {
		if let Err(e) = server.await {
			eprintln!("server error: {}", e);
		}
	});

	tx
}

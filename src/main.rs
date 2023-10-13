use log::{info, warn};
use actix_web::{get, web, App, HttpServer, Responder};

const DEFAULT_ADDRESS: String = String::from("localhost");

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {

    env_logger::init();
    let addr = std::env::var("FB2SERVER").map_err(|e|{
        warn!("The FB2SERVER environment variable not found");
        DEFAULT_ADDRESS
    })?;



    info!("Bind {} interface", addr);

    HttpServer::new(|| {
        App::new().service(greet)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
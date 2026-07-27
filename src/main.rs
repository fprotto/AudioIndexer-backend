mod browse;
mod files;
mod pathutil;

use actix_web::{middleware::Logger, web, App, HttpServer};
use clap::Parser;
use std::path::PathBuf;

/// A small read-only HTTP server that exposes a local music directory:
/// browse its structure and stream the files it contains.
#[derive(Parser, Debug)]
#[command(name = "music-server", version, about)]
struct Cli {
    /// Root directory containing the music files to serve.
    #[arg(short, long)]
    root: PathBuf,

    /// Address to bind to.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to.
    #[arg(short, long, default_value_t = 8080)]
    port: u16,
}

/// Shared, read-only application state.
pub struct AppState {
    /// Canonicalized path to the music root. All served/browsed paths are
    /// resolved and checked against this.
    pub root: PathBuf,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let cli = Cli::parse();

    let root = cli.root.canonicalize().unwrap_or_else(|e| {
        eprintln!(
            "error: root directory {:?} is not accessible: {e}",
            cli.root
        );
        std::process::exit(1);
    });

    if !root.is_dir() {
        eprintln!("error: root path {root:?} is not a directory");
        std::process::exit(1);
    }

    log::info!("serving music root: {root:?}");
    log::info!("listening on http://{}:{}", cli.host, cli.port);

    let state = web::Data::new(AppState { root });

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::new("%a \"%r\" %s %b bytes in %Dms"))
            .app_data(state.clone())
            .app_data(web::PathConfig::default())
            .service(
                web::scope("/api")
                    .route("/browse", web::get().to(browse::browse_root))
                    .route("/browse/{path:.*}", web::get().to(browse::browse_subpath))
                    .route("/files/{path:.*}", web::get().to(files::get_file)),
            )
    })
    .bind((cli.host.as_str(), cli.port))?
    .run()
    .await
}

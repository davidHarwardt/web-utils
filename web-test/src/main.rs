#![allow(unused)]

use axum::{routing::get, Router};
use include_tailwind::load_tailwind;
use maud::{html, Markup, DOCTYPE};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(index));
    #[cfg(debug_assertions)]
    let app = app.layer(tower_livereload::LiveReloadLayer::new());

    let listener = TcpListener::bind(("0.0.0.0", 3000)).await?;
    axum::serve(listener, app).await;
    Ok(())
}

async fn index() -> Markup {
    html! {
        (DOCTYPE);
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                meta name="X-UA-Compatible" content="id=edge";

                title { "test" };
                (load_tailwind!());
            }
            body."p-4" {
                div."mx-auto max-w-prose bg-slate-300 p-4" {
                    "some prose "
                };
            }
        }
    }
}


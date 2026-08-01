use std::sync::Arc;

use http_body_util::{BodyExt, StreamBody};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Body, Router, RouterBuilderDiscoverExt},
};
use vercel_runtime::{Error, Request, Response, ResponseBody, run, service_fn};

#[path = "../src/app.rs"]
mod app;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let router = Arc::new(Router::builder().discover().assets(load_assets()).build());

    run(service_fn(move |request| {
        let router = router.clone();
        async move { handle(router, request).await }
    }))
    .await
}

fn load_assets() -> AssetBundle {
    AssetBundle::load()
        .or_else(|_| AssetBundle::load_dir("target/release/assets"))
        .unwrap()
}

async fn handle(router: Arc<Router>, request: Request) -> Result<Response<ResponseBody>, Error> {
    let response = router.handle(request.map(Body::new)).await;
    let (parts, mut body) = response.into_parts();
    let (sender, receiver) = mpsc::channel(10);

    tokio::spawn(async move {
        while let Some(frame) = body.frame().await {
            if sender.send(frame).await.is_err() {
                break;
            }
        }
    });

    let body = StreamBody::new(ReceiverStream::new(receiver));
    Ok(Response::from_parts(parts, body.into()))
}

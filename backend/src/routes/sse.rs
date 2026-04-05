use axum::{
    extract::{Path, State},
    response::{Sse, sse::Event},
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use crate::{AppState, error::AppError, models::SseEvent};

pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let rx = {
        let senders = state.senders.read().await;
        senders.get(&id)
            .ok_or_else(|| AppError::NotFound(format!("job {} not found", id)))?
            .subscribe()
    };

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| {
            result.ok().map(|event| {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Ok(Event::default().data(data))
            })
        });

    Ok(Sse::new(stream))
}

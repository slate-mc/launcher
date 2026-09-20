use super::*;

#[tauri::command]
pub(super) async fn sessions_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<GameSessionSummary>, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .processes
        .active()
        .map(|processes| processes.into_iter().map(game_session_summary).collect())
        .map_err(process_state_error)
}

#[tauri::command]
pub(super) async fn session_force_stop(
    state: tauri::State<'_, DesktopState>,
    request: StopGameSessionRequest,
) -> Result<GameSessionSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    state
        .processes
        .force_stop(SessionId::from_uuid(request.id))
        .map(game_session_summary)
        .map_err(process_stop_error)
}

#[tauri::command]
pub(super) async fn session_log_subscribe(
    state: tauri::State<'_, DesktopState>,
    request: SubscribeSessionLogRequest,
    on_event: tauri::ipc::Channel<SessionLogEvent>,
) -> Result<SessionLogSubscription, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let session_id = SessionId::from_uuid(request.session_id);
    let process = state
        .processes
        .active_for_session(session_id)
        .map_err(process_state_error)?
        .ok_or_else(|| {
            AppError::new(
                "local.session_not_running",
                "That Minecraft game has already ended, so live output is no longer available.",
            )
        })?;
    let subscription_id = Uuid::new_v4();
    let mut tail = SessionLogTail::new(process.log_path);
    let snapshot = tail.snapshot().await.map_err(|_| {
        AppError::new(
            "local.session_log_unavailable",
            "slate could not load this Minecraft output.",
        )
        .retryable(true)
    })?;
    send_session_log_chunk(&on_event, subscription_id, session_id, snapshot)
        .map_err(|_| log_channel_error())?;

    let (cancel_sender, cancel_receiver) = tokio::sync::oneshot::channel();
    state.log_streams.insert(subscription_id, cancel_sender)?;
    let coordinator = state.log_streams.clone();
    let processes = state.processes.clone();
    tauri::async_runtime::spawn(async move {
        stream_session_log(
            processes,
            session_id,
            subscription_id,
            tail,
            on_event,
            cancel_receiver,
        )
        .await;
        coordinator.finish(subscription_id);
    });

    Ok(SessionLogSubscription {
        id: subscription_id,
        session_id: request.session_id,
    })
}

#[tauri::command]
pub(super) fn session_log_unsubscribe(
    state: tauri::State<'_, DesktopState>,
    request: UnsubscribeSessionLogRequest,
) -> Result<(), AppError> {
    state.log_streams.cancel(request.subscription_id)
}

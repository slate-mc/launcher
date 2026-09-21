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
pub(super) async fn instance_sessions_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceSessionsRequest,
) -> Result<Vec<SessionHistorySummary>, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let sessions = state
        .database
        .list_recent_sessions(instance_id, 10)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load recent game sessions."))?;
    Ok(sessions
        .into_iter()
        .filter_map(|session| {
            let state_value = match session.state {
                SessionState::Exited => SessionHistoryStateDto::Exited,
                SessionState::Failed => SessionHistoryStateDto::Failed,
                SessionState::Crashed => SessionHistoryStateDto::Crashed,
                SessionState::Cancelled => SessionHistoryStateDto::Cancelled,
                SessionState::Starting | SessionState::Running => return None,
            };
            let log_path = session_log_path(state.inner(), session.instance_id, session.id);
            Some(SessionHistorySummary {
                id: session.id.as_uuid(),
                instance_id: session.instance_id.as_uuid(),
                state: state_value,
                started_at: session.started_at,
                ended_at: session.ended_at,
                exit_code: session.exit_code,
                log_name: "Saved game output".to_owned(),
                log_available: log_path.is_file(),
            })
        })
        .collect())
}

#[tauri::command]
pub(super) async fn session_force_stop(
    state: tauri::State<'_, DesktopState>,
    request: StopGameSessionRequest,
) -> Result<GameSessionSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let session_id = SessionId::from_uuid(request.id);
    let stopped = state
        .processes
        .force_stop(session_id)
        .map_err(process_stop_error)?;
    tracing::warn!(
        instance_id = %stopped.instance_id,
        session_id = %session_id,
        "minecraft session force stop requested"
    );
    Ok(game_session_summary(stopped))
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
pub(super) async fn session_log_read(
    state: tauri::State<'_, DesktopState>,
    request: ReadSessionLogRequest,
) -> Result<SessionLogSnapshot, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let session_id = SessionId::from_uuid(request.session_id);
    let session = state
        .database
        .get_session(session_id)
        .await
        .map_err(|error| map_storage_error(error, "That saved game session is unavailable."))?;
    let mut tail = SessionLogTail::new(session_log_path(
        state.inner(),
        session.instance_id,
        session.id,
    ));
    let snapshot = tail.snapshot().await.map_err(|_| {
        AppError::new(
            "local.session_log_unavailable",
            "That saved game output is no longer available.",
        )
    })?;
    Ok(SessionLogSnapshot {
        session_id: session.id.as_uuid(),
        truncated: snapshot.truncated,
        text: snapshot.text,
    })
}

fn session_log_path(
    state: &DesktopState,
    instance_id: InstanceId,
    session_id: SessionId,
) -> PathBuf {
    state
        .paths
        .instance(instance_id)
        .join("logs")
        .join(format!("{session_id}.log"))
}

#[tauri::command]
pub(super) fn session_log_unsubscribe(
    state: tauri::State<'_, DesktopState>,
    request: UnsubscribeSessionLogRequest,
) -> Result<(), AppError> {
    state.log_streams.cancel(request.subscription_id)
}

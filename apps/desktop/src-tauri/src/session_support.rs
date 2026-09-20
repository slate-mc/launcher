use super::*;

pub(super) fn modpack_api_error(error: slate_modpack_client::ClientError) -> AppError {
    AppError::new("modpack.service_unavailable", error.user_message()).retryable(true)
}

pub(super) fn process_start_error(error: slate_process::ProcessError) -> AppError {
    if matches!(error, slate_process::ProcessError::InstanceAlreadyRunning) {
        AppError::new(
            "local.instance_running",
            "Minecraft is already running for this instance.",
        )
    } else if matches!(error, slate_process::ProcessError::AffinityUnavailable) {
        AppError::new(
            "local.cpu_affinity_unavailable",
            "slate could not apply this instance's CPU affinity. Clear the advanced CPU affinity setting and try again.",
        )
    } else {
        AppError::new(
            "local.launch_failed",
            "slate could not start the Minecraft process. Check the instance log and try again.",
        )
    }
}

pub(super) fn process_stop_error(error: slate_process::ProcessError) -> AppError {
    if matches!(error, slate_process::ProcessError::SessionNotFound) {
        AppError::new(
            "local.session_not_running",
            "That Minecraft session has already stopped.",
        )
    } else {
        AppError::new(
            "local.stop_failed",
            "slate could not force-close the Minecraft process.",
        )
        .retryable(true)
    }
}

pub(super) fn log_stream_state_error() -> AppError {
    AppError::new(
        "local.session_log_state_unavailable",
        "slate could not update the live log subscription.",
    )
    .retryable(true)
}

pub(super) fn log_channel_error() -> AppError {
    AppError::new(
        "local.session_log_channel_unavailable",
        "slate could not connect the live Minecraft log to this window.",
    )
    .retryable(true)
}

pub(super) fn send_session_log_chunk(
    channel: &tauri::ipc::Channel<SessionLogEvent>,
    subscription_id: Uuid,
    session_id: SessionId,
    chunk: LogChunk,
) -> tauri::Result<()> {
    channel.send(SessionLogEvent {
        subscription_id,
        session_id: session_id.as_uuid(),
        kind: match chunk.kind {
            LogChunkKind::Snapshot => SessionLogEventKindDto::Snapshot,
            LogChunkKind::Append => SessionLogEventKindDto::Append,
            LogChunkKind::Reset => SessionLogEventKindDto::Reset,
        },
        offset: chunk.offset.to_string(),
        truncated: chunk.truncated,
        text: chunk.text,
    })
}

pub(super) async fn stream_session_log(
    processes: ProcessSupervisor,
    session_id: SessionId,
    subscription_id: Uuid,
    mut tail: SessionLogTail,
    channel: tauri::ipc::Channel<SessionLogEvent>,
    mut cancel: tokio::sync::oneshot::Receiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = &mut cancel => return,
            _ = interval.tick() => {
                let mut caught_up = false;
                for _ in 0..4 {
                    match tail.next_chunk().await {
                        Ok(Some(chunk)) => {
                            if send_session_log_chunk(
                                &channel,
                                subscription_id,
                                session_id,
                                chunk,
                            ).is_err() {
                                return;
                            }
                        }
                        Ok(None) => {
                            caught_up = true;
                            break;
                        }
                        Err(_) => {
                            let _ = channel.send(SessionLogEvent {
                                subscription_id,
                                session_id: session_id.as_uuid(),
                                kind: SessionLogEventKindDto::Error,
                                offset: tail.offset().to_string(),
                                truncated: false,
                                text: "slate could not continue reading this session log.".to_owned(),
                            });
                            return;
                        }
                    }
                }

                match processes.active_for_session(session_id) {
                    Ok(Some(_)) => {}
                    Ok(None) if caught_up => {
                        let _ = channel.send(SessionLogEvent {
                            subscription_id,
                            session_id: session_id.as_uuid(),
                            kind: SessionLogEventKindDto::Closed,
                            offset: tail.offset().to_string(),
                            truncated: false,
                            text: String::new(),
                        });
                        return;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        let _ = channel.send(SessionLogEvent {
                            subscription_id,
                            session_id: session_id.as_uuid(),
                            kind: SessionLogEventKindDto::Error,
                            offset: tail.offset().to_string(),
                            truncated: false,
                            text: "slate could not verify the game process state.".to_owned(),
                        });
                        return;
                    }
                }
            }
        }
    }
}

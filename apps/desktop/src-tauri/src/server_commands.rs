use super::*;

#[tauri::command]
pub(super) async fn servers_list(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<SavedServerSummary>, AppError> {
    state
        .database
        .list_saved_servers(1_000)
        .await
        .map(|servers| servers.into_iter().map(saved_server_summary).collect())
        .map_err(|error| map_storage_error(error, "slate could not load your saved servers."))
}

#[tauri::command]
pub(super) async fn server_create(
    state: tauri::State<'_, DesktopState>,
    request: CreateSavedServerRequest,
) -> Result<SavedServerSummary, AppError> {
    let server = validated_saved_server(
        state.inner(),
        request.name,
        request.address,
        request.preferred_instance_id,
    )
    .await?;
    state
        .database
        .create_saved_server(server)
        .await
        .map(saved_server_summary)
        .map_err(|error| map_storage_error(error, "slate could not save that server."))
}

#[tauri::command]
pub(super) async fn server_update(
    state: tauri::State<'_, DesktopState>,
    request: UpdateSavedServerRequest,
) -> Result<SavedServerSummary, AppError> {
    let server = validated_saved_server(
        state.inner(),
        request.name,
        request.address,
        request.preferred_instance_id,
    )
    .await?;
    state
        .database
        .update_saved_server(ServerId::from_uuid(request.id), server)
        .await
        .map(saved_server_summary)
        .map_err(|error| map_storage_error(error, "slate could not update that server."))
}

#[tauri::command]
pub(super) async fn server_remove(
    state: tauri::State<'_, DesktopState>,
    request: RemoveSavedServerRequest,
) -> Result<(), AppError> {
    state
        .database
        .remove_saved_server(ServerId::from_uuid(request.id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not remove that server."))
}

#[tauri::command]
pub(super) async fn server_ping(
    request: PingServerRequest,
) -> Result<ServerStatusSummary, AppError> {
    ping_minecraft_server(&request.address)
        .await
        .map_err(|_| invalid_server_address())
}

async fn validated_saved_server(
    state: &DesktopState,
    name: String,
    address: String,
    preferred_instance_id: Option<Uuid>,
) -> Result<NewSavedServer, AppError> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::new(
            "validation.server_name",
            "Enter a server name between 1 and 80 characters.",
        )
        .with_field_error("name", "Use between 1 and 80 characters."));
    }
    let address = normalize_server_address(&address).map_err(|_| invalid_server_address())?;
    let preferred_instance_id = preferred_instance_id.map(InstanceId::from_uuid);
    if let Some(instance_id) = preferred_instance_id {
        state
            .database
            .get_instance(instance_id)
            .await
            .map_err(|error| map_storage_error(error, "Choose an instance that still exists."))?;
    }
    Ok(NewSavedServer {
        name,
        address,
        preferred_instance_id,
    })
}

fn invalid_server_address() -> AppError {
    AppError::new(
        "validation.server_address",
        "Enter a valid Minecraft server address.",
    )
    .with_field_error(
        "address",
        "Use a hostname or IP address, with an optional port.",
    )
}

fn saved_server_summary(server: SavedServerRecord) -> SavedServerSummary {
    SavedServerSummary {
        id: server.id.as_uuid(),
        name: server.name,
        address: server.address,
        preferred_instance_id: server.preferred_instance_id.map(InstanceId::as_uuid),
        created_at: server.created_at,
        updated_at: server.updated_at,
    }
}

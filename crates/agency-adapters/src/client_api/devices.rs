//! The account's devices: the list, a new device's request, and a signed change relayed to the root's node.
use super::{caller_of, is_owner_request, json_body, respond, Admitted, ClientApi, Route, Shared};
use agency_core::devices::{device_list, request_device_as};
use agency_proto::client::{DeviceList, DeviceRequestInput, NodeInfo};
use agency_proto::{DeviceChange, Signed};
use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

pub(super) const ROUTES: &[Route] = &[
    Route::get("/api/devices").for_guests().reported(),
    Route::post("/api/devices/requests").for_guests().reported(),
    Route::post("/api/devices/changes").for_guests().reported(),
];

pub(super) fn routes() -> Router<Arc<ClientApi>> {
    Router::new()
        .route(ROUTES[0].path, get(get_devices))
        .route(ROUTES[1].path, post(post_device_request))
        .route(ROUTES[2].path, post(post_device_change))
}

/// `list` with where and how to approve a device here, for the owner only: the command holds the path of
/// the running program, which can hold a user name.
fn with_node_info(api: &ClientApi, admitted: &Admitted, mut list: DeviceList) -> DeviceList {
    if !is_owner_request(api, admitted) {
        return list;
    }
    list.node = api.launch.as_ref().map(|launch| {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        NodeInfo {
            url: api.node.config().url,
            data_dir: launch.data_dir.clone(),
            command: launch.device_add_command(home.as_deref()),
        }
    });
    list
}

async fn get_devices(State(api): Shared, Extension(admitted): Extension<Admitted>) -> Response {
    if let Admitted::Guest(guest) = &admitted {
        return respond(agency_core::guest_reads::device_list(&api.node, guest));
    }
    respond(device_list(&api.node).map(|list| with_node_info(&api, &admitted, list)))
}

/// A new device asks to be added, for the owner's account or for the guest's hosted member; the answer
/// carries its pairing code.
async fn post_device_request(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    bytes: Bytes,
) -> Response {
    let result = json_body::<DeviceRequestInput>(&bytes, "device request")
        .and_then(|input| request_device_as(&api.node, &caller_of(&admitted), input));
    respond(result)
}

/// A device change signed by one of the account's devices, relayed to the org's root node.
async fn post_device_change(
    State(api): Shared,
    Extension(admitted): Extension<Admitted>,
    bytes: Bytes,
) -> Response {
    let result = async {
        let signed: Signed<DeviceChange> = json_body(&bytes, "device change")?;
        agency_core::devices::submit_device_change_as(&api.node, &caller_of(&admitted), signed)
            .await
    };
    respond(result.await)
}

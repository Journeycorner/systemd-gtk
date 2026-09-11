mod common;
use common::*;
use futures_util::StreamExt;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use systemd_gtk::{
    backend::{BackendError, BackendEvent, DbusBackend, SystemdBackend, Timeouts},
    model::{Scope, UnitAction, UnitId},
};

fn id(scope: Scope) -> UnitId {
    UnitId {
        scope,
        name: "example.service".into(),
    }
}
fn backend(bus: &TestBus) -> DbusBackend {
    DbusBackend::for_addresses(
        bus.address.clone(),
        bus.address.clone(),
        Timeouts {
            read: Duration::from_secs(2),
            operation: Duration::from_secs(3),
        },
    )
}

#[tokio::test]
async fn scopes_are_isolated_and_fast_jobs_are_tracked_with_interactive_auth() {
    let system = TestBus::new();
    let user = TestBus::new();
    let system_state = Arc::new(Mutex::new(FakeState::default()));
    let user_state = Arc::new(Mutex::new(FakeState {
        description: "User service".into(),
        ..Default::default()
    }));
    let _system_service = serve(&system.address, system_state.clone()).await;
    let _user_service = serve(&user.address, user_state.clone()).await;
    let backend = DbusBackend::for_addresses(
        system.address.clone(),
        user.address.clone(),
        Timeouts::default(),
    );
    assert_eq!(
        backend.list_units(Scope::System).await.unwrap()[0].description,
        "Test service"
    );
    let units = backend.list_units(Scope::User).await.unwrap();
    assert_eq!(units[0].id.scope, Scope::User);
    assert_eq!(units[0].description, "User service");
    backend
        .execute(id(Scope::User), UnitAction::Start)
        .await
        .unwrap();
    backend
        .execute(id(Scope::User), UnitAction::Restart)
        .await
        .unwrap();
    backend
        .execute(id(Scope::User), UnitAction::Stop)
        .await
        .unwrap();
    assert!(system_state.lock().unwrap().calls.is_empty());
    let calls = &user_state.lock().unwrap().calls;
    assert_eq!(
        calls.iter().map(|c| c.method.as_str()).collect::<Vec<_>>(),
        ["start", "restart", "stop"]
    );
    assert!(
        calls
            .iter()
            .all(|c| c.interactive && c.args == ["example.service", "replace"])
    );
}

#[tokio::test]
async fn enable_disable_arguments_reload_and_partial_failure_are_preserved() {
    let bus = TestBus::new();
    let state = Arc::new(Mutex::new(FakeState::default()));
    let _service = serve(&bus.address, state.clone()).await;
    let backend = backend(&bus);
    backend
        .execute(id(Scope::System), UnitAction::Enable)
        .await
        .unwrap();
    backend
        .execute(id(Scope::System), UnitAction::Disable)
        .await
        .unwrap();
    state.lock().unwrap().file_state = "enabled-runtime".into();
    backend
        .execute(id(Scope::System), UnitAction::Disable)
        .await
        .unwrap();
    state.lock().unwrap().reload_fails = true;
    assert!(matches!(
        backend.execute(id(Scope::System), UnitAction::Enable).await,
        Err(BackendError::PartialSuccess(_))
    ));
    let state = state.lock().unwrap();
    assert_eq!(state.file_state, "enabled");
    assert_eq!(state.calls[0].args, ["example.service", "false", "false"]);
    assert_eq!(state.calls[1].method, "reload");
    assert_eq!(state.calls[2].args, ["example.service", "false"]);
    assert_eq!(state.calls[4].args, ["example.service", "true"]);
    assert!(state.calls.iter().all(|c| c.interactive));
}

#[tokio::test]
async fn job_failures_denial_and_missing_jobs_do_not_report_success() {
    let bus = TestBus::new();
    let state = Arc::new(Mutex::new(FakeState {
        result: "failed".into(),
        ..Default::default()
    }));
    let _service = serve(&bus.address, state.clone()).await;
    let backend = DbusBackend::for_addresses(
        bus.address.clone(),
        bus.address.clone(),
        Timeouts {
            read: Duration::from_secs(1),
            operation: Duration::from_millis(300),
        },
    );
    assert!(matches!(
        backend.execute(id(Scope::System), UnitAction::Start).await,
        Err(BackendError::JobFailed(_))
    ));
    state.lock().unwrap().result = "deny".into();
    assert!(matches!(
        backend.execute(id(Scope::System), UnitAction::Start).await,
        Err(BackendError::Dbus(_))
    ));
    state.lock().unwrap().result = "hang".into();
    assert!(matches!(
        backend.execute(id(Scope::System), UnitAction::Start).await,
        Err(BackendError::OutcomeUnknown(_))
    ));
    assert_eq!(
        state.lock().unwrap().calls.len(),
        3,
        "failed writes must not be replayed"
    );
}

#[tokio::test]
async fn subscriptions_disconnect_and_reset_use_only_the_private_bus() {
    let mut bus = TestBus::new();
    let state = Arc::new(Mutex::new(FakeState::default()));
    let service = serve(&bus.address, state).await;
    let backend = backend(&bus);
    let mut events = backend.subscribe(Scope::System).await.unwrap();
    service
        .emit_signal(
            None::<&str>,
            MANAGER_PATH,
            "org.freedesktop.systemd1.Manager",
            "UnitFilesChanged",
            &(),
        )
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), events.next())
            .await
            .unwrap(),
        Some(BackendEvent::Changed)
    );
    backend.reset(Scope::System).await;
    assert_eq!(backend.list_units(Scope::System).await.unwrap().len(), 1);
    bus.stop();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), events.next())
            .await
            .unwrap(),
        Some(BackendEvent::Disconnected(_))
    ));
    backend.reset(Scope::System).await;
    assert!(backend.list_units(Scope::System).await.is_err());
}

#[tokio::test]
async fn reads_timeout_and_unit_files_use_paths_from_dbus() {
    let bus = TestBus::new();
    let directory = tempfile::tempdir().unwrap();
    let fragment = directory.path().join("example.service");
    let drop_in = directory.path().join("override.conf");
    std::fs::write(&fragment, "[Service]\nExecStart=/usr/bin/true").unwrap();
    std::fs::write(&drop_in, "[Service]\nNice=5").unwrap();
    let state = Arc::new(Mutex::new(FakeState {
        fragment: fragment.to_str().unwrap().into(),
        drop_ins: vec![drop_in.to_str().unwrap().into()],
        ..Default::default()
    }));
    let _service = serve(&bus.address, state.clone()).await;
    let backend = DbusBackend::for_addresses(
        bus.address.clone(),
        bus.address.clone(),
        Timeouts {
            read: Duration::from_millis(200),
            operation: Duration::from_secs(2),
        },
    );
    let content = backend.unit_files(id(Scope::System)).await.unwrap();
    assert!(content.text.contains("ExecStart") && content.text.contains("Nice=5"));
    state.lock().unwrap().list_hangs = true;
    assert!(matches!(
        backend.list_units(Scope::System).await,
        Err(BackendError::ReadTimeout)
    ));
}

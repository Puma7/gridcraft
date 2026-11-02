//! One background check at startup (and on Help ▸ Check for Updates…) and the modal prompt that
//! offers the new release: Update and Restart (direct self-update), Download or Later. The flow
//! itself is `craft_update::updater::Updater`.

use craft_update::CheckResultResponse;
use craft_update::updater::{Action, Status, Updater};

use crate::SheetApp;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct Runtime {
    /// Embedding applications must not update themselves with GridCraft releases.
    enabled: bool,
    updater: Updater,
    /// The check at start was asked for; `tick` starts it with the context.
    startup: bool,
    /// Help ▸ Check for Updates… was chosen; the next `tick` starts it.
    manual: bool,
    /// The result dialog of a requested check is showing.
    result_open: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self { enabled: true, updater: Updater::new(&crate::UPDATE_APP, VERSION), startup: false, manual: false, result_open: false }
    }
}

/// Ask for a check; the next frame's `tick` starts it. A manual one reports every outcome.
pub(crate) fn start(app: &mut SheetApp, manual: bool) {
    if !app.updates_rt.enabled {
        return;
    }
    if manual {
        app.updates_rt.manual = true;
    } else {
        app.updates_rt.startup = true;
    }
}

pub(crate) fn tick(app: &mut SheetApp, ctx: &egui::Context) {
    let rt = &mut app.updates_rt;
    if !rt.enabled {
        return;
    }
    if std::mem::take(&mut rt.manual) {
        rt.updater.check_now(ctx);
        rt.result_open = true;
    } else if rt.startup {
        rt.updater.check_at_start(ctx);
    }
    rt.updater.tick(ctx);
    // A requested check that found a release leaves the status idle and the prompt opens.
    if matches!(rt.updater.status(), Status::Idle) {
        rt.result_open = false;
    }
}

pub(crate) fn show(app: &mut SheetApp, ctx: &egui::Context) {
    if !app.updates_rt.enabled {
        return;
    }
    // The restart helper is scheduled before the app closes, so no update starts over unsaved work.
    let clean = !app.session.documents().iter().any(|d| d.is_dirty());
    match app.updates_rt.updater.show_if(ctx, clean) {
        Action::OpenUrl(url) => {
            if let Some(open) = &app.services.open_url {
                open(&url);
            }
        }
        // The guarded close path asks about unsaved work; the restart helper relaunches once the
        // process has exited.
        Action::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        Action::None => {}
    }
    if app.updates_rt.result_open {
        let rt = &mut app.updates_rt;
        match craft_update::CheckResultDialog::new(&crate::UPDATE_APP, VERSION).show(ctx, rt.updater.status()) {
            CheckResultResponse::Close => {
                rt.updater.dismiss_status();
                rt.result_open = false;
            }
            CheckResultResponse::CheckAgain => rt.updater.check_now(ctx),
            CheckResultResponse::None => {}
        }
    }
}

impl SheetApp {
    /// Prevent subsequent GridCraft-specific checks and binary replacement when its
    /// interface is hosted by a differently named application.
    pub fn disable_updates(&mut self) {
        self.updates_rt.enabled = false;
        self.updates_rt.startup = false;
        self.updates_rt.manual = false;
        self.updates_rt.result_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_embedding_host_cannot_request_a_gridcraft_update() {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        let mut app = SheetApp::new(session, Default::default());
        assert!(app.updates_rt.enabled);
        app.start_update_check();
        assert!(app.updates_rt.startup);
        app.disable_updates();
        app.start_update_check();
        app.run("app.checkForUpdates", serde_json::json!({})).unwrap();
        tick(&mut app, &egui::Context::default());
        assert!(!app.updates_rt.startup && !app.updates_rt.manual && !app.updates_rt.result_open);
        assert!(!app.updates_rt.enabled);
    }
}

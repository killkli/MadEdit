//! GitHub Release checks run outside the UI thread; downloads stay in the browser.
use std::{sync::mpsc::{self, Receiver}, time::{Duration, SystemTime, UNIX_EPOCH}};
use eframe::egui;
use serde::Deserialize;
use super::App;

const RELEASES: &str = "https://github.com/tenyi/MadEdit/releases/latest";
#[derive(Deserialize)]
struct Release { tag_name: String, draft: bool, prerelease: bool }

#[derive(Default)]
pub(super) struct UpdateState {
    pending: Option<Receiver<Result<Option<String>, String>>>,
    manual: bool,
    pub(super) available: Option<String>,
}
impl UpdateState {
    pub(super) fn busy(&self) -> bool { self.pending.is_some() }
}
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn due(last: u64, now: u64) -> bool { last == 0 || now < last || now - last >= 86400 }
fn newer(release: Release, current: &str) -> Result<Option<String>, String> {
    let remote = semver::Version::parse(release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name)).map_err(|e| e.to_string())?;
    let local = semver::Version::parse(current).map_err(|e| e.to_string())?;
    Ok((!release.draft && !release.prerelease && remote.pre.is_empty() && remote > local).then_some(release.tag_name))
}
fn fetch() -> Result<Option<String>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let body = agent.get("https://api.github.com/repos/tenyi/MadEdit/releases/latest")
        .header("User-Agent", concat!("MadEdit/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call().map_err(|e| e.to_string())?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    newer(serde_json::from_str(&body).map_err(|e| e.to_string())?, env!("CARGO_PKG_VERSION"))
}
impl App {
    pub(super) fn start_update_check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.updates.busy() { return; }
        self.settings.last_update_check = now();
        self.save_settings();
        let (tx, rx) = mpsc::channel();
        self.updates.pending = Some(rx);
        self.updates.manual = manual;
        if manual { self.set_status(self.t("updates.checking", "正在檢查更新…")); }
        let ctx = ctx.clone();
        std::thread::spawn(move || { let _ = tx.send(fetch()); ctx.request_repaint(); });
    }
    pub(super) fn poll_updates(&mut self, ctx: &egui::Context) {
        // Tests and automated screenshots must never access the network.
        if !cfg!(test) && self.screenshot.is_none() && self.settings.auto_check_updates
            && !self.updates.busy() && due(self.settings.last_update_check, now()) {
            self.start_update_check(ctx, false);
        }
        let Some(rx) = &self.updates.pending else { return; };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Update worker stopped".into()),
        };
        self.updates.pending = None;
        match result {
            Ok(Some(tag)) => {
                if self.updates.manual || tag != self.settings.skipped_update { self.updates.available = Some(tag); }
            }
            Ok(None) if self.updates.manual => self.set_status(self.t("updates.current", "目前已是最新版本")),
            Err(e) if self.updates.manual => self.set_status(format!("{}：{e}", self.t("updates.failed", "無法檢查更新"))),
            _ => (),
        }
    }
    pub(super) fn update_banner(&mut self, ui: &mut egui::Ui) {
        let Some(tag) = self.updates.available.clone() else { return; };
        egui::Panel::top("update-notice").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{} {tag}", self.t("updates.available", "有新版本")));
                ui.hyperlink_to(self.t("updates.download", "查看更新／前往下載"), RELEASES);
                if ui.button(self.t("updates.skip", "略過此版本")).clicked() {
                    self.settings.skipped_update = tag.clone();
                    self.save_settings();
                    self.updates.available = None;
                }
                if ui.button(self.t("updates.later", "稍後提醒")).clicked() { self.updates.available = None; }
            });
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release { Release { tag_name: tag.into(), draft: false, prerelease: false } }
    #[test]
    fn versions() {
        assert!(newer(release("v0.5.10"), "0.5.9").unwrap().is_some());
        for tag in ["v0.5.9", "v0.5.8", "v0.6.0-beta.1"] { assert!(newer(release(tag), "0.5.9").unwrap().is_none()); }
        let mut r = release("v1.0.0"); r.prerelease = true;
        assert!(newer(r, "0.5.9").unwrap().is_none());
        assert!(newer(release("invalid"), "0.5.9").is_err());
    }
    #[test]
    fn daily_throttle_and_clock_rollback() {
        assert!(due(0, 100)); assert!(!due(100, 86499)); assert!(due(100, 86500)); assert!(due(100, 50));
    }
}

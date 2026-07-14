//! The settings + account/sync message arms, split out of [`super::update`]
//! for the source LOC budget. `update` delegates these variants here unchanged.

use iced::Task;

use super::{HyggGui, Message, account_label};
use crate::theme::Palette;

impl HyggGui {
  pub(super) fn update_account(&mut self, message: Message) -> Task<Message> {
    match message {
      Message::SetTheme(theme) => {
        self.settings.theme = theme;
        self.palette = Palette::of(theme);
        self.settings.save();
        Task::none()
      }
      Message::SetImageMode(mode) => {
        self.settings.image_mode = mode;
        self.settings.save();
        self.maybe_load_assets()
      }
      Message::SetZoom(z) => {
        self.settings.text_zoom = z;
        self.settings.save();
        Task::none()
      }
      Message::SetColumn(c) => {
        self.settings.import_col = c as usize;
        self.settings.save();
        Task::none()
      }
      Message::Connect => self.begin_connect(),
      Message::Connected(Ok((username, token, me))) => {
        // Credentials validated: persist them and adopt the server-assigned
        // device id so pushed ops are tagged with this device.
        self.settings.username = Some(username);
        self.settings.api_token = Some(token);
        self.settings.device_id = Some(me.device_id.clone());
        self.settings.save();
        self.account.user.clear();
        self.account.token.clear();
        self.account.label = account_label(&me);
        self.account.status = "Connected.".to_string();
        self.account.busy = false;
        // Pull the account's library + positions now that we're connected.
        match self.settings.creds() {
          Some(creds) => self.resync(creds),
          None => Task::none(),
        }
      }
      Message::Connected(Err(e)) => {
        self.account.status = format!("Invalid username or token: {e}");
        self.account.busy = false;
        Task::none()
      }
      Message::AccountChecked(Ok(me)) => {
        self.account.label = account_label(&me);
        Task::none()
      }
      Message::AccountChecked(Err(e)) => {
        self.account.status = format!("Reconnect needed: {e}");
        Task::none()
      }
      Message::Disconnect => {
        self.settings.username = None;
        self.settings.api_token = None;
        self.settings.device_id = None;
        self.settings.save();
        self.account.label.clear();
        self.account.status.clear();
        Task::none()
      }
      Message::ToggleSyncEnabled(on) => {
        self.settings.sync_enabled = on;
        self.settings.save();
        // Turning sync back on: refresh the library from the server now.
        // Per-document pushes resume on the next save under the current scope.
        match (on, self.settings.creds()) {
          (true, Some(creds)) => self.resync(creds),
          _ => Task::none(),
        }
      }
      Message::SetAutoSyncScope(scope) => {
        self.settings.auto_sync_scope = scope;
        self.settings.save();
        Task::none()
      }
      _ => Task::none(),
    }
  }
}

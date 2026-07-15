//! Settings: reading preferences (font size, theme, column width) plus the
//! server URL. Persisted to localStorage on every change. This is where all
//! config lives now that the reader has no command line.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::{SettingsCtx, link};
use crate::build_info as bi;
use crate::components::{AccountSection, TopBar};
use crate::settings::{ImageMode, Theme};

#[component]
pub fn SettingsView() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let persist = move || settings.with(|s| s.save());

  view! {
    <div class="settings">
      // The shared bar keeps the gear visible here too; it renders as active
      // (lit) since this *is* the Settings page.
      <TopBar
        title=Signal::derive(|| "Settings".to_string())
        visible=Signal::derive(|| true)
        back_href=link("/")
      />

      <main class="settings__body">
        <section class="setting">
          <label>"Text size"</label>
          <div class="setting__row">
            <input type="range" min="0.7" max="1.6" step="0.05"
              prop:value=move || settings.with(|s| s.text_zoom).to_string()
              on:input=move |ev| {
                if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                  settings.update(|s| s.text_zoom = v);
                  persist();
                }
              }/>
            <span class="setting__value">
              {move || format!("{:.0}%", settings.with(|s| s.text_zoom) * 100.0)}
            </span>
          </div>
          <p class="setting__hint">
            "The column auto-fills the screen; this zooms it in or out."
          </p>
        </section>

        <section class="setting">
          <label>"Theme"</label>
          <div class="setting__row">
            {[("Dark", Theme::Dark), ("Light", Theme::Light), ("Sepia", Theme::Sepia)]
              .into_iter()
              .map(|(name, theme)| view! {
                <button class="chip"
                  class:chip--on=move || settings.with(|s| s.theme) == theme
                  on:click=move |_| {
                    settings.update(|s| s.theme = theme);
                    persist();
                  }>{name}</button>
              })
              .collect::<Vec<_>>()}
          </div>
        </section>

        <section class="setting">
          <label>"Figures & tables"</label>
          <div class="setting__row">
            {ImageMode::ALL
              .into_iter()
              .map(|mode| view! {
                <button class="chip"
                  class:chip--on=move || settings.with(|s| s.image_mode) == mode
                  on:click=move |_| {
                    settings.update(|s| s.image_mode = mode);
                    persist();
                  }>{mode.label()}</button>
              })
              .collect::<Vec<_>>()}
          </div>
          <p class="setting__hint">
            "How PDF images and tables render. A view-only choice — progress \
             still syncs with every device whichever you pick."
          </p>
        </section>

        <section class="setting">
          <label>"Column width"</label>
          <div class="setting__row">
            <input type="range" min="40" max="100" step="2"
              prop:value=move || settings.with(|s| s.import_col).to_string()
              on:input=move |ev| {
                if let Ok(v) = event_target_value(&ev).parse::<usize>() {
                  settings.update(|s| s.import_col = v);
                  persist();
                }
              }/>
            <span class="setting__value">
              {move || format!("{} cols", settings.with(|s| s.import_col))}
            </span>
          </div>
          <p class="setting__hint">
            "Applies to documents imported from now on."
          </p>
        </section>

        <section class="setting">
          <label>"Read-aloud speed"</label>
          <div class="setting__row">
            <input type="range" min="0.5" max="2" step="0.1"
              prop:value=move || settings.with(|s| s.tts_rate).to_string()
              on:input=move |ev| {
                if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                  settings.update(|s| s.tts_rate = v);
                  settings.with(|s| s.save());
                }
              }/>
            <span class="setting__value">
              {move || format!("{:.1}\u{00d7}", settings.with(|s| s.tts_rate))}
            </span>
          </div>
          <p class="setting__hint">
            "Tap the speaker button while reading to listen."
          </p>
        </section>

        <section class="setting">
          <label>"Server"</label>
          <input type="url" class="setting__text"
            prop:value=move || settings.with(|s| s.server_url.clone())
            on:change=move |ev| {
              settings.update(|s| s.server_url = event_target_value(&ev));
              persist();
            }/>
          <p class="setting__hint">
            "Used for optional sync. The reader works fully offline without it."
          </p>
        </section>

        <AccountSection/>

        <section class="setting">
          <label>"GitHub stars"</label>
          <label class="toggle">
            <input type="checkbox"
              prop:checked=move || settings.with(|s| s.show_github_stars)
              on:change=move |ev| {
                settings.update(|s| {
                  s.show_github_stars = event_target_checked(&ev);
                });
                persist();
              }/>
            <span>"Show the star count in the top bar"</span>
          </label>
          <p class="setting__hint">
            "A live GitHub star counter next to the settings gear — tap it to \
             star the repo."
          </p>
        </section>

        <section class="setting">
          <label>"About"</label>
          <div class="setting__row about__links">
            <A href=link("/about") attr:class="btn">"About hygg"</A>
            <A href=link("/credits") attr:class="btn">"Credits"</A>
          </div>
          <p class="setting__hint">
            {format!("Version {} \u{00b7} {}", bi::VERSION, bi::GIT_SHA)}
          </p>
        </section>
      </main>
    </div>
  }
}

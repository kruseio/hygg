//! The About page: which build of hygg this is — version, the git commit it was
//! built from (short hash + date, linking to that commit on GitHub), the
//! author, and the repository — plus a shortcut to Credits. Opened from
//! Settings; mirrors the native GUI About screen.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::build_info as bi;
use crate::components::TopBar;

#[component]
pub fn About() -> impl IntoView {
  view! {
    <TopBar
      title=Signal::derive(|| "About".to_string())
      visible=Signal::derive(|| true)
      back_href="/settings".to_string()
    />
    <main class="about">
      <section class="about__head">
        <h1>"hygg"</h1>
        <p>"A calm, offline-first document reader."</p>
      </section>

      <dl class="about__info panel">
        <div><dt>"Version"</dt><dd>{bi::VERSION}</dd></div>
        <div><dt>"Commit"</dt><dd>{bi::GIT_SHA}</dd></div>
        {(!bi::commit_timestamp().is_empty()).then(|| view! {
          <div><dt>"Committed"</dt><dd>{bi::commit_timestamp()}</dd></div>
        })}
        <div><dt>"Author"</dt><dd>{bi::AUTHOR}</dd></div>
        <div><dt>"License"</dt><dd>"AGPL-3.0-only"</dd></div>
      </dl>

      <div class="about__links">
        <a class="btn" href=bi::REPOSITORY target="_blank" rel="noopener">
          {github_icon()} "View on GitHub"
        </a>
        <a class="btn" href=bi::commit_url() target="_blank" rel="noopener">
          "View this commit"
        </a>
        <A href="/credits" attr:class="btn">"Credits"</A>
      </div>
    </main>
  }
}

/// GitHub mark, tinted via `currentColor`.
pub(super) fn github_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"
      aria-hidden="true">
      <path d="M12 .5C5.37.5 0 5.87 0 12.5c0 5.3 3.44 9.8 8.21 11.39.6.11.82-.26.82-.58 0-.29-.01-1.05-.02-2.06-3.34.73-4.04-1.61-4.04-1.61-.55-1.39-1.34-1.76-1.34-1.76-1.09-.75.08-.73.08-.73 1.21.09 1.84 1.24 1.84 1.24 1.07 1.84 2.81 1.31 3.5 1 .11-.78.42-1.31.76-1.61-2.67-.3-5.47-1.34-5.47-5.95 0-1.31.47-2.39 1.24-3.23-.13-.3-.54-1.53.12-3.18 0 0 1.01-.32 3.3 1.23a11.5 11.5 0 0 1 6.01 0c2.29-1.55 3.3-1.23 3.3-1.23.66 1.65.25 2.88.12 3.18.77.84 1.24 1.92 1.24 3.23 0 4.62-2.81 5.64-5.49 5.94.43.37.81 1.1.81 2.22 0 1.6-.01 2.9-.01 3.29 0 .32.22.7.83.58C20.56 22.29 24 17.8 24 12.5 24 5.87 18.63.5 12 .5z"/>
    </svg>
  }
}

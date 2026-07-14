use super::*;
use crate::repo::organizations::OrganizationMembership;

/// Everything needed to render one library entry's card + modal.
pub(crate) struct BookCard<'a> {
  pub user: &'a WebUser,
  pub book: &'a repo::books::BookRow,
  pub pct: i64,
  pub secs: i64,
  pub last_read: i64,
  pub doc_bytes: i64,
  pub meta_bytes: i64,
  pub has_blob: bool,
  pub org_name: Option<&'a str>,
  pub tags: &'a [String],
  pub organizations: &'a [OrganizationMembership],
  /// `(sharer email, access)` when this document was shared *to* the viewer.
  pub share: Option<&'a (String, String)>,
}

/// Render `(card, modal)` HTML for one document. The card is a clickable
/// summary; the modal (a CSS `:target` overlay) holds the advanced controls.
pub(crate) fn render_book(card: &BookCard) -> (String, String) {
  let book = card.book;
  let owned = book.owner_user_id == card.user.user_id;
  let visibility = if card.share.is_some() {
    "Shared with you"
  } else {
    card.org_name.unwrap_or("Private")
  };
  let modal_id = format!("book-{}", esc(&book.content_hash));
  let org_chip = if card.org_name.is_some() {
    format!(
      r#"<span class="org-chip" title="Organization document">{}</span>"#,
      icon("building")
    )
  } else {
    String::new()
  };
  // A share badge (icon + hover/focus popover) on documents shared to the user.
  let share_chip = match card.share {
    Some((from, access)) => format!(
      r##"<span class="share-badge" tabindex="0" role="button" aria-label="Shared by {from}, {access}">{icon}<span class="share-popover"><strong>Shared with you</strong><span>From {from}</span><span>{access}</span></span></span>"##,
      from = esc(from),
      access = esc(access_label(access)),
      icon = icon("share"),
    ),
    None => String::new(),
  };
  let document_label = if card.has_blob {
    format_bytes(card.doc_bytes)
  } else {
    "not on server".to_string()
  };
  let card_tags = if card.tags.is_empty() {
    String::new()
  } else {
    let chips: String = card
      .tags
      .iter()
      .map(|tag| format!(r#"<span class="tag">{}</span>"#, esc(tag)))
      .collect();
    format!(r#"<div class="book-tags">{chips}</div>"#)
  };

  let card_html = format!(
    r##"<a class="book-card" href="#{modal_id}">
        <div class="book-card-head"><h3>{title}</h3><span class="head-badges">{share_chip}{org_chip}<span class="badge">{format}</span></span></div>
        <div class="bar"><span style="width:{pct}%"></span></div>
        <div class="book-meta"><span>{pct}% read</span><span>{time}</span></div>
        <div class="book-storage"><span>Document {doc}</span><span>Metadata {meta}</span></div>
        <div class="book-foot">Last read {date}</div>
        {card_tags}
      </a>"##,
    title = esc(&book.title),
    format = esc(&book.format),
    share_chip = share_chip,
    org_chip = org_chip,
    pct = card.pct,
    time = humanize_duration(card.secs),
    doc = document_label,
    meta = format_bytes(card.meta_bytes),
    date = format_date_utc(card.last_read),
    card_tags = card_tags,
  );

  let modal_html = format!(
    r##"<div class="modal" id="{modal_id}" role="dialog" aria-modal="true" aria-labelledby="{modal_id}-title">
        <a class="modal-backdrop" href="#library" aria-label="Close"></a>
        <div class="modal-card">
          <div class="modal-head"><h3 id="{modal_id}-title">{title}</h3><a class="modal-close" href="#library" aria-label="Close">&times;</a></div>
          <div class="book-meta"><span class="badge">{format}</span><span>{visibility}</span><span>{pct}% read</span><span>{time}</span></div>
          <p class="book-foot">Last read {date}</p>
          <div class="modal-section"><h4>Storage</h4>
            <div class="storage-detail">
              <div class="storage-row"><span>Document</span><strong>{doc}</strong></div>
              <div class="storage-row"><span>Metadata</span><strong>{meta}</strong></div>
            </div>
          </div>
          {shared_section}
          {tags_section}
          {sync_section}
          {move_section}
          {delete_section}
        </div>
      </div>"##,
    title = esc(&book.title),
    format = esc(&book.format),
    visibility = esc(visibility),
    pct = card.pct,
    time = humanize_duration(card.secs),
    date = format_date_utc(card.last_read),
    doc = document_label,
    meta = format_bytes(card.meta_bytes),
    shared_section = shared_section(card),
    tags_section = tags_section(card),
    sync_section = sync_section(card, owned),
    move_section = move_section(card, owned),
    delete_section = delete_section(card, owned),
  );
  (card_html, modal_html)
}

/// The account-wide sync ceiling control (owner-only). Each of the owner's
/// devices may pick an equal-or-more-restrictive mode locally; this sets the
/// maximum they clamp against.
fn sync_section(card: &BookCard, owned: bool) -> String {
  if !owned {
    return String::new();
  }
  format!(
    r#"<div class="modal-section"><h4>Sync</h4>
      <p class="muted">How this document syncs across your devices. Each device may choose a more restrictive mode locally.</p>
      <form method="post" action="/app/books/{hash}/sync-mode" class="inline-form">{csrf}{select}<button type="submit">Save</button></form>
    </div>"#,
    hash = esc(&card.book.content_hash),
    csrf = csrf_input(card.user),
    select = sync_mode_select("sync_mode", &card.book.sync_mode),
  )
}

/// The "Shared with you" modal section for a document shared to the viewer:
/// who shared it, the access level, and an unshare (leave) control.
fn shared_section(card: &BookCard) -> String {
  let Some((from, access)) = card.share else {
    return String::new();
  };
  format!(
    r#"<div class="modal-section"><h4>Shared with you</h4>
      <p class="muted">Shared by <strong>{from}</strong> · {access}.</p>
      <form method="post" action="/app/books/{hash}/unshare" class="inline-form" onsubmit="return confirm('Remove this shared document from your library?')">{csrf}<button class="danger" type="submit">Unshare</button></form>
    </div>"#,
    from = esc(from),
    access = esc(access_label(access)),
    hash = esc(&card.book.content_hash),
    csrf = csrf_input(card.user),
  )
}

fn tags_section(card: &BookCard) -> String {
  let hash = esc(&card.book.content_hash);
  let mut chips = String::new();
  for tag in card.tags {
    chips.push_str(&format!(
      r#"<span class="tag">{name}<form method="post" action="/app/books/{hash}/tags/remove" class="inline-form">{csrf}<input type="hidden" name="tag" value="{name}"><button type="submit" aria-label="Remove tag">&times;</button></form></span>"#,
      name = esc(tag),
      hash = hash,
      csrf = csrf_input(card.user),
    ));
  }
  format!(
    r#"<div class="modal-section"><h4>Tags</h4>
      <div class="tag-row">{chips}</div>
      <form method="post" action="/app/books/{hash}/tags" class="inline-form">{csrf}<input name="tag" placeholder="Add tag" required><button type="submit">Add tag</button></form>
    </div>"#,
    chips = chips,
    hash = hash,
    csrf = csrf_input(card.user),
  )
}

fn move_section(card: &BookCard, owned: bool) -> String {
  if !owned {
    return String::new();
  }
  format!(
    r#"<div class="modal-section"><h4>Ownership</h4>
      <p class="muted">Transfer ownership to an organization you belong to, or keep it private.</p>
      <form method="post" action="/app/books/{}/organization" class="inline-form">{}{}<button type="submit">Transfer ownership</button></form>
    </div>"#,
    esc(&card.book.content_hash),
    csrf_input(card.user),
    organization_select(
      "organization_id",
      card.book.organization_id.as_deref(),
      card.organizations,
    ),
  )
}

fn delete_section(card: &BookCard, owned: bool) -> String {
  if !owned {
    return String::new();
  }
  let hash = esc(&card.book.content_hash);
  let mut actions = String::new();
  if card.has_blob {
    actions.push_str(&format!(
      r#"<form method="post" action="/app/books/{hash}/blob/delete" class="inline-form" onsubmit="return confirm('Delete this document from the server? Your local copy is kept. Reclaims {reclaim}.')">{csrf}<button class="danger" type="submit">Delete document · reclaim {reclaim}</button></form>"#,
      hash = hash,
      csrf = csrf_input(card.user),
      reclaim = format_bytes(card.doc_bytes),
    ));
  }
  actions.push_str(&format!(
    r#"<form method="post" action="/app/books/{hash}/delete" class="inline-form" onsubmit="return confirm('Delete this document and its metadata from the server? Reclaims {reclaim}.')">{csrf}<button class="danger" type="submit">Delete metadata · reclaim {reclaim}</button></form>"#,
    hash = hash,
    csrf = csrf_input(card.user),
    reclaim = format_bytes(card.doc_bytes + card.meta_bytes),
  ));
  format!(
    r#"<div class="modal-section"><h4>Delete</h4><div class="button-row">{actions}</div></div>"#
  )
}

//! Styling for the `/docs` documentation center: the search box, index cards,
//! the page/table-of-contents layout, prose typography, and search hits. Uses
//! the shared design tokens from `base.rs`.
pub(crate) const APP_CSS_DOCS: &str = r#"
    .doc-hero { padding:8px 0 4px; max-width:720px; }
    .doc-hero h1 { font-size:38px; margin:6px 0 8px; }
    .doc-hero p.muted { font-size:16px; }
    .doc-search { display:flex; gap:10px; margin:20px 0 26px; max-width:640px; }
    .doc-search-box { position:relative; flex:1; min-width:0; }
    .doc-search-box input { width:100%; }
    .doc-search-menu { position:absolute; left:0; right:0; top:calc(100% + 6px); z-index:40; margin:0;
      padding:6px; list-style:none; display:grid; gap:2px; background:var(--panel); border:1px solid var(--line);
      border-radius:10px; box-shadow:var(--shadow); max-height:min(60vh,430px); overflow-y:auto; }
    .doc-search-menu[hidden] { display:none; }
    .doc-search-option { display:grid; gap:3px; padding:9px 11px; border-radius:8px; cursor:pointer; }
    .doc-search-option.is-active { background:var(--panel-2); }
    .doc-search-crumb { color:var(--ink); font-weight:750; font-size:13px; white-space:nowrap;
      overflow:hidden; text-overflow:ellipsis; }
    .doc-search-snippet { color:var(--muted); font-size:12.5px; line-height:1.5; display:-webkit-box;
      -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
    .doc-search-snippet mark { color:var(--ink); }
    .doc-card-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:16px; }
    .doc-card { display:flex; flex-direction:column; gap:8px; padding:20px; border:1px solid var(--line);
      border-radius:10px; background:var(--panel); box-shadow:var(--shadow); text-decoration:none; color:inherit;
      transition:border-color .15s, transform .15s; }
    .doc-card:hover { border-color:var(--accent); transform:translateY(-2px); }
    .doc-card h2 { margin:0; font-size:18px; }
    .doc-card p { margin:0; color:var(--muted); font-size:14px; line-height:1.55; }
    .doc-card-more { margin-top:auto; color:var(--accent); font-weight:750; font-size:14px; }
    .doc-layout { display:grid; grid-template-columns:minmax(0,1fr) 232px; gap:40px; align-items:start; }
    .doc-layout-single { grid-template-columns:minmax(0,1fr); }
    .doc-toc { position:sticky; top:74px; order:2; align-self:start; display:grid; gap:2px;
      padding:14px 4px 14px 14px; border-left:1px solid var(--line); }
    .doc-toc-head { margin:0 0 6px 10px; color:var(--muted); font-size:12px; font-weight:850; text-transform:uppercase; letter-spacing:.04em; }
    .doc-toc nav { display:grid; gap:1px; }
    .doc-toc-item { display:block; padding:5px 10px; border-radius:6px; color:var(--muted);
      text-decoration:none; font-size:13px; font-weight:650; line-height:1.35; }
    .doc-toc-item:hover { color:var(--ink); background:var(--panel-2); }
    .doc-content { min-width:0; max-width:760px; }
    .doc-content > :first-child { margin-top:0; }
    .doc-content h1 { font-size:34px; margin:0 0 18px; scroll-margin-top:74px; }
    .doc-content h2 { font-size:24px; margin:34px 0 12px; padding-top:6px; scroll-margin-top:74px; }
    .doc-content h3 { font-size:18px; margin:26px 0 10px; scroll-margin-top:74px; }
    .doc-content h4 { font-size:15px; margin:20px 0 8px; scroll-margin-top:74px; }
    .doc-content p { margin:0 0 14px; }
    .doc-content ul,.doc-content ol { margin:0 0 14px; padding-left:24px; }
    .doc-content li { margin:5px 0; }
    .doc-content li > p { margin:0; }
    .doc-content a { color:var(--accent); text-decoration:none; font-weight:650; }
    .doc-content a:hover { text-decoration:underline; }
    .doc-content code { font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:.9em;
      padding:1px 5px; border-radius:5px; background:var(--panel-2); color:var(--ink); }
    .doc-content pre { margin:0 0 16px; }
    .doc-content pre code { padding:0; background:transparent; color:inherit; font-size:13px; }
    .doc-content blockquote { margin:0 0 16px; padding:6px 16px; border-left:3px solid var(--line);
      color:var(--muted); }
    .doc-content hr { border:0; border-top:1px solid var(--line); margin:26px 0; }
    .doc-content table { display:block; width:auto; min-width:0; max-width:100%; overflow-x:auto;
      border-collapse:collapse; margin:0 0 16px; font-size:14px; }
    .doc-content th,.doc-content td { border:1px solid var(--line); padding:8px 12px; text-align:left; white-space:nowrap; }
    .doc-content th { background:var(--panel-2); color:var(--ink); text-transform:none; font-size:13px; }
    .doc-content img { max-width:100%; height:auto; }
    mark { background:color-mix(in srgb, var(--accent) 34%, transparent); color:inherit; border-radius:3px; padding:0 2px; }
    mark.is-active { background:color-mix(in srgb, var(--accent) 60%, transparent); }
    /* The passage holding the jumped-to match: a brief tinted flash that fades. */
    .doc-hit-flash { animation:doc-flash 2.6s ease-out 1; border-radius:6px; }
    @keyframes doc-flash {
      0%,30% { background:color-mix(in srgb, var(--accent) 20%, transparent); box-shadow:0 0 0 6px color-mix(in srgb, var(--accent) 20%, transparent); }
      100% { background:transparent; box-shadow:0 0 0 6px transparent; }
    }
    .doc-results { margin-top:6px; }
    .doc-hit-list { display:grid; gap:10px; margin-top:14px; }
    .doc-hit { display:grid; gap:5px; padding:15px 18px; border:1px solid var(--line); border-radius:10px;
      background:var(--panel); box-shadow:var(--shadow); text-decoration:none; color:inherit;
      transition:border-color .15s, transform .15s; }
    .doc-hit:hover { border-color:var(--accent); transform:translateY(-1px); }
    .doc-hit-crumb { color:var(--ink); font-weight:800; font-size:14px; }
    .doc-hit-sep { color:var(--muted); margin:0 4px; font-weight:600; }
    .doc-hit-snippet { color:var(--muted); font-size:13px; line-height:1.55; }
    .doc-hit-snippet mark { color:var(--ink); }
    @media (max-width: 860px) {
      .doc-layout { grid-template-columns:minmax(0,1fr); }
      .doc-toc { position:static; order:-1; top:auto; border-left:0; border-bottom:1px solid var(--line);
        padding:0 0 14px; margin-bottom:8px; }
    }
"#;

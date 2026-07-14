//! Component styles layered after the base sheet: card badges, library
//! controls, tags, and the notification bell.
pub(crate) const APP_CSS_COMPONENTS: &str = r#"
    .head-badges { display:flex; align-items:center; gap:6px; flex:none; }
    .org-chip { display:inline-flex; align-items:center; color:var(--accent); }
    .org-chip svg { width:15px; height:15px; }
    .share-badge { position:relative; display:inline-flex; align-items:center; color:var(--accent); cursor:pointer; }
    .share-badge svg { width:15px; height:15px; }
    .share-popover { position:absolute; top:calc(100% + 6px); right:0; z-index:30; display:none; flex-direction:column; gap:2px;
      min-width:190px; padding:9px 11px; background:var(--panel); border:1px solid var(--line); border-radius:8px;
      box-shadow:0 8px 24px rgba(0,0,0,.32); font-size:12px; text-align:left; white-space:normal; }
    .share-badge:hover .share-popover, .share-badge:focus .share-popover, .share-badge:focus-within .share-popover { display:flex; }
    .share-popover strong { color:var(--ink); }
    .share-popover span { color:var(--muted); }
    .library-controls { display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin:12px 0; }
    .library-controls input[name="q"] { flex:1; min-width:160px; }
    .book-tags { display:flex; flex-wrap:wrap; gap:4px; margin-top:6px; }
    .tag-row { display:flex; flex-wrap:wrap; gap:6px; margin-bottom:8px; }
    .tag { display:inline-flex; align-items:center; gap:4px; padding:1px 8px; border-radius:999px;
      background:var(--panel); border:1px solid var(--line); color:var(--muted); font-size:11px; }
    .tag form { display:inline; }
    .tag button { background:none; border:none; color:var(--muted); cursor:pointer; padding:0; line-height:1; }
    .nav-user { display:flex; align-items:center; gap:8px; }
    .notif-menu { position:relative; }
    .notif-trigger { position:relative; display:inline-flex; cursor:pointer; padding:6px; border-radius:8px; list-style:none; }
    .notif-trigger::-webkit-details-marker { display:none; }
    .notif-trigger svg { width:20px; height:20px; }
    .notif-badge { position:absolute; top:-2px; right:-2px; background:var(--accent); color:#fff;
      border-radius:999px; font-size:10px; min-width:16px; height:16px; display:flex; align-items:center; justify-content:center; padding:0 4px; }
    .count-badge { display:inline-flex; align-items:center; justify-content:center; margin-left:6px;
      background:var(--accent); color:#fff; border-radius:999px; font-size:11px; min-width:18px; height:18px; padding:0 5px; }
    .notif-dropdown { position:absolute; right:0; top:100%; margin-top:6px; width:320px; max-height:400px; overflow:auto;
      background:var(--panel); border:1px solid var(--line); border-radius:10px; padding:6px; z-index:50; }
    .notif-item { display:flex; justify-content:space-between; gap:8px; padding:8px; }
    .notif-item + .notif-item { border-top:1px solid var(--line); }
    .notif-item strong { display:block; font-size:13px; }
    .notif-item span { color:var(--muted); font-size:12px; }
    .notif-item.notif-critical strong { color:#ff6b6b; }
    .notif-item form { display:inline; }
    .notif-item button { background:none; border:none; color:var(--muted); cursor:pointer; font-size:16px; }
    .notif-empty { padding:12px; color:var(--muted); font-size:13px; text-align:center; }
    .panel-head { display:flex; align-items:baseline; justify-content:space-between; gap:12px; flex-wrap:wrap; }
    .panel-head h2 { margin:0; }
    .device-quota { font-size:13px; font-weight:740; color:var(--muted); padding:2px 10px;
      border:1px solid var(--line); border-radius:999px; background:var(--panel-2); white-space:nowrap; }
"#;

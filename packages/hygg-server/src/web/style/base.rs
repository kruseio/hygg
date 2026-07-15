pub(crate) const APP_CSS_BASE: &str = r#"
    :root { --bg:#f4f6f8; --ink:#17201f; --muted:#65716f; --panel:#ffffff;
      --panel-2:#edf3f1; --line:#d9e1df; --accent:#116a5c; --accent-ink:#ffffff;
      --danger:#b42318; --danger-ink:#ffffff; --shadow:0 1px 2px rgba(16,24,40,.08);
      color-scheme: light dark; }
    @media (prefers-color-scheme: dark) {
      :root { --bg:#101312; --ink:#edf2f0; --muted:#9aa6a3; --panel:#171b1a;
        --panel-2:#1f2725; --line:#303a37; --accent:#5dbdac; --accent-ink:#071211;
        --danger:#ff867a; --danger-ink:#180302; --shadow:none; }
    }
    * { box-sizing:border-box; } body { margin:0; background:var(--bg);
      color:var(--ink); font:15px/1.5 ui-sans-serif,system-ui,-apple-system,Segoe UI,sans-serif;
      letter-spacing:0; }
    .sidenav-toggle { position:fixed; inset:0 auto auto 0; width:1px; height:1px; opacity:0; pointer-events:none; }
    .sidenav-backdrop { display:none; }
    .app-shell { display:grid; grid-template-columns:272px minmax(0,1fr); min-height:100vh; transition:grid-template-columns .18s ease; }
    .sidenav-toggle:checked ~ .app-shell { grid-template-columns:0 minmax(0,1fr); }
    .content-shell { min-width:0; } main { max-width:1160px; margin:0 auto; padding:26px 24px 64px; }
    .sidenav { position:sticky; top:0; align-self:start; height:100vh; overflow:auto; padding:18px 14px;
      border-right:1px solid var(--line); background:var(--panel); transition:transform .18s ease, opacity .18s ease, padding .18s ease; }
    .sidenav-toggle:checked ~ .app-shell .sidenav { transform:translateX(-100%); opacity:0; pointer-events:none; padding-left:0; padding-right:0; border-right:0; }
    .sidenav-links { display:grid; gap:10px; margin-top:24px; }
    .nav-group { border:1px solid transparent; border-radius:8px; }
    .nav-group summary { display:flex; align-items:center; gap:9px; min-height:38px; padding:8px 10px; border-radius:7px;
      color:var(--muted); cursor:pointer; list-style:none; font-size:12px; font-weight:850; text-transform:uppercase; }
    .nav-group summary::-webkit-details-marker { display:none; }
    .nav-group summary:hover { color:var(--ink); background:var(--panel-2); }
    .nav-group summary .nav-chevron { margin-left:auto; transition:transform .16s ease; }
    .nav-group[open] > summary .nav-chevron { transform:rotate(180deg); }
    .nav-group-items { display:grid; gap:3px; padding:2px 0 7px 28px; }
    .nav-group-items a { display:flex; min-height:36px; align-items:center; gap:9px; padding:7px 10px; border-radius:7px;
      color:var(--muted); text-decoration:none; font-weight:740; }
    .nav-group-items a:hover { color:var(--ink); background:var(--panel-2); }
    .icon { width:18px; height:18px; flex:none; fill:none; stroke:currentColor; stroke-width:2; stroke-linecap:round; stroke-linejoin:round; }
    .topbar { position:sticky; top:0; z-index:10; display:flex; gap:12px; align-items:center; justify-content:flex-end;
      min-height:58px; padding:0 24px; border-bottom:1px solid var(--line);
      background:var(--panel); background:color-mix(in srgb, var(--panel) 92%, transparent); backdrop-filter:blur(12px); }
    .topbar-spacer { flex:1; min-width:0; }
    .sidenav-toggle-button { display:inline-flex; align-items:center; justify-content:center; width:38px; height:38px;
      border:1px solid var(--line); border-radius:7px; background:var(--panel); color:var(--muted); cursor:pointer; }
    .sidenav-toggle-button:hover { color:var(--ink); background:var(--panel-2); }
    .brand { color:var(--ink); text-decoration:none; font-size:18px; font-weight:800; }
    .nav-user { margin-left:auto; display:flex; align-items:center; color:var(--muted); }
    .account-menu { position:relative; }
    .account-trigger { display:inline-flex; align-items:center; justify-content:center; gap:8px; min-height:40px;
      padding:3px 8px 3px 4px; border:1px solid var(--line); border-radius:999px; background:var(--panel); cursor:pointer; list-style:none; }
    .account-trigger::-webkit-details-marker { display:none; }
    .account-trigger:focus-visible { outline:2px solid color-mix(in srgb, var(--accent) 40%, transparent); outline-offset:2px; }
    .account-trigger:hover { background:var(--panel-2); color:var(--ink); }
    .account-trigger .nav-chevron { width:15px; height:15px; transition:transform .16s ease; }
    .account-menu[open] .account-trigger .nav-chevron { transform:rotate(180deg); }
    .account-trigger-name { max-width:132px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; color:var(--ink); font-weight:750; font-size:13px; }
    .account-avatar-icon { width:34px; height:34px; padding:6px; border:1px solid var(--line); border-radius:999px;
      background:var(--panel-2); color:var(--muted); }
    .account-trigger:hover .account-avatar-icon { color:var(--ink); }
    .account-dropdown { position:absolute; right:0; top:48px; z-index:30; width:264px; padding:8px;
      border:1px solid var(--line); border-radius:8px; background:var(--panel); box-shadow:0 12px 32px rgba(16,24,40,.16); }
    .account-dropdown-header { display:grid; gap:2px; padding:8px 10px 10px; border-bottom:1px solid var(--line); margin-bottom:6px; }
    .account-dropdown-header strong { color:var(--ink); font-size:14px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    .account-dropdown-header span { color:var(--muted); font-size:12px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    .account-dropdown form { margin:0; }
    .account-dropdown a,.account-dropdown button.dropdown-submit { display:flex; align-items:center; justify-content:flex-start; width:100%;
      gap:9px; min-height:36px; padding:8px 10px; border:0; border-radius:6px; background:transparent; color:var(--ink);
      text-decoration:none; font-weight:700; cursor:pointer; }
    .account-dropdown a span,.account-dropdown button.dropdown-submit span { color:var(--ink); font-size:14px; }
    .account-dropdown a:hover,.account-dropdown button.dropdown-submit:hover { background:var(--panel-2); }
    .account-dropdown button.dropdown-submit { font:inherit; }
    h1,h2 { margin:0 0 14px; line-height:1.2; font-weight:750; }
    h1 { font-size:32px; } h2 { font-size:18px; } .hero { padding:46px 0 30px; max-width:760px; }
    .hero h1 { font-size:48px; margin-bottom:12px; } .hero p { max-width:620px; font-size:17px; }
    .eyebrow,.muted { color:var(--muted); } .eyebrow { margin:0 0 8px; font-weight:700; }
    .landing-hero { display:block; min-height:auto; padding:42px 0 28px; }
    .landing-copy h1 { max-width:720px; margin:0 0 16px; font-size:64px; line-height:1.02; letter-spacing:0; }
    .landing-copy p { max-width:660px; font-size:18px; color:var(--muted); }
    .hero-proof { display:flex; gap:8px; flex-wrap:wrap; margin-top:20px; }
    .hero-proof span { padding:6px 9px; border:1px solid var(--line); border-radius:999px; background:var(--panel); color:var(--muted); font-size:12px; font-weight:800; }
    .landing-demo { width:100%; margin:0 0 38px; border:1px solid var(--line); border-radius:8px; overflow:hidden;
      background:#0f1716; box-shadow:0 22px 60px rgba(16,24,40,.18); color:#e9f6f2; }
    .demo-gif { display:block; width:100%; height:auto; object-fit:contain; background:#0f1716; }
    .terminal-bar { display:flex; align-items:center; gap:7px; min-height:42px; padding:0 14px; border-bottom:1px solid rgba(255,255,255,.12);
      background:rgba(255,255,255,.06); }
    .terminal-bar span { width:10px; height:10px; border-radius:999px; background:#f36f63; }
    .terminal-bar span:nth-child(2) { background:#f2c14e; } .terminal-bar span:nth-child(3) { background:#59c087; }
    .terminal-bar strong { margin-left:8px; color:#b9c8c4; font-size:13px; }
    .reader-window { display:grid; grid-template-columns:136px 1fr; gap:18px; position:relative; padding:18px; min-height:388px; }
    .reader-window aside { display:grid; align-content:start; gap:9px; padding-right:14px; border-right:1px solid rgba(255,255,255,.1); color:#8fa29d; font-size:13px; }
    .reader-window aside strong { color:#effbf8; margin-bottom:5px; } .reader-window aside span { padding:7px 8px; border-radius:6px; }
    .reader-window aside .active { background:rgba(93,189,172,.18); color:#f4fffc; }
    .reader-window section { min-width:0; padding-top:12px; }
    .reader-line { margin:0 0 16px; font:15px/1.65 ui-monospace,SFMono-Regular,Menlo,monospace; color:#dce8e4; }
    .reader-line.dim { color:#839691; } .reader-line.highlight { display:inline; padding:3px 5px; border-radius:5px; background:rgba(242,193,78,.18); color:#fff4cf; }
    .reader-progress { height:8px; margin-top:28px; border-radius:999px; background:rgba(255,255,255,.12); overflow:hidden; }
    .reader-progress span { display:block; width:68%; height:100%; background:#5dbdac; }
    .note-card { position:absolute; right:18px; bottom:18px; width:min(230px,45%); padding:14px; border:1px solid rgba(255,255,255,.14);
      border-radius:8px; background:rgba(9,17,16,.82); }
    .note-card p { margin:5px 0 0; color:#b9c8c4; font-size:13px; }
    .landing-section { padding:38px 0; } .section-kicker { margin-bottom:14px; color:var(--muted); font-weight:800; text-transform:uppercase; font-size:12px; }
    .landing-feature-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(250px,1fr)); gap:14px; }
    .landing-feature-grid div { display:grid; gap:7px; padding:18px; border:1px solid var(--line); border-radius:8px; background:var(--panel); box-shadow:var(--shadow); }
    .landing-feature-grid span { color:var(--muted); }
    .landing-band { display:grid; grid-template-columns:minmax(0,1fr) minmax(320px,520px); gap:26px; align-items:center;
      margin:24px 0; padding:28px; border:1px solid var(--line); border-radius:8px; background:var(--panel); box-shadow:var(--shadow); }
    .landing-band h2 { font-size:28px; } .flow-list { display:grid; gap:12px; margin:0; padding:0; list-style:none; }
    .flow-list li { display:grid; grid-template-columns:40px 1fr; gap:12px; align-items:center; }
    .flow-list strong { display:inline-flex; width:36px; height:36px; align-items:center; justify-content:center; border-radius:999px; background:var(--panel-2); color:var(--accent); }
    .flow-list span { color:var(--muted); } .cta-band { grid-template-columns:1fr auto; }
    .sync-visual { display:grid; gap:16px; }
    .sync-map { width:100%; min-height:220px; overflow:visible; }
    .sync-map text { font:800 12px/1 ui-sans-serif,system-ui,sans-serif; fill:var(--muted); }
    .sync-node rect { fill:var(--panel-2); stroke:var(--line); stroke-width:1.5; }
    .sync-node .sync-icon { fill:none; stroke:var(--accent); stroke-width:2; stroke-linecap:round; stroke-linejoin:round; }
    .sync-server rect { fill:var(--ink); stroke:var(--ink); } .sync-server text { fill:var(--bg); }
    .sync-server .sync-icon { stroke:var(--bg); }
    .sync-line { fill:none; stroke:var(--accent); stroke-width:3; stroke-linecap:round; stroke-dasharray:12 12;
      opacity:.74; animation:sync-dash 3.2s linear infinite; }
    .sync-line-alt { stroke:#c47a5a; animation-duration:3.6s; } .sync-dot { fill:var(--accent); }
    .sync-dot-alt { fill:#c47a5a; } @keyframes sync-dash { to { stroke-dashoffset:-48; } }
    @media (prefers-reduced-motion:reduce) { .sync-line { animation:none; } .sync-dot { display:none; } }
    .actions,.button-row { display:flex; gap:10px; flex-wrap:wrap; align-items:center; }
    .button,button { display:inline-flex; align-items:center; justify-content:center; min-height:38px;
      border:1px solid var(--accent); background:var(--accent); color:var(--accent-ink);
      border-radius:7px; padding:8px 13px; text-decoration:none; font-weight:750; cursor:pointer; }
    button:disabled { cursor:not-allowed; opacity:.62; } .button.secondary,button.secondary,button.ghost {
      background:transparent; color:var(--accent); }
    button.ghost { border-color:var(--line); color:var(--muted); } button.ghost:hover { color:var(--ink); }
    button.danger,.danger { border-color:var(--danger); background:var(--danger); color:var(--danger-ink); }
    .panel,.stat,.metric-card { background:var(--panel); border:1px solid var(--line); border-radius:8px;
      box-shadow:var(--shadow); padding:20px; margin:18px 0; overflow-x:auto; }
    .book-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(248px,1fr)); gap:14px; }
    .book-card { display:flex; flex-direction:column; gap:10px; padding:16px; border:1px solid var(--line);
      border-radius:10px; background:var(--panel-2); }
    a.book-card { text-decoration:none; color:inherit; cursor:pointer; transition:border-color .15s, transform .15s; }
    a.book-card:hover { border-color:var(--accent); transform:translateY(-2px); }
    .storage-meter { display:flex; flex-direction:column; gap:8px; margin:4px 0 16px; }
    .storage-meter-head { display:flex; justify-content:space-between; gap:10px; font-size:13px; font-weight:700; }
    .storage-meter .muted { margin:0; font-size:12px; }
    .modal { position:fixed; inset:0; z-index:60; display:none; align-items:center; justify-content:center; padding:18px; }
    .modal:target { display:flex; }
    .modal-backdrop { position:absolute; inset:0; background:rgba(0,0,0,.62); }
    .modal-card { position:relative; z-index:1; width:min(460px,100%); max-height:88vh; overflow:auto;
      display:flex; flex-direction:column; gap:14px; padding:20px; border:1px solid var(--line);
      border-radius:12px; background:var(--panel); box-shadow:0 24px 60px rgba(0,0,0,.4); }
    .modal-head { display:flex; align-items:flex-start; justify-content:space-between; gap:12px; }
    .modal-head h3 { margin:0; font-size:18px; word-break:break-word; }
    .modal-close { flex:none; font-size:22px; line-height:1; text-decoration:none; color:var(--muted); padding:0 4px; }
    .modal-close:hover { color:var(--ink); }
    .modal-section { display:flex; flex-direction:column; gap:8px; }
    .modal-section h4 { margin:0; font-size:12px; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); }
    .storage-detail { display:flex; flex-direction:column; gap:6px; }
    .storage-row { display:flex; justify-content:space-between; gap:10px; font-size:13px; }
    .storage-row span { color:var(--muted); }
    .book-card-head { display:flex; align-items:flex-start; justify-content:space-between; gap:10px; }
    .book-card-head h3 { margin:0; font-size:16px; line-height:1.3; word-break:break-word; }
    .badge { flex:none; padding:2px 8px; border-radius:999px; border:1px solid var(--line);
      background:var(--panel); color:var(--muted); font-size:11px; font-weight:800; text-transform:uppercase; }
    .book-card .bar { height:8px; border-radius:999px; background:rgba(255,255,255,.12); overflow:hidden; }
    .book-card .bar span { display:block; height:100%; background:var(--accent); }
    .book-meta { display:flex; flex-wrap:wrap; gap:10px; color:var(--muted); font-size:13px; }
    .book-meta span:first-child { color:var(--ink); font-weight:700; }
    .book-storage { display:flex; flex-wrap:wrap; gap:10px; color:var(--muted); font-size:12px; }
    .book-foot { color:var(--muted); font-size:12px; }
    .account-card { max-width:920px; padding:0; overflow:hidden; }
    .account-card-header { display:grid; grid-template-columns:auto minmax(0,1fr) auto; gap:14px; align-items:center;
      padding:22px 24px; border-bottom:1px solid var(--line); background:var(--panel); }
    .account-card-header h1 { margin:0; font-size:30px; }
    .account-card-header .eyebrow { margin-bottom:4px; }
    .account-avatar-large { display:inline-flex; align-items:center; justify-content:center; width:48px; height:48px;
      border:1px solid var(--line); border-radius:999px; background:var(--panel-2); color:var(--accent); }
    .account-avatar-large .account-avatar-icon { width:34px; height:34px; padding:5px; border:0; background:transparent; color:inherit; }
    .status-pill { display:inline-flex; align-items:center; justify-content:center; min-height:30px; padding:5px 10px;
      border-radius:999px; border:1px solid var(--line); font-size:12px; font-weight:850; white-space:nowrap; }
    .status-enabled { border-color:color-mix(in srgb, var(--accent) 30%, var(--line)); background:color-mix(in srgb, var(--accent) 12%, transparent);
      color:var(--accent); }
    .status-disabled { border-color:color-mix(in srgb, var(--danger) 30%, var(--line)); background:color-mix(in srgb, var(--danger) 10%, transparent);
      color:var(--danger); }
    .account-summary { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); border-bottom:1px solid var(--line); }
    .account-summary-item { display:grid; grid-template-columns:36px minmax(0,1fr); gap:10px; align-items:center; min-width:0;
      padding:18px 20px; border-right:1px solid var(--line); }
    .account-summary-item:last-child { border-right:0; }
    .summary-icon,.account-form-title > .icon { display:inline-flex; align-items:center; justify-content:center; width:36px; height:36px;
      border-radius:8px; background:var(--panel-2); color:var(--accent); padding:9px; }
    .account-summary-item span:not(.summary-icon) { display:block; margin-bottom:2px; color:var(--muted); font-size:12px;
      font-weight:800; text-transform:uppercase; }
    .account-summary-item strong { display:block; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:15px; }
    .account-security { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); }
    .account-security-form { display:grid; gap:14px; align-content:start; padding:22px 24px; }
    .account-security-form + .account-security-form { border-left:1px solid var(--line); }
    .account-form-title { display:grid; grid-template-columns:36px minmax(0,1fr); gap:10px; align-items:center; }
    .account-form-title h2 { margin:0 0 2px; font-size:17px; }
    .account-form-title span { color:var(--muted); font-size:13px; }
    .account-password-row { display:flex; gap:10px; align-items:center; }
    .account-password-row input { min-width:0; }
    .segmented-radio { display:grid; grid-template-columns:1fr 1fr; gap:3px; padding:3px; border:1px solid var(--line);
      border-radius:8px; background:var(--panel-2); }
    .segmented-radio label { position:relative; display:flex; min-height:38px; align-items:center; justify-content:center;
      border-radius:6px; color:var(--muted); font-weight:800; cursor:pointer; }
    .segmented-radio input { position:absolute; inset:0; opacity:0; cursor:pointer; }
    .segmented-radio label:has(input:checked) { background:var(--panel); color:var(--ink); box-shadow:0 1px 2px rgba(16,24,40,.08); }
    .segmented-radio label:has(input:disabled) { cursor:not-allowed; opacity:.55; }
    .segmented-radio input:focus-visible + span { outline:2px solid color-mix(in srgb, var(--accent) 35%, transparent);
      outline-offset:4px; border-radius:4px; }
    .auth { max-width:440px; margin:54px auto 0; } .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(160px,1fr)); gap:12px; }
    .stat strong { display:block; font-size:30px; line-height:1.1; } .stat span { color:var(--muted); }
    .dashboard-header { display:flex; align-items:end; justify-content:space-between; gap:18px; flex-wrap:wrap; margin:8px 0 18px; }
    .dashboard-header h1 { margin-bottom:6px; } .metric-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:12px; margin:18px 0; }
    .metric-card { margin:0; min-height:126px; display:flex; flex-direction:column; justify-content:space-between; }
    .metric-card span,.metric-card small { color:var(--muted); } .metric-card strong { font-size:34px; line-height:1.05; }
    .split-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(300px,1fr)); gap:18px; align-items:start; }
    .split-grid .panel { margin:0 0 18px; } .section-title { display:flex; align-items:start; justify-content:space-between; gap:12px; margin-bottom:14px; }
    .section-title p { margin:0; } .breakdown-row { display:grid; grid-template-columns:1fr minmax(96px,2fr) 56px; gap:12px; align-items:center; padding:10px 0; border-top:1px solid var(--line); }
    .breakdown-row:first-of-type { border-top:0; } .breakdown-row div:first-child { display:flex; align-items:center; justify-content:space-between; gap:10px; }
    .breakdown-row span,.breakdown-row small { color:var(--muted); } .bar { height:9px; border-radius:999px; background:var(--panel-2); overflow:hidden; }
    .bar span { display:block; height:100%; background:var(--accent); border-radius:999px; }
    .funnel-list { display:grid; gap:10px; } .funnel-row { display:grid; grid-template-columns:1fr auto auto; gap:14px; align-items:center; padding:10px 0; border-top:1px solid var(--line); }
    .funnel-row:first-child { border-top:0; } .funnel-row small { color:var(--muted); }
    .action-panel { display:flex; align-items:end; justify-content:space-between; gap:16px; flex-wrap:wrap; }
    .cli-quickstart { display:grid; align-content:start; }
    .quickstart-list { display:grid; gap:14px; margin:0 0 14px; padding:0; list-style:none; }
    .quickstart-list li { display:grid; grid-template-columns:34px minmax(0,1fr); gap:12px; align-items:start; }
    .quickstart-list li > span { display:inline-flex; align-items:center; justify-content:center; width:34px; height:34px;
      border-radius:999px; background:var(--panel-2); color:var(--accent); font-weight:850; }
    .quickstart-list strong { display:block; margin-bottom:3px; }
    .quickstart-list p { margin:0; color:var(--muted); }
    .quickstart-code { margin:8px 0 0; white-space:pre-wrap; }
    .passkey-add input { min-width:220px; } table { width:100%; min-width:680px; border-collapse:collapse; }
    th,td { border-bottom:1px solid var(--line); padding:10px 8px; text-align:left; vertical-align:middle; }
    th { color:var(--muted); font-size:12px; font-weight:800; text-transform:uppercase; }
    td form { display:flex; gap:8px; align-items:center; flex-wrap:wrap; margin:0; }
    td form select,td form input { width:auto; } td a { color:var(--accent); font-weight:700; margin-right:10px; }
    .badge { display:inline-flex; align-items:center; margin-left:8px; padding:2px 6px; border-radius:999px;
      background:var(--panel-2); color:var(--muted); font:11px/1.4 ui-sans-serif,system-ui,sans-serif; }
    tr:last-child td { border-bottom:0; } input,select,textarea { width:100%; min-height:38px;
      border:1px solid var(--line); border-radius:7px; padding:8px 10px; background:var(--panel);
      color:var(--ink); accent-color:var(--accent); }
    input[type="checkbox"],input[type="radio"] { width:auto; min-height:auto; } input:focus,select:focus,textarea:focus { outline:2px solid color-mix(in srgb, var(--accent) 35%, transparent);
      outline-offset:1px; border-color:var(--accent); } textarea { min-height:92px; resize:vertical; }
    input[aria-invalid="true"] { border-color:var(--danger); box-shadow:0 0 0 3px color-mix(in srgb, var(--danger) 14%, transparent); }
    .stack { display:grid; gap:11px; max-width:520px; } .inline-form { display:flex; gap:9px; align-items:center; flex-wrap:wrap; }
    .inline-form input,.inline-form select { width:auto; } label { color:var(--muted); }
    .radio-group { display:flex; gap:14px; align-items:center; flex-wrap:wrap; }
    .radio-group label { display:inline-flex; gap:6px; align-items:center; }
    pre { background:#111817; color:#edf8f5; padding:14px; border-radius:8px; overflow:auto; }
    .mono,.secret { font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:13px; }
    .secret-row { display:grid; grid-template-columns:minmax(0,1fr) auto; gap:10px; align-items:stretch; margin-top:14px; }
    .secret-row .secret { margin:0; min-height:42px; display:flex; align-items:center; }
    .secret-row .secret-command { margin:0; white-space:pre-wrap; word-break:break-all; }
    .copy-secret-button { gap:7px; align-self:stretch; min-width:98px; }
    .back-link { display:inline-flex; align-items:center; gap:6px; margin-bottom:14px; color:var(--muted);
      text-decoration:none; font-weight:700; } .back-link:hover { color:var(--ink); }
    .back-link .icon { width:18px; height:18px; }
    .error { color:var(--danger); font-weight:750; } .form-status { min-height:22px; margin:8px 0 0; color:var(--muted); }
    .form-status.error { color:var(--danger); }
    .toast-stack { position:fixed; top:18px; right:18px; z-index:90; display:grid; gap:10px; width:min(380px, calc(100vw - 36px)); pointer-events:none; }
    .toast { padding:12px 14px; border:1px solid var(--line); border-left-width:4px; border-radius:8px; background:var(--panel);
      color:var(--ink); box-shadow:0 18px 44px rgba(16,24,40,.18); font-weight:750; }
    .toast-error { border-left-color:var(--danger); }
    dl { display:grid; grid-template-columns:max-content 1fr; gap:9px 18px; }
    dt { color:var(--muted); } dd { margin:0; }
"#;

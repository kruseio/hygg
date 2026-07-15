pub(crate) const APP_CSS_RESPONSIVE: &str = r#"
    @media (max-width:760px) {
      .app-shell { display:block; } main { padding:18px 14px 44px; }
      .sidenav { position:fixed; left:0; top:0; z-index:40; width:min(304px,86vw); height:100vh; border-right:1px solid var(--line);
        border-bottom:0; box-shadow:0 20px 60px rgba(16,24,40,.24); transform:translateX(-100%); opacity:1; pointer-events:auto; }
      .sidenav-toggle:checked ~ .app-shell .sidenav { transform:translateX(0); opacity:1; pointer-events:auto; padding:18px 14px; border-right:1px solid var(--line); }
      .sidenav-toggle:checked + .sidenav-backdrop { display:block; position:fixed; inset:0; z-index:35; background:rgba(7,18,17,.45); }
      .sidenav-links { gap:10px; margin-top:14px; }
      .topbar { position:sticky; padding:10px 14px; } .nav-user { margin-left:auto; } .hero h1 { font-size:38px; }
      .account-trigger-name { display:none; }
      .landing-hero { padding:30px 0 20px; }
      .landing-copy h1 { font-size:42px; } .landing-copy p { font-size:16px; }
      .landing-demo { margin-bottom:24px; } .reader-window { grid-template-columns:1fr; }
      .sync-map { min-height:180px; }
      .reader-window aside { border-right:0; border-bottom:1px solid rgba(255,255,255,.1); padding:0 0 12px; }
      .note-card { position:static; width:100%; margin-top:18px; } .landing-band,.cta-band { grid-template-columns:1fr; padding:18px; }
      .panel,.stat { padding:16px; } .inline-form input,.inline-form select,.passkey-add input { width:100%; }
      .account-card { padding:0; }
      .account-card-header { grid-template-columns:auto minmax(0,1fr); padding:18px; }
      .account-card-header .status-pill { grid-column:1 / -1; justify-self:start; }
      .account-summary,.account-security { grid-template-columns:1fr; }
      .account-summary-item { border-right:0; border-bottom:1px solid var(--line); padding:16px 18px; }
      .account-summary-item:last-child { border-bottom:0; }
      .account-security-form { padding:18px; }
      .account-security-form + .account-security-form { border-left:0; border-top:1px solid var(--line); }
      .account-password-row { display:grid; grid-template-columns:1fr; }
      .secret-row { grid-template-columns:1fr; }
      .copy-secret-button { justify-self:start; align-self:start; }
      .button-row,.actions,.inline-form { width:100%; } .button,button { width:auto; }
    }
"#;

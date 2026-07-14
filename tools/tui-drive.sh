#!/usr/bin/env bash

# tools/tui-drive.sh — drive and screenshot terminal (TUI) programs headlessly
# via tmux.
#
# ── WHAT ──────────────────────────────────────────────────────────────────────
# A thin, generic wrapper around a detached tmux session. It launches a terminal
# UI in a real PTY, sends it real keystrokes, reads the exact rendered screen
# back (as text or ANSI), and can render any frame to a PNG. This is the
# supported way to do interactive / exploratory testing of hygg's TUI — or any
# TUI — from a script, from CI, or from an AI coding agent.
#
# ── WHEN TO USE (AGENT GUIDANCE) ──────────────────────────────────────────────
# If you are an automated agent and need to *actually run and interact with* a
# TUI (hygg, a pager, a REPL, an installer) — e.g. "open progit in hygg, page
# down, screenshot, quit" — do it with THIS script.
#
# Do NOT try to type into a GUI terminal (Terminal.app / iTerm) through desktop
# computer-use: those apps are granted at a restricted "click" tier that blocks
# synthetic keystrokes, and faking them via AppleScript/cliclick is off-limits.
# A tmux pane is a genuine PTY you own, so `send-keys` here is ordinary process
# control, not input injection. And unlike pixel screenshots, `capture-pane`
# gives deterministic, diff-able, assertable output — e.g. "after 22 page-downs
# the frame contains 'Getting Started' and the progress reads 22%".
#
# ── REQUIREMENTS ──────────────────────────────────────────────────────────────
#   tmux                    — all subcommands
#   python3 + pango-view    — only the `shot` (PNG) subcommand; `pango` package.
#                             On macOS: `brew install pango`.
#
# ── SUBCOMMANDS ───────────────────────────────────────────────────────────────
#   start  <name> <cmd> [--size WxH] [--settle S]   launch <cmd> in a fresh session
#   send   <name> [--repeat N] <keys...>            send keystrokes (tmux syntax)
#   text   <name> [--out FILE]                      capture the pane as plain text
#   ansi   <name> [--out FILE]                      capture the pane, colour kept
#   shot   <name> <out.png> [--settle S] [--font D] [--bg C]   render a frame → PNG
#   settle <seconds>                                sleep, letting the UI redraw
#   stop   <name>                                   kill the session
#   list                                            list live driver sessions
#
# Keys use tmux syntax: literal text is typed as-is; named keys are words, e.g.
#   Enter  Escape  Tab  Space  Up Down Left Right  C-d (Ctrl-D)  M-x (Alt-X)
#
# ── EXAMPLE: exercise hygg on the Pro Git PDF end-to-end ───────────────────────
#   d=./tools/tui-drive.sh
#   $d start hygg "hygg test-data/pdf/progit-1-50.pdf" --size 84x40 --settle 6
#   $d send  hygg gg                       # jump to the top (cover page)
#   $d shot  hygg /tmp/hygg-cover.png      # PNG of the current frame
#   $d send  hygg --repeat 22 C-d          # page down into the text
#   $d text  hygg                          # -> assert on the rendered screen
#   $d send  hygg ':q' Enter               # quit (hygg is vim-style)
#   $d stop  hygg
#
# NOTE: hygg persists reading progress per document, so a fresh `start` resumes
# where you left off — send `gg` (or `--repeat 40 C-u`) to jump back to the top.

set -Eeuo pipefail

SELF="${0##*/}"

die()  { printf '%s: %s\n' "$SELF" "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

# Namespace tmux sessions so `list`/`stop` only ever see driver sessions and we
# never collide with a human's own tmux.
prefix() { printf 'tuidrv_%s' "$1"; }

# Convert a `capture-pane -e` ANSI dump (stdin) to Pango markup (stdout) for
# pango-view. Handles 16/256/truecolour SGR; unknown escapes are dropped.
ansi_to_pango() {
  local py rc; py="$(mktemp -t tuidrv-conv.XXXXXX)"
  cat > "$py" <<'PYEOF'
import sys, re
def palette():
    base=[(0,0,0),(205,0,0),(0,205,0),(205,205,0),(0,0,238),(205,0,205),
          (0,205,205),(229,229,229),(127,127,127),(255,0,0),(0,255,0),
          (255,255,0),(92,92,255),(255,0,255),(0,255,255),(255,255,255)]
    p=list(base); lv=[0,95,135,175,215,255]
    for r in lv:
        for g in lv:
            for b in lv: p.append((r,g,b))
    for i in range(24): v=8+i*10; p.append((v,v,v))
    return p
PAL=palette(); DEF_FG=(208,208,208); DEF_BG=(0,0,0)
hexc=lambda c:"#%02x%02x%02x"%c
esc=lambda s:s.replace("&","&amp;").replace("<","&lt;").replace(">","&gt;")
fg=bg=None; bold=False; out=[]
def flush(t):
    if not t: return
    a='foreground="%s" background="%s"'%(hexc(fg or DEF_FG),hexc(bg or DEF_BG))
    if bold: a+=' weight="bold"'
    out.append('<span %s>%s</span>'%(a,esc(t)))
def sgr(params):
    global fg,bg,bold
    ints=[int(x) if x.isdigit() else 0 for x in (params or '0').split(';')]
    j=0
    while j<len(ints):
        n=ints[j]
        if n==0: fg=bg=None; bold=False
        elif n==1: bold=True
        elif n==22: bold=False
        elif n==39: fg=None
        elif n==49: bg=None
        elif 30<=n<=37: fg=PAL[n-30]
        elif 90<=n<=97: fg=PAL[n-90+8]
        elif 40<=n<=47: bg=PAL[n-40]
        elif 100<=n<=107: bg=PAL[n-100+8]
        elif n in (38,48):
            mode=ints[j+1] if j+1<len(ints) else 0; col=None
            if mode==5:
                idx=ints[j+2] if j+2<len(ints) else 0
                col=PAL[idx] if idx<len(PAL) else DEF_FG; j+=2
            elif mode==2:
                col=tuple((ints[j+2+k] if j+2+k<len(ints) else 0) for k in range(3)); j+=4
            if col is not None:
                if n==38: fg=col
                else: bg=col
        j+=1
data=sys.stdin.read()
data=re.sub(r'\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)','',data)  # strip OSC
buf=""; i=0; N=len(data)
while i<N:
    c=data[i]
    if c=='\x1b' and i+1<N and data[i+1]=='[':
        m=re.match(r'\x1b\[([0-9;]*)([A-Za-z])',data[i:])
        if m:
            if m.group(2)=='m': flush(buf); buf=""; sgr(m.group(1))
            i+=m.end(); continue
        i+=1; continue
    if c=='\x1b': i+=1; continue
    buf+=c; i+=1
flush(buf)
sys.stdout.write(''.join(out))
PYEOF
  python3 "$py"; rc=$?
  rm -f "$py"
  return "$rc"
}

need tmux
cmd="${1:-}"; shift || true

case "$cmd" in
  start)
    name="${1:-}"; command="${2:-}"
    [ -n "$name" ] && [ -n "$command" ] || die "usage: start <name> <cmd> [--size WxH] [--settle S]"
    shift 2
    size="120x40"; settle=0
    while [ $# -gt 0 ]; do
      case "$1" in
        --size)   size="${2:?}";   shift 2;;
        --settle) settle="${2:?}"; shift 2;;
        *) die "start: unknown arg '$1'";;
      esac
    done
    s="$(prefix "$name")"
    tmux kill-session -t "$s" 2>/dev/null || true
    tmux new-session -d -s "$s" -x "${size%x*}" -y "${size#*x}"
    tmux send-keys -t "$s" "$command" Enter
    [ "$settle" != 0 ] && sleep "$settle"
    printf 'started %s (%s): %s\n' "$name" "$size" "$command"
    ;;
  send)
    name="${1:?usage: send <name> [--repeat N] <keys...>}"; shift
    reps=1
    [ "${1:-}" = "--repeat" ] && { reps="${2:?}"; shift 2; }
    [ $# -gt 0 ] || die "send: no keys given"
    s="$(prefix "$name")"
    i=0; while [ "$i" -lt "$reps" ]; do tmux send-keys -t "$s" "$@"; i=$((i + 1)); done
    ;;
  text|ansi)
    name="${1:?usage: $cmd <name> [--out FILE]}"; shift
    out=""
    [ "${1:-}" = "--out" ] && { out="${2:?}"; shift 2; }
    s="$(prefix "$name")"
    flag=(); [ "$cmd" = "ansi" ] && flag=(-e)
    if [ -n "$out" ]; then tmux capture-pane "${flag[@]}" -p -t "$s" > "$out"
    else                   tmux capture-pane "${flag[@]}" -p -t "$s"; fi
    ;;
  shot)
    name="${1:?usage: shot <name> <out.png> [--settle S] [--font DESC] [--bg COLOR]}"
    out="${2:?out.png path required}"; shift 2
    font="Menlo 14"; bg="#000000"; settle=0
    while [ $# -gt 0 ]; do
      case "$1" in
        --font)   font="${2:?}";   shift 2;;
        --bg)     bg="${2:?}";     shift 2;;
        --settle) settle="${2:?}"; shift 2;;
        *) die "shot: unknown arg '$1'";;
      esac
    done
    need python3; need pango-view
    s="$(prefix "$name")"
    [ "$settle" != 0 ] && sleep "$settle"
    markup="$(mktemp -t tuidrv.XXXXXX)"; trap 'rm -f "$markup"' EXIT
    tmux capture-pane -e -p -t "$s" | ansi_to_pango > "$markup"
    pango-view --markup -q -o "$out" --font "$font" --background "$bg" "$markup"
    printf 'wrote %s\n' "$out"
    ;;
  settle) sleep "${1:?usage: settle <seconds>}";;
  stop)
    name="${1:?usage: stop <name>}"
    tmux kill-session -t "$(prefix "$name")" 2>/dev/null && echo "stopped $name" || echo "no such session: $name"
    ;;
  list)  tmux list-sessions 2>/dev/null | sed -n 's/^tuidrv_/  /p' || echo "no sessions";;
  ""|-h|--help|help)
    awk 'NR==1{next} /^#/{sub(/^# ?/,""); print; next} /^[[:space:]]*$/{next} {exit}' "$0"
    ;;
  *) die "unknown subcommand '$cmd' (try: $SELF help)";;
esac

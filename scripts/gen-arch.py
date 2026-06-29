#!/usr/bin/env python3
"""Generate ASCII architecture diagram. All lines 48 chars, verified."""

W = 48
COLS = 14          # inner width per box
ROWS = 3           # content rows per box
BW = COLS + 2      # box width with borders
GAP = 2            # gap between boxes
SPOUT = BW // 2   # index of ┬/┴ within box string


def make_box(content, top_spout=False, bot_spout=False):
    d = COLS // 2
    top = "┌" + ("─" * d) + ("┴" if top_spout else "─") + ("─" * (COLS - d - 1)) + "┐"
    bot = "└" + ("─" * d) + ("┬" if bot_spout else "─") + ("─" * (COLS - d - 1)) + "┘"
    lines = [top]
    for c in content:
        assert len(c) == COLS, f"content {c!r} is {len(c)} chars, need {COLS}"
        lines.append("│" + c + "│")
    lines.append(bot)
    return lines


def row(a, b=""):
    inner = a + (" " * GAP) + b if b else a
    pad = W - 2 - len(inner)
    assert pad >= 0
    return "│" + inner + " " * pad + "│"


def arrow_line(ch="│"):
    left = " " * SPOUT + ch + " " * (BW - SPOUT - 1)
    right = " " * SPOUT + ch + " " * (BW - SPOUT - 1)
    return row(left, right)


# ── Content (each string exactly COLS chars) ──────────────
t1 = make_box(["    CUPSd     ",
               "    :631      ",
               "              "], bot_spout=True)

t2 = make_box(["   cups-api   ",
               "  (Rust/axum) ",
               "    :8000     "], bot_spout=True)

b1 = make_box(["    lpstat    ",
               "    filters   ",
               "   backends   "], top_spout=True)

b2 = make_box(["   Supervisor ",
               "  API / MQTT  ",
               "   discovery  "], top_spout=True)

# ── Assemble ──────────────────────────────────────────────
lines = [
    "┌" + "─" * (W - 2) + "┐",
    "│" + "CUPS Print Server (Alpine)".center(W - 2) + "│",
    "│" + " " * (W - 2) + "│",
]

for i in range(ROWS + 2):
    lines.append(row(t1[i], t2[i]))

lines.append(arrow_line("│"))
lines.append(arrow_line("▼"))
lines.append(arrow_line("│"))

for i in range(ROWS + 2):
    lines.append(row(b1[i], b2[i]))

lines.append("└" + "─" * (W - 2) + "┘")

for i, line in enumerate(lines):
    assert len(line) == W, f"line {i}: {len(line)} != {W}"
    print(line)

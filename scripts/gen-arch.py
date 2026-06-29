#!/usr/bin/env python3
"""Generate the ASCII architecture diagram for README.md.

All lines are asserted to be exactly W characters wide.
"""

W = 48


def boxed(lines, w):
    """Return box lines for content lines, inner width w.
    Returns [top, *content, bottom] where bottom may be patched later.
    """
    result = ["┌" + "─" * w + "┐"]
    for line in lines:
        result.append("│" + line.ljust(w) + "│")
    result.append("└" + "─" * w + "┘")
    return result


def row(*parts):
    """Join parts with 2-space gap and pad to W."""
    inner = "  ".join(parts)
    pad = W - 2 - len(inner)
    return "│" + inner + " " * pad + "│"


# ── Content ───────────────────────────────────────────────
b1 = boxed([" CUPSd   ", "  :631    "], 10)
b2 = boxed(["   cups-api   ", " (Rust/axum) ", "    :8000    "], 18)
b3 = boxed([" lpstat,  ", " filters, ", " backends "], 11)
b4 = boxed(["  Supervisor API", " / MQTT discov."], 18)

# Patch bottom borders to add spouts (┬) for arrows
b1[-1] = "└────┬─────┘"
b2[-1] = "└────────┬─────────┘"

# Patch top borders to add receivers (┬) for arrows
b3[0] = "┌────┴─────┐"    # was top border
b4[0] = "┌─────────┴─────────┐"

# ── Assemble ──────────────────────────────────────────────
lines = [
    "┌" + "─" * (W - 2) + "┐",
    "│" + "CUPS Print Server (Alpine)".center(W - 2) + "│",
    "│" + " " * (W - 2) + "│",
]

# Top boxes (pad shorter one with blanks)
n = max(len(b1), len(b2))
for i in range(n):
    p1 = b1[i] if i < len(b1) else " " * len(b1[0])
    p2 = b2[i] if i < len(b2) else " " * len(b2[0])
    lines.append(row(p1, p2))

# Vertical arrows
lines.append(row(" │  " + " " * 4, "  │  " + " " * 13))
lines.append(row(" ▼  " + " " * 4, "  ▼  " + " " * 13))
lines.append(row(" │  " + " " * 4, "  │  " + " " * 13))

# Bottom boxes
n = max(len(b3), len(b4))
for i in range(n):
    p3 = b3[i] if i < len(b3) else " " * len(b3[0])
    p4 = b4[i] if i < len(b4) else " " * len(b4[0])
    lines.append(row(p3, p4))

lines.append("└" + "─" * (W - 2) + "┘")

# ── Verify ────────────────────────────────────────────────
for i, line in enumerate(lines):
    assert len(line) == W, f"line {i}: {len(line)} != {W}: {line!r}"
    print(line)

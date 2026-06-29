#!/usr/bin/env python3
"""Generate the ASCII architecture diagram for README.md."""

W = 48

def boxed(lines, w):
    """Return lines for a box of inner width w.
    lines: list of strings for content rows.
    """
    result = ["┌" + "─" * w + "┐"]
    for line in lines:
        result.append("│" + line.ljust(w) + "│")
    result.append("└" + "─" * w + "┘")
    return result

# Left box
b1 = boxed([" CUPSd   ", "  :631    "], 10)
# Right box
b2 = boxed(["   cups-api   ", " (Rust/axum) ", "    :8000    "], 18)
# Bottom left
b3 = boxed([" lpstat,  ", " filters, ", " backends "], 11)
# Bottom right
b4 = boxed(["  Supervisor API", " / MQTT discov."], 18)

def row(*parts):
    inner = "  ".join(parts)
    pad = W - 2 - len(inner)
    return "│" + inner + " " * pad + "│"

# Vertical connectors
vconn = row(" │  " + " " * 4, "  │  " + " " * 13)
vconn2 = row(" ▼  " + " " * 4, "  ▼  " + " " * 13)

lines = [
    "┌" + "─" * (W - 2) + "┐",
    "│" + "CUPS Print Server (Alpine)".center(W - 2) + "│",
    "│" + " " * (W - 2) + "│",
]

# Top boxes
n = max(len(b1), len(b2))
for i in range(n):
    p1 = b1[i] if i < len(b1) else " " * 12
    p2 = b2[i] if i < len(b2) else " " * 20
    lines.append(row(p1, p2))

lines.append(vconn)
lines.append(vconn2)

# Bottom boxes
n = max(len(b3), len(b4))
for i in range(n):
    p3 = b3[i] if i < len(b3) else " " * 13
    p4 = b4[i] if i < len(b4) else " " * 20
    lines.append(row(p3, p4))

lines.append("└" + "─" * (W - 2) + "┘")

for i, line in enumerate(lines):
    assert len(line) == W, f"line {i}: {len(line)} != {W}: {line!r}"
    print(line)

"""Render the grouped concept-store graph as PNG and SVG (requires Pillow)."""
import json
import math
from html import escape
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
GRAPH = json.loads((ROOT / "concept-store.json").read_text(encoding="utf-8"))
WIDTH, HEIGHT = 1440, 930
COLORS = {
    "fact": ("#eaf2ff", "#2860a4"),
    "belief": ("#e7f5ef", "#17745b"),
    "decision": ("#f2edff", "#7051a0"),
    "proposed": ("#fff3df", "#9a681b"),
}
image = Image.new("RGB", (WIDTH, HEIGHT), "#ffffff")
draw = ImageDraw.Draw(image)
svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">',
       '<title>MarjapussiAI concept-store dependency overview</title>',
       '<desc>Visible inputs feed exact facts and uncertain seat beliefs. Together these feed bidding and passing decisions, plus proposed coordinated game planning. Dashed boxes and arrows mark proposals.</desc>',
       '<rect width="100%" height="100%" fill="white"/>']


def font(size, bold=False):
    candidates = (["C:/Windows/Fonts/arialbd.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"]
                  if bold else ["C:/Windows/Fonts/arial.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"])
    for candidate in candidates:
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size)
    return ImageFont.load_default(size=size)


def text(x, y, value, size=20, color="#273449", bold=False):
    draw.text((x, y), value, font=font(size, bold), fill=color)
    weight = ' font-weight="700"' if bold else ""
    svg.append(f'<text x="{x}" y="{y}" dominant-baseline="text-before-edge" font-family="Arial, sans-serif" font-size="{size}" fill="{color}"{weight}>{escape(value)}</text>')


def line(points, color="#8390a4", dashed=False, width=2):
    if dashed:
        for (x1, y1), (x2, y2) in zip(points, points[1:]):
            length = math.hypot(x2-x1, y2-y1)
            for start in range(0, math.ceil(length), 14):
                a, b = start / length, min(start+8, length) / length
                draw.line((x1+(x2-x1)*a, y1+(y2-y1)*a,
                           x1+(x2-x1)*b, y1+(y2-y1)*b), fill=color, width=width)
    else:
        draw.line(points, fill=color, width=width)
    dash = ' stroke-dasharray="8 6"' if dashed else ""
    coords = " ".join(f"{x},{y}" for x, y in points)
    svg.append(f'<polyline points="{coords}" fill="none" stroke="{color}" stroke-width="{width}"{dash}/>')


def arrow(points, dashed=False):
    color = "#9a681b" if dashed else "#8390a4"
    line(points, color, dashed)
    (x1, y1), (x2, y2) = points[-2:]
    angle = math.atan2(y2-y1, x2-x1)
    wings = [(x2-10*math.cos(angle-sign*0.45), y2-10*math.sin(angle-sign*0.45)) for sign in (-1, 1)]
    triangle = [(x2, y2), *wings]
    draw.polygon(triangle, fill=color)
    coords = " ".join(f"{x},{y}" for x, y in triangle)
    svg.append(f'<polygon points="{coords}" fill="{color}"/>')


text(40, 28, "From visible cards to a useful decision", 34, bold=True)
text(40, 80, "MarjapussiAI concept store  |  7 October 2026  |  Arrows mean: provides an input to", 20)
for i, label in enumerate(["VISIBLE INPUTS", "HAND & SEAT", "TEAM & GAME", "ACTION"]):
    text(40 + 350*i, 140, label, 17, "#667389", True)

nodes = {n["id"]: n for n in GRAPH["nodes"]}
boxes = {n["id"]: (40+350*n["column"], 190+215*n["row"], 300, 185) for n in nodes.values()}
for source, target in GRAPH["edges"]:
    sx, sy, sw, sh = boxes[source]
    tx, ty, tw, th = boxes[target]
    a, b = (sx+sw, sy+sh/2), (tx-5, ty+th/2)
    middle = sx+sw+(34 if nodes[target]["kind"] == "proposed" else 23)
    arrow([a, (middle, a[1]), (middle, b[1]), b], nodes[target]["kind"] == "proposed")

for n in nodes.values():
    x, y, w, h = boxes[n["id"]]
    fill, accent = COLORS[n["kind"]]
    draw.rounded_rectangle((x, y, x+w, y+h), radius=14, fill=fill)
    svg.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="{fill}"/>')
    if n["kind"] == "proposed":
        line([(x+8,y),(x+w-8,y)], accent, True)
        line([(x+w,y+8),(x+w,y+h-8)], accent, True)
        line([(x+w-8,y+h),(x+8,y+h)], accent, True)
        line([(x,y+h-8),(x,y+8)], accent, True)
    text(x+16, y+14, n["subtitle"], 16, accent, True)
    text(x+16, y+42, n["title"], 21, "#18273c", True)
    for i, label in enumerate(n["lines"]):
        text(x+16, y+80+24*i, label, 18)

text(40, 830, "Exact facts", 18, COLORS["fact"][1], True)
text(220, 830, "Uncertain beliefs", 18, COLORS["belief"][1], True)
text(460, 830, "Decision concepts", 18, COLORS["decision"][1], True)
text(720, 830, "Dashed = proposed full planner", 18, COLORS["proposed"][1], True)
text(40, 872, "Published: main = core; v1-preview = agent snapshot. Latest corrections remain local. See the document for status and exact edges.", 18)
svg.append("</svg>")
(ROOT / "concept-store.svg").write_text("\n".join(svg)+"\n", encoding="utf-8", newline="\n")
image.save(ROOT / "concept-store.png", optimize=True)
print("Rendered concept-store.svg and concept-store.png")

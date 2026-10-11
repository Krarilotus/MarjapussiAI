"""Graphviz geometry for reasoning-graph.json views; no renderer or semantics.

Import layout_view(view), or run:
  python dag_layout.py reasoning-graph.json --selfcheck
  python dag_layout.py reasoning-graph.json --output layouts.json

Input: {id?, root: ID, nodes: [{id: ID, inputs: [{id: ID, label?: str}]}]}.
Each input points TO its consumer. Depth means shortest input-edge distance
from focus (root is distance zero), not Graphviz rank. Returned keys are
"<focus>:<depth>" plus "<root>:full". Focus IDs are never rewritten.
Coordinates are px, y-down; node x/y are TOP LEFT, edge path is an SVG d.
Edges have no drawn text; label is suitable for a title/hover inspector.
No graph-specific semantic status is inferred here.
"""

import argparse
from collections import deque
import json
import math
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile


DOT = Path("C:/Program Files/Graphviz/bin/dot.exe")
PX_PER_INCH = 96.0
PADDING = 24.0
MAX_NODES = 128
MAX_EDGES = 1024
MAX_LAYOUTS = 2048
MAX_NODE_APPEARANCES = 100_000
MAX_OUTPUT_BYTES = 64 * 1024 * 1024
DEFAULT_JSON = Path(__file__).parents[1] / "design/reasoning-graph.json"


def _validate(view):
    """Validate the entire view, including definitions unreachable from root."""
    if not isinstance(view, dict) or not isinstance(view.get("nodes"), list):
        raise ValueError("view must contain a nodes array")
    definitions = {}
    for node in view["nodes"]:
        if not isinstance(node, dict):
            raise ValueError("node must be an object")
        identity = node.get("id")
        if not isinstance(identity, str) or not identity or len(identity) > 256:
            raise ValueError("node id must be a nonempty string of at most 256 characters")
        if identity in definitions:
            raise ValueError(f"duplicate node id: {identity!r}")
        definitions[identity] = node
    if not definitions or len(definitions) > MAX_NODES:
        raise ValueError(f"view must have 1..{MAX_NODES} nodes")
    root = view.get("root")
    if not isinstance(root, str) or root not in definitions:
        raise ValueError(f"missing root definition: {root!r}")
    dependencies = {}
    edges = {}
    for consumer, node in definitions.items():
        inputs = node.get("inputs", [])
        if not isinstance(inputs, list):
            raise ValueError(f"inputs must be an array: {consumer!r}")
        dependencies[consumer] = []
        for item in inputs:
            if not isinstance(item, dict) or not isinstance(item.get("id"), str):
                raise ValueError(f"input must contain a string id: {consumer!r}")
            source = item["id"]
            if source not in definitions:
                raise ValueError(f"missing input {source!r} for {consumer!r}")
            pair = (source, consumer)
            if pair in edges:
                raise ValueError(f"duplicate dependency: {source!r} -> {consumer!r}")
            label = item.get("label", "")
            if not isinstance(label, str) or len(label) > 4096:
                raise ValueError(f"invalid dependency label: {pair!r}")
            dependencies[consumer].append(source)
            edges[pair] = label
    if len(edges) > MAX_EDGES:
        raise ValueError(f"view exceeds {MAX_EDGES} edges")
    visited, active = set(), set()

    def visit(identity):
        if identity in active:
            raise ValueError(f"dependency cycle through {identity!r}")
        if identity in visited:
            return
        active.add(identity)
        for source in sorted(dependencies[identity]):
            visit(source)
        active.remove(identity)
        visited.add(identity)

    for identity in sorted(definitions):
        visit(identity)
    return root, definitions, dependencies, edges


def _distances(focus, dependencies):
    distances = {focus: 0}
    queue = deque([focus])
    while queue:
        consumer = queue.popleft()
        for source in sorted(dependencies[consumer]):
            if source not in distances:
                distances[source] = distances[consumer] + 1
                queue.append(source)
    return distances


def _dot_graph(index, visible, edge_pairs, names):
    # Fixed, empty boxes: root owns text rendering. No labels alter routing.
    lines = [f"digraph g{index} {{",
             'graph [rankdir=BT, ranksep=0.8, nodesep=0.5, splines=true, ordering=out];',
             'node [shape=box, fixedsize=true, width=2.1875, height=0.729166667, label=""];',
             'edge [arrowsize=0.8];']
    lines.extend(f"{names[identity]};" for identity in sorted(visible))
    lines.extend(f"{names[source]} -> {names[consumer]};"
                 for source, consumer in sorted(edge_pairs))
    lines.append("}")
    return "\n".join(lines)


def _run_dot(source):
    if not DOT.is_file():
        raise ValueError(f"Graphviz dot not found: {DOT}")
    # File-backed capture prevents unbounded process output in memory. Both
    # files are temporary, removed on success/failure; nothing beside input.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            result = subprocess.run([str(DOT), "-Tplain"], input=source.encode("utf-8"),
                                    stdout=stdout, stderr=stderr, timeout=60, check=False)
        except subprocess.TimeoutExpired as exc:
            raise ValueError("Graphviz exceeded the 60-second layout bound") from exc
        stdout.seek(0, 2)
        size = stdout.tell()
        stderr.seek(0)
        error = stderr.read(8192).decode("utf-8", errors="replace")
        if result.returncode:
            raise ValueError(f"Graphviz exited {result.returncode}: {error}")
        if size > MAX_OUTPUT_BYTES:
            raise ValueError(f"Graphviz output exceeds {MAX_OUTPUT_BYTES} bytes")
        stdout.seek(0)
        return stdout.read().decode("utf-8")


def _path(points):
    if len(points) < 4 or (len(points) - 1) % 3:
        raise ValueError("unexpected Graphviz cubic spline point count")
    number = lambda value: f"{value:.3f}"
    path = f"M {number(points[0][0])} {number(points[0][1])}"
    for start in range(1, len(points), 3):
        path += " C " + " ".join(number(value) for point in points[start:start + 3] for value in point)
    return path


def _parse_plain(output, identities, edge_labels):
    layouts, current = [], None
    for line in output.splitlines():
        tokens = shlex.split(line)
        if not tokens:
            continue
        kind = tokens[0]
        if kind == "graph":
            if current is not None or len(tokens) != 4:
                raise ValueError("invalid Graphviz graph header")
            scale, width, height = map(float, tokens[1:])
            if scale != 1 or not all(math.isfinite(v) and v >= 0 for v in (width, height)):
                raise ValueError("unexpected Graphviz scaling or size")
            current = {"width": width * PX_PER_INCH + 2 * PADDING,
                       "height": height * PX_PER_INCH + 2 * PADDING,
                       "nodes": {}, "edges": []}
            graph_height = height * PX_PER_INCH
        elif kind == "node" and current is not None:
            identity = identities[tokens[1]]
            x, y, width, height = (float(v) * PX_PER_INCH for v in tokens[2:6])
            if identity in current["nodes"]:
                raise ValueError("duplicate Graphviz node")
            current["nodes"][identity] = {
                "x": x - width / 2 + PADDING, "y": graph_height - y - height / 2 + PADDING,
                "width": width, "height": height}
        elif kind == "edge" and current is not None:
            source, consumer = identities[tokens[1]], identities[tokens[2]]
            count = int(tokens[3])
            values = list(map(float, tokens[4:4 + count * 2]))
            if len(values) != count * 2:
                raise ValueError("truncated Graphviz spline")
            points = [[values[i] * PX_PER_INCH + PADDING,
                       graph_height - values[i + 1] * PX_PER_INCH + PADDING]
                      for i in range(0, len(values), 2)]
            if not all(math.isfinite(value) for point in points for value in point):
                raise ValueError("nonfinite Graphviz spline")
            current["edges"].append({"from": source, "to": consumer,
                                     "label": edge_labels[(source, consumer)],
                                     "path": _path(points), "points": points})
        elif kind == "stop" and current is not None:
            layouts.append(current)
            current = None
        else:
            raise ValueError(f"unexpected Graphviz plain record: {kind!r}")
    if current is not None:
        raise ValueError("truncated Graphviz output")
    return layouts


def layout_view(view):
    """All focus/depth geometries, plus root:full; validates before invoking dot.

    max_depths tells UI the finite expansion range for each focus. A leaf has
    one depth (1). omitted_inputs counts direct hidden dependencies per visible
    node; omitted_node_count counts unique hidden definitions reachable from
    focus. root_unreachable lists definitions outside the full root projection.
    Geometry uses node top-left positions; edge control points stop at the
    Graphviz arrow base. A renderer may attach its own small end marker.
    """
    root, definitions, dependencies, edge_labels = _validate(view)
    names = {identity: f"n{index}" for index, identity in enumerate(sorted(definitions))}
    identities = {name: identity for identity, name in names.items()}
    requests, max_depths, appearances = [], {}, 0
    for focus in sorted(definitions):
        distances = _distances(focus, dependencies)
        maximum = max(1, max(distances.values()))
        max_depths[focus] = maximum
        for depth in range(1, maximum + 1):
            visible = {identity for identity, distance in distances.items() if distance <= depth}
            pairs = {pair for pair in edge_labels if pair[0] in visible and pair[1] in visible}
            appearances += len(visible)
            requests.append((focus, depth, visible, pairs, set(distances)))
    if len(requests) > MAX_LAYOUTS or appearances > MAX_NODE_APPEARANCES:
        raise ValueError("view exceeds bounded layout batch capacity")
    dot_source = "\n".join(_dot_graph(index, visible, pairs, names)
                           for index, (_, _, visible, pairs, _) in enumerate(requests))
    geometries = _parse_plain(_run_dot(dot_source), identities, edge_labels)
    if len(geometries) != len(requests):
        raise ValueError("Graphviz returned the wrong layout count")
    layouts = {}
    for request, geometry in zip(requests, geometries):
        focus, depth, visible, pairs, reachable = request
        actual_edges = {(edge["from"], edge["to"]) for edge in geometry["edges"]}
        if set(geometry["nodes"]) != visible or actual_edges != pairs or len(actual_edges) != len(geometry["edges"]):
            raise ValueError("Graphviz changed the projection's nodes/edges")
        geometry.update(focus=focus, depth=depth, complete=visible == reachable,
                        omitted_node_count=len(reachable - visible),
                        omitted_inputs={identity: sum(source not in visible for source in dependencies[identity])
                                        for identity in sorted(visible)})
        layouts[f"{focus}:{depth}"] = geometry
    layouts[f"{root}:full"] = dict(layouts[f"{root}:{max_depths[root]}"], depth="full")
    result = {"schema": "marja.dag-layout/1", "units": "px", "node_origin": "top-left",
              "edge_direction": "input-to-consumer", "root": root,
              "max_depths": max_depths, "layouts": layouts,
              "root_unreachable": sorted(set(definitions) - _distances(root, dependencies).keys())}
    selfcheck_view(view, result)
    return result


def selfcheck_view(view, result):
    """Independent projection/overlap assertions, usable on saved geometry."""
    root, definitions, dependencies, edge_labels = _validate(view)
    assert result["root"] == root
    for key, layout in result["layouts"].items():
        distances = _distances(layout["focus"], dependencies)
        depth = layout["depth"]
        expected = set(distances) if depth == "full" else {
            identity for identity, distance in distances.items() if distance <= depth}
        assert set(layout["nodes"]) == expected, f"visible node mismatch: {key}"
        expected_edges = {pair for pair in edge_labels if pair[0] in expected and pair[1] in expected}
        actual = [(edge["from"], edge["to"]) for edge in layout["edges"]]
        assert len(actual) == len(set(actual)), f"duplicate visible edges: {key}"
        assert set(actual) == expected_edges, f"visible edge mismatch: {key}"
        assert layout["omitted_node_count"] == len(set(distances) - expected)
        for identity, count in layout["omitted_inputs"].items():
            assert count == sum(source not in expected for source in dependencies[identity])
        boxes = list(layout["nodes"].items())
        for index, (identity, box) in enumerate(boxes):
            assert all(math.isfinite(value) for value in box.values()), f"nonfinite box: {identity}"
            assert box["width"] > 0 and box["height"] > 0
            # Plain output rounds inches: allow a 0.1 px boundary tolerance.
            assert box["x"] >= -0.1 and box["y"] >= -0.1
            assert box["x"] + box["width"] <= layout["width"] + 0.1
            assert box["y"] + box["height"] <= layout["height"] + 0.1
            for other_id, other in boxes[index + 1:]:
                overlap_x = min(box["x"] + box["width"], other["x"] + other["width"]) - max(box["x"], other["x"])
                overlap_y = min(box["y"] + box["height"], other["y"] + other["height"]) - max(box["y"], other["y"])
                assert overlap_x <= 0.1 or overlap_y <= 0.1, f"node overlap: {key}: {identity}/{other_id}"


def _validation_selfcheck():
    invalid = [
        {"root": "a", "nodes": [{"id": "a"}, {"id": "a"}]},
        {"root": "a", "nodes": [{"id": "a", "inputs": [{"id": "missing"}]}]},
        {"root": "a", "nodes": [{"id": "a", "inputs": [{"id": "a"}]}]},
        {"root": "a", "nodes": [{"id": "a", "inputs": [{"id": "b"}, {"id": "b"}]}, {"id": "b"}]},
        {"root": "a", "nodes": [{"id": "a", "inputs": [{"id": "b"}]}, {"id": "b", "inputs": [{"id": "a"}]}]},
    ]
    for view in invalid:
        try:
            _validate(view)
        except ValueError:
            continue
        raise AssertionError("invalid graph passed validation")


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", nargs="?", type=Path, default=DEFAULT_JSON)
    parser.add_argument("--view", help="only this view id (default all)")
    parser.add_argument("--output", type=Path, help="JSON file; default stdout")
    parser.add_argument("--selfcheck", action="store_true", help="print compact checks instead of full geometry")
    args = parser.parse_args()
    try:
        if args.output and args.output.resolve() == args.input.resolve():
            raise ValueError("output must not overwrite input JSON")
        data = json.loads(args.input.read_text(encoding="utf-8-sig"))
        views = data.get("views", [data])
        if not isinstance(views, list):
            raise ValueError("views must be an array")
        selected = [view for view in views if not args.view or view.get("id") == args.view]
        if not selected:
            raise ValueError(f"view not found: {args.view!r}")
        identities = [view.get("id", str(index)) for index, view in enumerate(selected)]
        if len(set(identities)) != len(identities):
            raise ValueError("duplicate view ids")
        _validation_selfcheck()
        results = {identity: layout_view(view) for identity, view in zip(identities, selected)}
        if args.selfcheck:
            output = {"checked": True, "views": {
                identity: {"layouts": len(result["layouts"]),
                           "full_nodes": len(result["layouts"][f'{result["root"]}:full']["nodes"]),
                           "full_edges": len(result["layouts"][f'{result["root"]}:full']["edges"]),
                           "max_depth": max(result["max_depths"].values())}
                for identity, result in results.items()}}
        else:
            output = {"schema": "marja.dag-layout-package/1", "views": results}
        serialized = json.dumps(output, ensure_ascii=False, separators=(",", ":")) + "\n"
        if args.output:
            args.output.write_text(serialized, encoding="utf-8")
        else:
            print(serialized, end="")
    except (OSError, ValueError, AssertionError, KeyError) as exc:
        parser.exit(1, f"layout error: {exc}\n")


if __name__ == "__main__":
    main()

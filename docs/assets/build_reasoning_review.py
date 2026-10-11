"""Build the portable review page; never invokes the game or policy runtime."""

import argparse
import hashlib
import html
import json
import re
from pathlib import Path

from markdown_it import MarkdownIt
from dag_layout import layout_view


def validate_graph(model):
    for view in model["views"]:
        nodes = {node["id"]: node for node in view["nodes"]}
        if len(nodes) != len(view["nodes"]):
            raise ValueError(f"Duplicate node in {view['id']}")
        visited = set()

        def visit(identity, ancestors):
            if identity in ancestors:
                raise ValueError(f"Dependency cycle at {identity}")
            node = nodes[identity]
            if node["kind"] not in {"fact", "rule", "belief", "future", "decision"}:
                raise ValueError(f"Unknown kind for {identity}")
            for field in ("label", "summary", "returns", "operator", "epistemic"):
                if not node.get(field):
                    raise ValueError(f"Missing {field} on {identity}")
            visited.add(identity)
            for edge in node.get("inputs", []):
                visit(edge["id"], ancestors | {identity})

        visit(view["root"], set())
        if set(nodes) != visited:
            raise ValueError(f"Unreachable nodes in {view['id']}: {set(nodes)-visited}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--worked-example", type=Path,
                        help="Optional private worked-example narrative; remains outside the repository")
    parser.add_argument("--runtime-graph", type=Path,
                        help="Graph exported by reasoning_graph: evaluator fixture or captured runtime decision")
    args = parser.parse_args()
    docs = Path(__file__).resolve().parents[1]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    source_names = {
        "contract.md": "executable-reasoning.md",
        "worked-decision.md": "worked-decision.md",
        "boundary-review.md": "reasoning-boundary-review.md",
        "reasoning-graph.json": "reasoning-graph.json",
    }
    replacements = {source: target for target, source in source_names.items()}
    contents = {}
    for target, source in source_names.items():
        source_path = args.worked_example if target == "worked-decision.md" and args.worked_example else docs / "design" / source
        text = source_path.read_text(encoding="utf-8")
        for old, new in replacements.items():
            text = text.replace(f"]({old})", f"]({new})")
        (output / target).write_text(text, encoding="utf-8", newline="\n")
        contents[target] = text
    model = json.loads(contents["reasoning-graph.json"])
    runtime_note = "No runtime capture is included."
    if args.runtime_graph:
        runtime = json.loads(args.runtime_graph.read_text(encoding="utf-8"))
        if runtime.get("schema") != model["schema"]:
            raise ValueError("Runtime graph schema does not match the review graph")
        model["views"].extend(runtime["views"])
        if runtime.get("evidence_kind") == "captured_runtime_projection":
            runtime_note = (
                "The captured view projects supplied runtime evidence; evaluator dependencies, "
                "authored-definition references and continuation provenance are distinguished. "
                "Capture identity and limits are in its inspector; the run manifest supplies policy provenance."
            )
        elif runtime.get("evidence_kind") == "synthetic_capture_fixture":
            runtime_note = "The captured-schema view is a synthetic structural fixture, not policy execution evidence."
        else:
            runtime_note = "The implemented value view evaluates an illustrative terminal fixture, not live policy execution."
        (output / "reasoning-graph.json").write_text(
            json.dumps(model, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    validate_graph(model)
    for view in model["views"]:
        geometry = layout_view(view)
        view["layouts"] = geometry["layouts"]
        view["max_depths"] = geometry["max_depths"]
    renderer = MarkdownIt("commonmark", {"html": True}).enable("table")

    def chapters(markdown, first_open=True):
        sections = re.split(r"(?m)^## ", markdown)
        result = renderer.render(sections[0])
        for index, section in enumerate(sections[1:]):
            title, _, body = section.partition("\n")
            opened = " open" if index == 0 and first_open else ""
            result += f'<details class="chapter"{opened}><summary>{html.escape(title)}</summary>'
            result += renderer.render(body) + "</details>"
        return result

    template = (docs / "assets" / "reasoning-review.html").read_text(encoding="utf-8")
    fields = {
        "@@CONTRACT@@": chapters(contents["contract.md"]),
        "@@EXAMPLE@@": chapters(contents["worked-decision.md"], first_open=False),
        "@@BOUNDARY@@": renderer.render(contents["boundary-review.md"]),
        "@@MODEL@@": json.dumps(model, ensure_ascii=False).replace("</", "<\\/"),
        "@@SCRIPT@@": (docs / "assets" / "reasoning-review.js").read_text(encoding="utf-8"),
        "@@VALIDATION@@": (
            f"Review-only · proposal source {html.escape(model.get('source_revision', 'unrecorded'))} · 11 October 2026. "
            "The worked example distinguishes retained replay evidence from illustrative beliefs. "
            f"{runtime_note} "
            "The Proposed view remains a design proposal; no deployment or playing-strength result is claimed."
        ),
    }
    for token, replacement in fields.items():
        if token not in template:
            raise ValueError(f"Template missing {token}")
        template = template.replace(token, replacement)
    if re.search(r"@@[A-Z]+@@", template):
        raise ValueError("Unresolved template token")
    (output / "index.html").write_text(template, encoding="utf-8", newline="\n")
    manifest = []
    for name in [*source_names, "index.html"]:
        data = (output / name).read_bytes()
        manifest.append({"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(json.dumps({"output": str(output), "files": len(manifest), "bytes": sum(x["bytes"] for x in manifest),
                      "graph_views": len(model["views"]), "graph_definitions_valid": True}))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Run the production Scrivener import/reopen/compile path on one private book.

This harness writes only beneath ignored generated/ and reports/ directories.
The source project is fingerprinted before and after and is never an output
directory. A reviewed mapping JSON is required so uncertain binder mapping is
not silently promoted to an assumed manuscript structure.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import subprocess
import sys

import real_manuscript_regression as regression


ROOT = pathlib.Path(__file__).resolve().parents[1]


def resolved_from_root(path: pathlib.Path) -> pathlib.Path:
    return (ROOT / path).resolve() if not path.is_absolute() else path.resolve()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=pathlib.Path, help=".scriv directory or .scrivx within fixtures/")
    parser.add_argument("--mapping", required=True, type=pathlib.Path, help="reviewed ScrivenerImportPlan JSON")
    parser.add_argument("--book-type", required=True, choices=("fiction", "nonfiction", "custom"))
    parser.add_argument("--reference", type=pathlib.Path, help="optional, potentially unmatched historical publication")
    parser.add_argument("--title", help="optional destination title; otherwise use source metadata")
    parser.add_argument("--run-id", required=True, help="unique local run name; never reuse")
    parser.add_argument("--generated-root", type=pathlib.Path, default=pathlib.Path("generated/back-catalogue"))
    parser.add_argument("--reports-root", type=pathlib.Path, default=pathlib.Path("reports/back-catalogue"))
    args = parser.parse_args()

    source = resolved_from_root(args.source)
    mapping = resolved_from_root(args.mapping)
    fixture_root = (ROOT / "fixtures").resolve()
    if not source.is_relative_to(fixture_root) or not source.exists():
        parser.error("--source must exist within the protected repository fixtures directory")
    if not mapping.is_file():
        parser.error("--mapping must name a reviewed import plan JSON file")
    reference = resolved_from_root(args.reference) if args.reference else None
    if reference and (not reference.is_relative_to(fixture_root) or not reference.is_file()):
        parser.error("--reference must be a regular file within fixtures/")

    if pathlib.Path(args.run_id).name != args.run_id or args.run_id in {".", ".."}:
        parser.error("--run-id must be a single directory name")
    generated = resolved_from_root(args.generated_root) / args.run_id
    reports = resolved_from_root(args.reports_root) / args.run_id
    if not generated.parent.is_relative_to((ROOT / "generated").resolve()):
        parser.error("--generated-root must be inside the ignored repository generated/ directory")
    if not reports.parent.is_relative_to((ROOT / "reports").resolve()):
        parser.error("--reports-root must be inside the ignored repository reports/ directory")
    if generated.exists() or reports.exists():
        parser.error("run ID already exists under generated/ or reports/; choose a new ID")
    generated.mkdir(parents=True)
    reports.mkdir(parents=True)
    config = generated / "isolated-config"
    config.mkdir()
    env = os.environ.copy()
    env["XDG_CONFIG_HOME"] = str(config)
    env["CARGO_NET_OFFLINE"] = "true"

    def run(command: list[str]) -> str:
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        if result.returncode:
            raise RuntimeError(
                f"command failed ({result.returncode}): {command[0]}\n"
                f"{result.stderr.strip() or result.stdout.strip()}"
            )
        return result.stdout

    source_before = regression.tree_fingerprint(source)
    report: dict[str, object] = {
        "created_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "commit": run(["git", "rev-parse", "HEAD"]).strip(),
        "working_tree_dirty": bool(run(["git", "status", "--porcelain"]).strip()),
        "source": str(source),
        "source_sha256_before": source_before,
        "mapping": str(mapping),
        "mapping_sha256": regression.hashlib.sha256(mapping.read_bytes()).hexdigest(),
        "reference": str(reference) if reference else None,
        "reference_match_status": "unconfirmed candidate" if reference else "not supplied",
        "fixture_sha256_manifest": "passed",
    }
    cli = ROOT / "target/debug/writers-ide"
    try:
        run(["cargo", "build", "--offline", "-p", "writers-ide"])
        inventory_result = json.loads(run([str(cli), "inspect-scrivener", str(source)]))
        inventory_path = reports / "inventory.json"
        inventory_path.write_text(json.dumps(inventory_result, ensure_ascii=False, indent=2), encoding="utf-8")
        project = generated / "project"
        title_args = [args.title] if args.title else []
        run([
            str(cli), "import-scrivener", str(source), str(project), str(mapping),
            *title_args, f"--book-type={args.book_type}",
        ])
        persisted = regression.project_text_counts(project)
        fidelity = regression.verify_imported_source_content(project)
        report.update({
            "inventory": {
                "format_version": inventory_result["inventory"].get("format_version"),
                "creator": inventory_result["inventory"].get("creator"),
                "binder_items": len(inventory_result["inventory"].get("items", [])),
                "unresolved_proposed_mappings": len(inventory_result["preview"].get("unresolved_source_ids", [])),
                "unsupported_warning_count": len(inventory_result["preview"].get("unsupported", [])),
            },
            "project": str(project),
            "persisted_counts": persisted,
            "source_content_fidelity": fidelity,
            "import_diagnostics_local": str(project / "exports/scrivener-import-report.json"),
        })
        output = generated / "publication.epub"
        run([str(cli), "compile", str(project), "epub", str(output)])
        generated_semantics = regression.epub_semantics(output)
        check_command = regression.epubcheck_command()
        check = regression.epubcheck_result(check_command, output, env=env)
        report["generated_epub"] = {
            "path": str(output),
            "sha256": regression.hashlib.sha256(output.read_bytes()).hexdigest(),
            "bytes": output.stat().st_size,
            "epubcheck_version": "EPUBCheck v4.2.6" if check_command else "unavailable",
            "epubcheck": check,
            "semantics": {key: value for key, value in generated_semantics.items() if key != "tokens_private"},
        }
        if reference:
            reference_semantics = regression.epub_semantics(reference)
            report["reference_epubcheck"] = regression.epubcheck_result(check_command, reference, env=env)
            report["reference_semantics"] = {
                key: value for key, value in reference_semantics.items() if key != "tokens_private"
            }
            report["reference_differential_local"] = regression.detailed_epub_difference(
                reference_semantics, generated_semantics, fidelity
            )
        report["project_markdown_diagnostics"] = regression.canonical_markdown_diagnostics(project)
        report["status"] = "provisional"
    except Exception as error:  # preserve a useful local failure record
        report["status"] = "failed"
        report["failure"] = str(error)
    finally:
        source_after = regression.tree_fingerprint(source)
        report["source_sha256_after"] = source_after
        report["source_unchanged"] = source_before == source_after
        report["fixture_sha256_after"] = subprocess.run(
            ["sha256sum", "--quiet", "-c", "SHA256SUMS"],
            cwd=fixture_root,
        ).returncode == 0
    summary = reports / "summary.json"
    summary.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Local private report: {summary}")
    print(f"Generated project/publication: {generated}")
    print(f"Status: {report.get('status')}; source unchanged: {report['source_unchanged']}; fixture checksums after: {report['fixture_sha256_after']}")
    return 0 if report.get("status") == "provisional" and report.get("source_unchanged") and report.get("fixture_sha256_after") else 1


if __name__ == "__main__":
    raise SystemExit(main())

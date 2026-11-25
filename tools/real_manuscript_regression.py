#!/usr/bin/env python3
"""Run private, local-only Scrivener import and publication regressions.

The fixture defaults are repository-relative. Reports and generated projects are
private local artifacts and must remain under the ignored regression folders.
"""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import difflib
import hashlib
import html.parser
import json
import os
import platform
import posixpath
import pathlib
import re
import shutil
import subprocess
import sys
import time
import unicodedata
import zipfile
import xml.etree.ElementTree as ET
from urllib.parse import unquote, urlsplit


BOOKS = {
    "book-01": ("book-01/source/AcceleratedFluency.scriv", "book-01/reference/Accelerated_Fluency.epub", "nonfiction"),
    "book-02": ("book-02/source/ZenGardens.scriv", "book-02/reference/How_To_Create_a_Zen_Garden.epub", "nonfiction"),
    "book-03": ("book-03/source/Les Retraités-Pirates.scriv", "book-03/reference/Les Retraités-Pirates.epub", "fiction"),
}


class TextCollector(html.parser.HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.text: list[str] = []
        self.tags: collections.Counter[str] = collections.Counter()
        self.headings: list[str] = []
        self.links: list[str] = []
        self.images: list[str] = []
        self.ids: set[str] = set()
        self._heading_depth = 0
        self._heading_parts: list[str] = []
        self._suppressed_depth = 0
        self._suppressed = {"head", "script", "style"}

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.tags[tag] += 1
        if tag in self._suppressed:
            self._suppressed_depth += 1
        attrs_map = dict(attrs)
        if attrs_map.get("id"):
            self.ids.add(attrs_map["id"])
        if tag == "a":
            self.links.append(attrs_map.get("href") or "")
        elif tag == "img":
            self.images.append(attrs_map.get("src") or "")
        if re.fullmatch(r"h[1-6]", tag):
            self._heading_depth += 1

    def handle_endtag(self, tag: str) -> None:
        if tag in self._suppressed and self._suppressed_depth:
            self._suppressed_depth -= 1
        if re.fullmatch(r"h[1-6]", tag) and self._heading_depth:
            self.headings.append("".join(self._heading_parts).strip())
            self._heading_parts.clear()
            self._heading_depth -= 1
        if tag in {"p", "div", "li", "br", "tr", "blockquote", "section"}:
            self.text.append("\n")

    def handle_data(self, data: str) -> None:
        if self._suppressed_depth:
            return
        self.text.append(data)
        if self._heading_depth:
            self._heading_parts.append(data)


def run(command: list[str], *, env: dict[str, str], capture: bool = True) -> str:
    result = subprocess.run(command, env=env, text=True, capture_output=capture)
    if result.returncode:
        detail = result.stderr[-4000:] if result.stderr else "(no stderr)"
        raise RuntimeError(f"command failed ({result.returncode}): {command[0]} …\n{detail}")
    return result.stdout if capture else ""


def tree_fingerprint(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    for item in sorted(path.rglob("*")):
        if item.is_symlink():
            raise RuntimeError(f"fixture contains a symlink; refusing to follow it: {item}")
        if item.is_file():
            digest.update(item.relative_to(path).as_posix().encode())
            digest.update(b"\0")
            with item.open("rb") as handle:
                for block in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(block)
            digest.update(b"\0")
    return digest.hexdigest()


def make_plan(inventory: dict, book_type: str) -> dict:
    items = inventory["items"]
    by_id = {item["source_document_id"]: item for item in items}
    children: dict[str, list[dict]] = collections.defaultdict(list)
    for item in items:
        parent = item.get("parent_source_id")
        if parent:
            children[parent].append(item)
    decisions: dict[str, dict] = {}

    def descendants_have_nested_structure(item: dict) -> bool:
        descendants = children[item["source_document_id"]]
        return any(
            child.get("type_code", "").lower() in {"folder", "draftfolder"}
            or bool(children[child["source_document_id"]])
            for child in descendants
        )

    for item in items:
        source_id = item["source_document_id"]
        parent_id = item.get("parent_source_id")
        code = (item.get("type_code") or "").lower()
        area = item.get("area", "unknown")
        nested = children[source_id]

        if area == "trash":
            kind = "Skip"
        elif area in {"research", "unknown"}:
            kind = "Research"
        elif book_type == "nonfiction":
            if code in {"folder", "draftfolder"} or nested:
                kind = "Section"
            elif parent_id is None:
                kind = "Chapter"
            else:
                parent_kind = decisions[parent_id]["destination_kind"]
                kind = "Chapter" if parent_kind == "Section" else "Section"
        elif code in {"draftfolder", "folder"}:
            if parent_id is None or descendants_have_nested_structure(item):
                kind = "Section"
            else:
                parent_kind = decisions.get(parent_id, {}).get("destination_kind")
                kind = "Chapter" if parent_kind == "Section" else "Section"
        elif nested:
            kind = "Section"
        elif parent_id is None:
            kind = "Section"
        else:
            parent_kind = decisions[parent_id]["destination_kind"]
            if parent_kind == "Chapter":
                kind = "Scene"
            else:
                kind = "Chapter"

        decisions[source_id] = {
            "source_document_id": source_id,
            "destination_kind": kind,
            "title_override": None,
            "include_in_import": kind != "Skip",
        }
    del by_id
    return {"decisions": list(decisions.values())}


def epub_semantics(path: pathlib.Path) -> dict:
    with zipfile.ZipFile(path) as archive:
        names = set(archive.namelist())
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = next(node for node in container.iter() if node.tag.endswith("rootfile"))
        package_path = rootfile.attrib["full-path"]
        package_dir = pathlib.PurePosixPath(package_path).parent
        package = ET.fromstring(archive.read(package_path))
        ns = {"opf": "http://www.idpf.org/2007/opf", "dc": "http://purl.org/dc/elements/1.1/"}
        manifest = {
            node.attrib["id"]: (package_dir / unquote(node.attrib["href"])).as_posix()
            for node in package.findall("opf:manifest/opf:item", ns)
        }
        spine = [node.attrib["idref"] for node in package.findall("opf:spine/opf:itemref", ns)]
        collectors = []
        nav_collector = None
        for item_id in spine:
            href = manifest[item_id]
            if href not in names:
                raise RuntimeError(f"EPUB spine points to missing content: {path.name}")
            source = archive.read(href).decode("utf-8", errors="replace")
            collector = TextCollector()
            collector.feed(source)
            collectors.append((href, collector))
        nav_item = next(
            (node for node in package.findall("opf:manifest/opf:item", ns)
             if "nav" in node.attrib.get("properties", "").split()),
            None,
        )
        navigation: list[dict] = []
        spine_target_index = {
            posixpath.normpath(manifest[item_id]): index
            for index, item_id in enumerate(spine)
        }
        if nav_item is not None:
            nav_path = (package_dir / unquote(nav_item.attrib["href"])).as_posix()
            if nav_path in names:
                nav_root = ET.fromstring(archive.read(nav_path))

                def nav_walk(ordered_list: ET.Element, level: int) -> None:
                    for li in ordered_list:
                        if li.tag.rsplit("}", 1)[-1] != "li":
                            continue
                        anchor = next((child for child in li if child.tag.rsplit("}", 1)[-1] == "a"), None)
                        if anchor is not None:
                            label = " ".join("".join(anchor.itertext()).split())
                            target = anchor.attrib.get("href", "").split("#", 1)[0]
                            resolved = posixpath.normpath(
                                posixpath.join(posixpath.dirname(nav_path), unquote(target))
                            )
                            navigation.append({
                                "level": level,
                                "label": label,
                                "spine_target": spine_target_index.get(resolved),
                            })
                        for child in li:
                            if child.tag.rsplit("}", 1)[-1] == "ol":
                                nav_walk(child, level + 1)

                for nav in nav_root.iter():
                    if nav.tag.rsplit("}", 1)[-1] == "nav":
                        nav_type = next((value for key, value in nav.attrib.items() if key.endswith("type")), "")
                        if nav_type == "toc":
                            listing = next((child for child in nav if child.tag.rsplit("}", 1)[-1] == "ol"), None)
                            if listing is not None:
                                nav_walk(listing, 1)
                            break
        else:
            toc_id = next((node.attrib.get("toc") for node in package if node.tag.endswith("spine")), None)
            ncx_item = next((node for node in package.findall("opf:manifest/opf:item", ns)
                             if node.attrib.get("id") == toc_id), None)
            if ncx_item is not None:
                ncx_path = (package_dir / unquote(ncx_item.attrib["href"])).as_posix()
                if ncx_path in names:
                    ncx = ET.fromstring(archive.read(ncx_path))

                    def ncx_walk(container: ET.Element, level: int) -> None:
                        for point in container:
                            if point.tag.rsplit("}", 1)[-1] != "navPoint":
                                continue
                            label_node = next((node for node in point.iter() if node.tag.rsplit("}", 1)[-1] == "text"), None)
                            content = next((node for node in point if node.tag.rsplit("}", 1)[-1] == "content"), None)
                            target = "" if content is None else content.attrib.get("src", "").split("#", 1)[0]
                            resolved = posixpath.normpath(
                                posixpath.join(posixpath.dirname(ncx_path), unquote(target))
                            )
                            navigation.append({
                                "level": level,
                                "label": " ".join("".join(label_node.itertext()).split()) if label_node is not None else "",
                                "spine_target": spine_target_index.get(resolved),
                            })
                            ncx_walk(point, level + 1)

                    nav_map = next((node for node in ncx.iter() if node.tag.rsplit("}", 1)[-1] == "navMap"), None)
                    if nav_map is not None:
                        ncx_walk(nav_map, 1)

        raw_text = "\n".join("".join(collector.text) for _, collector in collectors)
        normal_text = unicodedata.normalize("NFC", raw_text)
        tokens = re.findall(r"\w+|[^\w\s]", normal_text, flags=re.UNICODE)
        metadata = {}
        for child in package:
            if child.tag.endswith("metadata"):
                metadata = {
                    node.tag.rsplit("}", 1)[-1]: (node.text or "").strip()
                    for node in child
                    if node.tag.rsplit("}", 1)[-1] in {"title", "creator", "language", "identifier"}
                }
        headings = [heading for _, c in collectors for heading in c.headings if heading]
        counts = collections.Counter()
        for _, collector in collectors:
            counts.update(collector.tags)
        asset_hashes = []
        decoded_asset_hashes = []
        try:
            from PIL import Image
            from io import BytesIO
            image_decoder = Image
        except ImportError:
            image_decoder = None
        missing_image_references = 0
        for href, collector in collectors:
            for image_ref in collector.images:
                image_path = posixpath.normpath(
                    posixpath.join(posixpath.dirname(href), unquote(image_ref.split("#", 1)[0]))
                )
                if image_path in names:
                    image_bytes = archive.read(image_path)
                    asset_hashes.append(hashlib.sha256(image_bytes).hexdigest())
                    if image_decoder is not None:
                        try:
                            with image_decoder.open(BytesIO(image_bytes)) as decoded:
                                rgba = decoded.convert("RGBA")
                                canonical = (
                                    str(rgba.size).encode("ascii") + b"\0" + rgba.tobytes()
                                )
                            decoded_asset_hashes.append(hashlib.sha256(canonical).hexdigest())
                        except Exception:
                            decoded_asset_hashes.append("unreadable")
                else:
                    missing_image_references += 1
        link_hashes = []
        broken_link_references = 0
        internal_link_count = 0
        external_link_count = 0
        unsupported_link_scheme_count = 0
        ids_by_path = {href: collector.ids for href, collector in collectors}
        for source_href, collector in collectors:
            for link in collector.links:
                link_hashes.append(hashlib.sha256(link.encode("utf-8")).hexdigest())
                parsed = urlsplit(link)
                if parsed.scheme:
                    if parsed.scheme.lower() in {"http", "https", "mailto"}:
                        external_link_count += 1
                    else:
                        unsupported_link_scheme_count += 1
                    continue
                internal_link_count += 1
                target_path = source_href
                if parsed.path:
                    target_path = posixpath.normpath(
                        posixpath.join(
                            posixpath.dirname(source_href),
                            unquote(parsed.path.lstrip("/")),
                        )
                    )
                    if target_path not in names:
                        broken_link_references += 1
                        continue
                if parsed.fragment and parsed.fragment not in ids_by_path.get(target_path, set()):
                    broken_link_references += 1
        return {
            "spine_documents": len(spine),
            "text_token_count": len(tokens),
            "text_sha256": hashlib.sha256(normal_text.encode()).hexdigest(),
            "headings": headings,
            "paragraph_count": counts["p"],
            "list_items": counts["li"],
            "table_rows": counts["tr"],
            "table_cells": counts["td"] + counts["th"],
            "bold_elements": counts["b"] + counts["strong"],
            "italic_elements": counts["i"] + counts["em"],
            "images": sum(len(c.images) for _, c in collectors),
            "image_asset_sha256": sorted(asset_hashes),
            "decoded_image_sha256": sorted(decoded_asset_hashes),
            "missing_image_references": missing_image_references,
            "links": sum(len(c.links) for _, c in collectors),
            "internal_links": internal_link_count,
            "external_links": external_link_count,
            "unsupported_link_schemes": unsupported_link_scheme_count,
            "broken_link_references": broken_link_references,
            "link_target_sha256": sorted(link_hashes),
            "notes": sum(1 for href, c in collectors if "footnote" in href.lower() or "endnote" in href.lower())
            + counts["aside"],
            "navigation_text_sha256": hashlib.sha256(
                json.dumps(navigation, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            ).hexdigest() if navigation else None,
            "navigation_links": len(navigation),
            "navigation_entries": navigation,
            "metadata": metadata,
            "tokens_private": tokens,
        }


def project_text_counts(project: pathlib.Path) -> dict:
    import sqlite3

    db = sqlite3.connect(project / "story.db")
    try:
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        units = []
        for table in ("scene_content", "content_unit"):
            if table not in tables:
                continue
            path_column = "relative_path"
            id_column = "scene_id" if table == "scene_content" else "id"
            for entity_id, relative_path in db.execute(f"SELECT {id_column}, {path_column} FROM {table}"):
                text_path = project / relative_path
                content = text_path.read_text(encoding="utf-8") if text_path.exists() else ""
                units.append((table, entity_id, content))
        return {
            "text_units": len(units),
            "word_count": sum(len(re.findall(r"\w+", text, flags=re.UNICODE)) for _, _, text in units),
            "content_sha256": hashlib.sha256("\0".join(text for _, _, text in units).encode()).hexdigest(),
        }
    finally:
        db.close()


def verify_imported_source_content(project: pathlib.Path) -> dict:
    """Compare extracted Scrivener bodies with their persisted destinations.

    Only counts and hashes are returned, so report summaries never need to
    contain manuscript text.
    """
    import sqlite3

    db = sqlite3.connect(project / "story.db")
    db.row_factory = sqlite3.Row
    checked = 0
    matched = 0
    missing: list[str] = []
    changed: list[str] = []
    excluded_trash = 0
    research_checked = 0
    research_matched = 0
    manuscript_bodies = 0
    body_items = 0
    try:
        sources = db.execute(
            """SELECT id, source_document_id, raw_fields, mapping_decision
               FROM import_source_item ORDER BY source_document_id"""
        ).fetchall()
        for source in sources:
            raw = json.loads(source["raw_fields"] or "{}")
            binder = raw.get("binder_item", {})
            original = binder.get("content_markdown")
            if original is None:
                continue
            body_items += 1
            area = binder.get("area", "unknown")
            if area == "trash":
                excluded_trash += 1
                continue
            destinations = db.execute(
                """SELECT destination_type, destination_id
                   FROM entity_provenance WHERE source_item_id = ?""",
                (source["id"],),
            ).fetchall()
            source_hash = hashlib.sha256(original.encode("utf-8")).hexdigest()
            if area in {"research", "unknown"}:
                research_checked += 1
                exact_research = False
                for destination in destinations:
                    if destination["destination_type"] != "reading":
                        continue
                    row = db.execute(
                        "SELECT extracted_text FROM reading_document WHERE id = ?",
                        (destination["destination_id"],),
                    ).fetchone()
                    if row and hashlib.sha256((row["extracted_text"] or "").encode("utf-8")).hexdigest() == source_hash:
                        exact_research = True
                        break
                if exact_research:
                    research_matched += 1
                else:
                    missing.append(source["source_document_id"])
                continue
            if not destinations:
                missing.append(source["source_document_id"])
                continue
            checked += 1
            manuscript_bodies += 1
            found = False
            exact = False
            for destination in destinations:
                if destination["destination_type"] == "content_unit":
                    row = db.execute(
                        "SELECT relative_path FROM content_unit WHERE id = ?",
                        (destination["destination_id"],),
                    ).fetchone()
                elif destination["destination_type"] == "scene":
                    row = db.execute(
                        "SELECT relative_path FROM scene_content WHERE scene_id = ?",
                        (destination["destination_id"],),
                    ).fetchone()
                else:
                    continue
                if row is None:
                    continue
                found = True
                path = project / row["relative_path"]
                if path.is_file():
                    persisted = path.read_text(encoding="utf-8")
                    if hashlib.sha256(persisted.encode("utf-8")).hexdigest() == source_hash:
                        exact = True
                        break
            if exact:
                matched += 1
            elif not found:
                missing.append(source["source_document_id"])
            else:
                changed.append(source["source_document_id"])
        return {
            "source_items_with_body": body_items,
            "source_body_items_total": body_items,
            "manuscript_bodies_checked": manuscript_bodies,
            "exactly_preserved": matched,
            "research_bodies_checked": research_checked,
            "research_bodies_exactly_preserved": research_matched,
            "trash_bodies_intentionally_excluded": excluded_trash,
            "missing_destination_count": len(missing),
            "changed_body_count": len(changed),
            "missing_source_ids": missing,
            "changed_source_ids": changed,
        }
    finally:
        db.close()


def compare_tokens(reference: dict, generated: dict) -> dict:
    left, right = reference.pop("tokens_private"), generated.pop("tokens_private")
    matcher = difflib.SequenceMatcher(a=left, b=right, autojunk=True)
    equal = sum(block.size for block in matcher.get_matching_blocks())
    left_counts, right_counts = collections.Counter(left), collections.Counter(right)
    return {
        "reference_tokens": len(left),
        "generated_tokens": len(right),
        "ordered_matching_tokens": equal,
        "missing_token_instances": sum((left_counts - right_counts).values()),
        "additional_token_instances": sum((right_counts - left_counts).values()),
        "sequence_similarity": round(matcher.ratio(), 6),
    }


def epubcheck_result(command: list[str] | None, path: pathlib.Path, *, env: dict[str, str]) -> dict:
    if command is None:
        return {"status": "unavailable", "errors": None, "warnings": None}
    checked = subprocess.run(
        [*command, str(path)], env=env, text=True, capture_output=True
    )
    output = f"{checked.stdout}\n{checked.stderr}"
    summary = re.search(
        r"Messages:\s*(\d+) fatals?\s*/\s*(\d+) errors?\s*/\s*(\d+) warnings?\s*/\s*(\d+) infos?",
        output,
    )
    codes = collections.Counter(re.findall(r"(?:ERROR|FATAL)\(([A-Z]+-\d+)\)", output))
    no_errors = summary is not None and int(summary.group(1)) == 0 and int(summary.group(2)) == 0
    return {
        "status": "passed" if checked.returncode == 0 and no_errors else "failed",
        "exit_code": checked.returncode,
        "fatal_count": int(summary.group(1)) if summary else None,
        "errors": int(summary.group(2)) if summary else None,
        "warnings": int(summary.group(3)) if summary else None,
        "info_count": int(summary.group(4)) if summary else None,
        "diagnostic_codes": dict(sorted(codes.items())),
        "diagnostics_local": output.strip(),
    }


def epubcheck_command() -> list[str] | None:
    path = shutil.which("epubcheck")
    if path is None:
        return None
    resolved = pathlib.Path(path).resolve()
    return ["java", "-jar", str(resolved)] if zipfile.is_zipfile(resolved) else [path]


def compare_publication_structure(reference: dict, generated: dict) -> dict:
    fields = (
        "spine_documents", "paragraph_count", "list_items", "table_rows",
        "table_cells", "bold_elements", "italic_elements", "images", "links",
        "notes", "navigation_links", "navigation_text_sha256", "metadata",
    )
    differences = {}
    for field in fields:
        if reference.get(field) != generated.get(field):
            differences[field] = {
                "reference": reference.get(field),
                "generated": generated.get(field),
            }
    reference_assets = collections.Counter(reference.get("image_asset_sha256", []))
    generated_assets = collections.Counter(generated.get("image_asset_sha256", []))
    if reference_assets != generated_assets:
        differences["image_assets"] = {
            "missing_hash_count": sum((reference_assets - generated_assets).values()),
            "additional_hash_count": sum((generated_assets - reference_assets).values()),
        }
    reference_pixels = collections.Counter(reference.get("decoded_image_sha256", []))
    generated_pixels = collections.Counter(generated.get("decoded_image_sha256", []))
    if reference_pixels != generated_pixels:
        differences["decoded_image_assets"] = {
            "missing_hash_count": sum((reference_pixels - generated_pixels).values()),
            "additional_hash_count": sum((generated_pixels - reference_pixels).values()),
        }
    reference_navigation = [
        (entry.get("level"), unicodedata.normalize("NFC", entry.get("label", "")))
        for entry in reference.get("navigation_entries", [])
    ]
    generated_navigation = [
        (entry.get("level"), unicodedata.normalize("NFC", entry.get("label", "")))
        for entry in generated.get("navigation_entries", [])
    ]
    nav_matcher = difflib.SequenceMatcher(
        a=reference_navigation, b=generated_navigation, autojunk=False
    )
    nav_equal = sum(block.size for block in nav_matcher.get_matching_blocks())
    if reference_navigation != generated_navigation:
        differences["navigation_sequence"] = {
            "reference_entries": len(reference_navigation),
            "generated_entries": len(generated_navigation),
            "ordered_matching_entries": nav_equal,
            "sequence_similarity": round(nav_matcher.ratio(), 6),
        }
    if reference.get("missing_image_references") != generated.get("missing_image_references"):
        differences["missing_image_references"] = {
            "reference": reference.get("missing_image_references"),
            "generated": generated.get("missing_image_references"),
        }
    return differences


def regression_acceptance(books: dict[str, dict], epubcheck_version: str | None) -> str:
    compiled = [report["compile"]["epubcheck"] for report in books.values() if "compile" in report]
    if not compiled:
        return "not validated; no publication artifacts were compiled"
    if not epubcheck_version or any(check.get("status") == "unavailable" for check in compiled):
        return "provisional; EPUBCheck unavailable for one or more generated publications"
    if any(check.get("status") != "passed" or check.get("errors", 0) for check in compiled):
        return "provisional; generated EPUB validation failed or returned an incomplete result"
    if any(check.get("warnings", 0) for check in compiled):
        return "provisional; generated EPUBCheck warnings require investigation"
    scope = "all three generated EPUBs pass EPUBCheck" if len(compiled) == len(BOOKS) else "targeted generated EPUB passes EPUBCheck"
    return f"provisional; {scope}; discrepancy triage and human review remain"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-root", type=pathlib.Path, default=pathlib.Path("fixtures"))
    parser.add_argument("--generated-root", type=pathlib.Path, default=pathlib.Path("generated"))
    parser.add_argument("--reports-root", type=pathlib.Path, default=pathlib.Path("reports"))
    parser.add_argument("--book", choices=["all", *BOOKS], default="all")
    parser.add_argument("--phase", choices=["inspect", "import", "compile", "full"], default="full")
    parser.add_argument("--project-root", type=pathlib.Path, help="Existing imported-project root for --phase compile")
    parser.add_argument("--baseline-summary", type=pathlib.Path, help="Compare with a previous local WIDE run summary")
    parser.add_argument("--run-id", help="Private report/output directory name; defaults to UTC timestamp")
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parents[1]
    fixture_root = (root / args.fixture_root).resolve() if not args.fixture_root.is_absolute() else args.fixture_root
    generated_root = (root / args.generated_root).resolve() if not args.generated_root.is_absolute() else args.generated_root
    reports_root = (root / args.reports_root).resolve() if not args.reports_root.is_absolute() else args.reports_root
    run_id = args.run_id or dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_generated = generated_root / run_id
    run_reports = reports_root / run_id
    run_generated.mkdir(parents=True, exist_ok=False)
    run_reports.mkdir(parents=True, exist_ok=False)

    check = subprocess.run(["sha256sum", "--quiet", "-c", "SHA256SUMS"], cwd=fixture_root)
    if check.returncode:
        raise RuntimeError("fixture SHA-256 manifest failed; refusing to start regression run")
    for relative_source, relative_reference, _ in BOOKS.values():
        if not (fixture_root / relative_source).is_dir() or not (fixture_root / relative_reference).is_file():
            raise RuntimeError(f"configured Scrivener source or reference EPUB is unavailable under {fixture_root}")

    preferences = run_generated / "isolated-config"
    preferences.mkdir()
    env = os.environ.copy()
    env["XDG_CONFIG_HOME"] = str(preferences)
    env["CARGO_NET_OFFLINE"] = "true"
    run(["cargo", "build", "--offline", "-p", "writers-ide"], env=env)
    cli = root / "target/debug/writers-ide"
    selected = [args.book] if args.book != "all" else list(BOOKS)
    run_results = {
        "run_id": run_id,
        "timestamp_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "commit": run(["git", "rev-parse", "HEAD"], env=env).strip(),
        "working_tree_dirty": bool(run(["git", "status", "--porcelain"], env=env).strip()),
        "harness_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
        "python_version": platform.python_version(),
        "pillow_version": None,
        "epubcheck_version": None,
        "fixture_checksum": "passed",
        "phase": args.phase,
        "books": {},
    }
    epubcheck = epubcheck_command()
    try:
        import PIL
        run_results["pillow_version"] = PIL.__version__
    except ImportError:
        pass
    if epubcheck:
        version = subprocess.run([*epubcheck, "--version"], env=env, text=True, capture_output=True)
        version_output = (version.stdout or "") + (version.stderr or "")
        if re.search(r"EPUBCheck v\d", version_output):
            run_results["epubcheck_version"] = re.search(r"EPUBCheck v[^\s]+", version_output).group(0)
        else:
            run_results["epubcheck_version"] = "detected but unusable: " + (version.stderr.strip() or "version command failed")
            epubcheck = None
    baseline_summary = args.baseline_summary
    if baseline_summary is None:
        candidate = root / "baselines/current/summary.json"
        baseline_summary = candidate if candidate.is_file() else None
    if baseline_summary is not None and not baseline_summary.is_absolute():
        baseline_summary = root / baseline_summary
    previous = json.loads(baseline_summary.read_text(encoding="utf-8")) if baseline_summary else None
    run_results["baseline_summary"] = str(baseline_summary) if baseline_summary else None

    for book_id in selected:
        book_started = time.perf_counter()
        timings = {}
        relative_source, relative_reference, book_type = BOOKS[book_id]
        source = fixture_root / relative_source
        reference = fixture_root / relative_reference
        source_before = tree_fingerprint(source)
        report = {"book_type": book_type, "source_fingerprint_before": source_before}
        inventory_path = run_reports / f"{book_id}-inventory.json"
        inventory = None
        project = (
            ((root / args.project_root) if not args.project_root.is_absolute() else args.project_root)
            / f"{book_id}-project"
            if args.project_root
            else run_generated / f"{book_id}-project"
        )
        if args.phase in {"inspect", "import", "full"}:
            stage_started = time.perf_counter()
            inventory = json.loads(run([str(cli), "inspect-scrivener", str(source)], env=env))
            timings["inspect_seconds"] = round(time.perf_counter() - stage_started, 3)
            inventory_path.write_text(json.dumps(inventory, ensure_ascii=False, indent=2), encoding="utf-8")
            report["inventory"] = {
                "version": inventory["inventory"].get("format_version"),
                "creator": inventory["inventory"].get("creator"),
                "items": len(inventory["inventory"]["items"]),
                "proposed_unresolved_items": len(inventory["preview"]["unresolved_source_ids"]),
                "unsupported_warnings": len(inventory["preview"]["unsupported"]),
            }
            if args.phase == "inspect":
                run_results["books"][book_id] = report
                continue
        if args.phase in {"import", "full"}:
            stage_started = time.perf_counter()
            plan = make_plan(inventory["inventory"], book_type)
            mapping = run_reports / f"{book_id}-mapping.json"
            mapping.write_text(json.dumps(plan, ensure_ascii=False, indent=2), encoding="utf-8")
            import_out = run([str(cli), "import-scrivener", str(source), str(project), str(mapping), inventory["inventory"]["title"], f"--book-type={book_type}"], env=env)
            timings["import_seconds"] = round(time.perf_counter() - stage_started, 3)
            report["import_message"] = import_out.strip().splitlines()[-1] if import_out.strip() else "completed"
            report["project"] = str(project)
            report["persisted"] = project_text_counts(project)
            report["source_content_fidelity"] = verify_imported_source_content(project)
            import_report = project / "exports/scrivener-import-report.json"
            if import_report.exists():
                imported = json.loads(import_report.read_text(encoding="utf-8"))
                report["import_status"] = imported.get("import_run", {}).get("status")
                report["imported_scene_count"] = len(imported.get("imported_scene_ids", []))
        if args.phase in {"compile", "full"}:
            stage_started = time.perf_counter()
            if args.phase == "compile" and not args.project_root:
                raise RuntimeError("--phase compile requires --project-root pointing to previously imported projects")
            output = run_generated / f"{book_id}.epub"
            run([str(cli), "compile", str(project), "epub", str(output)], env=env)
            generated_semantics = epub_semantics(output)
            reference_semantics = epub_semantics(reference)
            generated_epubcheck = epubcheck_result(epubcheck, output, env=env)
            reference_epubcheck = epubcheck_result(epubcheck, reference, env=env)
            semantic_diff = compare_tokens(reference_semantics, generated_semantics)
            structure_diff = compare_publication_structure(reference_semantics, generated_semantics)
            report["compile"] = {
                "path": str(output),
                "bytes": output.stat().st_size,
                "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
                "epubcheck": generated_epubcheck,
            }
            report["reference"] = {key: value for key, value in reference_semantics.items() if key != "tokens_private"}
            report["reference"]["epubcheck"] = reference_epubcheck
            report["generated"] = {key: value for key, value in generated_semantics.items() if key != "tokens_private"}
            report["semantic_comparison"] = semantic_diff
            report["publication_structure_differences"] = structure_diff
            if previous and book_id in previous.get("books", {}):
                old = previous["books"][book_id].get("generated")
                report["baseline_comparison"] = (
                    compare_publication_structure(old, generated_semantics)
                    if old else {"status": "previous baseline has no semantic record"}
                )
            timings["compile_and_compare_seconds"] = round(time.perf_counter() - stage_started, 3)
        source_after = tree_fingerprint(source)
        report["source_unchanged"] = source_before == source_after
        report["source_fingerprint_after"] = source_after
        if not report["source_unchanged"]:
            raise RuntimeError(f"source Scrivener project changed during read-only test: {book_id}")
        timings["total_seconds"] = round(time.perf_counter() - book_started, 3)
        report["timings_seconds"] = timings
        run_results["books"][book_id] = report

    run_results["acceptance"] = regression_acceptance(
        run_results["books"], run_results["epubcheck_version"]
    )
    json_path = run_reports / "summary.json"
    json_path.write_text(json.dumps(run_results, ensure_ascii=False, indent=2), encoding="utf-8")
    lines = [f"# Private real-manuscript regression — {run_id}", "", f"Commit: `{run_results['commit']}`", "", "Fixture SHA-256: passed", ""]
    for book_id, report in run_results["books"].items():
        lines.extend([f"## {book_id}", "", f"- Import: {report.get('import_status', 'not run')}", f"- Source unchanged: {report.get('source_unchanged', 'not checked')}"])
        if "compile" in report:
            generated_check = report["compile"]["epubcheck"]
            reference_check = report["reference"].get("epubcheck", {})
            lines.append(
                f"- Generated EPUBCheck: {generated_check.get('status')}; "
                f"{generated_check.get('errors')} errors, {generated_check.get('warnings')} warnings"
            )
            lines.append(
                f"- Reference EPUBCheck: {reference_check.get('status')}; "
                f"{reference_check.get('errors')} errors, {reference_check.get('warnings')} warnings"
            )
        fidelity = report.get("source_content_fidelity")
        if fidelity:
            lines.append(
                f"- Source bodies: {fidelity['source_body_items_total']} total; "
                f"{fidelity['exactly_preserved']}/{fidelity['manuscript_bodies_checked']} "
                "manuscript bodies exact; "
                f"{fidelity['research_bodies_exactly_preserved']}/"
                f"{fidelity['research_bodies_checked']} research bodies exact; "
                f"{fidelity['trash_bodies_intentionally_excluded']} Trash bodies excluded"
            )
        if "semantic_comparison" in report:
            diff = report["semantic_comparison"]
            lines.extend([f"- Reference/generated text tokens: {diff['reference_tokens']} / {diff['generated_tokens']}", f"- Missing/additional token instances: {diff['missing_token_instances']} / {diff['additional_token_instances']}", f"- Ordered sequence similarity: {diff['sequence_similarity']}"])
        lines.append("")
    lines.append("Status: provisional; a passing process exit is not a fidelity acceptance result.")
    (run_reports / "summary.md").write_text("\n".join(lines), encoding="utf-8")
    print(f"Private regression reports: {run_reports}")
    print(f"Generated projects and EPUBs: {run_generated}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # keep report paths and errors visible without emitting manuscript text
        print(f"real-manuscript regression failed: {exc}", file=sys.stderr)
        raise SystemExit(1)

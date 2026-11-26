#!/usr/bin/env python3
"""Read-only inventory and cautious matching for the private back catalogue.

The JSON report is content-bearing through local paths and hashes. Keep it
under ignored reports/ and never publish it. The script never executes corpus
files and invokes only the production read-only `inspect-scrivener` command.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
import re
import shutil
import subprocess
import sys
import time
import unicodedata
import zipfile
from difflib import SequenceMatcher
from html.parser import HTMLParser
from pathlib import Path
from xml.etree import ElementTree


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCRIVENER = ROOT / "fixtures/back-catalogue/scrivener"
DEFAULT_PUBLICATIONS = ROOT / "fixtures/back-catalogue/epub"
PUBLICATION_FORMATS = {
    "doc", "docx", "epub", "html", "htm", "mobi", "odt", "pdf", "rtf", "txt"
}
ASSET_FORMATS = {"jpg", "jpeg", "png", "gif", "svg", "webp"}


class TextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.skip = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag.lower() in {"script", "style", "head", "nav"}:
            self.skip += 1

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() in {"script", "style", "head", "nav"} and self.skip:
            self.skip -= 1

    def handle_data(self, data: str) -> None:
        if not self.skip:
            self.parts.append(data)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_hash(path: Path) -> str:
    digest = hashlib.sha256()
    for child in sorted(path.rglob("*")):
        if child.is_symlink():
            digest.update(b"SYMLINK\0" + child.relative_to(path).as_posix().encode())
            digest.update(os.readlink(child).encode("utf-8", "surrogateescape"))
        elif child.is_file():
            digest.update(child.relative_to(path).as_posix().encode("utf-8"))
            digest.update(bytes.fromhex(sha256_file(child)))
    return digest.hexdigest()


def classify_file(path: Path) -> str:
    """Separate candidate publications from supporting visual assets."""
    suffix = path.suffix.lower().lstrip(".")
    if suffix in ASSET_FORMATS:
        return "supporting_asset"
    if suffix in PUBLICATION_FORMATS:
        return "publication_candidate"
    return "other_resource"


def binder_depths(items: list[dict]) -> dict[str, int]:
    """Calculate each binder item's depth without assuming a flat outline."""
    by_id = {item.get("source_document_id"): item for item in items}
    depths: dict[str, int] = {}
    for item in items:
        source_id = item.get("source_document_id")
        if not source_id:
            continue
        cursor = item
        seen: set[str] = set()
        depth = 0
        while cursor.get("parent_source_id"):
            parent = cursor["parent_source_id"]
            if parent in seen or parent not in by_id:
                break
            seen.add(parent)
            depth += 1
            cursor = by_id[parent]
        depths[source_id] = depth
    return depths


def binder_structure(items: list[dict]) -> list[dict[str, object]]:
    """Keep hierarchy and classification evidence while omitting manuscript text."""
    depths = binder_depths(items)
    return [
        {
            "source_document_id": item.get("source_document_id"),
            "parent_source_id": item.get("parent_source_id"),
            "position": item.get("position"),
            "depth": depths.get(item.get("source_document_id"), 0),
            "title": item.get("title"),
            "type_code": item.get("type_code"),
            "area": item.get("area"),
            "include_in_compile": item.get("include_in_compile"),
            "section_type_id": item.get("section_type_id"),
            "content_bytes": len((item.get("content_markdown") or "").encode("utf-8")),
            "has_notes": bool(item.get("notes_markdown")),
            "has_synopsis": bool(item.get("synopsis")),
            "metadata_field_names": sorted((item.get("fields") or {}).keys()),
            "embedded_assets": [
                {"media_type": asset.get("media_type"), "sha256": asset.get("sha256")}
                for asset in item.get("embedded_assets", [])
            ],
        }
        for item in items
    ]


def norm_words(value: str) -> list[str]:
    value = unicodedata.normalize("NFC", html.unescape(value)).casefold()
    return re.findall(r"[^\W_]+(?:['’][^\W_]+)?", value, flags=re.UNICODE)


def language_hint(value: str, declared: str | None = None) -> dict[str, str]:
    if declared:
        return {"value": declared, "basis": "source metadata"}
    scripts: dict[str, int] = {}
    for char in value:
        if char.isalpha():
            name = unicodedata.name(char, "")
            for script in ("HANGUL", "CYRILLIC", "GREEK", "ARABIC", "HEBREW", "HIRAGANA", "KATAKANA", "CJK"):
                if script in name:
                    scripts[script.lower()] = scripts.get(script.lower(), 0) + 1
                    break
    if scripts:
        script, count = max(scripts.items(), key=lambda item: item[1])
        return {"value": f"{script}-script (language uncertain)", "basis": f"Unicode script heuristic; {count} letters"}
    try:
        from langdetect import DetectorFactory, detect_langs

        DetectorFactory.seed = 0
        candidates = detect_langs(value[:100_000])
        if candidates:
            return {
                "value": candidates[0].lang,
                "basis": f"optional langdetect candidate p={candidates[0].prob:.2f}; not authoritative",
            }
    except Exception:  # Optional local classifier may reject sparse/empty project text.
        pass
    return {"value": "Latin-script (language unclassified)", "basis": "local language classifier unavailable or insufficient text"}


def word_shingles(words: list[str], width: int = 3) -> set[tuple[str, ...]]:
    return {tuple(words[index : index + width]) for index in range(max(0, len(words) - width + 1))}


def jaccard(left: set[tuple[str, ...]], right: set[tuple[str, ...]]) -> float:
    if not left or not right:
        return 0.0
    return len(left & right) / len(left | right)


def epub_text(path: Path) -> tuple[str, dict[str, str]]:
    with zipfile.ZipFile(path) as archive:
        container = ElementTree.fromstring(archive.read("META-INF/container.xml"))
        rootfile = next(iter(container.iter("{urn:oasis:names:tc:opendocument:xmlns:container}rootfile")))
        opf = rootfile.attrib["full-path"]
        package = ElementTree.fromstring(archive.read(opf))
        ns = {"opf": "http://www.idpf.org/2007/opf", "dc": "http://purl.org/dc/elements/1.1/"}
        metadata = {}
        for key in ("title", "creator", "language", "identifier", "date"):
            element = package.find(f".//dc:{key}", ns)
            if element is not None and element.text:
                metadata[key] = element.text.strip()
        manifest = {n.attrib.get("id"): n.attrib.get("href") for n in package.findall(".//opf:manifest/opf:item", ns)}
        base = Path(opf).parent
        spine = [n.attrib.get("idref") for n in package.findall(".//opf:spine/opf:itemref", ns)]
        texts = []
        for ident in spine:
            href = manifest.get(ident)
            if not href:
                continue
            source = (base / href).as_posix()
            source = str(Path(*Path(source).parts))
            if source not in archive.namelist():
                continue
            parser = TextExtractor()
            parser.feed(archive.read(source).decode("utf-8", "replace"))
            texts.extend(parser.parts)
        return " ".join(texts), metadata


def document_text(path: Path, staging: Path) -> tuple[str, dict[str, str]]:
    suffix = path.suffix.lower()
    if suffix == ".epub":
        return epub_text(path)
    if suffix in {".txt", ".html", ".htm"}:
        raw = path.read_bytes()
        text = raw.decode("utf-8", "replace")
        if suffix in {".html", ".htm"}:
            parser = TextExtractor()
            parser.feed(text)
            text = " ".join(parser.parts)
        return text, {}
    if suffix in {".docx", ".odt"}:
        with zipfile.ZipFile(path) as archive:
            content_name = "word/document.xml" if suffix == ".docx" else "content.xml"
            root = ElementTree.fromstring(archive.read(content_name))
            if suffix == ".docx":
                text = " ".join(node.text or "" for node in root.iter() if node.tag.endswith("}t"))
            else:
                def flatten_odt(node) -> str:
                    pieces = [node.text or ""]
                    for child in node:
                        local = child.tag.rsplit("}", 1)[-1]
                        if local == "s":
                            count = next((value for key, value in child.attrib.items() if key.rsplit("}", 1)[-1] == "c"), "1")
                            pieces.append(" " * int(count))
                        elif local == "tab":
                            pieces.append("\t")
                        elif local == "line-break":
                            pieces.append("\n")
                        else:
                            pieces.append(flatten_odt(child))
                        pieces.append(child.tail or "")
                    return "".join(pieces)

                paragraphs = [node for node in root.iter() if node.tag.rsplit("}", 1)[-1] in {"p", "h"}]
                text = "\n".join(flatten_odt(node) for node in paragraphs)
            metadata = {}
            meta_name = "docProps/core.xml" if suffix == ".docx" else "meta.xml"
            if meta_name in archive.namelist():
                meta_root = ElementTree.fromstring(archive.read(meta_name))
                for node in meta_root.iter():
                    if node.text and node.text.strip():
                        metadata[node.tag.rsplit("}", 1)[-1].lower()] = node.text.strip()
            return text, metadata
    if suffix == ".rtf":
        pandoc = shutil.which("pandoc")
        if pandoc:
            result = subprocess.run([pandoc, "--from=rtf", "--to=plain", str(path)], capture_output=True, check=False)
            if result.returncode == 0:
                return result.stdout.decode("utf-8", "replace"), {}
        raw = path.read_bytes().decode("cp1252", "replace")
        text = re.sub(r"\\'[0-9a-fA-F]{2}", " ", raw)
        text = re.sub(r"\\[a-zA-Z]+-?\d* ?|[{}]", " ", text)
        return text, {}
    if suffix == ".doc":
        tool = shutil.which("catdoc")
        if tool:
            result = subprocess.run([tool, str(path)], capture_output=True, check=False)
            if result.returncode == 0:
                return result.stdout.decode("utf-8", "replace"), {}
        return "", {"text_extraction": "unavailable (catdoc missing or failed)"}
    if suffix == ".pdf":
        tool = shutil.which("pdftotext")
        if tool:
            result = subprocess.run([tool, str(path), "-"], capture_output=True, check=False)
            if result.returncode == 0:
                return result.stdout.decode("utf-8", "replace"), {}
        return "", {"text_extraction": "unavailable (pdftotext missing or failed)"}
    if suffix == ".mobi":
        tool = shutil.which("ebook-meta")
        metadata = {}
        if tool:
            result = subprocess.run([tool, str(path)], capture_output=True, text=True, check=False)
            for line in result.stdout.splitlines():
                if ":" in line:
                    key, value = line.split(":", 1)
                    if key.strip().lower() in {"title", "author", "language", "publisher", "published"}:
                        metadata[key.strip().lower()] = value.strip()
        converter = shutil.which("ebook-convert")
        if converter:
            staging.mkdir(parents=True, exist_ok=True)
            destination = staging / f"{hashlib.sha256(str(path).encode()).hexdigest()[:16]}.txt"
            result = subprocess.run([converter, str(path), str(destination)], capture_output=True, check=False)
            if result.returncode == 0 and destination.is_file():
                return destination.read_text(encoding="utf-8", errors="replace"), metadata
            metadata["text_extraction"] = "ebook-convert failed; metadata-only match"
        return "", metadata
    return "", {}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scrivener-root", type=Path, default=DEFAULT_SCRIVENER)
    parser.add_argument("--publication-root", type=Path, default=DEFAULT_PUBLICATIONS)
    parser.add_argument("--cli", type=Path, default=ROOT / "target/debug/writers-ide")
    parser.add_argument("--output", type=Path, required=True, help="JSON destination inside ignored reports/")
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    if not output.resolve().is_relative_to((ROOT / "reports").resolve()):
        parser.error("report output must be inside the repository's ignored reports/ directory")
    if not args.cli.is_file():
        parser.error(f"production CLI not found: {args.cli}; build writers-ide first")

    scrivx_files = sorted(args.scrivener_root.rglob("*.scrivx"))
    projects = []
    project_words: dict[str, list[str]] = {}
    for manifest in scrivx_files:
        project = manifest.parent
        if project.is_symlink() or any(parent.is_symlink() for parent in project.parents if parent == args.scrivener_root or args.scrivener_root in parent.parents):
            continue
        result = subprocess.run([str(args.cli), "inspect-scrivener", str(project)], capture_output=True, text=True, check=False)
        record: dict[str, object] = {
            "path": project.relative_to(ROOT).as_posix() if project.is_relative_to(ROOT) else str(project),
            "manifest_path": manifest.relative_to(ROOT).as_posix() if manifest.is_relative_to(ROOT) else str(manifest),
            "sha256_tree": tree_hash(project),
            "inspection_status": "ok" if result.returncode == 0 else "failed",
            "nested_within_scrivener_project": next(
                (
                    parent.relative_to(ROOT).as_posix() if parent.is_relative_to(ROOT) else str(parent)
                    for parent in project.parents
                    if parent != args.scrivener_root and parent.suffix.lower() == ".scriv"
                ),
                None,
            ),
        }
        if result.returncode:
            record["error"] = result.stderr.strip()[:1000]
            projects.append(record)
            continue
        payload = json.loads(result.stdout)
        inv = payload["inventory"]
        items = inv.get("items", [])
        depths = binder_depths(items)
        text = "\n".join(item.get("content_markdown") or "" for item in items)
        project_words[record["path"]] = norm_words(text)
        record.update({
            "title": inv.get("title"),
            "scrivener_creator": inv.get("creator"),
            "creator_version_hint": f"Scrivener creator tag {inv.get('creator')} (source-reported; platform/version compatibility not independently established)",
            "manifest_revision": inv.get("format_version"),
            "project_metadata": inv.get("project_metadata", {}),
            "source_fingerprint": inv.get("source_fingerprint"),
            "binder_item_count": len(items),
            "document_type_counts": _count(item.get("type_code", "unknown") for item in items),
            "included_flags": _count(str(item.get("include_in_compile", "unknown")) for item in items),
            "binder_depth_counts": _count(str(depth) for depth in depths.values()),
            "maximum_binder_depth": max(depths.values(), default=0),
            "top_level_item_count": sum(not item.get("parent_source_id") for item in items),
            "binder_structure": binder_structure(items),
            "content_bearing_items": sum(bool(item.get("content_markdown")) for item in items),
            "word_count_estimate": len(norm_words(text)),
            "content_fingerprint": hashlib.sha256(" ".join(norm_words(text)).encode()).hexdigest(),
            "language": language_hint(text, inv.get("project_metadata", {}).get("Language")),
            "section_type_names": sorted(set(inv.get("section_types", {}).values())),
            "section_type_definitions": inv.get("section_types", {}),
            "embedded_asset_count": sum(len(item.get("embedded_assets", [])) for item in items),
            "embedded_asset_media_types": _count(
                asset.get("media_type", "unknown")
                for item in items for asset in item.get("embedded_assets", [])
            ),
            "metadata_field_name_counts": _count(
                field_name for item in items for field_name in (item.get("fields") or {})
            ),
            "items_with_notes": sum(bool(item.get("notes_markdown")) for item in items),
            "items_with_synopsis": sum(bool(item.get("synopsis")) for item in items),
            "source_warnings": inv.get("warnings", []),
            "unsupported_count": len(inv.get("unsupported", [])) + sum(len(item.get("unsupported", [])) for item in items),
            "preview": {key: payload.get("preview", {}).get(key) for key in ("total_items", "manuscript_items", "research_items", "skipped_items")},
            "has_compile_settings": inv.get("compile_settings") is not None,
        })
        projects.append(record)

    publications = []
    publication_words: dict[str, list[str]] = {}
    mobi_staging = ROOT / "generated" / f"back-catalogue-inventory-{time.time_ns()}" / "mobi-text"
    for path in sorted(p for p in args.publication_root.rglob("*") if p.is_file()):
        if path.is_symlink():
            continue
        text, metadata = document_text(path, mobi_staging)
        relative_path = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)
        tokens = norm_words(text)
        if tokens:
            publication_words[relative_path] = tokens
        publications.append({
            "path": relative_path,
            "format": path.suffix.lower().lstrip("."),
            "classification": classify_file(path),
            "size_bytes": path.stat().st_size,
            "sha256": sha256_file(path),
            "metadata": metadata,
            "word_count_estimate": len(tokens) if tokens else None,
            "content_fingerprint": hashlib.sha256(" ".join(tokens).encode()).hexdigest() if tokens else None,
            "language": language_hint(text, metadata.get("language")) if text else {"value": "unclassified", "basis": "text extraction unavailable for this format"},
        })

    matches = []
    for project in projects:
        if project.get("inspection_status") != "ok":
            continue
        source_name = Path(str(project["path"])).name.removesuffix(".scriv")
        source_tokens = set(norm_words(source_name))
        ranked = []
        for publication in publications:
            if publication["classification"] != "publication_candidate":
                continue
            title = str(publication.get("metadata", {}).get("title", ""))
            stem_tokens = set(norm_words(Path(str(publication["path"])).stem))
            title_tokens = set(norm_words(title))
            overlap = len(source_tokens & (stem_tokens | title_tokens)) / max(1, len(source_tokens))
            source_name = " ".join(norm_words(Path(str(project["path"])).name.removesuffix(".scriv")))
            stem_name = " ".join(norm_words(Path(str(publication["path"])).stem))
            title_name = " ".join(norm_words(title))
            path_ratio = max(SequenceMatcher(None, source_name, candidate).ratio() for candidate in (stem_name, title_name))
            content_score = None
            if project.get("content_fingerprint") and publication.get("content_fingerprint"):
                content_score = jaccard(
                    word_shingles(project_words.get(project["path"], [])),
                    word_shingles(publication_words.get(publication["path"], [])),
                )
            score = max(overlap, path_ratio, content_score or 0.0)
            if score >= 0.2:
                confidence = "strong candidate" if overlap >= 0.8 or path_ratio >= 0.9 or (content_score or 0.0) >= 0.9 else "possible candidate" if overlap >= 0.5 or path_ratio >= 0.7 or (content_score or 0.0) >= 0.45 else "uncertain candidate"
                evidence = {"name_token_overlap": round(overlap, 3), "filename_similarity": round(path_ratio, 3), "content_3gram_jaccard": round(content_score or 0.0, 3)}
                ranked.append((score, publication, evidence, confidence))
        ranked.sort(key=lambda value: (-value[0], str(value[1]["path"])))
        for rank, (score, publication, evidence, confidence) in enumerate(ranked[:5], start=1):
            matches.append({
                "scrivener_project": project["path"],
                "candidate_publication": publication["path"],
                "rank": rank,
                "confidence": confidence,
                "score": round(score, 3),
                "evidence": evidence,
                "confirmed_match": False,
            })

    by_content: dict[str, list[str]] = {}
    for project in projects:
        fingerprint = project.get("content_fingerprint")
        if fingerprint:
            by_content.setdefault(str(fingerprint), []).append(str(project["path"]))
    duplicates = [{"basis": "exact normalized extracted text", "paths": paths} for paths in by_content.values() if len(paths) > 1]
    by_source_fingerprint: dict[str, list[str]] = {}
    for project in projects:
        fingerprint = project.get("source_fingerprint")
        if fingerprint:
            by_source_fingerprint.setdefault(str(fingerprint), []).append(str(project["path"]))
    duplicates.extend(
        {"basis": "identical Scrivener source fingerprint", "paths": paths}
        for paths in by_source_fingerprint.values() if len(paths) > 1
    )
    project_similarity = []
    project_paths = sorted(project_words)
    project_shingles = {path: word_shingles(project_words[path]) for path in project_paths}
    for index, left in enumerate(project_paths):
        for right in project_paths[index + 1 :]:
            similarity = jaccard(project_shingles[left], project_shingles[right])
            if similarity >= 0.08:
                project_similarity.append({
                    "left": left,
                    "right": right,
                    "content_3gram_jaccard": round(similarity, 4),
                    "classification": "possible related edition/project; inspect source history before asserting revision relationship",
                })

    controlled_comparisons = []
    for source in sorted((ROOT / "fixtures").glob("book-*/source/*.scriv")):
        candidates = []
        result = subprocess.run([str(args.cli), "inspect-scrivener", str(source)], capture_output=True, text=True, check=False)
        if result.returncode:
            controlled_comparisons.append({"controlled_source": source.relative_to(ROOT).as_posix(), "status": "inspection failed", "error": result.stderr.strip()[:500]})
            continue
        inv = json.loads(result.stdout)["inventory"]
        words = norm_words("\n".join(item.get("content_markdown") or "" for item in inv.get("items", [])))
        shingles = word_shingles(words)
        for path in project_paths:
            score = jaccard(shingles, project_shingles[path])
            if score >= 0.03:
                candidates.append({"back_catalogue_project": path, "content_3gram_jaccard": round(score, 4)})
        candidates.sort(key=lambda item: -item["content_3gram_jaccard"])
        controlled_comparisons.append({
            "controlled_source": source.relative_to(ROOT).as_posix(),
            "title": inv.get("title"),
            "scrivener_creator": inv.get("creator"),
            "manifest_revision": inv.get("format_version"),
            "word_count_estimate": len(words),
            "nearest_back_catalogue_projects": candidates[:5],
            "exact_content_matches": [
                path for path in project_paths
                if jaccard(shingles, project_shingles[path]) == 1.0
            ],
            "matching_source_fingerprints": [
                project["path"] for project in projects
                if project.get("source_fingerprint") == inv.get("source_fingerprint")
            ],
            "classification": "similarity leads only; confirm with source metadata and revision evidence",
        })

    staging_relative = mobi_staging.relative_to(ROOT).as_posix()
    if mobi_staging.exists() and mobi_staging.is_relative_to(ROOT / "generated"):
        shutil.rmtree(mobi_staging)
        run_root = mobi_staging.parent
        if run_root.is_relative_to(ROOT / "generated") and not any(run_root.iterdir()):
            run_root.rmdir()

    result = {
        "schema_version": 1,
        "generated_utc": __import__("datetime").datetime.now(__import__("datetime").timezone.utc).isoformat(),
        "notice": "Private local inventory. Language and publication matches are candidates, not authoritative classifications.",
        "scrivener_project_manifest_count": len(projects),
        "publication_file_count": sum(p["classification"] == "publication_candidate" for p in publications),
        "supporting_asset_file_count": sum(p["classification"] == "supporting_asset" for p in publications),
        "other_resource_file_count": sum(p["classification"] == "other_resource" for p in publications),
        "mobi_conversion_staging": f"{staging_relative} (temporary extracted text removed after analysis)",
        "projects": projects,
        "publications": publications,
        "publication_match_candidates": matches,
        "exact_normalized_text_duplicate_candidates": duplicates,
        "possible_related_project_pairs": project_similarity,
        "controlled_fixture_comparisons": controlled_comparisons,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "output": output.relative_to(ROOT).as_posix() if output.is_relative_to(ROOT) else str(output),
        "scrivener_project_manifest_count": len(projects),
        "publication_file_count": sum(p["classification"] == "publication_candidate" for p in publications),
        "supporting_asset_file_count": sum(p["classification"] == "supporting_asset" for p in publications),
        "other_resource_file_count": sum(p["classification"] == "other_resource" for p in publications),
        "projects_inspected": sum(p.get("inspection_status") == "ok" for p in projects),
        "inspection_failures": sum(p.get("inspection_status") != "ok" for p in projects),
        "publication_match_candidates": len(matches),
        "exact_text_duplicate_groups": len(duplicates),
    }, ensure_ascii=False))
    return 0 if all(p.get("inspection_status") == "ok" for p in projects) else 1


def _count(values) -> dict[str, int]:
    result: dict[str, int] = {}
    for value in values:
        key = str(value)
        result[key] = result.get(key, 0) + 1
    return result


if __name__ == "__main__":
    sys.exit(main())

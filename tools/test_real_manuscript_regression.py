import tempfile
import unittest
import zipfile
import os
from unittest import mock
from pathlib import Path

from real_manuscript_regression import (
    detailed_epub_difference,
    epub_document_details,
    epub_semantics,
    epub_semantic_css_rules,
    epubcheck_command,
    epubcheck_result,
    make_plan,
    regression_acceptance,
)


class EpubSemanticTests(unittest.TestCase):
    def test_xhtml_details_retain_paragraph_formatting_tables_assets_and_links(self):
        details = epub_document_details(
            '''<html xmlns="http://www.w3.org/1999/xhtml"><body>
              <h2>Chapter One</h2><p>A <strong>bold</strong> word with an en dash –.</p>
              <pre>  intentional code</pre><p>Visible **literal markers**.</p>
              <figure><img src="../images/stone.png" alt="Stone garden"/><figcaption>Figure 1</figcaption></figure>
              <table><tr><th>Year</th><th>Place</th></tr><tr><td>2026</td><td>Kyoto</td></tr></table>
              <p><a href="#chapter">Chapter link</a></p>
            </body></html>''',
            "OPS/chapter.xhtml",
        )
        self.assertEqual([item["text"] for item in details["paragraphs"]], [
            "A bold word with an en dash –.", "Visible **literal markers**.", "Chapter link"
        ])
        self.assertEqual(details["headings"][0]["level"], 2)
        self.assertEqual(details["preformatted"][0]["text"], "  intentional code")
        self.assertEqual(details["visible_markdown_artifacts"][0]["text"], "Visible **literal markers**.")
        self.assertEqual(len(details["tables"][0]["rows"]), 2)
        self.assertEqual(details["images"][0]["alt"], "Stone garden")
        self.assertEqual(details["links"][0]["href"], "#chapter")
        self.assertIn("U+2013 EN DASH", details["unicode_counts"])

    def test_malformed_reference_xhtml_is_reported_and_recovered_for_differential(self):
        details = epub_document_details(
            '<html><body><ul><li><p>First row</p><p>Second row</li></ul>'
            '<p><a href="mailto:editor@example.org?subject=Erratum&bcc=copy@example.org">Contact</a></p>'
            '</body></html>',
            "OPS/legacy.xhtml",
        )
        self.assertTrue(details["recovered_from_malformed_xhtml"])
        self.assertIsNotNone(details["parse_error"])
        self.assertEqual([paragraph["text"] for paragraph in details["paragraphs"]], [
            "First row", "Second row", "Contact",
        ])
        self.assertEqual(details["links"][0]["href"], "mailto:editor@example.org?subject=Erratum&bcc=copy@example.org")

    def test_detailed_comparison_reports_paragraph_location_and_unicode_delta(self):
        reference = {"documents": [{"path": "ref/chapter.xhtml", "paragraphs": [
            {"path": "/body/p[1]", "text": "The garden–wall."},
            {"path": "/body/p[2]", "text": "A second paragraph."},
        ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []}
        generated = {"documents": [{"path": "wide/chapter.xhtml", "paragraphs": [
            {"path": "/body/p[1]", "text": "The gardenwall."},
            {"path": "/body/p[2]", "text": "A second paragraph."},
        ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []}
        difference = detailed_epub_difference(
            reference,
            generated,
            {
                "manuscript_bodies_checked": 2,
                "exactly_preserved": 2,
                "missing_destination_count": 0,
                "changed_body_count": 0,
            },
        )
        change = difference["paragraphs"]["changed_regions"][0]
        self.assertEqual(change["reference"][0]["path"], "/body/p[1]")
        self.assertIn("U+2013", str(difference["characters"]["missing_from_generated"]))
        char_edit = change["character_differences"][0]["edits"][0]
        self.assertEqual(char_edit["operation"], "delete")
        self.assertEqual(char_edit["reference_text"], "–")
        self.assertEqual(char_edit["missing_characters"][0]["codepoint"], "U+2013")
        self.assertIn("Compile IR", change["probable_stage"])
        self.assertEqual(change["stage_evidence"]["status"], "source-bodies-exact-after-reopen")
        self.assertTrue(any(
            "canonical manuscript persistence" in stage
            for stage in change["stage_evidence"]["ruled_out"]
        ))

    def test_detailed_comparison_reports_import_mismatch_as_candidate_stage(self):
        reference = {"documents": [{"path": "ref.xhtml", "paragraphs": [
            {"path": "/body/p[1]", "text": "Expected source paragraph."},
        ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []}
        generated = {"documents": [{"path": "wide.xhtml", "paragraphs": [
            {"path": "/body/p[1]", "text": "Different paragraph."},
        ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []}
        difference = detailed_epub_difference(
            reference,
            generated,
            {
                "manuscript_bodies_checked": 3,
                "exactly_preserved": 2,
                "missing_destination_count": 1,
                "changed_body_count": 0,
            },
        )
        evidence = difference["paragraphs"]["changed_regions"][0]["stage_evidence"]
        self.assertEqual(evidence["status"], "source-body-mismatch")
        self.assertIn("Scrivener extraction or import mapping", evidence["probable_stages"])
        self.assertEqual(evidence["evidence"], "2/3 checked manuscript bodies match after reopen; 1 destinations are missing and 0 persisted bodies differ.")

    def test_detailed_comparison_lists_css_and_metadata_differences(self):
        reference = {
            "documents": [],
            "css_summary": {"font_families": ["Serif"], "declarations": ["font-size: 10pt"]},
            "metadata": {"title": "Reference", "language": "fr"},
            "navigation_entries": [],
        }
        generated = {
            "documents": [],
            "css_summary": {"font_families": ["Sans"], "declarations": ["font-size: 12pt"]},
            "metadata": {"title": "Generated", "language": "fr"},
            "navigation_entries": [],
        }
        difference = detailed_epub_difference(reference, generated)
        self.assertEqual(
            difference["css"]["differences"]["font_families"],
            {"removed_from_generated": ["Serif"], "added_in_generated": ["Sans"]},
        )
        self.assertEqual(
            difference["metadata"]["differences"]["title"],
            {"reference": "Reference", "generated": "Generated"},
        )

    def test_detailed_comparison_finds_missing_semantic_emphasis_by_location(self):
        reference_doc = epub_document_details(
            '<html><body><p>Keep <strong>this phrase</strong> prominent.</p></body></html>',
            "ref/chapter.xhtml",
        )
        generated_doc = epub_document_details(
            '<html><body><p>Keep this phrase prominent.</p></body></html>',
            "wide/chapter.xhtml",
        )
        difference = detailed_epub_difference(
            {"documents": [reference_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
            {"documents": [generated_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
        )
        self.assertEqual(reference_doc["formatting"][0]["kind"], "bold")
        missing = difference["semantic_formatting"]["missing_from_generated"]
        self.assertEqual(missing, [{
            "kind": "bold", "text": "this phrase", "count": 1,
            "spine_index": 0, "container_kind": "p",
            "container_text": "Keep this phrase prominent.",
            "locations": [{
                "document": "ref/chapter.xhtml", "path": "/html/body[1]/p[1]/strong[1]",
                "source_element": "strong",
                "spine_index": 0, "container_kind": "p",
                "container_text": "Keep this phrase prominent.",
            }],
        }])
        self.assertEqual(
            difference["semantic_formatting"]["reference"][0]["document"],
            "ref/chapter.xhtml",
        )

    def test_detailed_comparison_normalises_equivalent_bold_xhtml_elements(self):
        reference_doc = epub_document_details(
            '<html><body><p class="scrivener-bold">meaningful emphasis</p></body></html>',
            "ref.xhtml",
            epub_semantic_css_rules([("book.css", ".scrivener-bold { font-weight: bold; }")]),
        )
        generated_doc = epub_document_details(
            '<html><body><p><strong>meaningful emphasis</strong></p></body></html>', "wide.xhtml"
        )
        difference = detailed_epub_difference(
            {"documents": [reference_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
            {"documents": [generated_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
        )
        self.assertEqual(difference["semantic_formatting"]["missing_from_generated"], [])
        self.assertEqual(difference["semantic_formatting"]["additional_in_generated"], [])

    def test_semantic_css_comparison_handles_mixed_emphasis_and_small_caps(self):
        rules = epub_semantic_css_rules([(
            "book.css",
            ".bold-italic { font-weight: bold; font-style: italic; }\n"
            "span.small-caps { font-variant: small-caps; }",
        )])
        details = epub_document_details(
            '<html><body><p><span class="bold-italic">important</span> '
            '<span class="small-caps">NOTE</span></p></body></html>',
            "chapter.xhtml",
            rules,
        )
        self.assertEqual(
            [(mark["kind"], mark["text"]) for mark in details["formatting"]],
            [("bold", "important"), ("italic", "important"), ("small-caps", "NOTE")],
        )

    def test_detailed_comparison_reports_emphasis_moved_between_paragraphs(self):
        reference_doc = epub_document_details(
            '<html><body><p><strong>same words</strong> here.</p><p>Elsewhere.</p></body></html>',
            "ref.xhtml",
        )
        generated_doc = epub_document_details(
            '<html><body><p>same words here.</p><p><strong>same words</strong> elsewhere.</p></body></html>',
            "wide.xhtml",
        )
        difference = detailed_epub_difference(
            {"documents": [reference_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
            {"documents": [generated_doc], "css_summary": {}, "metadata": {}, "navigation_entries": []},
        )
        self.assertEqual(
            difference["semantic_formatting"]["missing_from_generated"][0]["container_text"],
            "same words here.",
        )
        self.assertEqual(
            difference["semantic_formatting"]["additional_in_generated"][0]["container_text"],
            "same words elsewhere.",
        )

    def test_character_report_distinguishes_substitution_and_added_accent(self):
        difference = detailed_epub_difference(
            {"documents": [{"path": "reference.xhtml", "paragraphs": [
                {"path": "/p[1]", "text": "café — stone"},
            ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []},
            {"documents": [{"path": "generated.xhtml", "paragraphs": [
                {"path": "/p[1]", "text": "cafe – stone!"},
            ]}], "css_summary": {}, "metadata": {}, "navigation_entries": []},
        )
        char_edits = difference["paragraphs"]["changed_regions"][0]["character_differences"][0]["edits"]
        codepoints = {
            entry["codepoint"]
            for edit in char_edits
            for key in ("missing_characters", "additional_characters")
            for entry in edit[key]
        }
        self.assertTrue({"U+00E9", "U+2014", "U+2013", "U+0021"}.issubset(codepoints))
        self.assertTrue(all(edit["reference_range"] and edit["generated_range"] for edit in char_edits))

    def test_ambiguous_binder_items_are_preserved_as_non_publication_research(self):
        inventory = {
            "items": [
                {
                    "source_document_id": "planning-folder",
                    "type_code": "Folder",
                    "area": "unknown",
                    "parent_source_id": None,
                },
                {
                    "source_document_id": "character-notes",
                    "type_code": "Text",
                    "area": "unknown",
                    "parent_source_id": "planning-folder",
                },
            ]
        }
        plan = make_plan(inventory, "fiction")
        self.assertEqual(
            [decision["destination_kind"] for decision in plan["decisions"]],
            ["Research", "Research"],
        )

    def test_selected_matter_under_project_notes_is_mapped_without_publishing_private_root(self):
        inventory = {
            "items": [
                {"source_document_id": "notes", "type_code": "Folder", "area": "unknown", "parent_source_id": None},
                {"source_document_id": "front", "type_code": "Text", "area": "unknown", "parent_source_id": "notes"},
                {"source_document_id": "back", "type_code": "Text", "area": "unknown", "parent_source_id": "notes"},
            ],
            "compile_settings": {"formats": [{
                "add_default_front_matter": True,
                "default_front_matter_id": "front",
                "add_default_back_matter": True,
                "default_back_matter_id": "back",
            }]},
        }
        plan = make_plan(inventory, "fiction")
        kinds = {item["source_document_id"]: item["destination_kind"] for item in plan["decisions"]}
        self.assertEqual(kinds, {"notes": "Research", "front": "FrontMatter", "back": "BackMatter"})

    def test_conflicting_compile_matter_roles_remain_unmapped(self):
        inventory = {
            "items": [{"source_document_id": "matter", "type_code": "Text", "area": "unknown", "parent_source_id": None}],
            "compile_settings": {"formats": [
                {"add_default_front_matter": True, "default_front_matter_id": "matter"},
                {"add_default_back_matter": True, "default_back_matter_id": "matter"},
            ]},
        }
        plan = make_plan(inventory, "fiction")
        self.assertEqual(plan["decisions"][0]["destination_kind"], "Unmapped")
        self.assertTrue(plan["decisions"][0]["include_in_import"])

    def test_trash_remains_excluded_even_if_selected_as_compile_matter(self):
        inventory = {
            "items": [{"source_document_id": "discarded", "type_code": "Text", "area": "trash", "parent_source_id": None}],
            "compile_settings": {"formats": [{
                "add_default_front_matter": True,
                "default_front_matter_id": "discarded",
            }]},
        }
        plan = make_plan(inventory, "fiction")
        self.assertEqual(plan["decisions"][0]["destination_kind"], "Skip")
        self.assertFalse(plan["decisions"][0]["include_in_import"])

    def test_acceptance_does_not_claim_epubcheck_is_missing_after_success(self):
        books = {
            "book-01": {"compile": {"epubcheck": {"status": "passed", "errors": 0, "warnings": 0}}},
            "book-02": {"compile": {"epubcheck": {"status": "passed", "errors": 0, "warnings": 0}}},
            "book-03": {"compile": {"epubcheck": {"status": "passed", "errors": 0, "warnings": 0}}},
        }
        result = regression_acceptance(books, "EPUBCheck v4.2.6")
        self.assertIn("all three generated EPUBs pass EPUBCheck", result)
        self.assertIn("discrepancy triage and human review remain", result)
        self.assertNotIn("EPUBCheck unavailable", result)

    def test_acceptance_marks_missing_epubcheck_as_unvalidated(self):
        books = {"book-01": {"compile": {"epubcheck": {"status": "unavailable"}}}}
        self.assertIn("EPUBCheck unavailable", regression_acceptance(books, None))

    def test_link_destinations_and_broken_internal_references_are_counted(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "synthetic.epub"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr(
                    "META-INF/container.xml",
                    '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
                    '<rootfiles><rootfile full-path="OPS/package.opf" '
                    'media-type="application/oebps-package+xml"/></rootfiles></container>',
                )
                archive.writestr(
                    "OPS/package.opf",
                    '<package xmlns="http://www.idpf.org/2007/opf" version="3.0">'
                    '<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">'
                    '<dc:title>Synthetic</dc:title></metadata>'
                    '<manifest><item id="chapter" href="text/chapter.xhtml" '
                    'media-type="application/xhtml+xml"/></manifest>'
                    '<spine><itemref idref="chapter"/></spine></package>',
                )
                archive.writestr(
                    "OPS/text/chapter.xhtml",
                    '<html xmlns="http://www.w3.org/1999/xhtml"><body>'
                    '<p><span id="target">Target</span> '
                    '<a href="#target">valid internal</a> '
                    '<a href="missing.xhtml#lost">broken internal</a> '
                    '<a href="https://example.org/read">external</a></p>'
                    '</body></html>',
                )

            result = epub_semantics(path)

        self.assertEqual(result["links"], 3)
        self.assertEqual(result["internal_links"], 2)
        self.assertEqual(result["external_links"], 1)
        self.assertEqual(result["unsupported_link_schemes"], 0)
        self.assertEqual(result["broken_link_references"], 1)
        self.assertEqual(len(result["link_target_sha256"]), 3)

    def test_epubcheck_diagnostics_are_reported(self):
        completed = mock.Mock(
            returncode=1,
            stdout="ERROR(RSC-005): OPS/chapter.xhtml(5,1): invalid XHTML\nMessages: 0 fatals / 1 errors / 2 warnings / 0 infos",
            stderr="",
        )
        with mock.patch("real_manuscript_regression.subprocess.run", return_value=completed):
            result = epubcheck_result(["epubcheck"], Path("book.epub"), env={})

        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["errors"], 1)
        self.assertEqual(result["warnings"], 2)
        self.assertEqual(result["diagnostic_codes"], {"RSC-005": 1})

    def test_epubcheck_unavailable_is_explicit(self):
        self.assertEqual(
            epubcheck_result(None, Path("book.epub"), env={}),
            {"status": "unavailable", "errors": None, "warnings": None},
        )

    def test_epubcheck_detects_historical_reference_style_markup_defects(self):
        command = epubcheck_command()
        if command is None:
            self.skipTest("EPUBCheck is not installed")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "synthetic-defects.epub"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr("mimetype", "application/epub+zip", compress_type=zipfile.ZIP_STORED)
                archive.writestr(
                    "META-INF/container.xml",
                    '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
                    '<rootfiles><rootfile full-path="OPS/package.opf" '
                    'media-type="application/oebps-package+xml"/></rootfiles></container>',
                )
                archive.writestr(
                    "OPS/package.opf",
                    '<?xml version="1.0" encoding="UTF-8"?>'
                    '<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id">'
                    '<metadata xmlns:dc="http://purl.org/dc/elements/1.1/" '
                    'xmlns:opf="http://www.idpf.org/2007/opf">'
                    '<dc:title>Synthetic</dc:title><dc:identifier id="id">urn:synthetic:test</dc:identifier>'
                    '<dc:language>en</dc:language><dc:date opf:event="publication">YYYY / YYYY-MM / YYYY-MM-DD</dc:date>'
                    '</metadata><manifest><item id="chapter" href="chapter.xhtml" '
                    'media-type="application/xhtml+xml"/><item id="ncx" href="toc.ncx" '
                    'media-type="application/x-dtbncx+xml"/></manifest>'
                    '<spine toc="ncx"><itemref idref="chapter"/></spine></package>',
                )
                archive.writestr(
                    "OPS/toc.ncx",
                    '<ncx version="2005-1" xmlns="http://www.daisy.org/z3986/2005/ncx/">'
                    '<head><meta name="dtb:uid" content="urn:synthetic:test"/></head>'
                    '<docTitle><text>Synthetic</text></docTitle><navMap><navPoint id="n1" playOrder="1">'
                    '<navLabel><text>Synthetic</text></navLabel><content src="chapter.xhtml"/>'
                    '</navPoint></navMap></ncx>',
                )
                archive.writestr(
                    "OPS/chapter.xhtml",
                    '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Synthetic</title></head>'
                    '<body><ul><li><p>Nested paragraph</p></li></ul>'
                    '<a href="mailto:writer@example.test?subject=Erratum Discovery&amp;body=Please report it">Email</a>'
                    '</body></html>',
                )
            result = epubcheck_result(command, path, env=os.environ.copy())

        self.assertEqual(result["status"], "failed")
        self.assertGreaterEqual(result["diagnostic_codes"].get("OPF-054", 0), 1, result["diagnostics_local"])
        self.assertGreaterEqual(result["diagnostic_codes"].get("RSC-005", 0), 1, result["diagnostics_local"])
        self.assertGreaterEqual(result["diagnostic_codes"].get("RSC-020", 0), 1, result["diagnostics_local"])


if __name__ == "__main__":
    unittest.main()

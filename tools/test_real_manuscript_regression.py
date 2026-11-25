import tempfile
import unittest
import zipfile
import os
from unittest import mock
from pathlib import Path

from real_manuscript_regression import (
    epub_semantics,
    epubcheck_command,
    epubcheck_result,
    regression_acceptance,
)


class EpubSemanticTests(unittest.TestCase):
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

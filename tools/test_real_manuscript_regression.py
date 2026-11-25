import tempfile
import unittest
import zipfile
from pathlib import Path

from real_manuscript_regression import epub_semantics


class EpubSemanticTests(unittest.TestCase):
    def test_link_destinations_and_broken_internal_references_are_counted(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "synthetic.epub"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr(
                    "META-INF/container.xml",
                    '<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
                    '<rootfiles><rootfile full-path="OPS/package.opf"/></rootfiles></container>',
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


if __name__ == "__main__":
    unittest.main()

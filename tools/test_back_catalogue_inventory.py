import importlib.util
import tempfile
import unittest
import zipfile
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("back_catalogue_inventory.py")
SPEC = importlib.util.spec_from_file_location("back_catalogue_inventory", MODULE_PATH)
assert SPEC and SPEC.loader
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class BackCatalogueInventoryTests(unittest.TestCase):
    def test_word_normalization_preserves_unicode_words_and_apostrophes(self):
        self.assertEqual(
            inventory.norm_words("Les Retraités – l’été 1185–1333"),
            ["les", "retraités", "l’été", "1185", "1333"],
        )

    def test_shingle_similarity_is_empty_for_unrelated_short_inputs(self):
        self.assertEqual(inventory.jaccard(inventory.word_shingles(["one", "two"]), inventory.word_shingles(["one", "two"])), 0)

    def test_shingle_similarity_detects_repeated_publication_content(self):
        source = inventory.word_shingles(inventory.norm_words("A quiet garden has a stone path beside the water"))
        edition = inventory.word_shingles(inventory.norm_words("A quiet garden has a stone path beside the water"))
        self.assertEqual(inventory.jaccard(source, edition), 1.0)

    def test_language_guess_is_explicitly_non_authoritative(self):
        result = inventory.language_hint(
            "The quick brown fox writes a short English sentence for language detection.",
            "en-GB",
        )
        self.assertEqual(result, {"value": "en-GB", "basis": "source metadata"})

    def test_odt_text_extraction_preserves_paragraphs_spans_and_spaces(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.odt"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr(
                    "content.xml",
                    '<document xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">'
                    '<text:p>Garden <text:span>wall</text:span><text:s text:c="2"/>stone.</text:p>'
                    '<text:h>Second chapter</text:h></document>',
                )
                archive.writestr("meta.xml", "<meta/>")
            extracted, _ = inventory.document_text(path, Path(temporary) / "staging")
            self.assertEqual(extracted, "Garden wall  stone.\nSecond chapter")


if __name__ == "__main__":
    unittest.main()

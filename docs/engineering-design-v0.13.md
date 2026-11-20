# Writer's IDE — Engineering Design v0.13

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.12

## EPUB reference fixture

The reading importer now has a synthetic, standards-shaped EPUB fixture created entirely in a temporary directory. It includes the required mimetype entry, container document, OPF metadata/manifest/spine, and an XHTML chapter. The test imports via the same `import_reference` path used by the application, verifies the title and extracted spine text, and compares source bytes before and after to ensure the reference is untouched.

This proves basic EPUB package/spine extraction without relying on internet access or undistributed user material. It does not claim broad compatibility with DRM-protected, malformed, encrypted, or unusual real-world EPUB packages.

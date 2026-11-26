# Engineering Design v2.45 addendum — Scrivener layout inclusion in Compile IR

**Date:** 26 November 2026  
**Status:** Implemented with automated verification; human publication UAT pending

This addendum extends the v2.44 project lifecycle design without changing the
canonical manuscript model.

## Source inclusion and layout presentation

Imported Scrivener publication profiles now apply a binder document's
`IncludeInCompile=No` to ordinary manuscript entries when building that
profile's ordered assembly. The document, stable WIDE identity, prose and
metadata remain in the project. Inclusion affects the profile assembly and
shared Compile IR only.

Scrivener layout `Include Text=No` is represented as a per-compile-unit body
suppression rule in `CompilePresentation`. The unit can still emit its
configured title/heading, while its body text and source-map range are omitted
from the generated representation. Canonical prose is unchanged and remains
available in the editor and other publication profiles. The rule uses the
stable assembly-item identity and is persisted with the imported format
profile. Existing profile compilation sends this presentation through the
same Compile IR renderer used by other formats.

The suppression identifier set is optional in serialized presentation data;
older edition recipes deserialize with an empty set. Empty identifiers are
invalid. Existing Scrivener default front/back matter selection continues to
be format-specific; ordinary project compilation still excludes alternative
matter by default.

## Verification contract

The synthetic regression imports a Scrivener source with one included scene,
one source-excluded scene, and an imported layout that includes the scene title
but excludes its text. It verifies that the profile assembly omits the
source-excluded scene, retains the configured heading, suppresses the body in
the generated artifact, and leaves both source documents intact. Ordinary
project compilation continues to include the included scene's prose.

This does not establish full Scrivener publication fidelity or author
acceptance. Unsupported layout prefixes/suffixes and real-project visual
publication differences remain subject to the warnings and regression
workflow in `REAL_MANUSCRIPT_REGRESSION.md`.

# Artwork provenance

Product artwork must be authored for ScribbleCap or carry an explicit compatible
licence and attribution. Do not trace, extract, recolour or adapt another app's
icons, stamp set, screenshots, sounds, fonts or branded illustrations.

| Asset | Source and evidence | Licence / notes |
|---|---|---|
| App capture-frame / loop-arrow icon | `icon/scribblecap.svg`; introduced in commit `8a15483` with an original-design description and AI co-author attribution. PNG and platform icon files are exports. | Distributed with this project's MIT OR Apache-2.0 code; the history is provenance evidence, not a legal originality opinion. |
| Signal check, alert and spark stamps | New normalized vector coordinates in `crates/core/src/geometry/primitives.rs`, authored during M2-10, 2026-11-25; preview in `crates/core/tests/golden/render/signal_stamps.png`. No competitor asset or reference image used. | MIT OR Apache-2.0. Generic symbol ideas are independently expressed; no exclusive rights in generic checks, punctuation or geometric shapes are claimed. |
| New marker, highlighter, stamp and step toolbar glyphs | Inline paths in `ui/src/App.svelte`, authored during the same change from basic geometric primitives. | MIT OR Apache-2.0. No downloaded icon set. |
| Step digits / text font | Unmodified Noto Sans Regular; source, SHA-256 and copyright in `crates/core/assets/fonts/README.md` and `OFL.txt`. | SIL OFL 1.1; separate from project code licensing. The built app includes the complete OFL notice in Third-party notices. |
| Rendering fixtures | Core renderer outputs from synthetic test documents, including the Signal golden. | No competitor screenshots or artwork used as fixtures. |

Earlier stamp names remain readable for file compatibility; the new editor palette
exposes the newly authored Signal set. Clean-room behavioral notes are specifications,
not permission to copy implementation or artwork. Neither review of this change nor
the existing icon commit establishes worldwide copyright/trademark clearance.

Before shipping, retain third-party notices and check final branding for confusing
similarity. The word ScribbleCap and app icon have not received a comprehensive
trademark clearance search in this work. The README's Evernote/Skitch references
are historical context with an independence disclaimer, not product branding.
No dependency versions were added by this editor change. Existing transitive
dependency licences and release notice requirements were not exhaustively audited;
the font notice panel is not a complete inventory of all linked libraries.

Official background (US; other jurisdictions can differ):
- [Copyright Office Circular 33](https://www.copyright.gov/circs/circ33.pdf): distinguishes ideas/common symbols from protectable creative expression.
- [USPTO likelihood of confusion](https://www.uspto.gov/trademarks/search/likelihood-confusion): branding concerns depend on marks and related goods/services.

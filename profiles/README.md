# Canonical thumbnail profile

`srgb.icc` is a 588-byte canonical sRGB profile generated with Little CMS 2's `cmsCreate_sRGBProfile` / `cmsSaveProfileToMem`. Its creation timestamp is fixed to 2000-01-01 and optional profile ID cleared for reproducible identity. The generated profile's copyright tag permits free use; no Little CMS executable/library is bundled.

The Rust helper embeds this profile for bounded ImageMagick/LCMS conversion of source ICC-tagged images. Its SHA-256 participates in the input fingerprint. Display/monitor profiles are not inspected or reproduced.

`fixtures/profiles/wide-rgb.icc` is independently generated test data: D65 white, primaries R=(0.64,0.33), G=(0.21,0.71), B=(0.15,0.06), gamma 2.2, with the same fixed timestamp. The independent Little CMS 8-bit reference transforms [120,150,100] to canonical sRGB [105,151,97]. It is not a redistributed proprietary vendor profile.

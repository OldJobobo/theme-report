# Bundled sRGB profile

`srgb.icc` is a 588-byte sRGB profile generated with Little CMS 2 (`cmsCreate_sRGBProfile` and `cmsSaveProfileToMem`). Its creation timestamp is fixed to 2000-01-01 and its optional profile ID is cleared, so the file is byte-for-byte reproducible. The profile's copyright tag permits free use. No Little CMS executable or library is bundled.

The analyzer embeds this profile and uses it as the target when ImageMagick/LCMS converts ICC-tagged wallpapers, within the limits in `docs/measurement.md`. Its SHA-256 is part of the input fingerprint. Display and monitor profiles are not read or reproduced.

`fixtures/profiles/wide-rgb.icc` is test data generated for this project, not taken from elsewhere: D65 white, primaries R=(0.64,0.33), G=(0.21,0.71), B=(0.15,0.06), gamma 2.2, with the same fixed timestamp. An independent Little CMS 8-bit transform, run outside the analyzer as a reference, maps [120,150,100] in it to [105,151,97] in the bundled sRGB profile.

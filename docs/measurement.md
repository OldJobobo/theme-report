# Measurement model v4

## Colors and contrast

Scored solid colors use opaque six-digit RGB hex. sRGB channels are normalized to [0,1]; linearization is `v/12.92` for `v<=0.04045`, otherwise `((v+0.055)/1.055)^2.4`. Relative luminance is `0.2126R + 0.7152G + 0.0722B`. Contrast is `(max(L1,L2)+0.05)/(min(L1,L2)+0.05)`.

Composition is straight alpha over encoded sRGB channels, matching the inspected Qt surface token usage, not linear-light compositing. Layers are applied back to front. Evidence includes a hex display approximation, but ratios are calculated from unrounded f64 composite channels.

Perceptual difference is Euclidean distance in OKLab using the published sRGB/D65 transform coefficients in `src/color.rs`. It is not distance between cylindrical OKLCH coordinates and not RGB distance. Achromatic colors need no hue handling. No gamut-changing color-vision simulation contributes to v1 scoring.

Source references identify logical files and configuration roles; `profile.defaults` means the pinned compiled runtime fallback, not an authored file. Machine-level role overrides are labeled `user.shell.toml`. References are remediation targets, not complete token-alias lineage or source-line traces. Foreground/background display approximations and exact ratios remain in the evidence.

Text uses 4.5:1; known indicator roles use 3:1 under restored ruleset 5. This helper does not infer large text from unknown runtime font settings. Exact hard floors and losses are published in `rules/v1.md`.

## Wallpaper sampling

Spatial model: center-cropped 160×90, 8×8 samples. Native images retain the pinned Rust nearest-neighbor model; color-managed/high-resolution thumbnails use `imagemagick-lcms-thumbnail-v4`. Video retains `center-crop-160x90-grid8-video5-v2`.

1. Accept static PNG/JPEG/WebP only (animated WebP and APNG are limitations). Keep encoded inputs ≤16 MiB in bounded memory snapshots; stream larger inputs up to 256 MiB into private immutable disk copies with full SHA-256. Originals are never resized or modified. Animation, unsupported non-ICC color metadata, and sampled transparency remain limitations.
2. The Rust decoder retains its 8192×8192 / 32-million-pixel / 256 MiB allocation bounds, 512 MiB child address-space and 4-second CPU limits, and 6-second parent deadline. Larger encoded JPEGs use decoder-assisted reduction when available. ICC-tagged and resolution-limited images use bounded ImageMagick/LCMS thumbnails: forced PNG/JPEG/WebP coders only, first frame, one CPU thread, OpenCL disabled, 256 MiB memory, no mapped/disk pixel cache, 16384 dimension and 64MP area limits, 2 GiB child address-space, 8-second CPU and 4-second wall limits, exactly 57,600 RGBA output bytes. No external image format delegate is requested.
3. Apply EXIF orientation. Embedded ICC is converted to bundled canonical sRGB (`profiles/srgb.icc`), not ignored; untagged supported channels are assumed sRGB. Center-crop with nearest/Point filtering. JPEG decoder-assisted downsampling makes this a representative thumbnail rather than a full-resolution pixel minimum. Native decoder, external decoder build, bundled profile, and model identities participate in cache fingerprints.
4. Sample an 8×8 grid at integer coordinates `x=(2i+1)*160/16`, `y=(2j+1)*90/16`, for `i,j=0..7`, in row-major order. This is a representative stress sample, not an exhaustive pixel minimum or percentile estimate.
5. Compose lock input background directly over each sample. For launcher, compose its scrim over the sample, then its card, then selected fill. Evaluate the corresponding text against those modeled surfaces.
6. Use the minimum contrast across samples and complete supported backgrounds, with one fixed budget per rule. Any incomplete expected input leaves dependent wallpaper scoring unassessed. Decode corruption belongs to validity, not a second wallpaper deduction.
7. Reuse the same samples for translucent wallpaper-backed lock text in core readability. If core readability owns an opaque or sampled lock/reference-text defect, retain wallpaper evidence without a second deduction. Launcher-over-wallpaper remains an explicitly labeled stress scenario, not measured live application-backed contrast.

The model is deliberately unblurred and fixed-size. It does not claim to reproduce the lock's actual blur, wallpaper placement on each monitor, launcher content over applications, or display color management. These are named assumptions in every report. There is no monitor discovery or screenshot analysis in the score.

## Application-background stress tests

Model `application-gray17-v4` uses 17 evenly spaced encoded-sRGB gray backgrounds from black through white. Translucent bar/popup/menu/launcher/notification/polkit/tooltip text and shared selected/focus fills are composited over those reference backgrounds. Launcher text also includes its known scrim and selected fill. These are bounded reference scenarios, not captured application windows or a guarantee over every RGB color, gradient, or geometry.

Fill grading uses worst sampled OKLab separation: ≥0.02 passes; weaker fills incur a half-budget caution, never a full failure based solely on unknown redundant labels/icons/borders. Visible popup borders use worst sampled contrast across each native solid/gradient border color and each popup interior. Resolve aliases with cycle detection; gradients accept 2–8 supported color stops and an optional finite degree angle, sampling 17 colors per adjacent pair. This is not exhaustive spatial or rendered-pixel verification. V5 border stops follow native RGB/RGBA/hex parsing, preserve each stop's alpha, multiply by the surface border-alpha with native alpha-byte rounding, and interpolate premultiplied encoded-sRGB components. Unsupported border tokens/geometry are diagnosed explicitly rather than blamed on popup transparency. Malformed/unsupported values stay honest limitations. The compact panel labels assessed modeled categories “Modeled” unless they need attention.

## Video sampling

MP4/M4V/MOV, WebM/MKV, and AVI are recognized as native wallpaper media. Capture streams each video into a private immutable temporary file while computing full-content SHA-256; media bytes are not held in the report or UI memory. Duplicate content shares staging and decoding.

`ffprobe` validates the first video stream, dimensions, duration, frame rate, pixel format, and color metadata. The model supports opaque 8-bit SDR RGB/YUV, untagged or supported BT.709/sRGB metadata. HDR, wider-gamut/color-managed, alpha-bearing, unknown-duration, or unsupported codec inputs remain unassessed. Video duration is bounded to 600 seconds and frame rate to 240 fps. This does not make valid Omarchy media a theme failure.

Up to five frames are requested at 0%, 25%, 50%, 75%, and 95% of duration, clamped to the last frame interval; duplicate seek positions are omitted for very short media. Seek times are requests, not certification of exact presentation timestamps. Each frame uses FFmpeg nearest-neighbor center-crop to 160×90 and the same 8×8 grid. FFmpeg respects rotation metadata. Decoded SDR RGB is modeled as encoded sRGB, not monitor/color-managed runtime pixels. The minimum across sampled frames and other complete media drives the existing wallpaper rules, with no additional per-frame budget.

Each probe/frame subprocess has one decoding/filter thread, a 2-second wall deadline, 8-second CPU limit, 2 GiB address-space limit, 256 MiB single-allocation ceiling, and bounded stdout/stderr. Frame output is exactly 43,200 bytes. Per-video frame scheduling stops after 10 seconds. Input demuxers are restricted to recognized local containers; only file/pipe protocols are allowed, MOV external data references are disabled, and audio/subtitle/data output is discarded. Missing tools/codecs, exhausted limits, unsupported metadata, or unavailable sample frames create limitations; definite malformed containers/frames belong only to background validity.

FFmpeg/ffprobe build/version identity participates in the fingerprint. Reports include seek positions, sample counts, source SHA-256, decoder metadata, and the explicit representative-sampling assumption. Five temporal samples do not prove every frame, transition, playback policy, lock scrim/blur, or monitor transform safe.

## Bounds and operational states

Text input ceiling: 1 MiB each. Snapshot ceiling: 128 MiB including the selected background and relevant runtime contracts. File reads reject non-regular files and use nonblocking/no-follow opens on Unix.

Background discovery is non-recursive. Dotfiles, subdirectories (such as stock `intros/`) and files that are neither a known media extension nor recognizable image/video content (such as `README.md`) are ignored and listed in `validity.backgrounds` evidence as `ignored`. Up to 32 wallpapers are sampled in name order; extra files are limitations. Theme files must resolve within the theme root: an escaping `colors.toml`/`shell.toml` aborts the audit, while an escaping wallpaper (or a `backgrounds/` directory that resolves elsewhere) is never read and becomes a limitation. The explicit current background is separately authorized input.

Large-image/video snapshot ceilings are 256 MiB per input and a shared 1 GiB per capture, on disk, with a 10-second capture deadline. Owned temporary snapshots are removed on normal completion. Other unsupported formats are identified from bounded header/extension checks and remain unassessed; their descriptors include filename, size, and modification time rather than content hashes. Small PNG/JPEG inputs use bounded immutable byte snapshots; larger ones use the shared media disk staging and deduplication.

Unique background content is decoded once per audit. Image work stops scheduling new decodes after 25 seconds. Current writers wait at most 35 seconds for the OS advisory lock. Capture/publication consistency retries at most three times, waiting 150 ms and then 450 ms between attempts so a theme switch can finish.

Media copies and decoder scratch files live in a per-process `run-<pid>` directory inside the cache. It is removed on normal exit and on SIGTERM/SIGINT/SIGHUP (closing the panel terminates the analyzer). Decoder subprocesses are tied to the analyzer with `PR_SET_PDEATHSIG`. Each audit sweeps `run-<pid>` directories whose process no longer exists, and top-level `.tmp*` files older than two minutes left by older releases.

Resource limits, unstable capture, unavailable decoding, and unknown runtime contracts are limitations/operational outcomes, not invented theme deductions. Best-effort manifest checks cannot guarantee atomic reads from arbitrarily mutating directory trees or a perfect match to transient compositor state. No audit executes theme content.

## Determinism and fingerprint

SHA-256 manifest v4 includes analyzer/rules/model identifiers, fixed profile ID, sorted logical input roles with length-prefixed bytes, full-content disk-media hashes/container identities, image/video decoder build identities and canonical ICC profile hash, unsupported-input descriptors, and missing-role markers. Relevant Color/Style/template/geometry/palette-fallback sources and machine-level shell overrides are captured for current audits. The ordered selected/background roles and image bytes participate, even when duplicate content is decoded only once.

Absolute provenance paths, watched paths, audit timestamp, installed package version, and volatile UI state are not scored payload fields. The current cache additionally checks target path/name identity.

A separate metadata manifest covers every input role: length, modification and change times, inode, device, and resolved link target, plus listed/ignored background names and the decoder executables' metadata. It is taken before any content is read and compared again after capture and after analysis to detect concurrent changes without re-reading media. `current.manifest` records the manifest alongside the published report's fingerprint. When both match, a current audit reuses the cache without reading theme content at all. Any metadata change falls back to a full capture and fingerprint comparison; `--force` always re-audits. Tested host and VM results compare exact scored goldens where possible; documented f64 tolerance for cross-platform vectors is 1e-9. Display rounding never alters decisions.

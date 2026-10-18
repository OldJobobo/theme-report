# Measurement model v4

## Colors and contrast

Scored solid colors use opaque six-digit RGB hex. sRGB channels are normalized to [0,1]; linearization is `v/12.92` for `v<=0.04045`, otherwise `((v+0.055)/1.055)^2.4`. Relative luminance is `0.2126R + 0.7152G + 0.0722B`. Contrast is `(max(L1,L2)+0.05)/(min(L1,L2)+0.05)`.

Compositing uses straight alpha over encoded sRGB channels, as Omarchy's Qt surfaces do, not linear light. Layers are applied back to front. Evidence shows a rounded hex value, but ratios come from the unrounded f64 composite channels.

Perceptual difference is Euclidean distance in OKLab, using the published sRGB (D65) transform coefficients in `src/color.rs`. It is not a distance between OKLCH coordinates, and not an RGB distance, so gray colors need no hue handling. No color-vision simulation affects scoring.

Source references name the file and key to change. `profile.defaults` means the runtime fallback compiled into the analyzer, not a file you wrote. Overrides from your own `~/.config/omarchy/shell.toml` are labeled `user.shell.toml`. References point at what to edit; they are not full alias chains or line numbers. The evidence keeps both the rounded foreground and background colors and the exact ratios.

Text targets 4.5:1; known indicator roles target 3:1. The analyzer never treats text as large, because the runtime font size is unknown. Exact thresholds and losses are in `rules/v1.md`.

## Wallpaper sampling

Each wallpaper is center-cropped to 160×90 and sampled on an 8×8 grid. Ordinary images use the analyzer's built-in nearest-neighbor decoder. Color-managed and high-resolution images use the `imagemagick-lcms-thumbnail-v4` model. Video uses `center-crop-160x90-grid8-video5-v2`.

1. Accept static PNG, JPEG and WebP only; animated WebP and APNG are limitations. Encoded inputs up to 16 MiB are held in bounded memory snapshots. Larger inputs, up to 256 MiB, are streamed into private read-only disk copies with a full SHA-256. Originals are never resized or modified. Animation, unsupported non-ICC color metadata and sampled transparency are limitations.
2. The built-in decoder is limited to 8192×8192 pixels, 32 million pixels and 256 MiB per allocation. It runs in a child process with a 512 MiB address space, a 4-second CPU limit and a 6-second deadline set by the parent. Larger JPEGs use decoder-assisted reduction when available. ICC-tagged images and images over the size limit go through bounded ImageMagick/LCMS thumbnails with these limits: PNG, JPEG and WebP coders only, first frame only, one CPU thread, OpenCL off, 256 MiB memory, no mapped or disk pixel cache, 16384-pixel dimensions, 64-megapixel area, 2 GiB address space, 8 seconds of CPU, 4 seconds of wall time, and exactly 57,600 bytes of RGBA output. No external format delegate is used.
3. Apply EXIF orientation. An embedded ICC profile is converted to the bundled sRGB profile (`profiles/srgb.icc`), not ignored; untagged images are assumed to be sRGB. Center-crop with nearest-neighbor (Point) filtering. Because JPEG reduction happens in the decoder, the result is a representative thumbnail, not a full-resolution pixel minimum. The built-in decoder, the external decoder build, the bundled profile and the model name all feed the cache fingerprint.
4. Sample an 8×8 grid at integer coordinates `x=(2i+1)*160/16`, `y=(2j+1)*90/16`, for `i,j=0..7`, in row-major order. This is a representative stress sample, not an exhaustive pixel minimum or a percentile estimate.
5. Composite the lock input background directly over each sample. For the launcher, composite its scrim over the sample, then its card, then its selected fill. Measure the matching text against those modeled surfaces.
6. Use the minimum contrast across all samples of all supported backgrounds, with one fixed budget per rule. If any expected input is incomplete, the dependent wallpaper rules are unassessed. A corrupt file is charged to validity, not deducted again here.
7. Core readability reuses the same samples for translucent lock text over the wallpaper. When core readability already charges a lock or reference-text deficit, the wallpaper evidence is kept without a second deduction. Launcher over wallpaper is a labeled stress scenario, not a measurement of the live launcher over applications.

The model is unblurred and fixed-size on purpose. It does not reproduce the lock's blur, wallpaper placement on each monitor, the launcher over applications, or display color management. Every report lists these assumptions. Scoring uses no monitor discovery or screenshots.

## Application-background stress tests

Model `application-gray17-v4` uses 17 evenly spaced encoded-sRGB grays from black to white. Translucent text on the bar, popups, menus, launcher, notifications, polkit dialog and tooltips, and the shared selected and focus fills, are composited over each gray. Launcher text also includes its scrim and selected fill. These are fixed reference scenarios, not captured application windows, and not a guarantee for every color, gradient or geometry.

Fill checks use the worst sampled OKLab separation. A separation of at least 0.02 passes. A weaker fill is a half-budget caution, never a full failure, because labels, icons or borders the analyzer cannot see may also mark the state.

Visible popup borders use the worst sampled contrast between each border color (solid or gradient) and each popup interior. Aliases resolve with cycle detection. Gradients accept 2–8 supported color stops and an optional finite angle in degrees, and 17 colors are sampled per adjacent pair of stops. This is not exhaustive spatial or rendered-pixel verification. Border stops are parsed as RGB, RGBA or hex the way Omarchy parses them. Each stop keeps its alpha, which is multiplied by the surface's `border-alpha` with Omarchy's alpha-byte rounding, and stops are interpolated as premultiplied encoded-sRGB components. Unsupported border tokens or geometry get their own diagnosis instead of being blamed on popup transparency. Malformed or unsupported values are reported as limitations. The panel labels these modeled categories “Modeled” unless they need attention.

## Video sampling

MP4, M4V, MOV, WebM, MKV and AVI are recognized as wallpaper media. Each video is streamed into a private read-only temporary file while its full SHA-256 is computed; media bytes never enter the report or the panel. Duplicate files share one copy and one decode.

`ffprobe` checks the first video stream's dimensions, duration, frame rate, pixel format and color metadata. Supported video is opaque 8-bit SDR RGB or YUV, either untagged or tagged BT.709 or sRGB. HDR, wide-gamut or color-managed video, video with alpha, unknown duration and unsupported codecs are unassessed. Duration is limited to 600 seconds and frame rate to 240 fps. Video the model can't handle is never a theme failure.

Up to five frames are requested at 0%, 25%, 50%, 75% and 95% of the duration, clamped to the last frame interval. Very short videos skip duplicate positions. Seek times are requests, not exact presentation timestamps. Each frame is center-cropped to 160×90 by FFmpeg with nearest-neighbor scaling and sampled on the same 8×8 grid. FFmpeg applies rotation metadata. Decoded SDR RGB is treated as encoded sRGB, not as color-managed monitor output. The minimum across sampled frames and other complete media drives the wallpaper rules; frames add no budget of their own.

Each probe and frame subprocess gets one decode and filter thread, a 2-second wall deadline, an 8-second CPU limit, a 2 GiB address space, a 256 MiB single-allocation ceiling, and bounded stdout and stderr. Frame output is exactly 43,200 bytes. No new frames are scheduled for a video after 10 seconds. Demuxers are restricted to the recognized local containers, only the file and pipe protocols are allowed, MOV external data references are disabled, and audio, subtitle and data streams are discarded. Missing tools or codecs, exhausted limits, unsupported metadata and missing frames produce limitations. A container or frame that is definitely malformed is charged only to background validity.

The FFmpeg and ffprobe build and version feed the fingerprint. Reports include seek positions, sample counts, the source SHA-256, decoder metadata and the representative-sampling assumption. Five frames do not prove every frame, transition, playback setting, lock scrim or blur, or monitor transform safe.

## Bounds and operational states

Each text input is capped at 1 MiB. The in-memory snapshot is capped at 128 MiB, including the selected background and the runtime files the analyzer checks. File reads reject anything that is not a regular file and open files non-blocking, without following symlinks.

Background discovery does not recurse. Dotfiles, subdirectories (such as stock `intros/`) and files that are neither a known media extension nor recognizable image or video content (such as `README.md`) are skipped and listed as `ignored` in the `validity.backgrounds` evidence. Up to 32 wallpapers are sampled in name order; extra files are limitations. Theme files must resolve inside the theme folder. A `colors.toml` or `shell.toml` that points outside it stops the audit. A wallpaper that points outside it, or a `backgrounds/` folder that resolves elsewhere, is never read and becomes a limitation. The currently selected background is allowed as a separate input.

Disk copies of large images and videos are capped at 256 MiB per input and 1 GiB in total per audit, with a 10-second deadline for copying. The analyzer removes its copies when it finishes. Other unsupported formats are recognized from a bounded header and extension check and are unassessed; they are described by filename, size and modification time instead of a content hash. Small PNG and JPEG files are held as bounded read-only byte snapshots; larger ones use the shared disk copies and deduplication.

Each unique background is decoded once per audit. No new image decodes start after 25 seconds. A current-theme audit waits at most 35 seconds for the cache lock. If the theme changes while it is being read or published, the analyzer retries at most three times, waiting 150 ms and then 450 ms so a theme switch can finish.

Media copies and decoder scratch files live in a per-process `run-<pid>` folder inside the cache. It is removed on normal exit and on SIGTERM, SIGINT and SIGHUP (closing the panel stops the analyzer). Decoder subprocesses are tied to the analyzer with `PR_SET_PDEATHSIG`. Each audit removes `run-<pid>` folders whose process no longer exists, and top-level `.tmp*` files older than two minutes left by older versions.

Resource limits, a theme that keeps changing during the audit, unavailable decoding and unverified runtime behavior are reported as limitations or errors, never as theme deductions. The change detection is best-effort: it cannot guarantee a consistent read of a folder that is being rewritten, or a perfect match with what the compositor shows at that moment. No audit executes theme content.

## Determinism and fingerprint

The SHA-256 fingerprint (manifest v4) covers the analyzer, ruleset and model identifiers, the profile ID, the sorted input roles with length-prefixed bytes, full-content hashes and container identities for disk media, the image and video decoder builds, the bundled ICC profile hash, descriptors for unsupported inputs, and markers for missing roles. Current audits also capture the relevant Omarchy runtime sources (color, style, template, geometry and palette fallbacks) and your own `shell.toml` overrides. The selected and background roles, in order, and their image bytes are included even when duplicate content is decoded only once.

Absolute paths, watched paths, the audit time, the installed package version and panel state are not part of the scored data. The current-theme cache also checks the theme's path and name.

A separate metadata manifest covers every input: length, modification and change times, inode, device and resolved link target, plus the listed and ignored background names and the decoder executables' metadata. It is taken before any content is read and compared again after capture and after analysis, which detects changes without re-reading media. `current.manifest` stores it next to the published report's fingerprint. When both match, a current audit reuses the cache without reading theme content at all. Any metadata change triggers a full capture and fingerprint comparison, and `--force` always re-audits. Golden tests compare exact scored results where possible, with a 1e-9 f64 tolerance for cross-platform values. Display rounding never changes a decision.

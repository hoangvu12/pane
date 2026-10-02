# Maintained GPUI CE dependency

Pane uses [hoangvu12/gpui-ce](https://github.com/hoangvu12/gpui-ce), branch
`pane/source-over-alpha`, to carry the Windows alpha correction required by
[#63](https://github.com/hoangvu12/pane/issues/63), under
[#61](https://github.com/hoangvu12/pane/issues/61). Cargo uses an immutable commit,
not the branch tip: `2b9e644e3f89a38eacebc713fb0d1807c618c76c`. All four declarations in `crates/pane/Cargo.toml` (including
the test dependency) move together. The fork's internal path dependencies resolve
to that same Git source, preserving one GPUI type identity across renderer,
platforms and editable controls. `Cargo.lock` records the full closure, and
`deny.toml` permits the maintained source. No vendored platform crate or local
path override is required.

## Provenance and scope

The base is upstream GPUI CE `17d9c8e8fdb30a329d817ca06bff424e8e848f1a`.
The source reference is Zui `ec16c62b83caa58b61cd8039db4b5721bacf948f`, specifically
[its DirectX blend functions](https://github.com/hoangvu12/zui/blob/ec16c62b83caa58b61cd8039db4b5721bacf948f/crates/gpui_windows/src/directx_renderer.rs).
Zui is a separate Zed GPUI lineage, not the dependency used by Pane. Both Windows
renderer packages are Apache-2.0; their license and attribution files are retained.
The fork adds its patch rationale and maintenance instructions in `PANE-FORK.md`.

Only ordinary scene blending and path-sprite destination alpha change from
additive to source-over (`As + Ad * (1 - As)`). RGB behavior is unchanged.
Path rasterization already uses source-over and subpixel text intentionally does
not write alpha; neither needs a patch. CE already has in-window blur support;
no additional Zui blur engine is imported. macOS still selects the old Selection
material; its material change and native verification belong to later work.
No Linux compositor integration is added.

This renderer correction alone does not establish useful native glass. The
prototype also removed outer panel shadows that obscured the desktop. Its branch,
working tree and copied Windows dependency are preserved independently by #62;
this ticket neither rewrites nor removes them.

## Reproduce

From Pane, with the repository's Rust toolchain and Windows native prerequisites:

```powershell
cargo fetch --locked
cargo build --locked -j1 -p pane
cargo xtask guests
cargo test --locked -j1 -p pane --test window --test command_search
```

For renderer regression, clone the maintained fork separately and check out the
exact revision in Pane's manifest. From that checkout:

```powershell
$env:GPUI_ALPHA_OUTPUT_DIR = "$PWD/target/alpha-output"
cargo +1.98.1 test --locked -j1 -p gpui_ce_windows --features test-support --lib translucent_ -- --nocapture
cargo +1.98.1 test --locked -j1 -p gpui_ce_windows --features test-support --lib
```

The fixture uses a hidden HWND and real Direct3D rendering and texture readback.
It never presents, changes focus or captures the desktop. It tests transparent
clear pixels, each single layer, and red/blue overlap through both ordinary quad
and path-sprite rendering. Two 50% layers should yield approximately premultiplied
RGBA `[64, 0, 128, 191]`, leaving 25% of the backdrop visible. Optional PNGs retain
actual GPU RGBA and side-by-side black/green composite previews; the previews
illustrate alpha and do not simulate or certify native blur.

To prove sensitivity, in an isolated fork checkout restore the two destination
alpha values to `ONE`, leaving tests in place: both should fail with alpha 254/255 (8-bit rounding).
Restore only the ordinary blend fix: the quad case should pass and the path case
should still fail. Restore both and run the whole renderer suite.

## Update or retire

Review a new CE revision and the fork diff; port only fixes still needed. Run the
renderer positive/negative controls, launcher suites and native Windows launch.
Publish a new immutable fork commit, update all renderer/platform/control pins
and regenerate the lockfile without unrelated dependency upgrades. Check
`cargo metadata --locked` for one `gpui-ce` package and one CE source revision.
Keep platform packages in the same closure even when testing only Windows.

When upstream contains the correction and output regression, retire the fork by
moving all pins together to that verified upstream revision and updating the
source allow list. Do not repair a dependency by editing Cargo's cache.
Native macOS/Linux results remain separate work; compiling their source or
passing GPUI's test platform does not establish native runtime behavior.

## Recorded validation

[The renderer evidence record](research/gpui-alpha/README.md) retains native GPU
readbacks, positive/negative test output and SHA-256 artifact hashes. Cargo fetched
the published fork; metadata resolves 19 CE packages to that single source and one
`gpui-ce` identity. Lockfile review confirms that only those 19 source entries
changed, with no package version or dependency-list changes.

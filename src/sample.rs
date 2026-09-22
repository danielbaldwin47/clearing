//! PROTOTYPE ONLY (wayfinder ticket #3). Not product code.
//!
//! `--sample` bypasses the scanner and builds the concept mock-up's sample tree
//! (`HOME` in scratchpad/concept/scenes.py) in memory, so both variants of the
//! main screen are judged on identical data. Sizes are the mock-up's GiB figures
//! converted to bytes; file and folder counts are invented.
//!
//! The mock-up's pre-aggregated rows ("9 smaller items") are kept as folders of
//! that name so the root stays at nine entries, exactly as drawn. `.cache` is
//! the exception: its "31 smaller items" are 31 real, strongly skewed children,
//! so today's Compact view can be seen on realistic data.
use crate::scan::Node;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Regen,
    Media,
    Archive,
    Code,
    Apps,
    Data,
    Mixed,
}
use Kind::*;

pub struct Meta {
    pub kind: Kind,
    /// Set on the mock-up's pre-aggregated "N smaller items" folders.
    pub tail_count: Option<usize>,
}
static META: OnceLock<HashMap<PathBuf, Meta>> = OnceLock::new();
pub fn meta(path: &Path) -> Option<&'static Meta> {
    META.get()?.get(path)
}

/// The paths the mock-up shows as collected, in its order.
pub const COLLECTED: [&str; 3] = [
    "~/Projects/webshop/node_modules",
    "~/.cache/pip",
    "~/Projects/atlas/target",
];
/// The mock-up's selected Tile, nested two levels below the root's first entry.
pub const SELECTED: &str = "~/Projects/atlas/target";

const GIB: f64 = 1_073_741_824.0;

struct Spec {
    name: String,
    gib: f64,
    kind: Kind,
    files: u64,
    dirs: u64,
    is_file: bool,
    tail: Option<usize>,
    children: Vec<Spec>,
}
fn folder(name: &str, kind: Kind, children: Vec<Spec>) -> Spec {
    Spec {
        name: name.into(),
        gib: 0.0,
        kind,
        files: 0,
        dirs: 0,
        is_file: false,
        tail: None,
        children,
    }
}
/// A folder the mock-up never opens: it has a size and counts but no children.
fn leaf(name: &str, gib: f64, kind: Kind, files: u64, dirs: u64) -> Spec {
    Spec {
        gib,
        files,
        dirs,
        ..folder(name, kind, vec![])
    }
}
fn file(name: &str, gib: f64, kind: Kind) -> Spec {
    Spec {
        gib,
        files: 1,
        is_file: true,
        ..folder(name, kind, vec![])
    }
}
/// The mock-up's "N smaller items" row, kept as one folder holding N invented,
/// geometrically shrinking children.
fn tail(
    n: usize,
    gib: f64,
    kind: Kind,
    files: u64,
    dirs: u64,
    ratio: f64,
    as_files: bool,
    names: &[&str],
) -> Spec {
    let weights: Vec<f64> = (0..n).map(|i| ratio.powi(i as i32)).collect();
    let sum: f64 = weights.iter().sum();
    let children = weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let name = match names.get(i) {
                Some(name) => (*name).to_string(),
                None => format!(
                    "{}-{:03}.{}",
                    names[i % names.len()].split('.').next().unwrap_or("item"),
                    i + 1,
                    names[i % names.len()].rsplit('.').next().unwrap_or("bin")
                ),
            };
            let share = w / sum;
            if as_files {
                file(&name, gib * share, kind)
            } else {
                leaf(
                    &name,
                    gib * share,
                    kind,
                    ((files as f64 * share) as u64).max(1),
                    ((dirs as f64 * share) as u64).max(1),
                )
            }
        })
        .collect();
    Spec {
        tail: Some(n),
        ..folder(&format!("{n} smaller items"), kind, children)
    }
}

/// Round 6: a folder whose invented contents sum to `gib`, so deep items exist
/// without changing any size the earlier rounds were judged on. Each entry is
/// (name, share of `gib`, is_file); a share of 0.0 takes whatever is left.
fn deep(name: &str, gib: f64, kind: Kind, files: u64, dirs: u64, parts: Vec<(&str, f64, bool)>) -> Spec {
    let named: f64 = parts.iter().map(|p| p.1).sum();
    let blanks = parts.iter().filter(|p| p.1 == 0.0).count().max(1) as f64;
    let children = parts
        .into_iter()
        .map(|(n, share, is_file)| {
            let share = if share == 0.0 { (1.0 - named) / blanks } else { share };
            if is_file {
                file(n, gib * share, kind)
            } else {
                leaf(
                    n,
                    gib * share,
                    kind,
                    ((files as f64 * share) as u64).max(1),
                    ((dirs as f64 * share) as u64).max(1),
                )
            }
        })
        .collect();
    folder(name, kind, children)
}
/// `deep` with one level more: `sub` replaces the named child with its own parts.
fn deeper(mut spec: Spec, sub: Spec) -> Spec {
    if let Some(c) = spec.children.iter_mut().find(|c| c.name == sub.name) {
        let gib = c.gib;
        let total: f64 = leaf_gib(&sub);
        let scale = gib / total;
        *c = scale_spec(sub, scale);
    }
    spec
}
fn leaf_gib(s: &Spec) -> f64 {
    if s.children.is_empty() { s.gib } else { s.children.iter().map(leaf_gib).sum() }
}
fn scale_spec(mut s: Spec, k: f64) -> Spec {
    s.gib *= k;
    s.children = s.children.into_iter().map(|c| scale_spec(c, k)).collect();
    s
}

fn home() -> Spec {
    // `.cache`: the six folders the mock-up names, then 31 real smaller ones
    // summing to the mock-up's 8.8 GiB, from gigabytes down to a few KiB.
    const SMALL_CACHES: [(&str, f64); 28] = [
        ("thumbnails", 1434.0),
        ("google-chrome", 1126.0),
        ("spotify", 870.0),
        ("mesa_shader_cache", 635.0),
        ("yarn", 492.0),
        ("pnpm", 420.0),
        ("ms-playwright", 369.0),
        ("typescript", 297.0),
        ("pre-commit", 246.0),
        ("vscode-cpptools", 195.0),
        ("bazel", 154.0),
        ("deno", 123.0),
        ("fontconfig", 92.0),
        ("nvidia", 72.0),
        ("tracker3", 56.0),
        ("gstreamer-1.0", 41.0),
        ("ibus", 31.0),
        ("flatpak", 26.0),
        ("pylint", 20.0),
        ("black", 15.0),
        ("mypy", 12.0),
        ("zoom", 10.0),
        ("babl", 8.0),
        ("gegl-0.4", 6.0),
        ("matplotlib", 5.0),
        ("nim", 4.0),
        ("rclone", 3.0),
        ("wal", 2.0),
    ];
    let mut cache = vec![
        deep("mozilla", 9.8, Regen, 96_000, 5_100, vec![("firefox", 1.0, false)]),
        leaf("pip", 7.7, Regen, 31_000, 9_800),
        leaf("yay", 6.1, Regen, 18_500, 2_300),
        leaf("JetBrains", 5.9, Regen, 22_000, 3_900),
        deeper(
            deep("huggingface", 5.2, Regen, 1_900, 410, vec![("hub", 1.0, false)]),
            deep("hub", 5.2, Regen, 1_900, 410, vec![("models--meta-llama--Llama-3-8B", 0.62, false), ("models--openai--whisper-large", 0.28, false), ("datasets--squad", 0.0, false)]),
        ),
        leaf("go-build", 4.4, Regen, 61_000, 257),
    ];
    let tiny = [
        ("event-sound-cache.tdb.x86_64", 12.0 / 1024.0),
        ("motd.legal-displayed", 4.0 / 1024.0),
    ];
    let fixed: f64 = SMALL_CACHES.iter().map(|(_, mib)| mib).sum::<f64>()
        + tiny.iter().map(|(_, mib)| mib).sum::<f64>();
    let electron = 8.8 * 1024.0 - fixed;
    cache.push(leaf("electron", electron / 1024.0, Regen, 21_400, 1_260));
    for (name, mib) in SMALL_CACHES {
        cache.push(leaf(
            name,
            mib / 1024.0,
            Regen,
            (mib * 11.0) as u64 + 3,
            (mib / 3.0) as u64 + 1,
        ));
    }
    for (name, mib) in tiny {
        cache.push(file(name, mib / 1024.0, Regen));
    }

    folder(
        "~",
        Mixed,
        vec![
            folder(
                "Projects",
                Regen,
                vec![
                    folder(
                        "atlas",
                        Regen,
                        vec![
                            deeper(
                                deep("target", 41.8, Regen, 184_000, 9_200, vec![("debug", 0.70, false), ("release", 0.0, false)]),
                                deep("debug", 29.26, Regen, 150_000, 7_000, vec![("deps", 0.63, false), ("incremental", 0.27, false), ("build", 0.0, false)]),
                            ),
                            leaf(".git", 6.2, Code, 41_000, 310),
                            leaf("assets", 3.0, Media, 1_240, 38),
                            leaf("src", 1.1, Code, 2_900, 210),
                        ],
                    ),
                    folder(
                        "webshop",
                        Regen,
                        vec![
                            deep("node_modules", 18.9, Regen, 512_000, 61_000, vec![("@next", 0.31, false), ("@swc", 0.20, false), ("esbuild", 0.13, false), ("sharp", 0.07, false), ("typescript", 0.06, false), ("@img", 0.05, false), ("playwright-core", 0.04, false), ("lodash", 0.0, false), ("react-dom", 0.0, false), ("webpack", 0.0, false), ("@babel", 0.0, false), ("caniuse-lite", 0.0, false)]),
                            leaf(".next", 7.4, Regen, 21_500, 1_900),
                            leaf("media", 4.1, Media, 860, 24),
                            leaf("src", 1.2, Code, 3_400, 420),
                        ],
                    ),
                    folder(
                        "ml-notebooks",
                        Data,
                        vec![
                            deep("data", 14.0, Data, 5_200, 64, vec![("imagenet-subset.tar", 0.44, true), ("processed", 0.35, false), ("raw", 0.0, false)]),
                            leaf(".venv", 6.9, Regen, 78_000, 7_900),
                            leaf("runs", 1.4, Data, 9_600, 480),
                        ],
                    ),
                    tail(
                        9,
                        12.4,
                        Code,
                        96_000,
                        18_000,
                        0.72,
                        false,
                        &[
                            "dotfiles",
                            "blog",
                            "advent-of-code",
                            "homelab",
                            "talks",
                            "cli-tools",
                            "playground",
                            "resume",
                            "scratch",
                        ],
                    ),
                ],
            ),
            folder(
                ".local",
                Apps,
                vec![
                    folder(
                        "Steam",
                        Apps,
                        vec![
                            deeper(
                                deep("Baldurs Gate 3", 38.6, Apps, 12_400, 1_150, vec![("Data", 0.935, false), ("bin", 0.0, false)]),
                                deep("Data", 36.09, Apps, 11_000, 900, vec![("Gustav.pak", 0.43, true), ("Textures.pak", 0.27, true), ("VirtualTextures.pak", 0.17, true), ("Models.pak", 0.0, true), ("Localization", 0.0, false)]),
                            ),
                            deep("Factorio", 12.2, Apps, 8_900, 720, vec![("data", 0.78, false), ("saves", 0.15, false), ("bin", 0.0, false)]),
                            leaf("shadercache", 10.4, Regen, 64_000, 96),
                        ],
                    ),
                    deeper(
                        deep("containers", 19.5, Data, 410_000, 52_000, vec![("storage", 1.0, false)]),
                        deeper(
                            deep("storage", 19.5, Data, 410_000, 52_000, vec![("overlay", 0.91, false), ("volumes", 0.0, false)]),
                            deep("overlay", 17.75, Data, 380_000, 48_000, vec![("3f9c1e7a0b52", 0.38, false), ("a81d0c44e6f9", 0.24, false), ("7be2f90d13c8", 0.16, false), ("c0ffee51d2aa", 0.09, false), ("e4d5a6b7c8d9", 0.0, false), ("51a2b3c4d5e6", 0.0, false), ("0a9b8c7d6e5f", 0.0, false)]),
                        ),
                    ),
                    tail(
                        12,
                        5.3,
                        Mixed,
                        88_000,
                        12_800,
                        0.7,
                        false,
                        &[
                            "pipx",
                            "lib",
                            "bin",
                            "state",
                            "opt",
                            "go",
                            "fonts",
                            "icons",
                            "applications",
                            "include",
                            "man",
                            "etc",
                        ],
                    ),
                ],
            ),
            folder(
                "Videos",
                Media,
                vec![
                    folder(
                        "raw-footage",
                        Media,
                        vec![
                            file("2024-trip.mov", 22.1, Media),
                            file("interview-a.mov", 14.2, Media),
                            deep("b-roll", 11.7, Media, 214, 6, vec![("harbour-wide.mov", 0.34, true), ("market-2.mov", 0.22, true), ("drone-cliffs.mov", 0.18, true), ("street-night.mov", 0.0, true), ("rain-window.mov", 0.0, true), ("crowd.mov", 0.0, true)]),
                        ],
                    ),
                    deep("exports", 12.9, Media, 86, 4, vec![("trip-final-4k.mp4", 0.46, true), ("trip-final-1080.mp4", 0.19, true), ("interview-cut.mp4", 0.15, true), ("teaser.mp4", 0.0, true), ("stills", 0.0, false)]),
                    tail(
                        5,
                        3.8,
                        Media,
                        5,
                        0,
                        0.6,
                        true,
                        &[
                            "screen-recording-03.mkv",
                            "talk-rehearsal.mp4",
                            "drone-test.mov",
                            "intro-v2.mp4",
                            "timelapse.mp4",
                        ],
                    ),
                ],
            ),
            folder(".cache", Regen, cache),
            folder(
                "Downloads",
                Archive,
                vec![
                    file("dataset.tar.zst", 11.8, Archive),
                    file("win11.iso", 6.4, Archive),
                    file("ubuntu-26.04.iso", 5.9, Archive),
                    tail(
                        212,
                        9.4,
                        Mixed,
                        212,
                        0,
                        0.975,
                        true,
                        &[
                            "installer.AppImage",
                            "backup.zip",
                            "lecture.mp4",
                            "scan.pdf",
                            "photos.tar",
                            "driver.deb",
                            "export.csv",
                            "invoice.pdf",
                        ],
                    ),
                ],
            ),
            folder("VMs", Archive, vec![file("win11.qcow2", 28.0, Archive)]),
            deep("Documents", 5.6, Code, 14_800, 1_250, vec![("taxes", 0.34, false), ("thesis", 0.27, false), ("scans", 0.21, false), ("notes", 0.0, false), ("letters", 0.0, false)]),
            deep("Pictures", 4.2, Media, 9_700, 310, vec![("2025", 0.46, false), ("2024", 0.31, false), ("Screenshots", 0.0, false), ("wallpapers", 0.0, false)]),
            tail(
                14,
                0.9,
                Mixed,
                42_000,
                18_750,
                0.75,
                false,
                &[
                    ".config",
                    ".mozilla",
                    ".var",
                    ".npm",
                    ".cargo",
                    ".rustup",
                    "Music",
                    "Desktop",
                    ".themes",
                    ".icons",
                    ".ssh",
                    ".gnupg",
                    "Templates",
                    "Public",
                ],
            ),
        ],
    )
}

fn build(
    spec: Spec,
    parent: Option<&Path>,
    meta: &mut HashMap<PathBuf, Meta>,
    next: &mut u64,
) -> Node {
    let path = match parent {
        Some(parent) => parent.join(&spec.name),
        None => PathBuf::from(&spec.name),
    };
    *next += 1;
    let identity = (1, *next);
    meta.insert(
        path.clone(),
        Meta {
            kind: spec.kind,
            tail_count: spec.tail,
        },
    );
    let mut children: Vec<Node> = spec
        .children
        .into_iter()
        .map(|c| build(c, Some(&path), meta, next))
        .collect();
    // The scanner's order: largest first, then by name.
    children.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
    let (bytes, files, directories) = if children.is_empty() {
        (
            ((spec.gib * GIB / 4096.0).round() as u64).max(1) * 4096,
            spec.files,
            if spec.is_file { 0 } else { spec.dirs + 1 },
        )
    } else {
        (
            children.iter().map(|c| c.bytes).sum(),
            children.iter().map(|c| c.files).sum(),
            children.iter().map(|c| c.directories).sum::<u64>() + 1,
        )
    };
    Node {
        name: spec.name,
        path,
        bytes,
        apparent: bytes,
        files,
        directories,
        errors: 0,
        is_dir: !spec.is_file,
        is_symlink: false,
        shared: false,
        identity,
        children,
    }
}

pub fn tree() -> Node {
    let mut meta = HashMap::new();
    let mut next = 0;
    let root = build(home(), None, &mut meta, &mut next);
    let _ = META.set(meta);
    root
}

/// The child-index route from `root` to `path`, if every component is present.
pub fn route_to(root: &Node, path: &str) -> Option<Vec<usize>> {
    let relative = Path::new(path).strip_prefix(&root.path).ok()?;
    let mut node = root;
    let mut route = Vec::new();
    for name in relative.components() {
        let at = node
            .children
            .iter()
            .position(|c| c.path.file_name() == Some(name.as_os_str()))?;
        route.push(at);
        node = &node.children[at];
    }
    Some(route)
}

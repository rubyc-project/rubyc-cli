//! Installed-library artifact matrix (local-safe: file reads only, no
//! execution, no `dlopen`).
//!
//! Every library in the install dir must be structurally sound: each
//! current-version `.rci` decodes, each `.so` carries the ELF magic and
//! the RubyC preamble, and every decoded lib is covered by
//! `libraries/index.tsv` (the `using`-discovery source). sollibs
//! installed without a sidecar (older backfills) only get the image
//! checks; version-stale sidecars report without failing (the consume
//! path falls back to sources for those) — the `.rci` gap closes as
//! backfill re-emits them, it never fails this matrix.

use std::collections::BTreeSet;

#[test]
fn installed_library_artifacts_decode_and_index() {
    let libraries = rubyc::extensions::install_root().join("libraries");
    if !libraries.is_dir() {
        return;
    }
    let mut rcis: Vec<std::path::PathBuf> = Vec::new();
    let mut sos: Vec<std::path::PathBuf> = Vec::new();
    let entries = std::fs::read_dir(&libraries).expect("list install dir");
    for entry in entries.flatten() {
        let path = entry.path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("rci") => rcis.push(path),
            Some("so") => sos.push(path),
            _ => {}
        }
    }
    if rcis.is_empty() && sos.is_empty() {
        return;
    }
    // Fragments decode with the expected lib name. Version-mismatched
    // sidecars (older toolchain) are stale, not corrupt: the consume
    // path falls back to sources for those, so the matrix reports
    // them without failing (re-emit via backfill closes the gap).
    let mut decoded_libs = BTreeSet::new();
    let mut stale = Vec::new();
    for path in &rcis {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        match rubyc::native::rci::decode(&bytes) {
            Ok(frag) => {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                assert_eq!(frag.lib_name, stem, "fragment name matches file {}", path.display());
                decoded_libs.insert(frag.lib_name);
            }
            Err(e) if e.contains("version") => {
                stale.push(path.display().to_string());
            }
            Err(e) => panic!("decode {}: {e}", path.display()),
        }
    }
    if !stale.is_empty() {
        eprintln!("[matrix] {} stale sidecar(s), re-emit via backfill:", stale.len());
        for s in &stale {
            eprintln!("[matrix]   stale: {s}");
        }
    }
    // Images carry ELF magic plus the shared-library preamble
    // (jump-over-magic at 64, dog magic at 66 — same pins as interop).
    for path in &sos {
        let mut head = [0u8; 74];
        let mut file = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
        use std::io::Read as _;
        file.read_exact(&mut head).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(&head[0..4], b"\x7fELF", "ELF magic in {}", path.display());
        assert_eq!(&head[64..66], &[0xEB, 0x08], "preamble jump in {}", path.display());
        assert_eq!(
            &head[66..74],
            &rubyc::native::container::MAGIC,
            "dog magic in {}",
            path.display()
        );
    }
    // Index coverage: every decoded lib owns at least one row.
    let index = rubyc::native::index::ensure(&libraries).expect("index loads");
    let _ = index;
    let text = std::fs::read_to_string(libraries.join(rubyc::native::index::INDEX_FILE))
        .expect("index.tsv exists after ensure");
    let mut indexed_libs = BTreeSet::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        if let (Some(_), Some(_), Some(lib)) = (cols.next(), cols.next(), cols.next()) {
            indexed_libs.insert(lib.to_owned());
        }
    }
    for lib in &decoded_libs {
        assert!(
            indexed_libs.contains(lib),
            "index.tsv covers {lib} ({} rows total)",
            indexed_libs.len()
        );
    }
}

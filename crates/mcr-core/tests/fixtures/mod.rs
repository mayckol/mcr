//! Shared fixtures: each is (local, ancestor, incoming).

/// One non-conflicting change per side plus one true conflict.
pub fn mixed() -> (&'static str, &'static str, &'static str) {
    let ancestor = "alpha\nbeta\ngamma\ndelta\nepsilon";
    let local = "alpha-LOCAL\nbeta\ngamma\ndelta\nepsilon"; // local-only change line 1
    let incoming = "alpha\nbeta\ngamma\ndelta-INCOMING\nepsilon"; // incoming-only change line 4
    (local, ancestor, incoming)
}

/// Both sides change the same line differently -> conflict.
pub fn conflict() -> (&'static str, &'static str, &'static str) {
    let ancestor = "one\ntwo\nthree";
    let local = "one\ntwo-LEFT\nthree";
    let incoming = "one\ntwo-RIGHT\nthree";
    (local, ancestor, incoming)
}

/// One side deletes lines the other modifies.
pub fn delete_vs_modify() -> (&'static str, &'static str, &'static str) {
    let ancestor = "a\nb\nc\nd";
    let local = "a\nd"; // local deletes b, c
    let incoming = "a\nb-MOD\nc\nd"; // incoming modifies b
    (local, ancestor, incoming)
}

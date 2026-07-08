//! Multi-file merge/compare session store (`manager`) and the git plumbing it
//! rests on (`discovery`). Both are free of any UI framework so they can back the
//! standalone MCR app, its CLI, and an embedding host (fftracking) alike.

pub mod discovery;
pub mod manager;

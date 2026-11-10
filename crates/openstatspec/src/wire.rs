//! These strings are defined by the external OpenStatSpec spec; they change only when it renames them.

/// The .sav/.zsav profile: `dataset.spec_version` and its manifest.
pub const SAV_PROFILE: &str = "spss-sav-zsav-1.0";
pub const SAV_MANIFEST: &str = "spss-sav-zsav-1.0.json";

/// Request contracts of the syntax frontend profiles 0.1, 0.2 and 0.3, oldest first.
pub const FRONTEND_CONTRACTS: [&str; 3] = [
    "openstatspec-spss-syntax-frontend-v0.1",
    "openstatspec-spss-syntax-frontend-v0.2",
    "openstatspec-spss-syntax-frontend-v0.3",
];

/// Manifests of the syntax frontend profiles 0.1, 0.2 and 0.3, oldest first.
pub const FRONTEND_MANIFESTS: [&str; 3] = [
    "spss-syntax-frontend-0.1.json",
    "spss-syntax-frontend-0.2.json",
    "spss-syntax-frontend-0.3.json",
];

/// Diagnostic codes of the syntax frontend profiles.
pub const SYNTAX_ERROR: &str = "spss_syntax_error";
pub const UNSUPPORTED_COMMAND: &str = "unsupported_spss_command";

//! Types for the package manager

use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

/// Source scope
#[derive(Debug, Clone, PartialEq)]
pub enum SourceScope {
    User,
    Project,
    Temporary,
}

/// Origin of a resource
#[derive(Debug, Clone, PartialEq)]
pub enum SourceOrigin {
    Package,
    TopLevel,
}

/// Path metadata for a resolved resource
#[derive(Debug, Clone)]
pub struct PathMetadata {
    pub source: String,
    pub scope: SourceScope,
    pub origin: SourceOrigin,
    pub base_dir: Option<String>,
}

/// A resolved resource with metadata
#[derive(Debug, Clone)]
pub struct ResolvedResource {
    pub path: String,
    pub enabled: bool,
    pub metadata: PathMetadata,
}

/// All resolved paths by resource type
#[derive(Debug, Clone, Default)]
pub struct ResolvedPaths {
    pub extensions: Vec<ResolvedResource>,
    pub skills: Vec<ResolvedResource>,
    pub prompts: Vec<ResolvedResource>,
    pub themes: Vec<ResolvedResource>,
}

/// Action when a source is missing
#[derive(Debug, Clone, PartialEq)]
pub enum MissingSourceAction {
    Install,
    Skip,
    Error,
}

/// Progress event during package operations
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub type_: String,  // "start", "progress", "complete", "error"
    pub action: String, // "install", "remove", "update", "clone", "pull"
    pub source: String,
    pub message: Option<String>,
}

/// Progress callback type
pub type ProgressCallback = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

/// Configured package entry
#[derive(Debug, Clone)]
pub struct ConfiguredPackage {
    pub source: String,
    pub scope: String, // "user" or "project"
    pub filtered: bool,
    pub installed_path: Option<String>,
}

/// Package update information
#[derive(Debug, Clone)]
pub struct PackageUpdate {
    pub source: String,
    pub display_name: String,
    pub type_: String, // "npm" or "git"
    pub scope: String, // "user" or "project"
}

// ============================================================================
// Parsed source types
// ============================================================================

#[derive(Debug, Clone)]
pub struct NpmSource {
    pub spec: String,
    pub name: String,
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct GitSource {
    pub repo: String,
    pub host: String,
    pub path_: String,
    pub r#ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LocalSource {
    pub path_: String,
}

#[derive(Debug, Clone)]
pub enum ParsedSource {
    Npm(NpmSource),
    Git(GitSource),
    Local(LocalSource),
}

// ============================================================================
// Resource type constants
// ============================================================================

pub(crate) const RESOURCE_TYPES: [&str; 4] = ["extensions", "skills", "prompts", "themes"];
pub(crate) const NETWORK_TIMEOUT_MS: u64 = 10000;
pub(crate) const UPDATE_CHECK_CONCURRENCY: usize = 4;
pub(crate) const GIT_UPDATE_CONCURRENCY: usize = 4;

// ============================================================================
// Package manifest (pick field in package.json)
// ============================================================================

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub(crate) struct PickManifest {
    #[serde(default)]
    pub(crate) extensions: Vec<String>,
    #[serde(default)]
    pub(crate) skills: Vec<String>,
    #[serde(default)]
    pub(crate) prompts: Vec<String>,
    #[serde(default)]
    pub(crate) themes: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub(crate) struct PackageJson {
    #[serde(default)]
    pub(crate) pick: Option<PickManifest>,
    #[serde(default)]
    pub(crate) version: Option<String>,
}

// ============================================================================
// Resource accumulation
// ============================================================================

#[derive(Debug, Clone)]
pub(crate) struct ResourceEntry {
    pub(crate) metadata: PathMetadata,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ResourceAccumulator {
    pub(crate) extensions: HashMap<String, ResourceEntry>,
    pub(crate) skills: HashMap<String, ResourceEntry>,
    pub(crate) prompts: HashMap<String, ResourceEntry>,
    pub(crate) themes: HashMap<String, ResourceEntry>,
}

// ============================================================================
// Package source type matching SettingsManager's PackageSource
// ============================================================================

/// Package source from settings (string or filtered object)
#[derive(Debug, Clone)]
pub struct PackageSource {
    pub source: String,
    pub filter: Option<PackageFilter>,
}

/// Filter for package resources
#[derive(Debug, Clone, Default)]
pub struct PackageFilter {
    pub extensions: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub prompts: Option<Vec<String>>,
    pub themes: Option<Vec<String>>,
}

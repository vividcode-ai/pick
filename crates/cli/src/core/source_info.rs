//! Source information for resources

/// Scope of a source resource
#[derive(Debug, Clone, PartialEq)]
pub enum SourceScope {
    User,
    Project,
    Temporary,
}

impl SourceScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceScope::User => "user",
            SourceScope::Project => "project",
            SourceScope::Temporary => "temporary",
        }
    }
}

/// Origin of a resource
#[derive(Debug, Clone, PartialEq)]
pub enum SourceOrigin {
    Package,
    TopLevel,
}

impl SourceOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceOrigin::Package => "package",
            SourceOrigin::TopLevel => "top-level",
        }
    }
}

/// Source information for a resolved resource
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub path: String,
    pub source: String,
    pub scope: SourceScope,
    pub origin: SourceOrigin,
    pub base_dir: Option<String>,
}

/// Options for creating synthetic source info
pub struct SyntheticSourceOptions {
    pub source: String,
    pub scope: Option<SourceScope>,
    pub origin: Option<SourceOrigin>,
    pub base_dir: Option<String>,
}

/// Path metadata for a resolved resource
#[derive(Debug, Clone)]
pub struct PathMetadata {
    pub source: String,
    pub scope: SourceScope,
    pub origin: SourceOrigin,
    pub base_dir: Option<String>,
}

pub fn create_source_info(path: &str, metadata: &PathMetadata) -> SourceInfo {
    SourceInfo {
        path: path.to_string(),
        source: metadata.source.clone(),
        scope: metadata.scope.clone(),
        origin: metadata.origin.clone(),
        base_dir: metadata.base_dir.clone(),
    }
}

pub fn create_synthetic_source_info(path: &str, options: SyntheticSourceOptions) -> SourceInfo {
    SourceInfo {
        path: path.to_string(),
        source: options.source.clone(),
        scope: options.scope.unwrap_or(SourceScope::Temporary),
        origin: options.origin.unwrap_or(SourceOrigin::TopLevel),
        base_dir: options.base_dir,
    }
}

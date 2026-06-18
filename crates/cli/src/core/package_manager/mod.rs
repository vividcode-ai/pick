mod resolve;
mod types;

use resolve::*;
use types::*;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::CONFIG_DIR_NAME;
use crate::core::settings::{Settings, SettingsManager};

use tokio::sync::Mutex;
// ============================================================================
// DefaultPackageManager
// ============================================================================

/// The main package manager
pub struct DefaultPackageManager {
    cwd: PathBuf,
    agent_dir: PathBuf,
    settings_manager: Arc<Mutex<SettingsManager>>,
    progress_callback: Option<ProgressCallback>,
}

impl DefaultPackageManager {
    pub fn new(cwd: PathBuf, agent_dir: PathBuf, settings_manager: SettingsManager) -> Self {
        Self {
            cwd,
            agent_dir,
            settings_manager: Arc::new(Mutex::new(settings_manager)),
            progress_callback: None,
        }
    }

    pub fn set_progress_callback(&mut self, callback: Option<ProgressCallback>) {
        self.progress_callback = callback;
    }

    fn emit_progress(&self, event: ProgressEvent) {
        if let Some(ref cb) = self.progress_callback {
            cb(event);
        }
    }

    fn get_base_dir_for_scope(&self, scope: &SourceScope) -> PathBuf {
        match scope {
            SourceScope::Project => self.cwd.join(CONFIG_DIR_NAME),
            SourceScope::User => self.agent_dir.clone(),
            SourceScope::Temporary => self.cwd.clone(),
        }
    }

    fn resolve_path(&self, input: &str) -> PathBuf {
        let path = Path::new(input);
        if path.is_absolute() {
            path.to_path_buf()
        } else if input.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(&input[2..])
            } else {
                self.cwd.join(input)
            }
        } else {
            self.cwd.join(input)
        }
    }

    fn resolve_path_from_base(&self, input: &str, base_dir: &Path) -> PathBuf {
        let path = Path::new(input);
        if path.is_absolute() {
            path.to_path_buf()
        } else if input.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(&input[2..])
            } else {
                base_dir.join(input)
            }
        } else {
            base_dir.join(input)
        }
    }

    fn read_pick_manifest(&self, package_root: &Path) -> Option<PickManifest> {
        let package_json_path = package_root.join("package.json");
        if !package_json_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&package_json_path).ok()?;
        let pkg: PackageJson = serde_json::from_str(&content).ok()?;
        pkg.pick
    }

    fn get_installed_npm_version(&self, installed_path: &Path) -> Option<String> {
        let package_json_path = installed_path.join("package.json");
        if !package_json_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&package_json_path).ok()?;
        let pkg: PackageJson = serde_json::from_str(&content).ok()?;
        pkg.version
    }

    fn get_npm_command(&self) -> (String, Vec<String>) {
        ("npm".to_string(), vec![])
    }

    fn get_npm_install_root(&self, scope: &SourceScope, temporary: bool) -> PathBuf {
        if temporary {
            return std::env::temp_dir().join("pick-extensions").join("npm");
        }
        match scope {
            SourceScope::Project => self.cwd.join(CONFIG_DIR_NAME).join("npm"),
            SourceScope::User => self.agent_dir.join("npm"),
            SourceScope::Temporary => std::env::temp_dir().join("pick-extensions").join("npm"),
        }
    }

    fn get_managed_npm_install_path(&self, source: &NpmSource, scope: &SourceScope) -> PathBuf {
        match scope {
            SourceScope::Temporary => std::env::temp_dir()
                .join("pick-extensions")
                .join("npm")
                .join("node_modules")
                .join(&source.name),
            SourceScope::Project => self
                .cwd
                .join(CONFIG_DIR_NAME)
                .join("npm")
                .join("node_modules")
                .join(&source.name),
            SourceScope::User => self
                .agent_dir
                .join("npm")
                .join("node_modules")
                .join(&source.name),
        }
    }

    fn get_npm_install_path(&self, source: &NpmSource, scope: &SourceScope) -> PathBuf {
        let managed_path = self.get_managed_npm_install_path(source, scope);
        if scope != &SourceScope::User
            || managed_path.exists()
            || !self
                .agent_dir
                .join("npm")
                .join("node_modules")
                .join(&source.name)
                .exists()
        {
            return managed_path;
        }
        // Check legacy global npm root
        self.agent_dir
            .join("npm")
            .join("node_modules")
            .join(&source.name)
    }

    fn get_git_install_path(&self, source: &GitSource, scope: &SourceScope) -> PathBuf {
        match scope {
            SourceScope::Temporary => {
                let hash = sha256_hash(&format!("git-{}-{}", source.host, source.path_));
                std::env::temp_dir()
                    .join("pick-extensions")
                    .join("git")
                    .join(&hash[..8])
                    .join(&source.path_)
            }
            SourceScope::Project => self
                .cwd
                .join(CONFIG_DIR_NAME)
                .join("git")
                .join(&source.host)
                .join(&source.path_),
            SourceScope::User => self
                .agent_dir
                .join("git")
                .join(&source.host)
                .join(&source.path_),
        }
    }

    fn get_package_identity(&self, source: &str, scope: Option<&SourceScope>) -> String {
        let parsed = parse_source(source);
        match parsed {
            ParsedSource::Npm(ref npm) => format!("npm:{}", npm.name),
            ParsedSource::Git(ref git) => format!("git:{}/{}", git.host, git.path_),
            ParsedSource::Local(ref local) => {
                if let Some(s) = scope {
                    let base_dir = self.get_base_dir_for_scope(s);
                    format!(
                        "local:{}",
                        self.resolve_path_from_base(&local.path_, &base_dir)
                            .to_string_lossy()
                    )
                } else {
                    format!(
                        "local:{}",
                        self.resolve_path(&local.path_).to_string_lossy()
                    )
                }
            }
        }
    }

    fn parse_npm_spec_(&self, spec: &str) -> (String, Option<String>) {
        parse_npm_spec(spec)
    }

    pub async fn add_source_to_settings(&self, source: &str, local: bool) -> bool {
        let mut settings = self.settings_manager.lock().await;
        let scope = if local {
            SourceScope::Project
        } else {
            SourceScope::User
        };
        let identity = self.get_package_identity(source, Some(&scope));

        let target = if local {
            settings.get_project()
        } else {
            settings.get_global()
        };
        let mut extensions = target.extensions.clone().unwrap_or_default();
        if extensions.contains(&identity) {
            return true;
        }
        extensions.push(identity);

        let s = Settings {
            extensions: Some(extensions),
            ..Default::default()
        };

        let result = if local {
            settings.set_project(s)
        } else {
            settings.set_global(s)
        };
        result.is_ok()
    }

    pub async fn remove_source_from_settings(&self, source: &str, local: bool) -> bool {
        let mut settings = self.settings_manager.lock().await;
        let scope = if local {
            SourceScope::Project
        } else {
            SourceScope::User
        };
        let identity = self.get_package_identity(source, Some(&scope));

        let target = if local {
            settings.get_project()
        } else {
            settings.get_global()
        };
        let mut extensions = target.extensions.clone().unwrap_or_default();
        let before = extensions.len();
        extensions.retain(|e| {
            let e_identity = self.get_package_identity(e, Some(&scope));
            e_identity != identity
        });
        if extensions.len() == before {
            return false;
        }

        let s = Settings {
            extensions: Some(extensions),
            ..Default::default()
        };

        let result = if local {
            settings.set_project(s)
        } else {
            settings.set_global(s)
        };
        result.is_ok()
    }

    pub async fn list_configured_packages(&self) -> Vec<ConfiguredPackage> {
        let settings = self.settings_manager.lock().await;
        let mut packages = Vec::new();

        // Read from global settings
        let global = settings.get_global();
        if let Some(extensions) = &global.extensions {
            for source in extensions {
                let parsed = parse_source(source);
                let installed_path =
                    self.get_installed_path_for_source(&parsed, &SourceScope::User);
                packages.push(ConfiguredPackage {
                    source: source.clone(),
                    scope: "user".to_string(),
                    filtered: false,
                    installed_path,
                });
            }
        }

        // Read from project settings
        let project = settings.get_project();
        if let Some(extensions) = &project.extensions {
            for source in extensions {
                let parsed = parse_source(source);
                let installed_path =
                    self.get_installed_path_for_source(&parsed, &SourceScope::Project);
                packages.push(ConfiguredPackage {
                    source: source.clone(),
                    scope: "project".to_string(),
                    filtered: false,
                    installed_path,
                });
            }
        }

        packages
    }

    pub async fn check_for_available_updates(&self) -> Vec<PackageUpdate> {
        let packages = self.list_configured_packages().await;
        let mut updates = Vec::new();

        for pkg in packages {
            let parsed = parse_source(&pkg.source);
            match parsed {
                ParsedSource::Npm(ref npm) => {
                    if let Some(ref installed_path) = pkg.installed_path {
                        if let Some(current_version) =
                            self.get_installed_npm_version(Path::new(installed_path))
                        {
                            let url = format!("https://registry.npmjs.org/{}", npm.name);
                            if let Ok(response) = reqwest::get(&url).await {
                                if let Ok(body) = response.text().await {
                                    if let Ok(data) =
                                        serde_json::from_str::<serde_json::Value>(&body)
                                    {
                                        if let Some(latest) = data
                                            .get("dist-tags")
                                            .and_then(|t| t.get("latest"))
                                            .and_then(|v| v.as_str())
                                        {
                                            if latest != &current_version {
                                                updates.push(PackageUpdate {
                                                    source: pkg.source,
                                                    display_name: npm.name.clone(),
                                                    type_: "npm".to_string(),
                                                    scope: pkg.scope,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ParsedSource::Git(ref git) => {
                    let scope = if pkg.scope == "project" {
                        SourceScope::Project
                    } else {
                        SourceScope::User
                    };
                    let install_path = self.get_git_install_path(git, &scope);
                    if install_path.exists() {
                        if let Ok(local) = self
                            .run_command_capture("git", &["rev-parse", "HEAD"], Some(&install_path))
                            .await
                        {
                            if let Ok(remote_output) = self
                                .run_command_capture("git", &["ls-remote", &git.repo, "HEAD"], None)
                                .await
                            {
                                let remote_sha =
                                    remote_output.split_whitespace().next().unwrap_or("");
                                if !local.trim().is_empty()
                                    && !remote_sha.is_empty()
                                    && remote_sha != local.trim()
                                {
                                    updates.push(PackageUpdate {
                                        source: pkg.source,
                                        display_name: format!("{}/{}", git.host, git.path_),
                                        type_: "git".to_string(),
                                        scope: pkg.scope,
                                    });
                                }
                            }
                        }
                    }
                }
                ParsedSource::Local(_) => {}
            }
        }

        updates
    }

    fn package_sources_match(
        &self,
        existing_source: &str,
        input_source: &str,
        scope: &SourceScope,
    ) -> bool {
        let left = self.get_source_match_key_for_settings(existing_source, scope);
        let right = self.get_source_match_key_for_input(input_source);
        left == right
    }

    fn get_source_match_key_for_input(&self, source: &str) -> String {
        let parsed = parse_source(source);
        match parsed {
            ParsedSource::Npm(ref npm) => format!("npm:{}", npm.name),
            ParsedSource::Git(ref git) => format!("git:{}/{}", git.host, git.path_),
            ParsedSource::Local(ref local) => format!(
                "local:{}",
                self.resolve_path(&local.path_).to_string_lossy()
            ),
        }
    }

    fn get_source_match_key_for_settings(&self, source: &str, scope: &SourceScope) -> String {
        let parsed = parse_source(source);
        match parsed {
            ParsedSource::Npm(ref npm) => format!("npm:{}", npm.name),
            ParsedSource::Git(ref git) => format!("git:{}/{}", git.host, git.path_),
            ParsedSource::Local(ref local) => {
                let base_dir = self.get_base_dir_for_scope(scope);
                format!(
                    "local:{}",
                    self.resolve_path_from_base(&local.path_, &base_dir)
                        .to_string_lossy()
                )
            }
        }
    }

    fn dedupe_packages(
        &self,
        packages: &[(PackageSource, SourceScope)],
    ) -> Vec<(PackageSource, SourceScope)> {
        let mut seen: HashMap<String, (PackageSource, SourceScope)> = HashMap::new();

        for (pkg, scope) in packages {
            let source_str = &pkg.source;
            let identity = self.get_package_identity(source_str, Some(scope));

            match seen.get(&identity) {
                None => {
                    seen.insert(identity, (pkg.clone(), scope.clone()));
                }
                Some((_, existing_scope)) => {
                    // Project wins over user
                    if matches!(scope, SourceScope::Project)
                        && matches!(existing_scope, SourceScope::User)
                    {
                        seen.insert(identity, (pkg.clone(), scope.clone()));
                    }
                }
            }
        }

        seen.into_values().collect()
    }

    // ========================================================================
    // Resource collection
    // ========================================================================

    fn collect_resource_files(dir: &Path, resource_type: &str) -> Vec<String> {
        match resource_type {
            "skills" => collect_skill_entries(dir, "skills", None, None),
            "extensions" => Self::collect_auto_extension_entries(dir),
            _ => {
                let pattern = match resource_type {
                    "prompts" => regex::Regex::new(r"(?i)\.md$").unwrap(),
                    "themes" => regex::Regex::new(r"(?i)\.json$").unwrap(),
                    _ => regex::Regex::new(r"(?i)\.(ts|js)$").unwrap(),
                };
                collect_files(dir, &pattern, true, None, None)
            }
        }
    }

    fn collect_auto_extension_entries(dir: &Path) -> Vec<String> {
        let mut entries = Vec::new();
        if !dir.exists() {
            return entries;
        }

        // First check if this directory itself has explicit extension entries
        let root_entries = Self::resolve_extension_entries(dir);
        if let Some(e) = root_entries {
            return e;
        }

        let mut ig = IgnoreMatcher::new();
        add_ignore_rules(&mut ig, dir, dir);

        let dir_entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return entries,
        };

        for entry in dir_entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }

            let full_path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir =
                metadata.is_dir() || (metadata.file_type().is_symlink() && full_path.is_dir());
            let is_file =
                metadata.is_file() || (metadata.file_type().is_symlink() && full_path.is_file());

            let rel_path = pathdiff::diff_paths(&full_path, dir)
                .unwrap_or_else(|| full_path.clone())
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");

            let ignore_path = if is_dir {
                format!("{}/", rel_path)
            } else {
                rel_path.clone()
            };
            if ig.ignores(&ignore_path) {
                continue;
            }

            if is_file && (name.ends_with(".ts") || name.ends_with(".js")) {
                entries.push(full_path.to_string_lossy().to_string());
            } else if is_dir {
                if let Some(resolved) = Self::resolve_extension_entries(&full_path) {
                    entries.extend(resolved);
                }
            }
        }

        entries
    }

    fn resolve_extension_entries(dir: &Path) -> Option<Vec<String>> {
        // Check package.json for pick.extensions
        let package_json_path = dir.join("package.json");
        if package_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json_path) {
                if let Ok(pkg) = serde_json::from_str::<PackageJson>(&content) {
                    if let Some(ref manifest) = pkg.pick {
                        if !manifest.extensions.is_empty() {
                            let entries: Vec<String> = manifest
                                .extensions
                                .iter()
                                .map(|ext_path| {
                                    let resolved = dir.join(ext_path);
                                    resolved.to_string_lossy().to_string()
                                })
                                .filter(|p| Path::new(p).exists())
                                .collect();
                            if !entries.is_empty() {
                                return Some(entries);
                            }
                        }
                    }
                }
            }
        }

        // Check for index.ts or index.js
        let index_ts = dir.join("index.ts");
        let index_js = dir.join("index.js");
        if index_ts.exists() {
            return Some(vec![index_ts.to_string_lossy().to_string()]);
        }
        if index_js.exists() {
            return Some(vec![index_js.to_string_lossy().to_string()]);
        }

        None
    }

    fn collect_package_resources(
        &self,
        package_root: &Path,
        accumulator: &mut ResourceAccumulator,
        filter: Option<&PackageFilter>,
        metadata: &PathMetadata,
    ) -> bool {
        if let Some(f) = filter {
            for resource_type in &RESOURCE_TYPES {
                let patterns = match *resource_type {
                    "extensions" => f.extensions.as_ref(),
                    "skills" => f.skills.as_ref(),
                    "prompts" => f.prompts.as_ref(),
                    "themes" => f.themes.as_ref(),
                    _ => None,
                };

                let target = self.get_target_map(accumulator, resource_type);
                if let Some(p) = patterns {
                    self.apply_package_filter(package_root, p, resource_type, target, metadata);
                } else {
                    self.collect_default_resources(package_root, resource_type, target, metadata);
                }
            }
            return true;
        }

        // Check for manifest in package.json
        if let Some(manifest) = self.read_pick_manifest(package_root) {
            for resource_type in &RESOURCE_TYPES {
                let entries = match *resource_type {
                    "extensions" => &manifest.extensions,
                    "skills" => &manifest.skills,
                    "prompts" => &manifest.prompts,
                    "themes" => &manifest.themes,
                    _ => continue,
                };
                self.add_manifest_entries(
                    entries,
                    package_root,
                    resource_type,
                    self.get_target_map(accumulator, resource_type),
                    metadata,
                );
            }
            return true;
        }

        // Convention-based: look for extensions/, skills/, prompts/, themes/ dirs
        let mut has_any = false;
        for resource_type in &RESOURCE_TYPES {
            let dir = package_root.join(resource_type);
            if dir.exists() {
                let files = Self::collect_resource_files(&dir, resource_type);
                let target = self.get_target_map(accumulator, resource_type);
                for f in &files {
                    self.add_resource(target, f, metadata, true);
                }
                has_any = true;
            }
        }

        has_any
    }

    fn collect_default_resources(
        &self,
        package_root: &Path,
        resource_type: &str,
        target: &mut HashMap<String, ResourceEntry>,
        metadata: &PathMetadata,
    ) {
        if let Some(manifest) = self.read_pick_manifest(package_root) {
            let entries = match resource_type {
                "extensions" => &manifest.extensions,
                "skills" => &manifest.skills,
                "prompts" => &manifest.prompts,
                "themes" => &manifest.themes,
                _ => return,
            };
            self.add_manifest_entries(entries, package_root, resource_type, target, metadata);
            return;
        }

        let dir = package_root.join(resource_type);
        if dir.exists() {
            let files = Self::collect_resource_files(&dir, resource_type);
            for f in &files {
                self.add_resource(target, f, metadata, true);
            }
        }
    }

    fn apply_package_filter(
        &self,
        package_root: &Path,
        user_patterns: &[String],
        resource_type: &str,
        target: &mut HashMap<String, ResourceEntry>,
        metadata: &PathMetadata,
    ) {
        let manifest_files = self.collect_manifest_files(package_root, resource_type);
        let all_files = manifest_files.0;

        if user_patterns.is_empty() {
            // Empty array = disable all
            for f in &all_files {
                self.add_resource(target, f, metadata, false);
            }
            return;
        }

        let enabled_by_user =
            apply_patterns(&all_files, user_patterns, &package_root.to_string_lossy());

        for f in &all_files {
            let enabled = enabled_by_user.contains(f);
            self.add_resource(target, f, metadata, enabled);
        }
    }

    fn collect_manifest_files(
        &self,
        package_root: &Path,
        resource_type: &str,
    ) -> (Vec<String>, HashSet<String>) {
        if let Some(manifest) = self.read_pick_manifest(package_root) {
            let entries = match resource_type {
                "extensions" => manifest.extensions,
                "skills" => manifest.skills,
                "prompts" => manifest.prompts,
                "themes" => manifest.themes,
                _ => return (vec![], HashSet::new()),
            };

            if !entries.is_empty() {
                let all_files =
                    self.collect_files_from_manifest_entries(&entries, package_root, resource_type);
                let manifest_patterns: Vec<String> = entries
                    .into_iter()
                    .filter(|e| is_override_pattern(e))
                    .collect();
                let enabled_by_manifest = if manifest_patterns.is_empty() {
                    all_files.iter().cloned().collect()
                } else {
                    let base_str = package_root.to_string_lossy().to_string();
                    apply_patterns(&all_files, &manifest_patterns, &base_str)
                };
                return (
                    enabled_by_manifest.iter().cloned().collect(),
                    enabled_by_manifest,
                );
            }
        }

        let convention_dir = package_root.join(resource_type);
        if !convention_dir.exists() {
            return (vec![], HashSet::new());
        }
        let all_files = Self::collect_resource_files(&convention_dir, resource_type);
        let set: HashSet<String> = all_files.iter().cloned().collect();
        (all_files, set)
    }

    fn add_manifest_entries(
        &self,
        entries: &[String],
        root: &Path,
        resource_type: &str,
        target: &mut HashMap<String, ResourceEntry>,
        metadata: &PathMetadata,
    ) {
        let all_files = self.collect_files_from_manifest_entries(entries, root, resource_type);
        let patterns: Vec<String> = entries
            .iter()
            .filter(|e| is_override_pattern(e))
            .cloned()
            .collect();
        let base_str = root.to_string_lossy().to_string();
        let enabled_paths = apply_patterns(&all_files, &patterns, &base_str);

        for f in &all_files {
            if enabled_paths.contains(f) {
                self.add_resource(target, f, metadata, true);
            }
        }
    }

    fn collect_files_from_manifest_entries(
        &self,
        entries: &[String],
        root: &Path,
        resource_type: &str,
    ) -> Vec<String> {
        let source_entries: Vec<&String> =
            entries.iter().filter(|e| !is_override_pattern(e)).collect();

        let mut resolved: Vec<PathBuf> = Vec::new();
        for entry in source_entries {
            if entry.contains('*') || entry.contains('?') {
                // Glob pattern - basic implementation
                let full_path = root.join(entry);
                if let Some(parent) = full_path.parent() {
                    if let Ok(read_dir) = std::fs::read_dir(parent) {
                        for dir_entry in read_dir.flatten() {
                            let name = dir_entry.file_name().to_string_lossy().to_string();
                            // Simple glob matching
                            let glob_pattern = entry.replace('*', ".*");
                            if let Ok(re) = regex::Regex::new(&format!("^{}$", glob_pattern)) {
                                if re.is_match(&name)
                                    || re.is_match(&dir_entry.path().to_string_lossy())
                                {
                                    resolved.push(dir_entry.path());
                                }
                            }
                        }
                    }
                }
            } else {
                resolved.push(root.join(entry));
            }
        }

        self.collect_files_from_paths(&resolved, resource_type)
    }

    fn collect_files_from_paths(&self, paths: &[PathBuf], resource_type: &str) -> Vec<String> {
        let mut files = Vec::new();
        for p in paths {
            if !p.exists() {
                continue;
            }
            if p.is_dir() {
                files.extend(Self::collect_resource_files(p, resource_type));
            } else if p.is_file() {
                files.push(p.to_string_lossy().to_string());
            }
        }
        files
    }

    fn get_target_map<'a>(
        &self,
        accumulator: &'a mut ResourceAccumulator,
        resource_type: &str,
    ) -> &'a mut HashMap<String, ResourceEntry> {
        match resource_type {
            "extensions" => &mut accumulator.extensions,
            "skills" => &mut accumulator.skills,
            "prompts" => &mut accumulator.prompts,
            "themes" => &mut accumulator.themes,
            _ => panic!("Unknown resource type: {}", resource_type),
        }
    }

    fn add_resource(
        &self,
        map: &mut HashMap<String, ResourceEntry>,
        path: &str,
        metadata: &PathMetadata,
        enabled: bool,
    ) {
        if path.is_empty() {
            return;
        }
        map.entry(path.to_string()).or_insert(ResourceEntry {
            metadata: metadata.clone(),
            enabled,
        });
    }

    fn create_accumulator(&self) -> ResourceAccumulator {
        ResourceAccumulator::default()
    }

    fn resource_precedence_rank(metadata: &PathMetadata) -> u8 {
        if matches!(metadata.origin, SourceOrigin::Package) {
            return 4;
        }
        let scope_base: u8 = match metadata.scope {
            SourceScope::Project => 0,
            _ => 2,
        };
        let source_add: u8 = if metadata.source == "local" { 0 } else { 1 };
        scope_base + source_add
    }

    fn get_installed_path_for_source(
        &self,
        parsed: &ParsedSource,
        scope: &SourceScope,
    ) -> Option<String> {
        match parsed {
            ParsedSource::Npm(npm) => {
                let path = self.get_managed_npm_install_path(npm, scope);
                if path.exists() {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                }
            }
            ParsedSource::Git(git) => {
                let path = self.get_git_install_path(git, scope);
                if path.exists() {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                }
            }
            ParsedSource::Local(local) => {
                let base_dir = self.get_base_dir_for_scope(scope);
                let path = self.resolve_path_from_base(&local.path_, &base_dir);
                if path.exists() {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                }
            }
        }
    }

    fn to_resolved_paths(&self, accumulator: ResourceAccumulator) -> ResolvedPaths {
        let map_to_resolved = |entries: HashMap<String, ResourceEntry>| -> Vec<ResolvedResource> {
            let mut resolved: Vec<ResolvedResource> = entries
                .into_iter()
                .map(|(path, entry)| ResolvedResource {
                    path,
                    enabled: entry.enabled,
                    metadata: entry.metadata,
                })
                .collect();

            resolved.sort_by(|a, b| {
                Self::resource_precedence_rank(&a.metadata)
                    .cmp(&Self::resource_precedence_rank(&b.metadata))
            });

            // Dedupe by canonical path
            let mut seen = HashSet::new();
            resolved.retain(|entry| {
                let canon = std::path::absolute(Path::new(&entry.path))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| entry.path.clone());
                seen.insert(canon)
            });

            resolved
        };

        ResolvedPaths {
            extensions: map_to_resolved(accumulator.extensions),
            skills: map_to_resolved(accumulator.skills),
            prompts: map_to_resolved(accumulator.prompts),
            themes: map_to_resolved(accumulator.themes),
        }
    }

    /// Add auto-discovered resources (from default directories)
    fn add_auto_discovered_resources(
        &self,
        accumulator: &mut ResourceAccumulator,
        global_base_dir: &Path,
        project_base_dir: &Path,
    ) {
        let project_metadata = PathMetadata {
            source: "auto".to_string(),
            scope: SourceScope::Project,
            origin: SourceOrigin::TopLevel,
            base_dir: Some(project_base_dir.to_string_lossy().to_string()),
        };

        let user_metadata = PathMetadata {
            source: "auto".to_string(),
            scope: SourceScope::User,
            origin: SourceOrigin::TopLevel,
            base_dir: Some(global_base_dir.to_string_lossy().to_string()),
        };

        let project_dirs = [
            ("extensions", project_base_dir.join("extensions")),
            ("skills", project_base_dir.join("skills")),
            ("prompts", project_base_dir.join("prompts")),
            ("themes", project_base_dir.join("themes")),
        ];

        let user_dirs = [
            ("extensions", global_base_dir.join("extensions")),
            ("skills", global_base_dir.join("skills")),
            ("prompts", global_base_dir.join("prompts")),
            ("themes", global_base_dir.join("themes")),
        ];

        // Project resources
        for (rtype, dir) in &project_dirs {
            if dir.exists() {
                let target = self.get_target_map(accumulator, rtype);
                let files = Self::collect_resource_files(dir, rtype);
                for f in files {
                    self.add_resource(target, &f, &project_metadata, true);
                }
            }
        }

        // User resources
        for (rtype, dir) in &user_dirs {
            if dir.exists() {
                let target = self.get_target_map(accumulator, rtype);
                let files = Self::collect_resource_files(dir, rtype);
                for f in files {
                    self.add_resource(target, &f, &user_metadata, true);
                }
            }
        }
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Resolve all resources
    pub async fn resolve(&self) -> ResolvedPaths {
        let mut accumulator = self.create_accumulator();

        let global_base_dir = self.agent_dir.clone();
        let project_base_dir = self.cwd.join(CONFIG_DIR_NAME);

        // Add auto-discovered resources
        self.add_auto_discovered_resources(&mut accumulator, &global_base_dir, &project_base_dir);

        self.to_resolved_paths(accumulator)
    }

    /// Install a package
    pub async fn install(&self, source: &str, local: bool) -> Result<(), String> {
        let scope = if local {
            SourceScope::Project
        } else {
            SourceScope::User
        };
        let parsed = parse_source(source);

        match parsed {
            ParsedSource::Npm(ref npm) => self.install_npm(npm, &scope, false).await,
            ParsedSource::Git(ref git) => self.install_git(git, &scope).await,
            ParsedSource::Local(ref local) => {
                let base_dir = self.get_base_dir_for_scope(&scope);
                let resolved = self.resolve_path_from_base(&local.path_, &base_dir);
                if !resolved.exists() {
                    return Err(format!(
                        "Path does not exist: {}",
                        resolved.to_string_lossy()
                    ));
                }
                Ok(())
            }
        }
    }

    /// Remove a package
    pub async fn remove(&self, source: &str, local: bool) -> Result<(), String> {
        let scope = if local {
            SourceScope::Project
        } else {
            SourceScope::User
        };
        let parsed = parse_source(source);

        match parsed {
            ParsedSource::Npm(ref npm) => self.uninstall_npm(npm, &scope).await,
            ParsedSource::Git(ref git) => self.remove_git(git, &scope).await,
            ParsedSource::Local(_) => Ok(()),
        }
    }

    pub async fn install_and_persist(&self, source: &str, local: bool) -> Result<(), String> {
        self.install(source, local).await?;
        self.add_source_to_settings(source, local).await;
        Ok(())
    }

    /// Remove + persist to settings
    pub async fn remove_and_persist(&self, source: &str, local: bool) -> Result<bool, String> {
        self.remove(source, local).await?;
        Ok(self.remove_source_from_settings(source, local).await)
    }

    /// Update packages (npm or git)
    pub async fn update(&self, source: Option<&str>) -> Result<(), String> {
        let packages = if let Some(src) = source {
            let scope = "user";
            vec![ConfiguredPackage {
                source: src.to_string(),
                scope: scope.to_string(),
                filtered: false,
                installed_path: None,
            }]
        } else {
            self.list_configured_packages().await
        };

        for pkg in &packages {
            let parsed = parse_source(&pkg.source);
            match &parsed {
                ParsedSource::Npm(_npm) => {
                    let (cmd, args) = self.get_npm_command();
                    let mut full_args = args.clone();
                    full_args.push("update".to_string());
                    full_args.push("--legacy-peer-deps".to_string());
                    self.run_command(&cmd, &full_args, None).await?;
                }
                ParsedSource::Git(git) => {
                    let install_path = self.get_git_install_path(git, &SourceScope::User);
                    if install_path.exists() {
                        let output = tokio::process::Command::new("git")
                            .args(["-C", &install_path.to_string_lossy(), "pull", "--ff-only"])
                            .output()
                            .await
                            .map_err(|e| format!("Git pull failed: {}", e))?;
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let _ = stderr;
                        }
                    }
                }
                ParsedSource::Local(_) => {}
            }
        }

        Ok(())
    }
}

// ============================================================================
// npm operations
// ============================================================================

impl DefaultPackageManager {
    async fn install_npm(
        &self,
        source: &NpmSource,
        scope: &SourceScope,
        temporary: bool,
    ) -> Result<(), String> {
        let install_root = self.get_npm_install_root(scope, temporary);
        self.ensure_npm_project(&install_root);
        let (cmd, args) = self.get_npm_command();
        let mut full_args = args.clone();
        full_args.push("install".to_string());
        full_args.push(source.spec.clone());
        full_args.push("--prefix".to_string());
        full_args.push(install_root.to_string_lossy().to_string());
        full_args.push("--legacy-peer-deps".to_string());

        self.run_command(&cmd, &full_args, None).await
    }

    async fn uninstall_npm(&self, source: &NpmSource, scope: &SourceScope) -> Result<(), String> {
        let install_root = self.get_npm_install_root(scope, false);
        if !install_root.exists() {
            return Ok(());
        }
        let (cmd, args) = self.get_npm_command();
        let mut full_args = args.clone();
        full_args.push("uninstall".to_string());
        full_args.push(source.name.clone());
        full_args.push("--prefix".to_string());
        full_args.push(install_root.to_string_lossy().to_string());

        self.run_command(&cmd, &full_args, None).await
    }

    fn ensure_npm_project(&self, install_root: &Path) {
        if !install_root.exists() {
            std::fs::create_dir_all(install_root).ok();
        }
        self.ensure_git_ignore(install_root);
        let package_json_path = install_root.join("package.json");
        if !package_json_path.exists() {
            let pkg_json = serde_json::json!({ "name": "pick-extensions", "private": true });
            std::fs::write(
                &package_json_path,
                serde_json::to_string_pretty(&pkg_json).unwrap(),
            )
            .ok();
        }
    }

    fn ensure_git_ignore(&self, dir: &Path) {
        if !dir.exists() {
            std::fs::create_dir_all(dir).ok();
        }
        let ignore_path = dir.join(".gitignore");
        if !ignore_path.exists() {
            std::fs::write(&ignore_path, "*\n!.gitignore\n").ok();
        }
    }
}

// ============================================================================
// git operations
// ============================================================================

impl DefaultPackageManager {
    async fn install_git(&self, source: &GitSource, scope: &SourceScope) -> Result<(), String> {
        let target_dir = self.get_git_install_path(source, scope);

        if target_dir.exists() {
            if let Some(ref r#ref) = source.r#ref {
                self.ensure_git_ref(
                    &target_dir,
                    &["fetch", "origin", r#ref.as_str()],
                    "FETCH_HEAD",
                )
                .await?;
            } else {
                // Just update
                self.run_command(
                    "git",
                    &[
                        "fetch",
                        "--prune",
                        "--no-tags",
                        "origin",
                        "+HEAD:refs/remotes/origin/HEAD",
                    ],
                    Some(&target_dir),
                )
                .await?;
            }
            return Ok(());
        }

        if let Some(parent) = target_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
        }

        self.run_command(
            "git",
            &["clone", &source.repo, &target_dir.to_string_lossy()],
            None,
        )
        .await?;

        if let Some(ref r#ref) = source.r#ref {
            self.run_command("git", &["checkout", r#ref.as_str()], Some(&target_dir))
                .await?;
        }

        // Install dependencies if package.json exists
        let package_json_path = target_dir.join("package.json");
        if package_json_path.exists() {
            let (cmd, args) = self.get_npm_command();
            let mut full_args = args.clone();
            full_args.push("install".to_string());
            full_args.push("--omit=dev".to_string());
            self.run_command(&cmd, &full_args, None).await?;
        }

        Ok(())
    }

    async fn remove_git(&self, source: &GitSource, scope: &SourceScope) -> Result<(), String> {
        let target_dir = self.get_git_install_path(source, scope);
        if !target_dir.exists() {
            return Ok(());
        }
        std::fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("Failed to remove {}: {}", target_dir.to_string_lossy(), e))
    }

    async fn ensure_git_ref(
        &self,
        target_dir: &Path,
        fetch_args: &[&str],
        ref_name: &str,
    ) -> Result<(), String> {
        self.run_command("git", fetch_args, Some(target_dir))
            .await?;

        // Check if we need to reset
        let local_head = self
            .run_command_capture("git", &["rev-parse", "HEAD"], Some(target_dir))
            .await?;
        let target_ref = format!("{}^{{commit}}", ref_name);
        let target_head = self
            .run_command_capture("git", &["rev-parse", &target_ref], Some(target_dir))
            .await?;

        if local_head.trim() == target_head.trim() {
            return Ok(());
        }

        self.run_command("git", &["reset", "--hard", &target_ref], Some(target_dir))
            .await?;
        self.run_command("git", &["clean", "-fdx"], Some(target_dir))
            .await?;

        // Reinstall dependencies
        let package_json_path = target_dir.join("package.json");
        if package_json_path.exists() {
            let (cmd, args) = self.get_npm_command();
            let mut full_args = args.clone();
            full_args.push("install".to_string());
            full_args.push("--omit=dev".to_string());
            self.run_command(&cmd, &full_args, None).await?;
        }

        Ok(())
    }
}

// ============================================================================
// Command execution helpers
// ============================================================================

impl DefaultPackageManager {
    async fn run_command<S: AsRef<str>>(
        &self,
        cmd: &str,
        args: &[S],
        cwd: Option<&Path>,
    ) -> Result<(), String> {
        let output = tokio::process::Command::new(cmd)
            .args(args.iter().map(|s| s.as_ref()))
            .current_dir(cwd.unwrap_or(&self.cwd))
            .output()
            .await
            .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let args_str: Vec<&str> = args.iter().map(|s| s.as_ref()).collect();
            return Err(format!(
                "{} {} failed: {}",
                cmd,
                args_str.join(" "),
                stderr.trim()
            ));
        }

        Ok(())
    }

    async fn run_command_capture(
        &self,
        cmd: &str,
        args: &[&str],
        cwd: Option<&Path>,
    ) -> Result<String, String> {
        let output = tokio::process::Command::new(cmd)
            .args(args)
            .current_dir(cwd.unwrap_or(&self.cwd))
            .output()
            .await
            .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "{} {} failed: {}",
                cmd,
                args.join(" "),
                stderr.trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

// ============================================================================
// Helper utilities
// ============================================================================

fn sha256_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_source_npm() {
        let parsed = parse_source("npm:some-package");
        match parsed {
            ParsedSource::Npm(npm) => {
                assert_eq!(npm.name, "some-package");
                assert!(!npm.pinned);
            }
            _ => panic!("Expected Npm source"),
        }
    }

    #[test]
    fn test_parse_source_npm_with_version() {
        let parsed = parse_source("npm:some-package@1.2.3");
        match parsed {
            ParsedSource::Npm(npm) => {
                assert_eq!(npm.name, "some-package");
                assert!(npm.pinned);
                assert_eq!(npm.spec, "some-package@1.2.3");
            }
            _ => panic!("Expected Npm source"),
        }
    }

    #[test]
    fn test_parse_source_local_path() {
        let parsed = parse_source("./local/path");
        match parsed {
            ParsedSource::Local(local) => {
                assert_eq!(local.path_, "./local/path");
            }
            _ => panic!("Expected Local source"),
        }
    }

    #[test]
    fn test_parse_source_git_shorthand() {
        let parsed = parse_source("user/repo");
        match parsed {
            ParsedSource::Git(git) => {
                assert_eq!(git.host, "github.com");
                assert!(git.repo.contains("github.com"));
            }
            _ => panic!("Expected Git source"),
        }
    }

    #[test]
    fn test_parse_source_git_url() {
        let parsed = parse_source("https://github.com/user/repo.git");
        match parsed {
            ParsedSource::Git(git) => {
                assert_eq!(git.host, "github.com");
                assert_eq!(git.path_, "user/repo");
            }
            _ => panic!("Expected Git source"),
        }
    }

    #[test]
    fn test_parse_npm_spec() {
        let (name, version) = parse_npm_spec("@scope/pkg@1.0.0");
        assert_eq!(name, "@scope/pkg");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_npm_spec_no_version() {
        let (name, version) = parse_npm_spec("pkg");
        assert_eq!(name, "pkg");
        assert_eq!(version, None);
    }

    #[test]
    fn test_split_patterns() {
        let entries = vec![
            "file1".to_string(),
            "!file2".to_string(),
            "file3".to_string(),
        ];
        let (plain, patterns) = split_patterns(&entries);
        assert_eq!(plain.len(), 2);
        assert_eq!(patterns.len(), 1);
    }

    #[test]
    fn test_ignore_matcher() {
        let mut ig = IgnoreMatcher::new();
        ig.add(&["*.log".to_string()]);
        assert!(ig.ignores("test.log"));
        assert!(!ig.ignores("test.txt"));
    }

    #[test]
    fn test_is_local_path() {
        assert!(is_local_path("./relative"));
        assert!(is_local_path("~/home"));
        assert!(!is_local_path("npm:package"));
        assert!(!is_local_path("user/repo"));
    }

    #[test]
    fn test_resource_precedence_rank() {
        let pkg_meta = PathMetadata {
            source: "remote".to_string(),
            scope: SourceScope::User,
            origin: SourceOrigin::Package,
            base_dir: None,
        };
        assert_eq!(
            DefaultPackageManager::resource_precedence_rank(&pkg_meta),
            4
        );

        let project_local = PathMetadata {
            source: "local".to_string(),
            scope: SourceScope::Project,
            origin: SourceOrigin::TopLevel,
            base_dir: None,
        };
        assert_eq!(
            DefaultPackageManager::resource_precedence_rank(&project_local),
            0
        );
    }
}

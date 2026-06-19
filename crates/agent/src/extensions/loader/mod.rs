//! Extension loader

pub mod factory;

pub use factory::*;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::types::{
    EventHandler, Extension, ExtensionAPI, ExtensionFactory, ExtensionFlag, ExtensionShortcut,
    FlagType, FlagValue, LoadExtensionsResult, RegisteredCommand, RegisteredTool, ToolDefinition,
};

// ============================================================================
// Extension API Implementation
// ============================================================================

struct ExtensionAPIImpl {
    extension_path: String,
    handlers: Mutex<HashMap<String, Vec<EventHandler>>>,
    tools: Mutex<Vec<ToolDefinition>>,
    commands: Mutex<Vec<(String, Option<String>)>>,
    shortcuts: Mutex<Vec<(String, Option<String>)>>,
    flags: Mutex<Vec<(String, Option<String>, FlagType, Option<FlagValue>)>>,
}

impl ExtensionAPIImpl {
    fn new(extension_path: String) -> Self {
        Self {
            extension_path,
            handlers: Mutex::new(HashMap::new()),
            tools: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            shortcuts: Mutex::new(Vec::new()),
            flags: Mutex::new(Vec::new()),
        }
    }

    fn into_extension(self) -> Extension {
        let mut ext = Extension::new(self.extension_path.clone(), self.extension_path);

        if let Ok(handlers) = self.handlers.into_inner() {
            ext.handlers = handlers;
        }
        if let Ok(tools) = self.tools.into_inner() {
            for tool in tools {
                ext.tools.insert(
                    tool.name.clone(),
                    RegisteredTool {
                        definition: tool,
                        extension_path: ext.path.clone(),
                    },
                );
            }
        }
        if let Ok(commands) = self.commands.into_inner() {
            for (name, description) in commands {
                ext.commands.insert(
                    name.clone(),
                    RegisteredCommand {
                        name,
                        description,
                        extension_path: ext.path.clone(),
                    },
                );
            }
        }
        if let Ok(shortcuts) = self.shortcuts.into_inner() {
            for (shortcut, description) in shortcuts {
                ext.shortcuts.insert(
                    shortcut.clone(),
                    ExtensionShortcut {
                        shortcut,
                        description,
                        extension_path: ext.path.clone(),
                    },
                );
            }
        }
        if let Ok(flags) = self.flags.into_inner() {
            for (name, description, flag_type, default) in flags {
                ext.flags.insert(
                    name.clone(),
                    ExtensionFlag {
                        name,
                        description,
                        flag_type,
                        default,
                        extension_path: ext.path.clone(),
                    },
                );
            }
        }

        ext
    }
}

impl ExtensionAPI for ExtensionAPIImpl {
    fn on_raw(&self, event_type: &str, handler: EventHandler) {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers
                .entry(event_type.to_string())
                .or_default()
                .push(handler);
        }
    }

    fn register_tool(&self, tool: ToolDefinition) {
        if let Ok(mut tools) = self.tools.lock() {
            tools.push(tool);
        }
    }

    fn register_command(&self, name: &str, description: Option<String>) {
        if let Ok(mut commands) = self.commands.lock() {
            commands.push((name.to_string(), description));
        }
    }

    fn register_shortcut(&self, shortcut: &str, description: Option<String>) {
        if let Ok(mut shortcuts) = self.shortcuts.lock() {
            shortcuts.push((shortcut.to_string(), description));
        }
    }

    fn register_flag(
        &self,
        name: &str,
        description: Option<String>,
        flag_type: FlagType,
        default: Option<FlagValue>,
    ) {
        if let Ok(mut flags) = self.flags.lock() {
            flags.push((name.to_string(), description, flag_type, default));
        }
    }
}

// ============================================================================
// Loading
// ============================================================================

/// Load an extension from a factory
pub async fn load_extension_from_factory(factory: &dyn ExtensionFactory) -> Extension {
    let api = ExtensionAPIImpl::new(format!("<factory:{}>", factory.name()));
    if let Err(e) = factory.init(&api).await {
        tracing::warn!("Extension {} init error: {}", factory.name(), e);
    }
    api.into_extension()
}

/// Load extensions from a list of factory objects
pub async fn load_extensions_from_factories(
    factories: &[Arc<dyn ExtensionFactory>],
) -> LoadExtensionsResult {
    let mut extensions = Vec::new();
    let errors = Vec::new();

    for factory in factories {
        let ext = load_extension_from_factory(factory.as_ref()).await;
        extensions.push(ext);
    }

    LoadExtensionsResult { extensions, errors }
}

// ============================================================================
// Dynamic loading (C ABI via JSON protocol)
// ============================================================================

/// JSON-based registration returned by dynamic extensions
#[derive(Serialize, Deserialize)]
struct DynamicExtensionRegistration {
    #[serde(default)]
    tools: Vec<ToolDefinition>,
    #[serde(default)]
    commands: Vec<DynamicCommand>,
    #[serde(default)]
    shortcuts: Vec<DynamicShortcut>,
    #[serde(default)]
    flags: Vec<DynamicFlag>,
}

#[derive(Serialize, Deserialize)]
struct DynamicCommand {
    name: String,
    description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DynamicShortcut {
    shortcut: String,
    description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DynamicFlag {
    name: String,
    description: Option<String>,
    flag_type: String,
    default: Option<serde_json::Value>,
}

/// Load extension symbols (metadata + optional registration) from a shared library.
fn load_extension_symbols(
    lib: &libloading::Library,
    path_str: &str,
) -> Result<(String, Option<String>), String> {
    // Load metadata symbol
    let metadata_fn: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char> = unsafe {
        lib.get(b"pick_ext_metadata\0").map_err(|_| {
            format!(
                "Extension '{}' missing 'pick_ext_metadata' symbol",
                path_str
            )
        })?
    };

    let metadata_cstr = unsafe { metadata_fn() };
    if metadata_cstr.is_null() {
        return Err(format!("Extension '{}' returned null metadata", path_str));
    }
    let metadata_str = unsafe { std::ffi::CStr::from_ptr(metadata_cstr) }
        .to_string_lossy()
        .to_string();

    let metadata: HashMap<String, String> = serde_json::from_str(&metadata_str)
        .map_err(|e| format!("Extension '{}' invalid metadata JSON: {}", path_str, e))?;

    let ext_name = metadata.get("name").cloned().unwrap_or_else(|| {
        std::path::Path::new(path_str)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    // Load registration symbol (optional - metadata-only extensions are OK)
    let reg_str: Option<String> = match unsafe {
        lib.get::<unsafe extern "C" fn() -> *const std::ffi::c_char>(b"pick_ext_register\0")
    } {
        Ok(register_fn) => {
            let reg_cstr = unsafe { register_fn() };
            if reg_cstr.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(reg_cstr) }
                        .to_string_lossy()
                        .to_string(),
                )
            }
        }
        Err(_) => None,
    };

    Ok((ext_name, reg_str))
}

/// Validate and apply extension registration data to an Extension struct.
fn validate_extension_factory(ext: &mut Extension, registration: DynamicExtensionRegistration) {
    for tool in registration.tools {
        ext.tools.insert(
            tool.name.clone(),
            RegisteredTool {
                definition: tool,
                extension_path: ext.path.clone(),
            },
        );
    }
    for cmd in registration.commands {
        ext.commands.insert(
            cmd.name.clone(),
            RegisteredCommand {
                name: cmd.name,
                description: cmd.description,
                extension_path: ext.path.clone(),
            },
        );
    }
    for shortcut in registration.shortcuts {
        ext.shortcuts.insert(
            shortcut.shortcut.clone(),
            ExtensionShortcut {
                shortcut: shortcut.shortcut,
                description: shortcut.description,
                extension_path: ext.path.clone(),
            },
        );
    }
    for flag in registration.flags {
        let flag_type = match flag.flag_type.as_str() {
            "boolean" => FlagType::Boolean,
            "string" => FlagType::String,
            _ => FlagType::String,
        };
        let default = flag.default.map(|v| match &v {
            serde_json::Value::Bool(b) => FlagValue::Bool(*b),
            serde_json::Value::String(s) => FlagValue::Str(s.clone()),
            _ => FlagValue::Str(v.to_string()),
        });
        ext.flags.insert(
            flag.name.clone(),
            ExtensionFlag {
                name: flag.name,
                description: flag.description,
                flag_type,
                default,
                extension_path: ext.path.clone(),
            },
        );
    }
}

/// Load an extension from a .so/.dll shared library using JSON protocol.
fn load_extension_from_library(path: &Path) -> Result<Extension, String> {
    let path_str = path.to_string_lossy().to_string();

    // SAFETY: Loading a shared library executes arbitrary code in the extension.
    let lib = unsafe {
        libloading::Library::new(path)
            .map_err(|e| format!("Failed to load extension library '{}': {}", path_str, e))?
    };

    let (ext_name, reg_str) = load_extension_symbols(&lib, &path_str)?;

    let mut ext = Extension::new(format!("dynamic:{}", ext_name), path_str);

    if let Some(ref reg_str) = reg_str
        && !reg_str.is_empty() {
            match serde_json::from_str::<DynamicExtensionRegistration>(reg_str) {
                Ok(registration) => {
                    validate_extension_factory(&mut ext, registration);
                }
                Err(_) => {
                    tracing::warn!(
                        "Extension '{}' returned unparseable registration JSON",
                        ext_name
                    );
                }
            }
        }

    // Keep the library loaded for the extension's lifetime
    std::mem::forget(lib);

    Ok(ext)
}

// ============================================================================
// Discovery
// ============================================================================

fn is_extension_file(name: &str) -> bool {
    name.ends_with(".cjs")
        || name.ends_with(".mjs")
        || name.ends_with(".js")
        || name.ends_with(".so")
        || name.ends_with(".dll")
        || name.ends_with(".wasm")
}

/// Discover extension files in a directory
pub fn discover_extensions_in_dir(dir: &Path) -> Vec<String> {
    let mut discovered = Vec::new();
    if !dir.is_dir() {
        return discovered;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && is_extension_file(name) {
                        discovered.push(path.to_string_lossy().to_string());
                    }
        }
    }

    discovered
}

// ============================================================================
// JSON Manifest Extensions
// ============================================================================

/// JSON manifest for a static extension (declares tools/commands/shortcuts/flags)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtensionManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tools: Vec<ToolDefinition>,
    #[serde(default)]
    commands: Vec<ManifestCommand>,
    #[serde(default)]
    shortcuts: Vec<ManifestShortcut>,
    #[serde(default)]
    flags: Vec<ManifestFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestCommand {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestShortcut {
    shortcut: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestFlag {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "type", default = "default_flag_type_str")]
    flag_type: String,
    #[serde(default)]
    default: Option<serde_json::Value>,
}

fn default_flag_type_str() -> String {
    "boolean".to_string()
}

/// Load an extension from a JSON manifest file.
fn load_extension_from_manifest(path: &Path) -> Option<Extension> {
    let content = std::fs::read_to_string(path).ok()?;
    let manifest: ExtensionManifest = serde_json::from_str(&content).ok()?;
    let name = manifest.name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });
    let path_str = path.to_string_lossy().to_string();

    let mut ext = Extension::new(format!("manifest:{}", name), path_str);

    for tool in manifest.tools {
        ext.tools.insert(
            tool.name.clone(),
            RegisteredTool {
                definition: tool,
                extension_path: ext.path.clone(),
            },
        );
    }
    for cmd in manifest.commands {
        ext.commands.insert(
            cmd.name.clone(),
            RegisteredCommand {
                name: cmd.name,
                description: cmd.description,
                extension_path: ext.path.clone(),
            },
        );
    }
    for shortcut in manifest.shortcuts {
        ext.shortcuts.insert(
            shortcut.shortcut.clone(),
            ExtensionShortcut {
                shortcut: shortcut.shortcut,
                description: shortcut.description,
                extension_path: ext.path.clone(),
            },
        );
    }
    for flag in manifest.flags {
        let flag_type = match flag.flag_type.as_str() {
            "boolean" => FlagType::Boolean,
            "string" => FlagType::String,
            _ => FlagType::Boolean,
        };
        let default = flag.default.map(|v| match &v {
            serde_json::Value::Bool(b) => FlagValue::Bool(*b),
            serde_json::Value::String(s) => FlagValue::Str(s.clone()),
            _ => FlagValue::Str(v.to_string()),
        });
        ext.flags.insert(
            flag.name.clone(),
            ExtensionFlag {
                name: flag.name,
                description: flag.description,
                flag_type,
                default,
                extension_path: ext.path.clone(),
            },
        );
    }

    Some(ext)
}

/// Check if a directory contains an extension manifest file.
fn find_extension_manifest(dir: &Path) -> Option<std::path::PathBuf> {
    // Check for extension.json, then package.json with pick field
    let extension_json = dir.join("extension.json");
    if extension_json.is_file() {
        // Validate it's parseable
        if let Ok(content) = std::fs::read_to_string(&extension_json)
            && serde_json::from_str::<ExtensionManifest>(&content).is_ok() {
                return Some(extension_json);
            }
    }
    // Check package.json with pick field
    let package_json = dir.join("package.json");
    if package_json.is_file()
        && let Ok(content) = std::fs::read_to_string(&package_json)
            && let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content)
                && pkg.get("pick").and_then(|v| v.as_object()).is_some() {
                    return Some(package_json);
                }
    None
}

/// Discover extension entry points from a directory.
fn resolve_extension_entries(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut entries = Vec::new();

    // Check for manifest first
    if let Some(manifest) = find_extension_manifest(dir) {
        entries.push(manifest);
        return entries;
    }

    // Check for index files
    for name in &["index.cjs", "index.mjs", "index.js"] {
        let index_path = dir.join(name);
        if index_path.is_file() {
            entries.push(index_path);
            return entries;
        }
    }

    entries
}

/// Discover extensions in a directory.
fn discover_extensions_in_dir_v2(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut discovered = Vec::new();
    if !dir.is_dir() {
        return discovered;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Subdirectories: resolve extension entries
            if path.is_dir() {
                let mut sub_entries = resolve_extension_entries(&path);
                discovered.append(&mut sub_entries);
                continue;
            }

            // Files: direct extension files
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && is_extension_file(name) {
                        discovered.push(path);
                    }
        }
    }

    discovered
}

/// Discover and load extensions from configured paths and registered factories
pub async fn discover_and_load_extensions(
    configured_paths: &[String],
    _cwd: &Path,
    _agent_dir: &Path,
) -> LoadExtensionsResult {
    let mut all_extensions: Vec<Extension> = Vec::new();
    let mut errors = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Load extensions from globally registered factories
    let factories = global_extension_registry().get_all();
    let factory_results = load_extensions_from_factories(&factories).await;
    all_extensions.extend(factory_results.extensions);

    // 2. Load extensions from configured paths
    for p in configured_paths {
        let resolved = std::path::PathBuf::from(p);
        let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        let canonical_str = canonical.to_string_lossy().to_string();

        if !seen_paths.insert(canonical_str.clone()) {
            continue;
        }

        if canonical.is_dir() {
            // Try manifest-based and dynamic extensions in directory
            let manifest = find_extension_manifest(&canonical);
            if let Some(manifest_path) = manifest
                && let Some(ext) = load_extension_from_manifest(&manifest_path) {
                    all_extensions.push(ext);
                    continue;
                }
            // Fall back to discovering individual files
            let discovered = discover_extensions_in_dir(&canonical);
            for file_path in discovered {
                if seen_paths.insert(file_path.clone()) {
                    match load_extension_from_library(std::path::Path::new(&file_path)) {
                        Ok(ext) => all_extensions.push(ext),
                        Err(e) => errors.push(super::types::ExtensionLoadError {
                            path: file_path,
                            error: e,
                        }),
                    }
                }
            }
        } else if canonical.is_file() {
            // Check file extension to decide loading strategy
            let ext = canonical.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext {
                "json" => {
                    if let Some(ext) = load_extension_from_manifest(&canonical) {
                        all_extensions.push(ext);
                    } else {
                        errors.push(super::types::ExtensionLoadError {
                            path: canonical_str,
                            error: "Failed to parse extension manifest".to_string(),
                        });
                    }
                }
                _ => match load_extension_from_library(&canonical) {
                    Ok(ext) => all_extensions.push(ext),
                    Err(e) => errors.push(super::types::ExtensionLoadError {
                        path: canonical_str,
                        error: e,
                    }),
                },
            }
        }
    }

    LoadExtensionsResult {
        extensions: all_extensions,
        errors,
    }
}

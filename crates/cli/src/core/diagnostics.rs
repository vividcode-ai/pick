//! Resource diagnostics types

/// A collision between two resources of the same type
#[derive(Debug, Clone)]
pub struct ResourceCollision {
    pub resource_type: String, // "extension", "skill", "prompt", "theme"
    pub name: String,
    pub winner_path: String,
    pub loser_path: String,
    pub winner_source: Option<String>,
    pub loser_source: Option<String>,
}

/// A diagnostic message about resources
#[derive(Debug, Clone)]
pub struct ResourceDiagnostic {
    pub type_: String, // "warning", "error", "collision"
    pub message: String,
    pub path: Option<String>,
    pub collision: Option<ResourceCollision>,
}

impl ResourceDiagnostic {
    pub fn warning(message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            type_: "warning".to_string(),
            message: message.into(),
            path,
            collision: None,
        }
    }

    pub fn error(message: impl Into<String>, path: Option<String>) -> Self {
        Self {
            type_: "error".to_string(),
            message: message.into(),
            path,
            collision: None,
        }
    }

    pub fn collision(collision: ResourceCollision) -> Self {
        let message = format!(
            "Collision on '{}': {} wins over {}",
            collision.name, collision.winner_path, collision.loser_path
        );
        Self {
            type_: "collision".to_string(),
            message,
            path: None,
            collision: Some(collision),
        }
    }
}

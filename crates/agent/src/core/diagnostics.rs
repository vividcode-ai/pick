//! Agent diagnostic types

/// Agent diagnostic message
#[derive(Debug, Clone)]
pub struct AgentDiagnostic {
    pub message: String,
    pub diagnostic_type: DiagnosticType,
}

/// Diagnostic severity
#[derive(Debug, Clone)]
pub enum DiagnosticType {
    Info,
    Warning,
    Error,
}

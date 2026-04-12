//! Core types for prompt management and skill registry.

use serde::{Deserialize, Serialize};

/// Maximum length for short text fields (names, categories, capabilities).
const MAX_NAME_LEN: usize = 256;
/// Maximum length for description/body text fields.
const MAX_BODY_LEN: usize = 100_000;
/// Maximum number of variables per prompt template.
const MAX_VARIABLES: usize = 50;
/// Maximum number of steps per pipeline.
const MAX_PIPELINE_STEPS: usize = 100;

/// Validation error for input types.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("{field} is required and cannot be empty")]
    Empty { field: &'static str },
    #[error("{field} exceeds maximum length of {max}")]
    TooLong { field: &'static str, max: usize },
    #[error("{field} must be between {min} and {max}")]
    OutOfRange {
        field: &'static str,
        min: f64,
        max: f64,
    },
    #[error("{field} contains invalid characters")]
    InvalidChars { field: &'static str },
    #[error("{field}: too many items (max {max})")]
    TooMany { field: &'static str, max: usize },
}

/// Validate a short identifier field (alphanumeric, hyphens, underscores, dots).
fn validate_name(value: &str, field: &'static str) -> Result<(), ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if trimmed.len() > MAX_NAME_LEN {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_NAME_LEN,
        });
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ' ')
    {
        return Err(ValidationError::InvalidChars { field });
    }
    Ok(())
}

/// Validate a body/description text field (non-empty, bounded length).
fn validate_body(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_BODY_LEN {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_BODY_LEN,
        });
    }
    Ok(())
}

/// A versioned prompt template with variable placeholders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub version: u32,
    /// Template body with `{{variable}}` placeholders.
    pub body: String,
    /// Variable declarations: name -> description.
    pub variables: Vec<PromptVariable>,
    /// Optional category (e.g. "agent-role", "system", "task").
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Whether this is the active version for its name.
    pub active: bool,
}

/// A variable declared in a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
}

/// Request to create or update a prompt template.
#[derive(Debug, Clone, Deserialize)]
pub struct PromptInput {
    pub name: String,
    pub body: String,
    pub variables: Vec<PromptVariable>,
    pub category: Option<String>,
}

impl PromptInput {
    /// Validate all fields, returning the first error found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name(&self.name, "name")?;
        validate_body(&self.body, "body")?;
        if self.variables.len() > MAX_VARIABLES {
            return Err(ValidationError::TooMany {
                field: "variables",
                max: MAX_VARIABLES,
            });
        }
        for v in &self.variables {
            validate_name(&v.name, "variable.name")?;
        }
        if let Some(ref cat) = self.category {
            validate_name(cat, "category")?;
        }
        Ok(())
    }
}

/// A skill registered by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub agent: String,
    pub host: String,
    pub capability: String,
    pub confidence: f64,
    pub description: String,
    pub last_used: Option<String>,
    pub registered_at: String,
}

/// Request to register a skill.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillInput {
    pub agent: String,
    pub host: String,
    pub capability: String,
    pub confidence: f64,
    pub description: String,
}

impl SkillInput {
    /// Validate all fields, returning the first error found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name(&self.agent, "agent")?;
        validate_name(&self.host, "host")?;
        validate_name(&self.capability, "capability")?;
        validate_body(&self.description, "description")?;
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(ValidationError::OutOfRange {
                field: "confidence",
                min: 0.0,
                max: 1.0,
            });
        }
        Ok(())
    }
}

/// Result of an A/B test comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbTestResult {
    pub test_id: String,
    pub prompt_a_id: String,
    pub prompt_b_id: String,
    pub task_ref: String,
    pub winner: Option<String>,
    pub tokens_a: u64,
    pub tokens_b: u64,
    pub score_a: f64,
    pub score_b: f64,
    pub created_at: String,
}

/// Token usage stats for a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStats {
    pub prompt_id: String,
    pub total_tokens: u64,
    pub usage_count: u64,
    pub avg_tokens: f64,
}

/// An immutable snapshot of a prompt bound to an agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnedPrompt {
    pub spawn_id: String,
    pub agent: String,
    pub task_id: String,
    /// The fully rendered prompt, frozen at spawn time.
    pub rendered_body: String,
    pub prompt_template_id: String,
    pub prompt_version: u32,
    pub spawned_at: String,
}

/// Prompt search query parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PromptQuery {
    pub name: Option<String>,
    pub category: Option<String>,
    pub active_only: Option<bool>,
}

/// Skill search query parameters.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SkillQuery {
    pub capability: Option<String>,
    pub agent: Option<String>,
    pub min_confidence: Option<f64>,
}

/// A declarative pipeline — ordered steps referencing prompt templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<PipelineStep>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// A single step in a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub order: u32,
    pub skill: String,
    pub prompt_name: String,
    pub agent: Option<String>,
    pub condition: Option<String>,
}

/// Request to create a pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct PipelineInput {
    pub name: String,
    pub description: String,
    pub steps: Vec<PipelineStep>,
}

impl PipelineInput {
    /// Validate all fields, returning the first error found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_name(&self.name, "name")?;
        validate_body(&self.description, "description")?;
        if self.steps.len() > MAX_PIPELINE_STEPS {
            return Err(ValidationError::TooMany {
                field: "steps",
                max: MAX_PIPELINE_STEPS,
            });
        }
        for s in &self.steps {
            validate_name(&s.skill, "step.skill")?;
            validate_name(&s.prompt_name, "step.prompt_name")?;
        }
        Ok(())
    }
}

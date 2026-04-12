//! Seed the 8 default skill prompts.
//!
//! Single source of truth: `claude-config/skills/*/SKILL.md`.
//! The `.claude/commands/` versions are deliberately compact (Claude Code slash commands).
//! There is no third copy — the `data/skills/` directory has been removed.

use rusqlite::{params, Connection};

// All skill prompts loaded from data/skills/*.md
const SKILL_CHECK: &str = include_str!("../../data/skills/check.md");
const SKILL_EXECUTE: &str = include_str!("../../data/skills/execute.md");
const SKILL_INTERVIEW: &str = include_str!("../../data/skills/interview.md");
const SKILL_PLANNER: &str = include_str!("../../data/skills/planner.md");
const SKILL_PREPARE: &str = include_str!("../../data/skills/prepare.md");
const SKILL_PROMPT: &str = include_str!("../../data/skills/prompt.md");
const SKILL_RELEASE: &str = include_str!("../../data/skills/release.md");
const SKILL_RESEARCH: &str = include_str!("../../data/skills/research.md");
const SKILL_SOLVE: &str = include_str!("../../data/skills/solve.md");

/// All skill prompts: (name, body).
pub(crate) const SKILLS: &[(&str, &str)] = &[
    ("skill-check", SKILL_CHECK),
    ("skill-execute", SKILL_EXECUTE),
    ("skill-interview", SKILL_INTERVIEW),
    ("skill-planner", SKILL_PLANNER),
    ("skill-prepare", SKILL_PREPARE),
    ("skill-prompt", SKILL_PROMPT),
    ("skill-release", SKILL_RELEASE),
    ("skill-research", SKILL_RESEARCH),
    ("skill-solve", SKILL_SOLVE),
];

pub fn seed(conn: &Connection) -> Result<(), String> {
    for (name, body) in SKILLS {
        let id = format!("pt-seed-{name}");
        conn.execute(
            "INSERT INTO prompt_templates \
             (id, name, version, body, variables, category, active) \
             VALUES (?1, ?2, 1, ?3, '[]', 'skill', 1) \
             ON CONFLICT(id) DO UPDATE SET body = excluded.body",
            params![id, name, body],
        )
        .map_err(|e| format!("seed skill {name}: {e}"))?;
    }
    tracing::debug!("Seeded {} skill prompts", SKILLS.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Required sections per skill prompt — if any of these are missing,
    /// the skill is broken. Catches accidental deletion of phases.
    const REQUIRED_SECTIONS: &[(&str, &[&str])] = &[
        (
            "skill-solve",
            &[
                "Phase 5c", // Devil's Advocate (ADR-039)
                "Phase 6",  // F-xx Extraction
                "Acceptance Invariants",
                "challenge", // workflow includes challenge step
            ],
        ),
        (
            "skill-planner",
            &[
                "Phase 2b",    // Spec Stress Test (ADR-039)
                "Phase 3",     // Review
                "Stress Test", // adversarial stress test
            ],
        ),
        (
            "skill-execute",
            &[
                "Phase 2b", // Parallel Thinking Advisors (ADR-039)
                "Phase 3",  // CI Batch Fix
                "Parallel Thinking Advisors",
            ],
        ),
        (
            "skill-research",
            &["Alternatives Analysis", "Recommended Approach"],
        ),
    ];

    #[test]
    fn skill_prompts_have_required_sections() {
        for &(skill_name, required) in REQUIRED_SECTIONS {
            let body = SKILLS
                .iter()
                .find(|(name, _)| *name == skill_name)
                .unwrap_or_else(|| panic!("skill {skill_name} not found in SKILLS"))
                .1;
            for section in required {
                assert!(
                    body.contains(section),
                    "Skill '{skill_name}' missing required section: '{section}'"
                );
            }
        }
    }

    #[test]
    fn all_skills_have_activation_section() {
        for &(name, body) in SKILLS {
            let has_activation = body.contains("Activation")
                || body.contains("activation")
                || body.contains("## When to use")
                || body.contains("# /");
            assert!(
                has_activation,
                "Skill '{name}' missing Activation/When-to-use section"
            );
        }
    }

    #[test]
    fn adversarial_phases_present_in_workflow_skills() {
        let solve = SKILLS.iter().find(|(n, _)| *n == "skill-solve").unwrap().1;
        assert!(
            solve.contains("Devil's Advocate"),
            "solve must have Devil's Advocate phase (ADR-039)"
        );

        let planner = SKILLS
            .iter()
            .find(|(n, _)| *n == "skill-planner")
            .unwrap()
            .1;
        assert!(
            planner.contains("Stress Test"),
            "planner must have Spec Stress Test phase (ADR-039)"
        );

        let execute = SKILLS
            .iter()
            .find(|(n, _)| *n == "skill-execute")
            .unwrap()
            .1;
        assert!(
            execute.contains("Parallel Thinking Advisors"),
            "execute must have Parallel Thinking Advisors phase (ADR-039)"
        );
    }
}

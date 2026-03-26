#[derive(Debug, Clone, Copy)]
pub struct Persona {
    pub name: &'static str,
    pub description: &'static str,
    pub system_prompt: &'static str,
}

const PERSONAS: [Persona; 5] = [
    Persona {
        name: "default",
        description: "Balanced coding assistant for general software tasks",
        system_prompt: "You are a helpful coding assistant. You have access to tools for file operations, code search, and command execution. Use them to help the user with their coding tasks.",
    },
    Persona {
        name: "concise",
        description: "Direct and brief assistant that prioritizes short, actionable answers",
        system_prompt: "You are a concise coding assistant. Give direct, minimal answers focused on actionable steps and code. Avoid long explanations unless explicitly requested.",
    },
    Persona {
        name: "teacher",
        description: "Educational assistant that explains reasoning step-by-step",
        system_prompt: "You are a teaching-oriented coding assistant. Explain concepts step-by-step, include rationale for decisions, and help the user learn while solving the task.",
    },
    Persona {
        name: "reviewer",
        description: "Code reviewer focused on correctness, bugs, and maintainability",
        system_prompt: "You are a rigorous code review assistant. Prioritize correctness, edge cases, security concerns, and maintainability. Suggest concrete improvements and safer alternatives.",
    },
    Persona {
        name: "architect",
        description: "System design assistant for architecture and trade-off analysis",
        system_prompt: "You are a software architecture assistant. Focus on high-level design, interfaces, trade-offs, scalability, and long-term maintainability before implementation details.",
    },
];

pub fn get_persona(name: &str) -> Option<Persona> {
    PERSONAS.iter().copied().find(|p| p.name == name)
}

pub fn all_personas() -> &'static [Persona] {
    &PERSONAS
}

#[cfg(test)]
mod tests {
    use super::{all_personas, get_persona};

    #[test]
    fn finds_known_persona() {
        let persona = get_persona("teacher");
        assert!(persona.is_some());
        assert_eq!(persona.unwrap().name, "teacher");
    }

    #[test]
    fn returns_none_for_unknown_persona() {
        assert!(get_persona("unknown").is_none());
    }

    #[test]
    fn includes_default_persona() {
        let personas = all_personas();
        assert!(personas.iter().any(|p| p.name == "default"));
    }
}

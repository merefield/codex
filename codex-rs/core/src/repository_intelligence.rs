use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use codex_mcp::mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::mcp_connection_manager::ToolInfo;

pub const DEVELOPER_INSTRUCTIONS: &str = "When suitable repository-intelligence MCP tools are available, prefer them for project structure, code search, symbol lookup, and code context before falling back to shell traversal such as `rg`, `find`, `sed`, or `cat`. Use shell tools when MCP tools are unavailable or insufficient.";

const MAX_DISPLAY_TOOLS_PER_CAPABILITY: usize = 2;
const REPOSITORY_HINTS: &[&str] = &[
    "code",
    "repo",
    "repository",
    "workspace",
    "project",
    "source",
    "symbol",
];
const STRUCTURE_HINTS: &[&str] = &[
    "file",
    "files",
    "tree",
    "directory",
    "directories",
    "workspace structure",
    "project structure",
];
const SEARCH_HINTS: &[&str] = &["search", "query", "find", "symbol", "lookup"];
const CONTEXT_HINTS: &[&str] = &[
    "context",
    "understand",
    "understanding",
    "explain",
    "overview",
];
const RELATIONSHIP_HINTS: &[&str] = &[
    "caller",
    "callee",
    "impact",
    "dependency",
    "dependencies",
    "reference",
    "references",
    "graph",
];

#[derive(Debug, Clone, Copy)]
enum RepoCapability {
    Structure,
    Search,
    Context,
    Relationship,
}

#[derive(Debug, Default)]
struct RepoProviderCapabilities {
    structure_tools: BTreeSet<String>,
    search_tools: BTreeSet<String>,
    context_tools: BTreeSet<String>,
    relationship_tools: BTreeSet<String>,
}

impl RepoProviderCapabilities {
    fn insert(&mut self, capability: RepoCapability, qualified_tool_name: &str) {
        let target = match capability {
            RepoCapability::Structure => &mut self.structure_tools,
            RepoCapability::Search => &mut self.search_tools,
            RepoCapability::Context => &mut self.context_tools,
            RepoCapability::Relationship => &mut self.relationship_tools,
        };
        target.insert(qualified_tool_name.to_string());
    }

    fn is_coherent(&self) -> bool {
        !self.search_tools.is_empty() && !self.context_tools.is_empty()
    }
}

pub fn merge_developer_instructions(existing: Option<&str>, enabled: bool) -> Option<String> {
    let existing =
        strip_managed_developer_instructions(existing).filter(|text| !text.trim().is_empty());

    match (existing, enabled) {
        (None, false) => None,
        (Some(existing), false) => Some(existing),
        (None, true) => Some(DEVELOPER_INSTRUCTIONS.to_string()),
        (Some(existing), true) => Some(format!("{existing}\n\n{DEVELOPER_INSTRUCTIONS}")),
    }
}

pub fn is_enabled_in_developer_instructions(instructions: Option<&str>) -> bool {
    instructions.is_some_and(|instructions| instructions.contains(DEVELOPER_INSTRUCTIONS))
}

fn strip_managed_developer_instructions(existing: Option<&str>) -> Option<String> {
    let existing = existing?.trim();
    if existing.is_empty() {
        return None;
    }

    let stripped = existing
        .lines()
        .filter(|line| line.trim() != DEVELOPER_INSTRUCTIONS)
        .collect::<Vec<_>>()
        .join("\n");
    let stripped = stripped.trim();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

pub(crate) fn build_mcp_developer_instructions(
    mcp_tools: &HashMap<String, ToolInfo>,
) -> Option<String> {
    let providers = detect_repo_providers(mcp_tools);
    if providers.is_empty() {
        return None;
    }

    let mut lines = vec![
        "## Repository-Intelligence MCP Tools".to_string(),
        "Prefer these MCP tools before raw shell traversal when they fit the task:".to_string(),
    ];

    for (server_name, capabilities) in providers {
        let mut line = format!(
            "- On `{server_name}`, use {} for code search or symbol lookup and {} for task-oriented code context.",
            format_tool_list(&capabilities.search_tools),
            format_tool_list(&capabilities.context_tools),
        );
        if !capabilities.structure_tools.is_empty() {
            line.push_str(&format!(
                " Use {} for project structure when needed.",
                format_tool_list(&capabilities.structure_tools),
            ));
        }
        if !capabilities.relationship_tools.is_empty() {
            line.push_str(&format!(
                " Use {} for relationship or impact follow-up when needed.",
                format_tool_list(&capabilities.relationship_tools),
            ));
        }
        lines.push(line);
    }

    lines.push(
        "- Fall back to shell traversal when these MCP tools are unavailable or insufficient."
            .to_string(),
    );

    Some(lines.join("\n"))
}

fn detect_repo_providers(
    mcp_tools: &HashMap<String, ToolInfo>,
) -> BTreeMap<String, RepoProviderCapabilities> {
    let mut providers: BTreeMap<String, RepoProviderCapabilities> = BTreeMap::new();

    for (qualified_tool_name, tool_info) in mcp_tools {
        if tool_info.server_name == CODEX_APPS_MCP_SERVER_NAME {
            continue;
        }

        let Some(capabilities) = classify_tool(tool_info) else {
            continue;
        };

        let provider = providers.entry(tool_info.server_name.clone()).or_default();
        for capability in capabilities {
            provider.insert(capability, qualified_tool_name);
        }
    }

    providers.retain(|_, capabilities| capabilities.is_coherent());
    providers
}

fn classify_tool(tool_info: &ToolInfo) -> Option<Vec<RepoCapability>> {
    let description = tool_info.tool.description.as_deref().unwrap_or_default();
    let haystack = format!("{} {}", tool_info.tool_name, description).to_ascii_lowercase();

    if !contains_any(&haystack, REPOSITORY_HINTS) {
        return None;
    }

    let mut capabilities = Vec::new();
    if contains_any(&haystack, STRUCTURE_HINTS) {
        capabilities.push(RepoCapability::Structure);
    }
    if contains_any(&haystack, SEARCH_HINTS) {
        capabilities.push(RepoCapability::Search);
    }
    if contains_any(&haystack, CONTEXT_HINTS) {
        capabilities.push(RepoCapability::Context);
    }
    if contains_any(&haystack, RELATIONSHIP_HINTS) {
        capabilities.push(RepoCapability::Relationship);
    }

    if capabilities.is_empty() {
        None
    } else {
        Some(capabilities)
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn format_tool_list(tools: &BTreeSet<String>) -> String {
    let display = tools
        .iter()
        .take(MAX_DISPLAY_TOOLS_PER_CAPABILITY)
        .map(|tool_name| format!("`{tool_name}`"))
        .collect::<Vec<_>>();

    let hidden = tools.len().saturating_sub(display.len());
    let base = match display.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => display.join(", "),
    };

    if hidden == 0 {
        base
    } else if hidden == 1 {
        format!("{base}, and 1 more")
    } else {
        format!("{base}, and {hidden} more")
    }
}

#[cfg(test)]
mod tests {
    use super::DEVELOPER_INSTRUCTIONS;
    use super::build_mcp_developer_instructions;
    use super::is_enabled_in_developer_instructions;
    use super::merge_developer_instructions;
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use std::sync::Arc;

    fn test_tool(
        qualified_name: &str,
        server_name: &str,
        tool_name: &str,
        description: &str,
    ) -> (String, ToolInfo) {
        (
            qualified_name.to_string(),
            ToolInfo {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
                tool_namespace: format!("mcp__{server_name}__"),
                tool: Tool {
                    name: tool_name.to_string().into(),
                    title: None,
                    description: Some(description.to_string().into()),
                    input_schema: Arc::new(JsonObject::default()),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: None,
                connector_name: None,
                connector_description: None,
                plugin_display_names: Vec::new(),
            },
        )
    }

    #[test]
    fn returns_none_when_disabled_without_existing_instructions() {
        assert_eq!(merge_developer_instructions(None, false), None);
    }

    #[test]
    fn injects_repository_intelligence_instructions_when_enabled() {
        assert_eq!(
            merge_developer_instructions(None, true),
            Some(DEVELOPER_INSTRUCTIONS.to_string())
        );
    }

    #[test]
    fn appends_repository_intelligence_instructions_once() {
        assert_eq!(
            merge_developer_instructions(Some("Stay focused"), true),
            Some(format!("Stay focused\n\n{DEVELOPER_INSTRUCTIONS}"))
        );
        assert_eq!(
            merge_developer_instructions(Some(DEVELOPER_INSTRUCTIONS), true),
            Some(DEVELOPER_INSTRUCTIONS.to_string())
        );
    }

    #[test]
    fn removes_managed_repository_intelligence_instructions_when_disabled() {
        assert_eq!(
            merge_developer_instructions(Some(DEVELOPER_INSTRUCTIONS), false),
            None
        );
        assert_eq!(
            merge_developer_instructions(
                Some(&format!("Stay focused\n\n{DEVELOPER_INSTRUCTIONS}")),
                false,
            ),
            Some("Stay focused".to_string())
        );
    }

    #[test]
    fn detects_repository_intelligence_marker_in_developer_instructions() {
        assert!(is_enabled_in_developer_instructions(Some(
            DEVELOPER_INSTRUCTIONS
        )));
        assert!(!is_enabled_in_developer_instructions(Some(
            "Prefer shell traversal for repository search."
        )));
        assert!(!is_enabled_in_developer_instructions(None));
    }

    #[test]
    fn skips_tool_specific_instructions_without_a_coherent_provider() {
        let tools = HashMap::from([test_tool(
            "mcp__repo__repo_files",
            "repo",
            "repo_files",
            "List repository files",
        )]);

        assert_eq!(build_mcp_developer_instructions(&tools), None);
    }

    #[test]
    fn builds_tool_specific_instructions_from_detected_capabilities() {
        let tools = HashMap::from([
            test_tool(
                "mcp__repo__repo_files",
                "repo",
                "repo_files",
                "List repository files and project structure",
            ),
            test_tool(
                "mcp__repo__repo_search",
                "repo",
                "repo_search",
                "Search source code symbols in the repository",
            ),
            test_tool(
                "mcp__repo__repo_context",
                "repo",
                "repo_context",
                "Build task-oriented code context for the repository",
            ),
            test_tool(
                "mcp__repo__repo_impact",
                "repo",
                "repo_impact",
                "Analyze impact across source code dependencies",
            ),
        ]);

        let instructions =
            build_mcp_developer_instructions(&tools).expect("expected repo-intel instructions");

        assert!(instructions.contains("## Repository-Intelligence MCP Tools"));
        assert!(instructions.contains("`mcp__repo__repo_files`"));
        assert!(instructions.contains("`mcp__repo__repo_search`"));
        assert!(instructions.contains("`mcp__repo__repo_context`"));
        assert!(instructions.contains("`mcp__repo__repo_impact`"));
    }

    #[test]
    fn omits_structure_guidance_when_structure_tools_are_missing() {
        let tools = HashMap::from([
            test_tool(
                "mcp__repo__repo_search",
                "repo",
                "repo_search",
                "Search source code symbols in the repository",
            ),
            test_tool(
                "mcp__repo__repo_context",
                "repo",
                "repo_context",
                "Build task-oriented code context for the repository",
            ),
        ]);

        let instructions =
            build_mcp_developer_instructions(&tools).expect("expected repo-intel instructions");

        assert!(instructions.contains("`mcp__repo__repo_search`"));
        assert!(instructions.contains("`mcp__repo__repo_context`"));
        assert!(!instructions.contains("project structure"));
    }

    #[test]
    fn ignores_codex_apps_tools_for_repo_intelligence() {
        let tools = HashMap::from([
            test_tool(
                "mcp__codex_apps__repo_files",
                CODEX_APPS_MCP_SERVER_NAME,
                "repo_files",
                "List repository files and project structure",
            ),
            test_tool(
                "mcp__codex_apps__repo_search",
                CODEX_APPS_MCP_SERVER_NAME,
                "repo_search",
                "Search source code symbols in the repository",
            ),
            test_tool(
                "mcp__codex_apps__repo_context",
                CODEX_APPS_MCP_SERVER_NAME,
                "repo_context",
                "Build task-oriented code context for the repository",
            ),
        ]);

        assert_eq!(build_mcp_developer_instructions(&tools), None);
    }
}

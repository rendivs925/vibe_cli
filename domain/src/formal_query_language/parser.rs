//! FQL Parser - converts natural language to Formal Query Language
//!
//! Uses keyword-based pattern matching and heuristics to extract
//! structured intent from natural language queries.

use super::types::*;
use regex::Regex;

/// Parser for converting natural language to FQL
#[derive(Debug, Clone)]
pub struct FqlParser {
    action_patterns: Vec<(Regex, FqlAction)>,
}

impl FqlParser {
    /// Create a new FQL parser
    pub fn new() -> Self {
        Self {
            action_patterns: Self::compile_action_patterns(),
        }
    }

    /// Parse natural language query into FQL
    pub fn parse(&self, query: &str) -> Option<FqlQuery> {
        let query_lower = query.to_lowercase();

        // Extract action
        let action = self.extract_action(&query_lower)?;

        // Extract target
        let target = self.extract_target(&query_lower)?;

        // Build base query
        let mut fql = FqlQuery::new(action, target);

        // Extract optional pattern
        if let Some(pattern) = self.extract_pattern(&query_lower) {
            fql.pattern = Some(pattern);
        }

        // Extract constraints
        for constraint in self.extract_constraints(&query_lower) {
            fql.constraints.push(constraint);
        }

        // Extract scope
        if let Some(scope) = self.extract_scope(&query_lower) {
            fql.scope = scope;
        }

        // Extract modifiers
        for modifier in self.extract_modifiers(&query_lower) {
            fql.modifiers.push(modifier);
        }

        Some(fql)
    }

    fn compile_action_patterns() -> Vec<(Regex, FqlAction)> {
        vec![
            (Regex::new(r"show|display").unwrap(), FqlAction::Show),
            (
                Regex::new(r"create|make|new|add").unwrap(),
                FqlAction::Create,
            ),
            (
                Regex::new(r"list|view|print|get").unwrap(),
                FqlAction::List,
            ),
            (Regex::new(r"read|cat|less|more").unwrap(), FqlAction::Read),
            (
                Regex::new(r"update|modify|change|edit").unwrap(),
                FqlAction::Update,
            ),
            (
                Regex::new(r"delete|remove|rm|destroy|clean|clear").unwrap(),
                FqlAction::Delete,
            ),
            (Regex::new(r"purge|wipe|erase").unwrap(), FqlAction::Purge),
            (
                Regex::new(r"start|launch|begin|initiate").unwrap(),
                FqlAction::Start,
            ),
            (
                Regex::new(r"stop|end|terminate|kill|halt").unwrap(),
                FqlAction::Stop,
            ),
            (
                Regex::new(r"restart|reboot|reload").unwrap(),
                FqlAction::Restart,
            ),
            (Regex::new(r"enable|activate").unwrap(), FqlAction::Enable),
            (
                Regex::new(r"disable|deactivate").unwrap(),
                FqlAction::Disable,
            ),
            (Regex::new(r"check|monitor").unwrap(), FqlAction::Check),
            (Regex::new(r"verify|validate|test").unwrap(), FqlAction::Validate),
            (
                Regex::new(r"find|search|locate|look for|grep").unwrap(),
                FqlAction::Find,
            ),
            (Regex::new(r"copy|cp|duplicate").unwrap(), FqlAction::Copy),
            (
                Regex::new(r"move|mv|transfer|relocate").unwrap(),
                FqlAction::Move,
            ),
            (
                Regex::new(r"install|setup|deploy").unwrap(),
                FqlAction::Install,
            ),
            (Regex::new(r"uninstall").unwrap(), FqlAction::Uninstall),
        ]
    }

    fn extract_action(&self, query: &str) -> Option<FqlAction> {
        for (pattern, action) in &self.action_patterns {
            if pattern.is_match(query) {
                return Some(action.clone());
            }
        }
        Some(FqlAction::List)
    }

    fn extract_target(&self, query: &str) -> Option<FqlTarget> {
        // Check for file paths
        if let Some(cap) = Regex::new(r"(?:in|at|from|to) +(\S+)")
            .unwrap()
            .captures(query)
        {
            let path = cap.get(1).map(|m| m.as_str()).unwrap_or("/");
            if path.starts_with('/') || path.starts_with("~/") {
                return Some(FqlTarget::Path(path.to_string()));
            }
        }

        // Check for processes
        if let Some(cap) = Regex::new(r"process(?:es)? +(\w+)")
            .unwrap()
            .captures(query)
        {
            let token = cap[1].to_string();
            let token_lower = token.to_lowercase();
            if [
                "list",
                "listing",
                "show",
                "all",
                "running",
                "process",
                "processes",
                "last",
                "recent",
            ]
            .contains(&token_lower.as_str())
            {
                return Some(FqlTarget::Process("*".to_string()));
            }
            return Some(FqlTarget::Process(token));
        }
        if query.contains("running") && query.contains("process") {
            return Some(FqlTarget::Process("*".to_string()));
        }

        // Check for services
        if let Some(cap) = Regex::new(r"service(?:s)? +(\w+)").unwrap().captures(query) {
            return Some(FqlTarget::Service(cap[1].to_string()));
        }
        for service in &[
            "nginx", "apache", "mysql", "postgres", "redis", "docker", "ssh",
        ] {
            if query.contains(service) {
                return Some(FqlTarget::Service(service.to_string()));
            }
        }

        // Check for system resources
        if query.contains("memory") || query.contains("ram") {
            return Some(FqlTarget::Memory);
        }
        if query.contains("cpu") || query.contains("processor") {
            return Some(FqlTarget::Cpu);
        }
        if query.contains("disk") || query.contains("space") {
            return Some(FqlTarget::Disk("*".to_string()));
        }

        // Check for GPU/graphics hardware
        if Regex::new(r"gpu|graphics card|graphics|video card|vga|nvidia|radeon|amd gpu")
            .unwrap()
            .is_match(query)
        {
            return Some(FqlTarget::Component("gpu".to_string()));
        }

        if query.contains("hardware") || query.contains("device") {
            return Some(FqlTarget::Resource("hardware".to_string()));
        }

        // Check for logs
        if query.contains("log")
            || query.contains("logs")
            || query.contains("journalctl")
            || query.contains("syslog")
            || query.contains("dmesg")
            || query.contains("/var/log")
            || query.contains("messages")
        {
            return Some(FqlTarget::Log("*".to_string()));
        }

        // Check for packages
        if query.contains("package") || query.contains("apt") || query.contains("yum") {
            return Some(FqlTarget::Package("*".to_string()));
        }

        // Check for users
        if query.contains("user") || query.contains("users") {
            return Some(FqlTarget::User("*".to_string()));
        }

        Some(FqlTarget::Resource("system".to_string()))
    }

    fn extract_pattern(&self, query: &str) -> Option<FqlPattern> {
        // Glob patterns
        if let Some(cap) = Regex::new(r"(\*+\.\w+)").unwrap().captures(query) {
            return Some(FqlPattern::Glob(cap[1].to_string()));
        }

        // Named/Called
        if let Some(cap) = Regex::new(r"(?:named|called) +([a-z0-9_\-\.]+)")
            .unwrap()
            .captures(query)
        {
            return Some(FqlPattern::Name(cap[1].to_string()));
        }

        // Extensions
        if let Some(cap) = Regex::new(r"\.(\w+) +files?").unwrap().captures(query) {
            return Some(FqlPattern::Extension(cap[1].to_string()));
        }

        // Older than
        if let Some(cap) = Regex::new(r"older than +(\d+ +(days?|hours?|minutes?))")
            .unwrap()
            .captures(query)
        {
            return Some(FqlPattern::OlderThan(cap[1].to_string()));
        }

        // Larger than
        if let Some(cap) = Regex::new(r"larger than +(\d+(?:\.\d+)?\s*[kmgt]?b)")
            .unwrap()
            .captures(query)
        {
            return Some(FqlPattern::LargerThan(cap[1].replace(' ', "")));
        }

        None
    }

    fn extract_constraints(&self, query: &str) -> Vec<FqlConstraint> {
        let mut constraints = vec![];

        if query.contains("safe") || query.contains("safely") || query.contains("careful") {
            constraints.push(FqlConstraint::SafeDelete);
        }

        if query.contains("dry run") || query.contains("pretend") || query.contains("simulate") {
            constraints.push(FqlConstraint::DryRun);
        }

        if query.contains("root") || query.contains("sudo") || query.contains("admin") {
            constraints.push(FqlConstraint::RequiresRoot);
        }

        if query.contains("recursive") || query.contains("recursively") || query.contains("-r") {
            constraints.push(FqlConstraint::Recursive(true));
        }

        if query.contains("force") || query.contains("-f") {
            constraints.push(FqlConstraint::Force(true));
        }

        if query.contains("interactive") || query.contains("-i") || query.contains("confirm") {
            constraints.push(FqlConstraint::Interactive);
        }

        if query.contains("verbose") || query.contains("-v") {
            constraints.push(FqlConstraint::Verbose);
        }

        if query.contains("quiet") || query.contains("-q") {
            constraints.push(FqlConstraint::Quiet);
        }

        if let Some(limit) = self.extract_limit(query) {
            constraints.push(FqlConstraint::Limit(limit));
        }

        constraints
    }

    fn extract_limit(&self, query: &str) -> Option<u64> {
        let patterns = [
            r"(?:last|tail|recent|past|previous)\s+(\d+)\s+lines?",
            r"-n\s*(\d+)",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(query) {
                    if let Ok(val) = cap[1].parse::<u64>() {
                        return Some(val);
                    }
                }
            }
        }

        None
    }

    fn extract_scope(&self, query: &str) -> Option<FqlScope> {
        if query.contains("recursive") || query.contains("recursively") || query.contains("all") {
            Some(FqlScope::Recursive)
        } else if query.contains("everything") || query.contains("entire") {
            Some(FqlScope::All)
        } else {
            None
        }
    }

    fn extract_modifiers(&self, query: &str) -> Vec<FqlModifier> {
        let mut modifiers = vec![];

        if query.contains("dry run") || query.contains("pretend") {
            modifiers.push(FqlModifier::DryRun);
        }

        if query.contains("quiet") || query.contains("-q") {
            modifiers.push(FqlModifier::Quiet);
        }

        if query.contains("verbose") || query.contains("-v") {
            modifiers.push(FqlModifier::Verbose);
        }

        if query.contains("json") {
            modifiers.push(FqlModifier::Json);
        }

        if query.contains("parallel") {
            modifiers.push(FqlModifier::Parallel);
        }

        modifiers
    }

    /// Get confidence score for a parse (0.0 to 1.0)
    pub fn confidence_score(&self, query: &str, fql: &FqlQuery) -> f32 {
        let mut score = 0.5; // Base score

        // Check action confidence
        if fql.action != FqlAction::List
            || Regex::new(r"list|show|display|view|get")
                .unwrap()
                .is_match(&query.to_lowercase())
        {
            score += 0.2;
        }

        // Check target confidence
        if !matches!(fql.target, FqlTarget::Resource(_)) {
            score += 0.2;
        }

        // Pattern match bonus
        if fql.pattern.is_some() {
            score += 0.1;
        }

        if score > 1.0 {
            1.0_f32
        } else {
            score
        }
    }
}

impl Default for FqlParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_processes() {
        let parser = FqlParser::new();
        let query = parser.parse("list all running processes").unwrap();

        assert!(matches!(query.action, FqlAction::List));
        assert!(matches!(query.target, FqlTarget::Process(_)));
    }

    #[test]
    fn test_parse_clean_logs() {
        let parser = FqlParser::new();
        let query = parser.parse("clean old logs in /var/log safely").unwrap();

        assert!(matches!(query.action, FqlAction::Delete));
        // Target can be either Log or Path (both are valid interpretations)
        assert!(
            matches!(query.target, FqlTarget::Log(_))
                || matches!(query.target, FqlTarget::Path(_))
                || matches!(query.target, FqlTarget::Directory(_))
        );
        assert!(query
            .constraints
            .iter()
            .any(|c| matches!(c, FqlConstraint::SafeDelete)));
    }

    #[test]
    fn test_parse_delete_files() {
        let parser = FqlParser::new();
        let query = parser
            .parse("delete all .tmp files in /tmp recursively")
            .unwrap();

        assert!(matches!(query.action, FqlAction::Delete));
        assert!(matches!(query.target, FqlTarget::Path(_)));
        assert!(query.pattern.is_some());
        assert_eq!(query.scope, FqlScope::Recursive);
    }

    #[test]
    fn test_parse_check_service() {
        let parser = FqlParser::new();
        let query = parser.parse("check if nginx is running").unwrap();

        assert!(matches!(query.action, FqlAction::Check));
        assert!(matches!(query.target, FqlTarget::Service(ref s) if s == "nginx"));
    }

    #[test]
    fn test_parse_gpu_query() {
        let parser = FqlParser::new();
        let query = parser.parse("show my gpu name").unwrap();

        assert!(matches!(query.action, FqlAction::Show));
        assert!(matches!(query.target, FqlTarget::Component(ref s) if s == "gpu"));
    }

    #[test]
    fn test_parse_journalctl_lines() {
        let parser = FqlParser::new();
        let query = parser.parse("check last 20 lines journalctl").unwrap();

        assert!(matches!(query.target, FqlTarget::Log(_)));
        assert!(query
            .constraints
            .iter()
            .any(|c| matches!(c, FqlConstraint::Limit(20))));
    }

    #[test]
    fn test_confidence_score() {
        let parser = FqlParser::new();
        let fql = parser.parse("list processes").unwrap();
        let score = parser.confidence_score("list processes", &fql);

        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_fql_string_output() {
        let parser = FqlParser::new();
        let query = parser.parse("delete old files in /tmp safely").unwrap();
        let fql_str = query.to_fql_string();

        assert!(fql_str.contains("ACTION(delete)"));
        assert!(fql_str.contains("CONSTRAINT(safe_delete)"));
    }
}

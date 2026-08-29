//! `treebank status` — one inventory of configuration, evidence and live state.
//!
//! The repository already has authoritative sources for each individual fact:
//! the language registry, tree-sitter manifests, roles, ledgers, fixtures,
//! known-deviation declarations and workflows. What it lacked was the join.
//! This module deliberately reads those sources rather than introducing a
//! second configuration file.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use treebank_lang::LangName;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Markdown,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub revision: Option<String>,
    pub summary: Summary,
    pub grammars: Vec<GrammarStatus>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GitHubStatus>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub languages: usize,
    pub grammars: usize,
    pub corpus_locks: usize,
    pub corpus_canaries: usize,
    pub current_corpus_evidence: usize,
    pub stale_corpus_evidence: usize,
    pub unbound_corpus_evidence: usize,
    pub lint_ratchets: usize,
    pub shape_fixture_grammars: usize,
    pub wasm_packs: usize,
    pub query_files: usize,
    pub non_rust_binding_grammars: usize,
}

#[derive(Debug, Serialize)]
pub struct GrammarStatus {
    pub grammar: String,
    pub languages: Vec<LanguageStatus>,
    pub versions: String,
    pub generate_cli: Option<String>,
    pub vocabulary: Option<String>,
    pub manifest: ManifestStatus,
    pub capabilities: CapabilitiesStatus,
    pub roles: RolesStatus,
    pub evidence: EvidenceStatus,
    pub tests: TestStatus,
    pub known_deviations: KnownDeviationStatus,
    pub distribution: DistributionStatus,
    pub external_scanner: bool,
    pub corpus_lock: bool,
    pub corpus_canary: bool,
    pub evidence_freshness: EvidenceFreshness,
}

#[derive(Debug, Serialize)]
pub struct LanguageStatus {
    pub name: String,
    pub extensions: Vec<String>,
    pub rosetta: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct ManifestStatus {
    pub scope: Option<String>,
    pub file_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CapabilitiesStatus {
    pub spans: bool,
    pub formatter: bool,
    pub printer: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct RolesStatus {
    pub supertypes: usize,
    pub facets: usize,
    pub named_nodes: usize,
    pub uncategorised: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct EvidenceStatus {
    pub corpora: Vec<CorpusStatus>,
    pub measurements: Vec<String>,
    pub known_gaps: Vec<DeclaredItem>,
    pub known_widenings: Vec<DeclaredItem>,
    pub deviations: Vec<DeclaredItem>,
    pub configuration_files: Option<u64>,
    pub indeterminate_files: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CorpusStatus {
    pub language: String,
    pub files: u64,
    pub passed: u64,
    pub failed: u64,
    pub grammar_gaps: u64,
    pub noise: u64,
    pub pass_rate: Option<String>,
    pub corpus_lock_sha256: Option<String>,
    pub grammar_sha256: Option<String>,
    pub grammar_revision: Option<String>,
    pub freshness: EvidenceFreshness,
    pub freshness_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFreshness {
    Current,
    Stale,
    Unbound,
}

#[derive(Debug, Serialize)]
pub struct DeclaredItem {
    pub summary: String,
    pub files: Option<u64>,
}

#[derive(Debug, Default, Serialize)]
pub struct TestStatus {
    pub corpus_cases: usize,
    pub negative_files: usize,
    pub shape_files: usize,
}

#[derive(Debug, Serialize)]
pub struct KnownDeviationStatus {
    pub shape: bool,
    pub fuzz: bool,
    pub lint: bool,
    pub version: bool,
}

#[derive(Debug, Serialize)]
pub struct DistributionStatus {
    pub bindings: Vec<String>,
    pub wasm_pack: bool,
    pub query_files: usize,
}

#[derive(Debug, Serialize)]
pub struct GitHubStatus {
    pub repository: String,
    pub default_branch: String,
    pub branch_protected: Option<bool>,
    pub workflows: Vec<WorkflowStatus>,
    pub open_issues: Vec<GitHubItem>,
    pub open_pull_requests: Vec<GitHubItem>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowStatus {
    pub name: String,
    pub state: String,
    pub latest: Option<WorkflowRun>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub status: String,
    pub conclusion: Option<String>,
    pub event: String,
    pub run_started_at: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitHubItem {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    full_name: String,
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    html_url: String,
    #[serde(default)]
    labels: Vec<LabelResponse>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LabelResponse {
    name: String,
}

#[derive(Debug, Deserialize)]
struct WorkflowsResponse {
    workflows: Vec<WorkflowResponse>,
}

#[derive(Debug, Deserialize)]
struct WorkflowResponse {
    id: u64,
    name: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct RunsResponse {
    workflow_runs: Vec<WorkflowRun>,
}

pub fn run(
    root: &Path,
    format: OutputFormat,
    github: bool,
    repository: Option<&str>,
    check: bool,
) -> Result<()> {
    let mut report = collect(root)?;
    let mut github_failed = false;
    if github {
        match collect_github(root, repository) {
            Ok(status) => report.github = Some(status),
            Err(error) => {
                github_failed = true;
                report
                    .errors
                    .push(format!("GitHub status unavailable: {error:#}"));
            }
        }
    }

    match format {
        OutputFormat::Table => print!("{}", render_table(&report)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Markdown => print!("{}", render_markdown(&report)),
    }

    if github_failed {
        bail!("status: live GitHub state was requested but unavailable");
    }
    if check && !report.errors.is_empty() {
        bail!("status: {} configuration error(s)", report.errors.len());
    }
    Ok(())
}

pub fn collect(root: &Path) -> Result<Report> {
    let crates_dir = root.join("crates");
    let mut errors = Vec::new();
    let mut grammars = Vec::new();
    let locked_languages = validate_corpus_locks(root, &mut errors);

    let grammar_languages: Vec<LangName> = LangName::ALL
        .iter()
        .copied()
        .filter(|lang| lang.grammar() == *lang)
        .collect();
    let expected: BTreeSet<String> = grammar_languages
        .iter()
        .map(|lang| lang.grammar_crate())
        .collect();

    if let Ok(entries) = std::fs::read_dir(&crates_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("grammar.js").is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !expected.contains(&name) {
                    errors.push(format!(
                        "{} has grammar.js but is absent from the language registry",
                        path.display()
                    ));
                }
            }
        }
    } else {
        errors.push(format!("cannot read {}", crates_dir.display()));
    }

    for grammar_lang in grammar_languages {
        let grammar = grammar_lang.as_str().to_string();
        let dir = crates_dir.join(grammar_lang.grammar_crate());
        let languages: Vec<LangName> = LangName::ALL
            .iter()
            .copied()
            .filter(|lang| lang.grammar() == grammar_lang)
            .collect();
        let language_status = languages
            .iter()
            .map(|lang| LanguageStatus {
                name: lang.as_str().to_string(),
                extensions: lang.extensions().iter().map(|x| x.to_string()).collect(),
                rosetta: lang.rosetta(),
            })
            .collect();

        for required in [
            "grammar.js",
            "tree-sitter.json",
            "roles.json",
            "ledger.toml",
            "src/grammar.json",
            "src/node-types.json",
            "src/parser.c",
        ] {
            if !dir.join(required).is_file() {
                errors.push(format!("{}: missing {required}", dir.display()));
            }
        }
        for required in ["test/corpus", "test/negative"] {
            if !dir.join(required).is_dir() {
                errors.push(format!("{}: missing {required}/", dir.display()));
            }
        }

        let ledger = read_toml(&dir.join("ledger.toml"), &mut errors);
        let ledger_language = ledger.get("language").and_then(toml::Value::as_str);
        if ledger_language != Some(grammar_lang.as_str()) {
            errors.push(format!(
                "{}: ledger language is {:?}, expected {}",
                dir.display(),
                ledger_language,
                grammar_lang
            ));
        }
        let versions = ledger
            .get("versions")
            .and_then(toml::Value::as_str)
            .map(one_line)
            .unwrap_or_else(|| "not declared".to_string());
        let generate_cli = ledger
            .get("generate_cli")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
        if generate_cli.is_none() {
            errors.push(format!("{}: ledger has no generate_cli", dir.display()));
        }
        let vocabulary = ledger
            .get("vocabulary")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
        let expected_vocabulary = treebank::vocabulary().version.clone();
        if vocabulary.as_deref() != Some(expected_vocabulary.as_str()) {
            errors.push(format!(
                "{}: ledger vocabulary is {:?}, expected {}",
                dir.display(),
                vocabulary,
                expected_vocabulary
            ));
        }

        let manifest = read_manifest(&dir.join("tree-sitter.json"), &languages, &mut errors);
        let roles = read_roles(&dir, &mut errors);
        let grammar_sha256 = match crate::grammar::source_sha256(&dir) {
            Ok(sha256) => sha256,
            Err(error) => {
                errors.push(format!(
                    "{}: cannot fingerprint grammar: {error:#}",
                    dir.display()
                ));
                String::new()
            }
        };
        let mut evidence = read_evidence(&ledger, grammar_lang.as_str());
        bind_evidence(
            &mut evidence,
            &grammar_sha256,
            &locked_languages,
            &dir.join("ledger.toml"),
            &mut errors,
        );
        if evidence.corpora.is_empty() {
            errors.push(format!(
                "{}: ledger has no corpus sweep evidence",
                dir.display()
            ));
        }

        let tests = TestStatus {
            corpus_cases: count_corpus_cases(&dir.join("test/corpus")),
            negative_files: count_files(&dir.join("test/negative")),
            shape_files: count_files(&dir.join("test/shape")),
        };
        if tests.corpus_cases == 0 {
            errors.push(format!("{}: corpus test suite is empty", dir.display()));
        }
        if tests.negative_files == 0 {
            errors.push(format!("{}: negative test suite is empty", dir.display()));
        }

        let configured: Vec<treebank_oracle::CapabilityFlags> = languages
            .iter()
            .map(|lang| treebank_oracle::capabilities_for(*lang))
            .collect();
        let capabilities = CapabilitiesStatus {
            spans: configured.iter().any(|caps| caps.spans),
            formatter: configured.iter().any(|caps| caps.formatter),
            printer: configured.iter().any(|caps| caps.printer),
        };
        let corpus_lock = languages
            .iter()
            .all(|lang| locked_languages.contains_key(lang.as_str()));
        let corpus_canary = languages.iter().any(|lang| has_canary(root, lang.as_str()));
        let evidence_freshness = aggregate_freshness(&evidence);

        grammars.push(GrammarStatus {
            grammar,
            languages: language_status,
            versions,
            generate_cli,
            vocabulary,
            manifest,
            capabilities,
            roles,
            evidence,
            tests,
            known_deviations: KnownDeviationStatus {
                shape: dir.join("shape_policy.toml").is_file(),
                fuzz: dir.join("fuzz_policy.toml").is_file(),
                lint: dir.join("lint_policy.toml").is_file(),
                version: dir.join("version_policy.toml").is_file(),
            },
            distribution: DistributionStatus {
                bindings: child_directories(&dir.join("bindings")),
                wasm_pack: root.join("tools/wasm-pack/list-grammars.sh").is_file(),
                query_files: count_extension(&dir.join("queries"), "scm"),
            },
            external_scanner: dir.join("src/scanner.c").is_file(),
            corpus_lock,
            corpus_canary,
            evidence_freshness,
        });
    }

    grammars.sort_by(|a, b| a.grammar.cmp(&b.grammar));
    let mut scopes: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for grammar in &grammars {
        if let Some(scope) = grammar.manifest.scope.as_deref() {
            scopes.entry(scope).or_default().push(&grammar.grammar);
        }
    }
    for (scope, owners) in scopes {
        if owners.len() > 1 {
            errors.push(format!(
                "tree-sitter scope {scope:?} is claimed by multiple grammars: {}",
                owners.join(", ")
            ));
        }
    }
    let summary = Summary {
        languages: LangName::ALL.len(),
        grammars: grammars.len(),
        corpus_locks: locked_languages.len(),
        corpus_canaries: grammars.iter().filter(|g| g.corpus_canary).count(),
        current_corpus_evidence: count_freshness(&grammars, EvidenceFreshness::Current),
        stale_corpus_evidence: count_freshness(&grammars, EvidenceFreshness::Stale),
        unbound_corpus_evidence: count_freshness(&grammars, EvidenceFreshness::Unbound),
        lint_ratchets: grammars.iter().filter(|g| g.known_deviations.lint).count(),
        shape_fixture_grammars: grammars.iter().filter(|g| g.tests.shape_files > 0).count(),
        wasm_packs: grammars.iter().filter(|g| g.distribution.wasm_pack).count(),
        query_files: grammars.iter().map(|g| g.distribution.query_files).sum(),
        non_rust_binding_grammars: grammars
            .iter()
            .filter(|g| g.distribution.bindings.iter().any(|name| name != "rust"))
            .count(),
    };

    let mut warnings = Vec::new();
    let unlocked = LangName::ALL.len().saturating_sub(summary.corpus_locks);
    if unlocked > 0 {
        warnings.push(format!(
            "{unlocked} of {} language corpora have no valid committed lock; their inputs are not mechanically reproducible",
            LangName::ALL.len()
        ));
    }
    let unratcheted = summary.grammars.saturating_sub(summary.lint_ratchets);
    if unratcheted > 0 {
        warnings.push(format!(
            "{unratcheted} of {} grammars run lint in advisory mode without lint_policy.toml",
            summary.grammars
        ));
    }
    if summary.query_files == 0 {
        warnings.push("no tree-sitter query files are configured".to_string());
    }
    if summary.non_rust_binding_grammars == 0 {
        warnings.push("no grammar has a binding directory beyond Rust".to_string());
    }
    let missing_shape: Vec<&str> = grammars
        .iter()
        .filter(|g| g.capabilities.spans && g.tests.shape_files == 0)
        .map(|g| g.grammar.as_str())
        .collect();
    if !missing_shape.is_empty() {
        warnings.push(format!(
            "span oracle configured but no checked-in shape fixtures: {}",
            missing_shape.join(", ")
        ));
    }
    let corpus_measurements: usize = grammars.iter().map(|g| g.evidence.corpora.len()).sum();
    if summary.unbound_corpus_evidence > 0 {
        warnings.push(format!(
            "{} of {corpus_measurements} corpus measurements are unbound; rerun their locked sweep to record corpus and grammar provenance",
            summary.unbound_corpus_evidence
        ));
    }
    for grammar in &grammars {
        for corpus in &grammar.evidence.corpora {
            if corpus.freshness == EvidenceFreshness::Stale {
                warnings.push(format!(
                    "{} corpus evidence is stale: {}",
                    corpus.language,
                    corpus.freshness_reasons.join("; ")
                ));
            }
        }
    }
    for g in &grammars {
        if g.corpus_lock && !g.corpus_canary {
            warnings.push(format!(
                "{} has a corpus lock but no configured canary",
                g.grammar
            ));
        }
    }

    Ok(Report {
        schema_version: 1,
        revision: git_revision(root),
        summary,
        grammars,
        warnings,
        errors,
        github: None,
    })
}

fn count_freshness(grammars: &[GrammarStatus], wanted: EvidenceFreshness) -> usize {
    grammars
        .iter()
        .flat_map(|grammar| &grammar.evidence.corpora)
        .filter(|corpus| corpus.freshness == wanted)
        .count()
}

fn aggregate_freshness(evidence: &EvidenceStatus) -> EvidenceFreshness {
    if evidence
        .corpora
        .iter()
        .any(|corpus| corpus.freshness == EvidenceFreshness::Stale)
    {
        EvidenceFreshness::Stale
    } else if evidence.corpora.is_empty()
        || evidence
            .corpora
            .iter()
            .any(|corpus| corpus.freshness == EvidenceFreshness::Unbound)
    {
        EvidenceFreshness::Unbound
    } else {
        EvidenceFreshness::Current
    }
}

fn bind_evidence(
    evidence: &mut EvidenceStatus,
    current_grammar_sha256: &str,
    locks: &BTreeMap<String, String>,
    ledger_path: &Path,
    errors: &mut Vec<String>,
) {
    for corpus in &mut evidence.corpora {
        for (field, value, valid) in [
            (
                "corpus_lock_sha256",
                corpus.corpus_lock_sha256.as_deref(),
                corpus.corpus_lock_sha256.as_deref().is_none_or(is_sha256),
            ),
            (
                "grammar_sha256",
                corpus.grammar_sha256.as_deref(),
                corpus.grammar_sha256.as_deref().is_none_or(is_sha256),
            ),
            (
                "grammar_revision",
                corpus.grammar_revision.as_deref(),
                corpus
                    .grammar_revision
                    .as_deref()
                    .is_none_or(is_git_revision),
            ),
        ] {
            if !valid {
                errors.push(format!(
                    "{}: {} sweep has invalid {field} {:?}",
                    ledger_path.display(),
                    corpus.language,
                    value
                ));
            }
        }

        if corpus.corpus_lock_sha256.is_none() {
            corpus
                .freshness_reasons
                .push("no corpus lock SHA-256 recorded".to_string());
        }
        if corpus.grammar_sha256.is_none() {
            corpus
                .freshness_reasons
                .push("no grammar SHA-256 recorded".to_string());
        }
        if corpus.grammar_revision.is_none() {
            corpus
                .freshness_reasons
                .push("no committed grammar revision recorded".to_string());
        }
        if !locks.contains_key(&corpus.language) {
            corpus
                .freshness_reasons
                .push("no valid committed corpus lock".to_string());
        }
        let (Some(recorded_lock), Some(recorded_grammar), Some(current_lock), Some(_)) = (
            corpus.corpus_lock_sha256.as_deref(),
            corpus.grammar_sha256.as_deref(),
            locks.get(&corpus.language),
            corpus.grammar_revision.as_deref(),
        ) else {
            continue;
        };

        if recorded_lock != current_lock {
            corpus.freshness_reasons.push(format!(
                "corpus lock changed (recorded {}, current {})",
                short_hash(recorded_lock),
                short_hash(current_lock)
            ));
        }
        if recorded_grammar != current_grammar_sha256 {
            corpus.freshness_reasons.push(format!(
                "grammar changed (recorded {}, current {})",
                short_hash(recorded_grammar),
                short_hash(current_grammar_sha256)
            ));
        }
        corpus.freshness = if corpus.freshness_reasons.is_empty() {
            EvidenceFreshness::Current
        } else {
            EvidenceFreshness::Stale
        };
    }
}

fn is_git_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn short_hash(value: &str) -> &str {
    &value[..value.len().min(12)]
}

fn validate_corpus_locks(root: &Path, errors: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut valid = BTreeMap::new();
    for language in LangName::ALL {
        let path = root
            .join("corpus-locks")
            .join(format!("{}.json", language.as_str()));
        if !path.is_file() {
            continue;
        }
        let (manifest, lock_sha256) =
            match treebank_corpus::fetch::Manifest::load_with_sha256(&path) {
                Ok(loaded) => loaded,
                Err(error) => {
                    errors.push(format!(
                        "{}: invalid corpus lock: {error:#}",
                        path.display()
                    ));
                    continue;
                }
            };
        let mut lock_errors = Vec::new();
        if manifest.language.as_deref() != Some(language.as_str()) {
            lock_errors.push(format!(
                "language is {:?}, expected {}",
                manifest.language, language
            ));
        }
        if manifest.packages.is_empty() {
            lock_errors.push("contains no packages".to_string());
        }
        let mut file_paths = BTreeSet::new();
        for package in &manifest.packages {
            let package_name = treebank_corpus::fetch::pkg_dir(&package.package, &package.version);
            let Some(artifact) = &package.artifact else {
                lock_errors.push(format!("{package_name} has no archive provenance"));
                continue;
            };
            if artifact.url.is_empty() || artifact.bytes == 0 || !is_sha256(&artifact.sha256) {
                lock_errors.push(format!("{package_name} has invalid archive provenance"));
            }
            for file in &package.files {
                let identity = format!("{package_name}/{}", file.path);
                if file.path.is_empty() || !is_sha256(&file.sha256) {
                    lock_errors.push(format!("{identity} has invalid file provenance"));
                }
                if !file_paths.insert(identity.clone()) {
                    lock_errors.push(format!("duplicate file identity {identity}"));
                }
            }
        }
        if lock_errors.is_empty() {
            valid.insert(language.as_str().to_string(), lock_sha256);
        } else {
            errors.push(format!(
                "{}: invalid corpus lock: {}",
                path.display(),
                lock_errors.join("; ")
            ));
        }
    }
    valid
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read_toml(path: &Path, errors: &mut Vec<String>) -> toml::Value {
    match std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))
        .and_then(|text| toml::from_str(&text).with_context(|| format!("parse {}", path.display())))
    {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("{error:#}"));
            toml::Value::Table(toml::Table::new())
        }
    }
}

fn read_manifest(path: &Path, languages: &[LangName], errors: &mut Vec<String>) -> ManifestStatus {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("read {}: {error}", path.display()));
            return ManifestStatus::default();
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("parse {}: {error}", path.display()));
            return ManifestStatus::default();
        }
    };
    let Some(grammar) = value
        .get("grammars")
        .and_then(serde_json::Value::as_array)
        .and_then(|xs| xs.first())
    else {
        errors.push(format!("{}: no grammars[0]", path.display()));
        return ManifestStatus::default();
    };
    let scope = grammar
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut file_types: Vec<String> = grammar
        .get("file-types")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect();
    file_types.sort();
    file_types.dedup();
    let expected: BTreeSet<&str> = languages
        .iter()
        .flat_map(|lang| lang.extensions().iter().copied())
        .collect();
    let claimed: BTreeSet<&str> = file_types.iter().map(String::as_str).collect();
    let missing: Vec<&str> = expected.difference(&claimed).copied().collect();
    if !missing.is_empty() {
        errors.push(format!(
            "{}: missing registered file types {}",
            path.display(),
            missing.join(", ")
        ));
    }
    ManifestStatus { scope, file_types }
}

fn read_roles(dir: &Path, errors: &mut Vec<String>) -> RolesStatus {
    let roles = match treebank::roles::RolesManifest::load(&dir.join("roles.json")) {
        Ok(roles) => roles,
        Err(error) => {
            errors.push(format!("{}: {error:#}", dir.display()));
            return RolesStatus::default();
        }
    };
    let nodes = match treebank::node_types::NodeTypes::load(&dir.join("src/node-types.json")) {
        Ok(nodes) => nodes,
        Err(error) => {
            errors.push(format!("{}: {error:#}", dir.display()));
            return RolesStatus::default();
        }
    };
    if let Err(error) = crate::roles_check(dir) {
        errors.push(format!("{}: roles: {error:#}", dir.display()));
    }
    RolesStatus {
        supertypes: nodes.supertypes.len(),
        facets: roles.facets.len(),
        named_nodes: nodes.named.len().saturating_sub(nodes.supertypes.len()),
        uncategorised: roles.uncategorised.len(),
    }
}

fn read_evidence(ledger: &toml::Value, grammar: &str) -> EvidenceStatus {
    let mut measurements = vec!["sweep".to_string()];
    for (name, key) in [
        ("shape", "shape_check"),
        ("mutation", "mutation_check"),
        ("errors", "error_positions"),
        ("fuzz", "fuzz_check"),
        ("incremental", "incremental_check"),
        ("recovery", "recovery_check"),
        ("roundtrip", "roundtrip_check"),
        ("reformat", "reformat_check"),
        ("kinds", "kinds_check"),
        ("lexical", "lexical_check"),
    ] {
        if ledger.get(key).is_some() {
            measurements.push(name.to_string());
        }
    }
    let mut evidence = EvidenceStatus {
        measurements,
        known_gaps: declared_items(ledger.get("known_gaps")),
        known_widenings: declared_items(ledger.get("known_widenings")),
        deviations: declared_items(ledger.get("deviations")),
        ..EvidenceStatus::default()
    };
    let Some(corpus) = ledger.get("corpus").and_then(toml::Value::as_table) else {
        return evidence;
    };
    if let Some(gaps) = corpus.get("gaps").and_then(toml::Value::as_table) {
        evidence.configuration_files = integer(gaps.get("config_files"));
        evidence.indeterminate_files = integer(gaps.get("indeterminate_files"));
    }
    for name in corpus.keys() {
        if !matches!(name.as_str(), "sweep" | "gaps")
            && !name.ends_with("_sweep")
            && !evidence.measurements.iter().any(|item| item == name)
        {
            evidence.measurements.push(name.clone());
        }
    }
    evidence.measurements.sort();
    for (name, value) in corpus {
        let Some(table) = value.as_table() else {
            continue;
        };
        if !name.ends_with("sweep") || !table.contains_key("files") {
            continue;
        }
        let language = if name == "sweep" {
            grammar.to_string()
        } else {
            name.strip_suffix("_sweep").unwrap_or(name).to_string()
        };
        evidence.corpora.push(CorpusStatus {
            language,
            files: integer(table.get("files")).unwrap_or(0),
            passed: integer(table.get("passed")).unwrap_or(0),
            failed: integer(table.get("failed")).unwrap_or(0),
            grammar_gaps: integer(table.get("gap_files")).unwrap_or(0),
            noise: integer(table.get("noise_files")).unwrap_or(0),
            pass_rate: table
                .get("pass_rate")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            corpus_lock_sha256: table
                .get("corpus_lock_sha256")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            grammar_sha256: table
                .get("grammar_sha256")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            grammar_revision: table
                .get("grammar_revision")
                .and_then(toml::Value::as_str)
                .map(str::to_string),
            freshness: EvidenceFreshness::Unbound,
            freshness_reasons: Vec::new(),
        });
    }
    evidence.corpora.sort_by(|a, b| a.language.cmp(&b.language));
    evidence
}

fn declared_items(value: Option<&toml::Value>) -> Vec<DeclaredItem> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .map(|table| {
            let summary = ["construct", "what", "signature", "name"]
                .iter()
                .find_map(|key| table.get(*key).and_then(toml::Value::as_str))
                .map(one_line)
                .unwrap_or_else(|| "declared item".to_string());
            DeclaredItem {
                summary,
                files: integer(table.get("files")),
            }
        })
        .collect()
}

fn integer(value: Option<&toml::Value>) -> Option<u64> {
    value
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn count_files(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_files(&path)
            } else if path.is_file() {
                1
            } else {
                0
            }
        })
        .sum()
}

fn count_extension(path: &Path, extension: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_extension(&path, extension)
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                1
            } else {
                0
            }
        })
        .sum()
}

fn child_directories(path: &Path) -> Vec<String> {
    let mut children: Vec<String> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()?
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().to_string())
        })
        .collect();
    children.sort();
    children
}

fn count_corpus_cases(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let separators: usize = entries
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                count_corpus_cases(&path) * 2
            } else {
                std::fs::read_to_string(path)
                    .map(|text| {
                        text.lines()
                            .filter(|line| {
                                let line = line.trim();
                                line.len() >= 3 && line.bytes().all(|b| b == b'=')
                            })
                            .count()
                    })
                    .unwrap_or(0)
            }
        })
        .sum();
    separators / 2
}

fn has_canary(root: &Path, language: &str) -> bool {
    let dir = root.join(".github/workflows");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let needle = format!("--lang {language}");
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("canary"))
            && std::fs::read_to_string(path)
                .map(|text| text.contains(&needle))
                .unwrap_or(false)
    })
}

fn git_revision(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn collect_github(root: &Path, repository: Option<&str>) -> Result<GitHubStatus> {
    let repo: RepoResponse = if let Some(repository) = repository {
        gh_api(root, &format!("repos/{repository}"))?
    } else {
        gh_json(
            root,
            &["repo", "view", "--json", "nameWithOwner,defaultBranchRef"],
        )
        .and_then(|value| {
            let full_name = value
                .get("nameWithOwner")
                .and_then(serde_json::Value::as_str)
                .context("gh repo view returned no nameWithOwner")?;
            let default_branch = value
                .pointer("/defaultBranchRef/name")
                .and_then(serde_json::Value::as_str)
                .context("gh repo view returned no default branch")?;
            Ok(RepoResponse {
                full_name: full_name.to_string(),
                default_branch: default_branch.to_string(),
            })
        })?
    };
    let issues: Vec<IssueResponse> = gh_api(
        root,
        &format!("repos/{}/issues?state=open&per_page=100", repo.full_name),
    )?;
    let mut open_issues = Vec::new();
    let mut open_pull_requests = Vec::new();
    for item in issues {
        let out = GitHubItem {
            number: item.number,
            title: item.title,
            url: item.html_url,
            labels: item.labels.into_iter().map(|label| label.name).collect(),
        };
        if item.pull_request.is_some() {
            open_pull_requests.push(out);
        } else {
            open_issues.push(out);
        }
    }
    open_issues.sort_by_key(|item| item.number);
    open_pull_requests.sort_by_key(|item| item.number);

    let workflow_response: WorkflowsResponse = gh_api(
        root,
        &format!("repos/{}/actions/workflows?per_page=100", repo.full_name),
    )?;
    let mut workflows = Vec::new();
    for workflow in workflow_response.workflows {
        let runs: RunsResponse = gh_api(
            root,
            &format!(
                "repos/{}/actions/workflows/{}/runs?branch={}&per_page=1",
                repo.full_name, workflow.id, repo.default_branch
            ),
        )?;
        workflows.push(WorkflowStatus {
            name: workflow.name,
            state: workflow.state,
            latest: runs.workflow_runs.into_iter().next(),
        });
    }
    workflows.sort_by(|a, b| a.name.cmp(&b.name));

    let protection = gh_raw(
        root,
        &format!(
            "repos/{}/branches/{}/protection",
            repo.full_name, repo.default_branch
        ),
    )?;
    let branch_protected = if protection.status.success() {
        Some(true)
    } else {
        let stderr = String::from_utf8_lossy(&protection.stderr);
        if stderr.contains("Branch not protected") || stderr.contains("HTTP 404") {
            Some(false)
        } else {
            None
        }
    };

    Ok(GitHubStatus {
        repository: repo.full_name,
        default_branch: repo.default_branch,
        branch_protected,
        workflows,
        open_issues,
        open_pull_requests,
    })
}

fn gh_api<T: for<'de> Deserialize<'de>>(root: &Path, endpoint: &str) -> Result<T> {
    let output = gh_raw(root, endpoint)?;
    if !output.status.success() {
        bail!(
            "gh api {endpoint}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| format!("decode gh api {endpoint}"))
}

fn gh_raw(root: &Path, endpoint: &str) -> Result<std::process::Output> {
    Command::new("gh")
        .args(["api", endpoint])
        .current_dir(root)
        .output()
        .with_context(|| "run gh; install and authenticate GitHub CLI for --github")
}

fn gh_json(root: &Path, args: &[&str]) -> Result<serde_json::Value> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| "run gh; install and authenticate GitHub CLI for --github")?;
    if !output.status.success() {
        bail!("gh: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    serde_json::from_slice(&output.stdout).context("decode gh output")
}

fn corpus_cell(grammar: &GrammarStatus) -> String {
    grammar
        .evidence
        .corpora
        .iter()
        .map(|corpus| {
            let rate = corpus
                .pass_rate
                .clone()
                .unwrap_or_else(|| "rate unknown".to_string());
            let measurement = format!(
                "{}/{} {rate}",
                grouped(corpus.passed),
                grouped(corpus.files)
            );
            if grammar.evidence.corpora.len() == 1 {
                measurement
            } else {
                format!("{} {measurement}", short_language(&corpus.language))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn corpus_table_cell(grammar: &GrammarStatus) -> String {
    if grammar.evidence.corpora.len() <= 1 {
        return corpus_cell(grammar);
    }
    grammar
        .evidence
        .corpora
        .iter()
        .map(|corpus| {
            format!(
                "{} {}",
                short_language(&corpus.language),
                corpus.pass_rate.as_deref().unwrap_or("rate unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn gaps_cell(grammar: &GrammarStatus) -> String {
    grammar
        .evidence
        .corpora
        .iter()
        .map(|corpus| {
            if grammar.evidence.corpora.len() == 1 {
                grouped(corpus.grammar_gaps)
            } else {
                format!(
                    "{} {}",
                    short_language(&corpus.language),
                    grouped(corpus.grammar_gaps)
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, byte) in digits.bytes().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(char::from(byte));
    }
    out
}

fn short_language(language: &str) -> &str {
    match language {
        "javascript" => "js",
        "typescript" => "ts",
        other => other,
    }
}

fn capability_cell(capabilities: &CapabilitiesStatus) -> String {
    let mut values = Vec::new();
    if capabilities.spans {
        values.push("span");
    }
    if capabilities.formatter {
        values.push("fmt");
    }
    if capabilities.printer {
        values.push("print");
    }
    if values.is_empty() {
        "verdict".to_string()
    } else {
        values.join(",")
    }
}

fn distribution_cell(distribution: &DistributionStatus) -> String {
    let mut values = distribution.bindings.clone();
    if distribution.wasm_pack {
        values.push("wasm".to_string());
    }
    values.push(format!("q{}", distribution.query_files));
    values.join(",")
}

fn freshness_label(freshness: EvidenceFreshness) -> &'static str {
    match freshness {
        EvidenceFreshness::Current => "current",
        EvidenceFreshness::Stale => "STALE",
        EvidenceFreshness::Unbound => "unbound",
    }
}

pub fn render_table(report: &Report) -> String {
    let mut out = String::new();
    let revision = report.revision.as_deref().unwrap_or("unknown");
    writeln!(
        out,
        "treebank status @ {revision} — {} languages, {} grammars",
        report.summary.languages, report.summary.grammars
    )
    .unwrap();
    writeln!(
        out,
        "{:<12} {:<17} {:<33} {:<18} {:<11} {:<7} {:<5} {:<14} {:<13} {:<5} {:<9} {}",
        "GRAMMAR",
        "LANGUAGES",
        "CORPUS PASS",
        "GRAMMAR GAPS",
        "TEST C/N/S",
        "K/W/D",
        "MEAS",
        "CAPS",
        "DIST",
        "LOCK",
        "EVIDENCE",
        "KNOWN DEVIATIONS"
    )
    .unwrap();
    for grammar in &report.grammars {
        let languages = grammar
            .languages
            .iter()
            .map(|lang| lang.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let known_deviations = [
            ("shape", grammar.known_deviations.shape),
            ("fuzz", grammar.known_deviations.fuzz),
            ("lint", grammar.known_deviations.lint),
            ("version", grammar.known_deviations.version),
        ]
        .into_iter()
        .filter_map(|(name, yes)| yes.then_some(name))
        .collect::<Vec<_>>()
        .join(",");
        writeln!(
            out,
            "{:<12} {:<17} {:<33} {:<18} {:<11} {:<7} {:<5} {:<14} {:<13} {:<5} {:<9} {}",
            grammar.grammar,
            languages,
            corpus_table_cell(grammar),
            gaps_cell(grammar),
            format!(
                "{}/{}/{}",
                grammar.tests.corpus_cases, grammar.tests.negative_files, grammar.tests.shape_files
            ),
            format!(
                "{}/{}/{}",
                grammar.evidence.known_gaps.len(),
                grammar.evidence.known_widenings.len(),
                grammar.evidence.deviations.len()
            ),
            grammar.evidence.measurements.len(),
            capability_cell(&grammar.capabilities),
            distribution_cell(&grammar.distribution),
            if grammar.corpus_lock { "yes" } else { "no" },
            freshness_label(grammar.evidence_freshness),
            if known_deviations.is_empty() {
                "—"
            } else {
                &known_deviations
            },
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "C/N/S = corpus cases / negative files / shape fixtures; K/W/D = known gaps / widenings / deviations; MEAS = recorded measurement dimensions"
    )
    .unwrap();
    writeln!(
        out,
        "configuration: {} lock(s), evidence {} current / {} stale / {} unbound, {} canary(s), {} lint ratchet(s), {} shape-fixture grammar(s), {} wasm pack(s), {} query file(s)",
        report.summary.corpus_locks,
        report.summary.current_corpus_evidence,
        report.summary.stale_corpus_evidence,
        report.summary.unbound_corpus_evidence,
        report.summary.corpus_canaries,
        report.summary.lint_ratchets,
        report.summary.shape_fixture_grammars,
        report.summary.wasm_packs,
        report.summary.query_files
    )
    .unwrap();
    if let Some(github) = &report.github {
        writeln!(
            out,
            "github: {} (default {}, protected: {}) — {} issue(s), {} PR(s)",
            github.repository,
            github.default_branch,
            match github.branch_protected {
                Some(true) => "yes",
                Some(false) => "NO",
                None => "unknown",
            },
            github.open_issues.len(),
            github.open_pull_requests.len()
        )
        .unwrap();
        for workflow in &github.workflows {
            let state = workflow.latest.as_ref().map_or("never run", |run| {
                run.conclusion.as_deref().unwrap_or(run.status.as_str())
            });
            writeln!(out, "  workflow {:<24} {state}", workflow.name).unwrap();
        }
        for issue in &github.open_issues {
            writeln!(out, "  issue #{:<4} {}", issue.number, issue.title).unwrap();
        }
        for pr in &github.open_pull_requests {
            writeln!(out, "  PR    #{:<4} {}", pr.number, pr.title).unwrap();
        }
    }
    if !report.warnings.is_empty() {
        writeln!(out, "warnings:").unwrap();
        for warning in &report.warnings {
            writeln!(out, "  - {warning}").unwrap();
        }
    }
    if !report.errors.is_empty() {
        writeln!(out, "configuration errors:").unwrap();
        for error in &report.errors {
            writeln!(out, "  - {error}").unwrap();
        }
    }
    out
}

pub fn render_markdown(report: &Report) -> String {
    let mut out = String::new();
    let revision = report.revision.as_deref().unwrap_or("unknown");
    writeln!(out, "# Treebank status\n").unwrap();
    writeln!(
        out,
        "Revision `{revision}` · {} languages · {} grammars · {} corpus lock(s) · evidence {} current / {} stale / {} unbound · {} canary(s)\n",
        report.summary.languages,
        report.summary.grammars,
        report.summary.corpus_locks,
        report.summary.current_corpus_evidence,
        report.summary.stale_corpus_evidence,
        report.summary.unbound_corpus_evidence,
        report.summary.corpus_canaries
    )
    .unwrap();
    writeln!(out, "| Grammar | Languages | Corpus pass | Grammar gaps | Tests C/N/S | Declared K/W/D | Measured | Capabilities | Distribution | Lock | Evidence | Known deviations |").unwrap();
    writeln!(
        out,
        "|---|---|---:|---:|---:|---:|---|---|---|---:|---|---|"
    )
    .unwrap();
    for grammar in &report.grammars {
        let languages = grammar
            .languages
            .iter()
            .map(|lang| lang.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let known_deviations = [
            ("shape", grammar.known_deviations.shape),
            ("fuzz", grammar.known_deviations.fuzz),
            ("lint", grammar.known_deviations.lint),
            ("version", grammar.known_deviations.version),
        ]
        .into_iter()
        .filter_map(|(name, yes)| yes.then_some(name))
        .collect::<Vec<_>>()
        .join(", ");
        writeln!(
            out,
            "| {} | {} | {} | {} | {}/{}/{} | {}/{}/{} | {} | {} | {} | {} | {} | {} |",
            grammar.grammar,
            languages,
            corpus_cell(grammar),
            gaps_cell(grammar),
            grammar.tests.corpus_cases,
            grammar.tests.negative_files,
            grammar.tests.shape_files,
            grammar.evidence.known_gaps.len(),
            grammar.evidence.known_widenings.len(),
            grammar.evidence.deviations.len(),
            grammar.evidence.measurements.join(", "),
            capability_cell(&grammar.capabilities),
            distribution_cell(&grammar.distribution),
            if grammar.corpus_lock { "yes" } else { "no" },
            freshness_label(grammar.evidence_freshness),
            if known_deviations.is_empty() {
                "—"
            } else {
                &known_deviations
            },
        )
        .unwrap();
    }
    writeln!(out, "\n`C/N/S` = exact corpus cases / negative files / shape fixtures. `K/W/D` = ledgered known gaps / widenings / deviations.\n").unwrap();

    if let Some(github) = &report.github {
        writeln!(out, "## GitHub\n").unwrap();
        writeln!(
            out,
            "Repository `{}` · default branch `{}` · branch protection **{}**\n",
            github.repository,
            github.default_branch,
            match github.branch_protected {
                Some(true) => "enabled",
                Some(false) => "NOT ENABLED",
                None => "unknown",
            }
        )
        .unwrap();
        writeln!(out, "| Workflow | State | Latest run |").unwrap();
        writeln!(out, "|---|---|---|").unwrap();
        for workflow in &github.workflows {
            let latest = workflow.latest.as_ref().map_or_else(
                || "never run".to_string(),
                |run| {
                    format!(
                        "[{}]({})",
                        run.conclusion.as_deref().unwrap_or(run.status.as_str()),
                        run.html_url
                    )
                },
            );
            writeln!(
                out,
                "| {} | {} | {} |",
                workflow.name, workflow.state, latest
            )
            .unwrap();
        }
        writeln!(out, "\n### Open issues\n").unwrap();
        if github.open_issues.is_empty() {
            writeln!(out, "None.\n").unwrap();
        } else {
            for issue in &github.open_issues {
                writeln!(out, "- [#{} {}]({})", issue.number, issue.title, issue.url).unwrap();
            }
            writeln!(out).unwrap();
        }
        writeln!(out, "### Open pull requests\n").unwrap();
        if github.open_pull_requests.is_empty() {
            writeln!(out, "None.\n").unwrap();
        } else {
            for pr in &github.open_pull_requests {
                writeln!(out, "- [#{} {}]({})", pr.number, pr.title, pr.url).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    if !report.warnings.is_empty() {
        writeln!(out, "## Warnings\n").unwrap();
        for warning in &report.warnings {
            writeln!(out, "- {warning}").unwrap();
        }
        writeln!(out).unwrap();
    }
    if !report.errors.is_empty() {
        writeln!(out, "## Configuration errors\n").unwrap();
        for error in &report.errors {
            writeln!(out, "- {error}").unwrap();
        }
        writeln!(out).unwrap();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        bind_evidence, collect, declared_items, one_line, read_evidence, render_markdown,
        EvidenceFreshness,
    };

    #[test]
    fn evidence_reads_one_or_several_sweeps() {
        let ledger: toml::Value = toml::from_str(
            r#"
            [[known_gaps]]
            construct = "a gap"
            files = 3

            [corpus.typescript_sweep]
            files = 10
            passed = 8
            failed = 2
            gap_files = 1
            noise_files = 1
            pass_rate = "80%"

            [corpus.javascript_sweep]
            files = 20
            passed = 19
            failed = 1
            gap_files = 1
            noise_files = 0
            pass_rate = "95%"
            "#,
        )
        .unwrap();
        let evidence = read_evidence(&ledger, "typescript");
        assert_eq!(evidence.corpora.len(), 2);
        assert_eq!(evidence.corpora[0].language, "javascript");
        assert_eq!(evidence.corpora[1].grammar_gaps, 1);
        assert_eq!(evidence.known_gaps[0].summary, "a gap");
    }

    #[test]
    fn declared_items_collapse_multiline_prose() {
        let value: toml::Value = toml::from_str(
            r#"
            [[items]]
            what = '''one
              two'''
            "#,
        )
        .unwrap();
        let items = declared_items(value.get("items"));
        assert_eq!(items[0].summary, "one two");
        assert_eq!(one_line(" a\n b "), "a b");
    }

    #[test]
    fn evidence_is_current_stale_or_unbound_from_exact_identities() {
        let lock = "a".repeat(64);
        let grammar = "b".repeat(64);
        let revision = "c".repeat(40);
        let ledger: toml::Value = toml::from_str(&format!(
            r#"
            [corpus.sweep]
            files = 10
            passed = 10
            failed = 0
            gap_files = 0
            noise_files = 0
            corpus_lock_sha256 = "{lock}"
            grammar_sha256 = "{grammar}"
            grammar_revision = "{revision}"
            "#,
        ))
        .unwrap();
        let mut locks = std::collections::BTreeMap::from([("rust".to_string(), lock.clone())]);
        let mut evidence = read_evidence(&ledger, "rust");
        let mut errors = Vec::new();
        bind_evidence(
            &mut evidence,
            &grammar,
            &locks,
            std::path::Path::new("ledger.toml"),
            &mut errors,
        );
        assert_eq!(evidence.corpora[0].freshness, EvidenceFreshness::Current);
        assert!(errors.is_empty());

        locks.insert("rust".to_string(), "d".repeat(64));
        bind_evidence(
            &mut evidence,
            &grammar,
            &locks,
            std::path::Path::new("ledger.toml"),
            &mut errors,
        );
        assert_eq!(evidence.corpora[0].freshness, EvidenceFreshness::Stale);
        assert!(evidence.corpora[0].freshness_reasons[0].contains("corpus lock changed"));

        let legacy: toml::Value = toml::from_str(
            "[corpus.sweep]\nfiles=1\npassed=1\nfailed=0\ngap_files=0\nnoise_files=0\n",
        )
        .unwrap();
        let mut evidence = read_evidence(&legacy, "rust");
        bind_evidence(
            &mut evidence,
            &grammar,
            &locks,
            std::path::Path::new("ledger.toml"),
            &mut errors,
        );
        assert_eq!(evidence.corpora[0].freshness, EvidenceFreshness::Unbound);
        assert_eq!(evidence.corpora[0].freshness_reasons.len(), 3);

        let invalid: toml::Value = toml::from_str(
            "[corpus.sweep]\nfiles=1\npassed=1\nfailed=0\ngap_files=0\nnoise_files=0\ncorpus_lock_sha256='not-a-hash'\ngrammar_sha256='not-a-hash'\ngrammar_revision='not-a-revision'\n",
        )
        .unwrap();
        let mut evidence = read_evidence(&invalid, "rust");
        let mut invalid_errors = Vec::new();
        bind_evidence(
            &mut evidence,
            &grammar,
            &locks,
            std::path::Path::new("ledger.toml"),
            &mut invalid_errors,
        );
        assert_eq!(invalid_errors.len(), 3);
    }

    #[test]
    fn markdown_is_a_real_document() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = collect(&root).unwrap();
        let markdown = render_markdown(&report);
        assert!(markdown.starts_with("# Treebank status\n"));
        assert!(markdown.contains("| Grammar | Languages |"));
        assert!(report.errors.is_empty(), "{:?}", report.errors);
    }

    #[test]
    fn missing_repository_configuration_is_reported_not_panicked() {
        let root =
            std::env::temp_dir().join(format!("treebank-status-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let report = collect(&root).unwrap();
        assert!(!report.errors.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}

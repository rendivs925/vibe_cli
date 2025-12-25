## Agent System Implementation Plan

Goal: Make the build/agent system fully agentic, production-ready, and autonomous for CRUD (create/read/update/delete), debugging, and code generation, with safe execution and grounded outputs.

### Current Gaps (from codebase scan)
- LSP integration: `infrastructure/src/lsp_client.rs` stubbed; no diagnostics/format tools.
- RAG backing store: `infrastructure/src/qdrant_storage.rs` has placeholder insert/search/retrieve.
- CLI flows: `presentation/src/cli.rs` has unimplemented edit/revise/suggest branches; session history not updated on apply.
- Agent loop: Hardcoded iteration limit (`max_iters = 5` in `agent_service.rs`) not config-driven.
- Validation: Generated code can still include fences/backticks; no language-aware preflight checks.
- Observability: Tool calls/results not persisted for audit beyond in-memory structs.
- Testing: No prompt/tool regression harness.
- Security: Network allowlist is overly restrictive; command safety relies on regex patterns; limited prompt-injection defenses; no request/response size/content checks; no rate limiting.
- Reliability: No circuit breakers or health checks; offline fallback is missing; dependency failures cascade.
- Caching/Data: Fixed TTLs, no integrity checks/migrations; semantic similarity based on word overlap.
- Performance: RAG prefiltering is naive; no streaming for large responses; single-process bottlenecks; no connection pooling.

### Implementation Phases

#### Phase 1: Stability & Safety (short-term)
1) Configurable agent loop
   - Add `agent.max_iterations`/`max_tools_per_iteration` to config; replace hardcoded limits; allow CLI/env overrides.
2) Validation before apply
   - Language-aware checks pre-buffer:
     - `.py`: `python -m py_compile`
     - `.js/.ts`: `node --check` or `ts-node --transpile-only` (configurable)
     - `.rs`: `cargo check` (scoped if possible)
   - On failure: capture stdout/stderr, feed back to model with “insufficient” guard; do not buffer.
3) Fence/markdown stripping
   - Enforce fence stripping in previews (done) and before writes; reject generations starting with fences/backticks and request plain text.
4) Audit trail
   - Persist tool_calls/tool_results summaries in `AgentResult`/`AgentResponse` and to disk (e.g., structured log/JSON) for later inspection.
5) Network & command security
   - Graduated trust levels (allow/user-approve/block) for domains; expand allowlist to common dev registries.
   - Request/response size limits; content-type validation; schema validation for generated commands.
   - Rate limiting per user/session; command content filtering; stronger prompt-injection detection beyond regex.
6) Circuit breakers & health checks
   - Circuit breakers around Ollama/API calls with backoff+jitter and clear user messaging.
   - Health checks for dependencies; graceful degradation paths (skip feature, retry, or fallback).
7) Offline fallback
   - Cache successful command patterns; rule-based command generation when LLM is unavailable; clear “offline mode” messaging.

#### Phase 2: Tooling Completeness
1) LSP client
   - Implement minimal LSP (initialize, textDocument/didOpen, diagnostics, formatting).
   - Expose as tools (`lsp_format`, `lsp_diagnostics`) with timeouts and workspace validation.
2) Qdrant storage
   - Implement insert/search/retrieve using a real Qdrant client; honor include/exclude patterns; robust error handling and fallbacks.
3) CLI UX (edit/revise/suggest)
   - Edit: prompt for new goal and rerun planner.
   - Revise: allow user to append constraints; rerun with prior context.
   - Suggest: generate 3 improvements; let user pick/cancel.
   - Update session history when operations apply.

#### Phase 3: Autonomy & Coverage
1) Full CRUD
   - Add delete/read ops to incremental planner (with confirmations) when goals imply removal/inspection.
   - Detect asset needs; inline minimal assets (CSS/JS) to keep outputs runnable offline.
2) RAG & context
   - Classifier to decide when to RAG vs. local search (goal/complexity-based).
   - Cache RAG results per goal to avoid repeated indexing in a run.
3) Grounding & fact checking
   - Post-generation fact validator: ensure final answers cite tool outputs; otherwise return “Insufficient context to answer.”
   - Enforce in build planner and final responses.
4) Caching & data integrity
   - Adaptive TTL + LRU; semantic similarity using embeddings (not word overlap).
   - Integrity validation/repair for caches; migration strategy for cache/DB changes.
   - Backup/export/import hooks for embeddings and caches.

#### Phase 4: Testing & Regression
1) Prompt/tool regression suite
   - Cases: empty repo scaffold, fence stripping, syntax-fix loop, git ops, RAG on/off, CRUD including deletes.
   - Run in CI (`cargo test` + scripted prompt checks).
2) Performance/resource limits
   - Configurable tool timeouts/output caps; metrics on latency/failures.

### Deliverables Checklist
- [ ] Config-driven agent iteration/tool limits.
- [ ] Language-aware validation before buffering operations.
- [ ] Fence stripping on write + rejection of fenced generations.
- [ ] Audit trail of tool calls/results persisted to disk.
- [ ] Network trust levels, size/content validation, rate limiting, and stronger prompt-injection detection.
- [ ] Circuit breakers with backoff/jitter; health checks; offline fallback with rule-based commands.
- [ ] LSP client tool (format/diagnostics).
- [ ] Qdrant storage implemented.
- [ ] CLI edit/revise/suggest flows; session history updates.
- [ ] CRUD coverage including deletes/reads with confirmations.
- [ ] RAG vs. local search classifier and caching.
- [ ] Fact-checker on final responses/build plans.
- [ ] Regression suite and CI hook.
- [ ] Adaptive cache TTL + semantic similarity; integrity checks/migrations/backups.

### Notes
- Keep everything configurable (flags/env) to balance speed vs. safety.
- Prefer inline assets/entrypoints to avoid external deps for generated apps/games.
- Maintain “insufficient context” guardrails to avoid hallucinated paths or content.

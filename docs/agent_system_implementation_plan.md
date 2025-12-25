## Agent System Implementation Plan

Goal: Make the build/agent system fully agentic, production-ready, and autonomous for CRUD (create/read/update/delete), debugging, and code generation, with safe execution and grounded outputs.

### Current Gaps (from codebase scan)
- `infrastructure/src/lsp_client.rs`: Missing full LSP protocol implementation.
- `infrastructure/src/qdrant_storage.rs`: Placeholder insertion/search/retrieval for vector DB.
- `presentation/src/cli.rs`: Unimplemented edit/revise/suggest flows in build UI; session history not updated on apply.
- `application/src/agent_service.rs`: Hardcoded iteration limit (`max_iters = 5`) instead of config.
- Syntax/error handling: Generated files can include markdown fences; validation is minimal; tools aren't always piped into validation.
- Observability: Tool calls/results now exist but aren’t persisted for audit.
- Testing: No regression harness for prompts/tools.

### Implementation Phases

#### Phase 1: Stability & Safety (short-term)
1) **Configurable Agent Loop**
   - Add `agent.max_iterations` and `agent.max_tools_per_iteration` to config; replace hardcoded `max_iters`.
   - Expose CLI flags/env overrides.
2) **Validation Before Apply**
   - Language-aware checks before buffering operations:
     - `.py`: `python -m py_compile`
     - `.js/.ts`: `node --check` or `ts-node --transpile-only` (configurable)
     - `.rs`: `cargo check` scoped to file/module when possible
   - On failure: capture stdout/stderr, return to model with “insufficient” guard; don’t buffer.
3) **Markdown Fence Prevention**
   - Enforce fence stripping in previews (done) and before file writes.
   - Reject generation containing leading ````` blocks; ask model to resend plain text.
4) **Audit Trail**
   - Persist tool_calls and tool_results (summaries) in `AgentResult`/`AgentResponse` and optionally to disk (`app.log` or structured JSON) for later inspection.

#### Phase 2: Tooling Completeness
1) **LSP Client**
   - Implement minimal LSP (initialize, open/close, diagnostics, formatting).
   - Wire as a tool (`lsp_format`, `lsp_diagnostics`) with timeouts and workspace validation.
2) **Qdrant Storage**
   - Implement insert/search/retrieve using actual Qdrant client.
   - Respect include/exclude patterns from config; add error handling and fallbacks.
3) **CLI UX (edit/revise/suggest)**
   - Implement edit: prompt for new goal and rerun planner.
   - Implement revise: allow user to append constraints; rerun planner with prior context.
   - Implement suggest: auto-generate 3 improvements; let user pick or cancel.
   - Update session history when operations are applied.

#### Phase 3: Autonomy & Coverage
1) **Full CRUD**
   - Add delete and read operations to incremental planner (with confirmations) when goals imply removal/inspection.
   - Detect asset needs; inline minimal assets (e.g., CSS/JS) to keep runnable without external files.
2) **RAG & Context**
   - Classifier to decide when to RAG vs. local search (goal/complexity-based).
   - Cache RAG results per goal to avoid repeat indexing in a run.
3) **Grounding & Fact Checking**
   - Post-generation fact validator: ensure final answers cite tool outputs; otherwise return “Insufficient context.”
   - Enforce this in build planner and final responses.

#### Phase 4: Testing & Regression
1) **Prompt/Tool Regression Suite**
   - Add cases for: empty repo scaffold (games), fenced code removal, syntax fix loop, git ops, RAG on/off.
   - Run in CI with `cargo test` plus script-driven prompt checks.
2) **Performance/Resource Limits**
   - Ensure tool timeouts/output caps are configurable; add metrics on tool latency and failures.

### Deliverables Checklist
- [ ] Config-driven agent iteration/tool limits.
- [ ] Language-aware validation before buffering operations.
- [ ] Fence stripping on write + rejection of fenced generations.
- [ ] Audit trail of tool calls/results persisted.
- [ ] LSP client tool (format/diagnostics).
- [ ] Qdrant storage implemented.
- [ ] CLI edit/revise/suggest flows; session history updates.
- [ ] CRUD coverage including deletes/reads with confirmations.
- [ ] RAG vs. local search classifier and caching.
- [ ] Fact-checker on final responses/build plans.
- [ ] Regression suite and CI hook.

### Notes
- Keep everything configurable (flags/env) to balance speed vs. safety.
- Prefer inline assets/entrypoints to avoid external deps for generated apps/games.
- Maintain “insufficient context” guardrails to avoid hallucinated paths or content.

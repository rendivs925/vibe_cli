# The Autonomous Neurosymbolic Roadmap

## **Vision**

Evolve Vibe CLI from a "command generator" into a **self-correcting, environment-aware system administrator**. The system must ground intent in formal logic, validate commands against local documentation, learn from failures, and adapt to each machine.

---

## **Current Implementation (Aligned to the Roadmap)**

**Intent Pipeline (Single Integrated Path)**
- **Intent Signal (Neural)**: Upstream analysis produces a structured `IntentSignal` with `action`, `target`, `objects`, `constraints`, and `params`.
- **Fuzzy Operation Resolver**: Domain operations are matched via token overlap and fuzzy similarity across operation intent, name, description, and examples.
- **Input Extraction**: Operation inputs are extracted from the query (paths, service names, log line counts, patterns) and fed into the generator.

**Known Heuristics (Still Present)**
- Domain resolution still includes limited string-based checks (domain id/description/entity name).
- Input extraction uses regex and string heuristics for services, logs, actions, and filters.

**Command Pipeline (Config-Driven Only)**
- **Document-Constrained Selection**: Generated candidates are **validated against manpages** and the best valid command is chosen before execution.
- **Safety + Risk + Proof**: Hard safety rules run first, probabilistic risk scoring follows, and formal proofs are produced for critical operations.
- **Learning Feedback**: The experience buffer is queried to filter out previously failed approaches before choosing a command.

---

## **Phase 1: The "Thinking" Foundation (Constraint & Translation)**

_Goal: Eliminate syntax hallucinations and ground all actions in formal logic._

### **1.1. Fuzzy Intent Normalization**

Instead of requiring a formal query language, the system resolves intent using **token overlap and fuzzy similarity** across domain operations.

- **Status:** [x] Implemented and wired.
- **Feature:** Fuzzy resolver over operation name/intent/description/examples.
- **Function:** Matches natural language (e.g., "clean logs") to the best operation by similarity score.
- **Benefit:** Removes rigid formalization while preserving symbolic grounding.

### **1.2. Document-Constrained Generation**

Commands are validated against local documentation before selection.

- **Status:** [x] Implemented and wired.
- **Feature:** `ManpageCrawler` + `SyntaxGrammarValidator`.
- **Function:** Parses `man <tool>` or `<tool> --help` and **filters out invalid flag candidates**.
- **Benefit:** Prevents invalid flags from being selected.

### **1.3. Basic Symbolic Validation**

- **Status:** [x] Implemented and wired.
- **Feature:** `SafetyEngine` with `HardRules`.
- **Function:** Rejects dangerous commands (e.g., root deletion) before execution.
- **Benefit:** Prevents catastrophic actions.

---

## **Phase 2: The "Learning" Loop (Experience & Adaptation)**

_Goal: Enable the system to learn from mistakes and refine its logic._

### **2.1. Step-by-Step Symbolic Backtracking**

- **Status:** [~] Partial.
- **Feature:** Manpage-validated **candidate selection** (live candidate pruning).
- **Function:** Invalid commands are pruned before execution.
- **Benefit:** Reduces hallucination loops and wasted attempts.
- **Gap:** Token-level LLM decoding monitor is not implemented; pruning happens after candidate generation.

### **2.2. The Experience Buffer (Memory)**

- **Status:** [x] Implemented and wired.
- **Feature:** `ExperienceBuffer` + `LearningService`.
- **Function:** Stores `query`, `command`, `failure_type`, and `corrections`.
- **Benefit:** Enables persistence of mistakes and best approaches.

### **2.3. Neural Context Injection (RAG for Logic)**

- **Status:** [x] Implemented and wired.
- **Feature:** `LearningService::get_context_for_query`.
- **Function:** Produces a **"Do Not Repeat"** list and recommended approaches; these are used to **filter candidate commands**.
- **Benefit:** Prevents repeating known failures.

---

## **Phase 3: Autonomous Evolution (The "Fully Leveraged" State)**

_Goal: Build a system model and learn local operational rules._

### **3.1. Knowledge Graph (KG) Construction**

- **Status:** [x] Implemented and wired.
- **Feature:** `KnowledgeGraph` + `GraphBuilder`.
- **Function:** Stores local entities (services, files, tools) and relationships.
- **Benefit:** Adds system-specific context beyond manpages.

### **3.2. Autonomous Rule Induction**

- **Status:** [x] Implemented and wired.
- **Feature:** `InductionEngine`.
- **Function:** Mines failure patterns from the experience buffer and writes new rules to the KG.
- **Benefit:** Learns machine-specific quirks automatically.

### **3.3. Probabilistic Symbolic Reasoning**

- **Status:** [x] Implemented and wired.
- **Feature:** `RiskScorer`.
- **Function:** Produces a probabilistic risk profile and mitigation steps based on command + history.
- **Benefit:** Handles real-world edge cases safely.

---

## **Phase 4: High-Assurance Verification (The "Certificate")**

_Goal: Provide mathematical proof of safety for critical operations._

### **4.1. Formal Verification Certificate**

- **Status:** [~] Partial.
- **Feature:** `ProofGenerator`.
- **Function:** Generates a safety proof for high-risk commands.
- **Output:** A user-readable proof summary before execution.
- **Benefit:** High-assurance validation for destructive actions.
- **Gap:** Proofs are heuristic; no external SMT solver integration yet.

---

## **Summary of the Fully Leveraged Architecture**

| Component             | Role                   | Intelligence Level                                             |
| --------------------- | ---------------------- | -------------------------------------------------------------- |
| **LLM (Neural)**      | **Intent Signal**      | "Here is the structured intent (action, target, constraints)." |
| **Fuzzy Resolver**    | **Matcher**            | "This intent best matches an operation by similarity."        |
| **Knowledge Graph**   | **Context Engine**     | "This server has specific constraints."                       |
| **Symbolic Verifier** | **Safety Officer**     | "This command satisfies safety constraints."                  |
| **Induction Engine**  | **Learner**            | "I detected a failure pattern and updated rules."             |

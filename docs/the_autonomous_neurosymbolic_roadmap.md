# The Autonomous Neurosymbolic Roadmap

## **Vision**

Evolve Vibe CLI from a "command generator" into a **self-correcting, environment-aware system administrator**. The system must ground intent in formal logic, validate commands against local documentation, learn from failures, and adapt to each machine.

---

## **Current Implementation (Aligned to the Roadmap)**

**Intent Pipeline (No Keyword Matching for Domain Resolution)**
- **Intent Signal (Neural)**: Upstream analysis produces a structured `IntentSignal` with `action`, `target`, `objects`, `constraints`, and `params`.
- **FQL Autoformalization (Symbolic)**: The signal is converted into `FqlQuery` when possible; otherwise, the query is parsed via `FqlParser`.
- **Semantic Operation Resolver**: Domain operations are matched by **FQL signature** (action + target + pattern), not fuzzy or keyword matching.
- **Input Extraction**: Operation inputs are extracted from the query/FQL (paths, service names, log line counts, patterns) and fed into the generator.

**Command Pipeline**
- **Document-Constrained Selection**: Generated candidates are **validated against manpages** and the best valid command is chosen before execution.
- **Safety + Risk + Proof**: Hard safety rules run first, probabilistic risk scoring follows, and formal proofs are produced for critical operations.
- **Learning Feedback**: The experience buffer is queried to filter out previously failed approaches before choosing a command.

---

## **Phase 1: The "Thinking" Foundation (Constraint & Translation)**

_Goal: Eliminate syntax hallucinations and ground all actions in formal logic._

### **1.1. Autoformalization Layer (The Mediator)**

Instead of generating Bash directly, the system translates user intent into a **Formal Query Language (FQL)**.

- **Feature:** `FqlParser` + `IntentSignal`.
- **Function:** Converts natural language (e.g., "clean logs") into structured logic: `ACTION(delete) & TARGET(log:*) & CONSTRAINT(safe_delete)`.
- **Benefit:** Decouples intent understanding from command syntax generation.

### **1.2. Document-Constrained Generation**

Commands are validated against local documentation before selection.

- **Feature:** `ManpageCrawler` + `SyntaxGrammarValidator`.
- **Function:** Parses `man <tool>` or `<tool> --help` and **filters out invalid flag candidates**.
- **Benefit:** Prevents invalid flags from being selected.

### **1.3. Basic Symbolic Validation**

- **Feature:** `SafetyEngine` with `HardRules`.
- **Function:** Rejects dangerous commands (e.g., root deletion) before execution.
- **Benefit:** Prevents catastrophic actions.

---

## **Phase 2: The "Learning" Loop (Experience & Adaptation)**

_Goal: Enable the system to learn from mistakes and refine its logic._

### **2.1. Step-by-Step Symbolic Backtracking**

- **Feature:** Manpage-validated **candidate selection** (live candidate pruning).
- **Function:** Invalid commands are pruned before execution.
- **Benefit:** Reduces hallucination loops and wasted attempts.

### **2.2. The Experience Buffer (Memory)**

- **Feature:** `ExperienceBuffer` + `LearningService`.
- **Function:** Stores `query`, `command`, `failure_type`, and `corrections`.
- **Benefit:** Enables persistence of mistakes and best approaches.

### **2.3. Neural Context Injection (RAG for Logic)**

- **Feature:** `LearningService::get_context_for_query`.
- **Function:** Produces a **"Do Not Repeat"** list and recommended approaches; these are used to **filter candidate commands**.
- **Benefit:** Prevents repeating known failures.

---

## **Phase 3: Autonomous Evolution (The "Fully Leveraged" State)**

_Goal: Build a system model and learn local operational rules._

### **3.1. Knowledge Graph (KG) Construction**

- **Feature:** `KnowledgeGraph` + `GraphBuilder`.
- **Function:** Stores local entities (services, files, tools) and relationships.
- **Benefit:** Adds system-specific context beyond manpages.

### **3.2. Autonomous Rule Induction**

- **Feature:** `InductionEngine`.
- **Function:** Mines failure patterns from the experience buffer and writes new rules to the KG.
- **Benefit:** Learns machine-specific quirks automatically.

### **3.3. Probabilistic Symbolic Reasoning**

- **Feature:** `RiskScorer`.
- **Function:** Produces a probabilistic risk profile and mitigation steps based on command + history.
- **Benefit:** Handles real-world edge cases safely.

---

## **Phase 4: High-Assurance Verification (The "Certificate")**

_Goal: Provide mathematical proof of safety for critical operations._

### **4.1. Formal Verification Certificate**

- **Feature:** `ProofGenerator`.
- **Function:** Generates a safety proof for high-risk commands.
- **Output:** A user-readable proof summary before execution.
- **Benefit:** High-assurance validation for destructive actions.

---

## **Summary of the Fully Leveraged Architecture**

| Component             | Role                   | Intelligence Level                                             |
| --------------------- | ---------------------- | -------------------------------------------------------------- |
| **LLM (Neural)**      | **Intent Signal**      | "Here is the structured intent (action, target, constraints)." |
| **Autoformalizer**    | **Translator**         | "This intent maps to FQL."                                    |
| **Knowledge Graph**   | **Context Engine**     | "This server has specific constraints."                       |
| **Symbolic Verifier** | **Safety Officer**     | "This command satisfies safety constraints."                  |
| **Induction Engine**  | **Learner**            | "I detected a failure pattern and updated rules."             |

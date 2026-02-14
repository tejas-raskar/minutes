# CLI Summary V1 (Gemini-First) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a production-usable CLI summary flow that generates and stores one meeting summary per recording.

**Architecture:** Keep CLI-only scope. Add an LLM provider interface so Gemini is the first adapter, while OpenAI/Ollama can be added later without changing CLI or command logic. Persist one summary in existing recording metadata (`recordings.notes`) for V1.

**Tech Stack:** Rust, clap, tokio, reqwest, serde_json, rusqlite

---

### Task 1: Stabilize Baseline Before New Feature

**Files:**
- Modify: `src/daemon/mod.rs`
- Create: `tests/daemon_start_tests.rs`

**Step 1:** Keep the startup readiness check fix (`daemon start` must fail if daemon fails to boot).

**Step 2:** Run regression test:
`cargo test --test daemon_start_tests -- --nocapture`

**Step 3:** Run full tests:
`cargo test`

**Step 4:** Commit only these files.

### Task 2: Add CLI Surface for Summaries (TDD)

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/main.rs`
- Modify: `src/cli/commands.rs`
- Create: `tests/summarize_cli_tests.rs`

**Step 1:** Write failing CLI tests for:
- `minutes summarize <id>` command exists
- command errors clearly when recording does not exist

**Step 2:** Run test to verify failure.

**Step 3:** Implement minimal `summarize` command plumbing and not-found behavior.

**Step 4:** Re-run tests and commit if green.

### Task 3: Provider Abstraction + Gemini Implementation (TDD)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Modify: `src/llm/mod.rs`
- Create: `src/llm/client.rs`
- Create: `src/llm/gemini.rs`
- Create: `src/llm/prompts.rs`
- Modify: `src/config/settings.rs`
- Modify: `src/cli/commands.rs`

**Step 1:** Add failing tests for summary generation workflow in `commands` layer:
- clear error when API key missing
- successful generation persists summary into recording notes

**Step 2:** Implement provider trait + factory from config.

**Step 3:** Implement Gemini adapter via `generateContent` API call and robust response parsing.

**Step 4:** Wire `minutes summarize <id>` to:
- fetch transcript segments
- generate summary
- save to DB (`recordings.notes`)
- print summary

**Step 5:** Update `minutes view <id>` to show saved summary if present.

**Step 6:** Run target tests + `cargo test` + CLI smoke checks.

**Step 7:** Commit feature changes atomically.

### Task 4: CLI Verification Loop (Required Before Completion)

**Commands:**
- `./target/debug/minutes --help`
- `./target/debug/minutes list`
- `./target/debug/minutes view <existing_id_prefix>`
- `./target/debug/minutes summarize <existing_id_prefix>` (with configured Gemini key)
- `./target/debug/minutes view <same_id_prefix>` (summary must be visible)

Record failures, fix root cause, retest, then commit.

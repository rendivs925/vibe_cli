# Vibe CLI: Codex-Level Minimalist TUI with Vim Keybindings

## Executive Summary

This document presents a comprehensive implementation plan for transforming Vibe CLI from a feature-rich interface to a Codex-level minimalist TUI with vim-style personalized keybindings.

## Design Philosophy

**From**: Feature-rich TUI with panels, buttons, widgets  
**To**: Invisible thinking surface that happens to be powerful

### Core Principles

1. **Cognitive Bandwidth Conservation** - Minimize UI elements
2. **Muscle Memory Optimization** - Keyboard-first flow  
3. **UI Invisibility** - Interface disappears during use
4. **Invisible Power** - Capability without visible complexity

---

## Architecture Overview

### Single Interaction Surface

The interface maintains a **single prompt** at all times, with temporary overlays that appear and disappear instantly:

```
┌────────────────────────────────────────────────────────────┐
│ Vibe CLI                                    [Settings] [Help] [Exit] │
├────────────────────────────────────────────────────────────┤
│                                                            │
│ > analyze auth flow for security issues                     │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│                                                            │
│ ─────────────────────────────────────────────────────────│
│ ⏎ send   ⌘K tools   ⌘O context   ⌘P palette   esc cancel │
└────────────────────────────────────────────────────────────┘
```

### Ephemeral Overlays (Not Panels)

Instead of persistent sidebars, the system uses temporary overlays:

- **Tool Picker (⌘K)**: Summoned, not persistent
- **Context Overlay (⌘O)**: File list, disappears after use
- **Command Palette (⌘P)**: Fuzzy search, appears on demand
- **Help**: Context-sensitive, not always visible

---

## Implementation Plan

### Phase 1: Core Minimalism (Week 1-2)

#### 1.1 TUI Framework Setup
- **Framework**: `ratatui` for Rust-native performance
- **Architecture**: Single state management with overlay system
- **Performance**: 60 FPS rendering, minimal resource usage

#### 1.2 Single Interaction Surface
- **Input Buffer**: Always-present prompt area
- **Navigation Hints**: Minimal single-line hints
- **Mode Indicators**: Subtle, not competing for attention
- **History**: Scrollable through previous commands

#### 1.3 Overlay System
- **Tool Picker**: Centered list, instant dismissal
- **Context Picker**: File list with search capability
- **Command Palette**: Fuzzy search interface
- **Help System**: Context-sensitive assistance

#### 1.4 Input Handling
- **Escape Key**: Dismiss any overlay instantly
- **Navigation**: hjkl for list navigation
- **Selection**: Enter/Space for confirmation
- **Search**: Type-to-filter in any overlay

---

### Phase 2: Vim-Style Keybindings (Week 3-4)

#### 2.1 Core Navigation System
```rust
pub struct VimKeyBindings {
    // Movement (vim standard)
    move_up: Key::Char('k'),        // j in vim
    move_down: Key::Char('j'),      // k in vim
    move_left: Key::Char('h'),      // vim standard
    move_right: Key::Char('l'),     // vim standard
    
    // Enhanced movement
    page_up: Key::Char('u'),         // Ctrl+u equivalent
    page_down: Key::Char('d'),       // Ctrl+d equivalent
    go_to_line: Key::Char('G'),      // vim G command
    go_to_start: Key::Char('g'),     // vim gg command
    
    // Word movement
    word_forward: Key::Char('w'),     // vim w
    word_backward: Key::Char('b'),   // vim b
    end_of_word: Key::Char('e'),     // vim e
    beginning_of_word: Key::Char('b'),  // vim b
}
```

#### 2.2 Modal System
```rust
pub enum VibeMode {
    Normal,      // hjkl navigation, command mode
    Insert,      // Typing in input fields
    Visual,      // Selecting files/tools
    Command,     // Command palette active
    Help,        // Vim help overlay
}
```

#### 2.3 Text Object Operations
```rust
// Select with vim-style operations
fn vim_select_with_objects() {
    // Visual line mode: V + navigation
    visual_line_mode: Key::Char('V'),
    
    // Visual block mode: Ctrl+v
    visual_block_mode: Key::Char('v'),
    
    // Select current word: diw (delete inner word, but for selection)
    select_current_word: KeySequence::new(&['d', 'i', 'w']),
    
    // Select to end: v$ (visual to line end)
    select_to_end: KeySequence::new(&['v', '$']),
}
```

#### 2.4 Command Mode Integration
```
:tools              ⌘K equivalent
:context src/auth  ⌘O equivalent  
:analyze            Enter command
:split              Split view
:quit               Exit without save
:w                  Save/write
:q!                 Quit without save
```

#### 2.5 Personalization System
```rust
pub struct VimProfile {
    name: String,
    keybindings: VimKeyBindings,
    
    // User preferences
    remap_leader: Option<Key>,        // Leader key like in vim
    visual_feedback: bool,             // Show mode changes
    case_sensitive_search: bool,        // vim-style search
    wrap_search: bool,                 // Wrap around edges
    
    // Tool-specific vim habits
    tool_picker_hotkeys: HashMap<String, KeySequence>,
    context_navigation_style: ContextNavStyle,
}
```

---

### Phase 3: Advanced Features (Week 5-6)

#### 3.1 Invisible Intelligence
- **Context-Aware Hints**: Single line suggestions, not UI elements
- **Smart Overlay Timing**: Auto-dismiss after 30 seconds idle
- **Intent Detection**: Analyze input to suggest relevant tools
- **Error Recovery**: Single-key fixes for common mistakes

#### 3.2 Power User Features
- **Command Palette**: Fuzzy search for power users
- **Split View**: Explicit command: `:split`
- **Editor Preview**: Read-only preview with `:preview`
- **Custom Keybindings**: Full remapping support

#### 3.3 Learning System
```rust
pub struct VimLearningSystem {
    // Track user patterns
    movement_patterns: HashMap<String, UsageFrequency>,
    command_sequences: HashMap<KeySequence, SuccessRate>,
    modal_transitions: HashMap<ModeTransition, Frequency>,
    
    // Adapt to user style
    preferred_navigation: NavigationStyle,
    common_mistakes: HashMap<MistakePattern, Correction>,
    optimization_suggestions: Vec<VimOptimization>,
}
```

#### 3.4 Performance Optimization
- **Rendering**: 60+ FPS for smooth interactions
- **Memory**: <50MB for TUI components
- **CPU**: <5% during idle TUI display
- **Latency**: <100ms response time for user input

---

## Visual Style Guidelines

### Typography
- **Monospace font only**
- **No bold** except for single headings
- **No emojis** except: ✓ success, ! warning, × error

### Color System
- **1 accent color** (primary action)
- **1 error color** (problems)
- **Everything else** grayscale

### Layout Rules
- **No borders** except separating interaction layers
- **Prefer whitespace** over boxes
- **Max 2 visual separators** on screen at any time

---

## Success Metrics

### You've succeeded when:
- **New user can type** `vibe analyze project` without hesitation
- **No visible elements** blink, pulse, or demand attention
- **UI automatically disappears** after 30 seconds of use
- **Keyboard shortcuts become subconscious** muscle memory
- **Power users discover features through usage**, not interface exploration
- **Terminal feels larger** and more spacious with less UI

---

## Implementation Checklist

### Week 1-2: Core Minimalism
- [ ] ratatui setup with minimal state management
- [ ] Single input line with history
- [ ] Overlay system for tools and context
- [ ] Basic hjkl navigation
- [ ] Escape key dismisses all overlays

### Week 3-4: Vim Integration
- [ ] Full vim-style keybinding system
- [ ] Modal system (Normal, Insert, Visual, Command)
- [ ] Text object operations (dw, dd, yy, etc.)
- [ ] Command mode with `:` syntax
- [ ] Personalized vim profiles

### Week 5-6: Advanced Features
- [ ] Learning system for optimization
- [ ] Command palette with fuzzy search
- [ ] Split view and editor preview
- [ ] Performance optimization
- [ ] Complete help system integration

---

## Technical Architecture

### Core State Management
```rust
pub struct VibeTuiApp {
    mode: VibeMode,
    input_buffer: String,
    current_overlay: Option<Overlay>,
    context: ProjectContext,
    vim_profile: VimProfile,
    learning_system: VimLearningSystem,
}

impl VibeTuiApp {
    fn handle_input(&mut self, key: Key) -> Action;
    fn show_overlay(&mut self, overlay: Overlay);
    fn dismiss_overlay(&mut self);
    fn execute_with_feedback(&mut self, tool: Tool);
    fn update_context(&mut self);
}
```

### Overlay System
```rust
pub enum Overlay {
    ToolPicker,      // ⌘K
    ContextPicker,    // ⌘O
    CommandPalette,   // ⌘P
    Help,           // Context-sensitive help
    VimProfile,      // Personalization settings
}
```

### Learning Integration
```rust
pub struct VimLearningSystem {
    movement_patterns: HashMap<String, UsageFrequency>,
    command_sequences: HashMap<KeySequence, SuccessRate>,
    optimization_suggestions: Vec<VimOptimization>,
    
    fn learn_from_interaction(&mut self, input: &str, result: ActionResult);
    fn suggest_optimization(&self) -> Option<VimOptimization>;
    fn adapt_keybindings(&mut self, patterns: &UsagePatterns);
}
```

---

## Key Questions for Implementation

1. **Primary Navigation**: Should hjkl be default, or offer emacs-style (C-n/C-p/C-f/C-b) as alternative?
2. **Visual Selection Priority**: Should visual mode work on files, tools, both, or configurable?
3. **Command Mode Syntax**: Should use `:` like vim, or `/` for search-style commands?
4. **Learning Scope**: Should system learn immediately, or wait for baseline usage patterns?
5. **Help Integration**: Should vim help be accessible via `?` or `:help` command in command mode?

---

## Conclusion

This plan transforms Vibe CLI from a feature-rich interface to a truly Codex-level minimalist TUI that respects user attention and cognitive bandwidth while providing powerful vim-style keybindings for personalization.

The core insight is that **the best interface is the one you forget exists** while you're working.

*Document created: 2025-01-15*
*Author: Vibe CLI Planning Team*
*Version: 1.0*
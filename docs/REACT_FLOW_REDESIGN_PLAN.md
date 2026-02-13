# ReAct Flow Redesign Plan

## Overview

Redesign the ReAct interactive flow to be more like Claude Code - natural, conversational, and flexible.

---

## Current Flow

```
AI Thought → Action → "Execute? [Y/n/r/g/a/x]:" → Execute → Observation → repeat
```

Problems:
- Static flow always goes: thought → action → execute → observation → repeat
- Users can't think multiple times before executing
- Confusing command options (Y/n/r/g/a/x)
- Not conversational

---

## New Claude Code-style Flow

```
User (natural language) → AI Reasoning → Suggested Command → Allow? y/n>
                                                      ├─ y/yes/Enter → Execute → Output → AI Reasoning → repeat
                                                      └─ n/any text → User Input → AI Reasoning → new suggestion → repeat
```

Key principles:
- User can think/question multiple times before executing
- Natural language input throughout
- Simple Y/n confirmation
- AI controls the actions, user provides direction

---

## Example: Simple

```
$ vibe_cli --react "find memory leaks"

--- REASONING ---
I'll help you find memory leaks on your system.

> I want to see which processes use most memory

Good idea! Let me check the top memory-consuming processes.

--- SUGGESTED COMMAND ---
ps aux --sort=-%mem | head -10

Allow? y/n> y
[Executes command]

--- OUTPUT ---
USER       PID %CPU %MEM  COMMAND
user     1234  5.2  3.2  chrome
user     5678  2.1  1.8  node

--- REASONING ---
Chrome and Node are using the most memory. Want me to investigate further?

> how do I check for memory leaks?

Valgrind is the standard tool for detecting memory leaks.

--- SUGGESTED COMMAND ---
valgrind --tool=memcheck --leak-check=full ./myapp

Allow? y/n> yes

[Executes command]

--- OUTPUT ---
==1234== HEAP SUMMARY
==1234== total heap usage: 100 allocs, 0 frees

--- REASONING ---
No memory leaks detected. Want to check anything else?

> /abort

Session ended.
```

---

## Example: Complex Multi-Step

```
$ vibe_cli --react "debug why my web server is slow"

--- REASONING ---
I'll help you debug the performance issue. Let me start by checking the current system state.

> ok

--- SUGGESTED COMMAND ---
uptime && free -h && df -h

Allow? y/n> y
[Executes command]

--- OUTPUT ---
 14:32:01 up 45 days,  3:22,  2 users, load average: 3.42, 2.85, 2.60
              total        used        free      shared  buff/cache   available
Mem:          31Gi        28Gi       1.2Gi       512Mi        1.8Gi       2.1Gi
Swap:         4.0Gi       3.8Gi       200Mi
Filesystem      Size  Used Avail Use% Mounted on
/dev/sda1      100G   95G   5G  100% /

--- REASONING ---
I see multiple issues:
1. High load average (3.42)
2. Memory nearly exhausted (28GB/31GB used)
3. Disk space critical (100% used, only 5GB free)

Let me check the web server processes first.

> yes

--- SUGGESTED COMMAND ---
ps aux --sort=-cpu | head -20

Allow? y/n> y
[Executes command]

--- OUTPUT ---
USER       PID %CPU %MEM  COMMAND
root      892  98.0  2.1  nginx
www-data 1234  45.2  8.2  php-fpm

--- REASONING ---
Nginx and PHP-FPM processes are consuming high CPU. Let me check the nginx error log.

--- SUGGESTED COMMAND ---
sudo tail -100 /var/log/nginx/error.log

Allow? y/n> 

(empty = execute)

[Executes command]

--- OUTPUT ---
2026/02/13 14:25:01 [emerg] worker process 1234 killed by signal 9

--- REASONING ---
Found the issue! The OOM killer is terminating PHP-FPM workers.

Let me check dmesg for confirmation.

> yes

--- SUGGESTED COMMAND ---
dmesg | grep -i "kill|oom" | tail -20

Allow? y/n> y
[Executes command]

--- OUTPUT ---
[12345.678901] Out of memory: Killed process 1234 (php-fpm)

--- REASONING ---
Confirmed! The OOM killer is terminating PHP-FPM workers due to memory exhaustion.

--- SUGGESTED COMMAND ---
sudo systemctl restart php8.2-fpm

Allow? y/n> n
let's just restart php-fpm first

--- REASONING ---
Good call. Restarting PHP-FPM will clear the stuck processes.

--- SUGGESTED COMMAND ---
sudo systemctl restart php-fpm

Allow? y/n> y
[Executes command]

--- OUTPUT ---
● php8.2-fpm.service - The PHP FastCGI Process Manager
     Active: active (running)

--- REASONING ---
PHP-FPM restarted. Let me verify the server is responding.

--- SUGGESTED COMMAND ---
curl -s -o /dev/null -w "%{http_code}" http://localhost/

Allow? y/n> y
[Executes command]

--- OUTPUT ---
200

--- REASONING ---
Server is responding with HTTP 200!

Summary:
1. Root cause: OOM killer terminating PHP-FPM workers
2. Fix applied: Restarted PHP-FPM
3. Server is now healthy

For long-term fix:
- Clear disk space (100% used)
- Add more RAM or reduce PHP-FPM memory limits

> /abort

Session ended.
```

---

## Key Patterns

1. **User confirms with**: `ok`, `yes`, or just presses Enter
2. **User provides new direction**: `n` + new request ("let's restart php-fpm first")
3. **User asks questions**: `n` + question ("how do I check for memory leaks?")
4. **User changes approach**: Multiple times user redirected the flow
5. **Command failures**: Handled gracefully, AI adapts
6. **Verification**: After each fix, verify with curl/check
7. **Summary**: End with reasoning + next steps

---

## Implementation Details

### Input Interpretation

After `Allow? y/n>`:

| Input | Action |
|-------|--------|
| `y`, `yes`, `Enter` | Execute suggestion |
| `n`, `no` | Go to free input mode |
| Any text | Treat as user direction, pass to AI |
| `/abort` | Exit session |
| `/help` | Show commands |
| `/skip` | Skip to next step |

### Free Input Mode

After user enters non-confirm input:

```
Allow? y/n> how do I check memory?
> (implicit free input)

Valgrind is the standard tool...

--- SUGGESTED COMMAND ---
valgrind --tool=memcheck --leak-check=full ./myapp

Allow? y/n>
```

### Label Changes

| Old | New |
|-----|-----|
| `THOUGHT` | `REASONING` |
| `ACTION` | `SUGGESTED COMMAND` |
| `OBSERVATION` | `OUTPUT` |

### Built-in Commands

| Command | Description |
|---------|-------------|
| `/abort` | Exit session |
| `/help` | Show available commands |
| `/skip` | Skip current action |
| `/context` | Show reasoning history |

---

## Integration with Scaling

When test-time compute scaling is enabled:

1. Run scaling to select best command from candidates
2. Apply before showing suggestion
3. Show "[Scaling selected: command]" indicator

---

## Files to Modify

| File | Changes |
|------|---------|
| `presentation/src/cli/handlers/react.rs` | Complete rewrite of input handling |
| Labels and prompts throughout | Update to new format |

---

## Questions

1. **Confirmation for destructive commands**: Should `remove`, `write` require extra confirmation?
2. **Empty input behavior**: Should empty input = execute or = ask for input?
3. **Scaling integration**: Should scaling run at each step or only when multiple candidates?

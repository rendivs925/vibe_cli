#!/bin/bash

# Vibe CLI Load Testing Infrastructure
# Simulates concurrent users and real-world usage patterns

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_RESULTS_DIR="${PROJECT_ROOT}/test_results"
LOAD_TEST_LOG="${TEST_RESULTS_DIR}/load_test.log"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() {
    echo -e "${BLUE}[LOAD_TEST]${NC} $1" | tee -a "$LOAD_TEST_LOG"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$LOAD_TEST_LOG"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOAD_TEST_LOG"
}

# User simulation function
simulate_user() {
    local user_id=$1
    local scenario_file=$2
    local duration=$3

    log_info "User $user_id starting simulation (duration: ${duration}s)"

    local start_time
    start_time=$(date +%s)
    local end_time
    end_time=$((start_time + duration))
    local query_count=0
    local success_count=0

    while [ $(date +%s) -lt $end_time ]; do
        # Select random scenario
        local scenario
        scenario=$(shuf -n 1 "$scenario_file" 2>/dev/null || echo "list files")
        scenario="${scenario%"${scenario##*[![:space:]]}"}" # Trim trailing whitespace

        # Add some realistic delay between queries (1-5 seconds)
        local delay=$((RANDOM % 5 + 1))
        sleep $delay

        # Execute query with timeout
        if timeout 30s bash -c "
            cd '$PROJECT_ROOT'
            echo 'y' | cargo run --bin vibe_cli -- '$scenario' > /dev/null 2>&1
        " 2>/dev/null; then
            success_count=$((success_count + 1))
        fi

        query_count=$((query_count + 1))

        # Progress update every 10 queries
        if [ $((query_count % 10)) -eq 0 ]; then
            log_info "User $user_id: $query_count queries processed ($success_count successful)"
        fi
    done

    local success_rate=0
    if [ $query_count -gt 0 ]; then
        success_rate=$((success_count * 100 / query_count))
    fi

    # Return results for aggregation
    printf '%s,%s,%s,%s\n' "$user_id" "$query_count" "$success_count" "$success_rate"

    log_success "User $user_id completed: $query_count queries, $success_count successful (${success_rate}% success rate)"
    echo "$user_id,$query_count,$success_count,$success_rate"
}

# Generate realistic scenario files
generate_scenario_files() {
    log_info "Generating realistic scenario files..."

    # Developer queries
    cat > "${TEST_RESULTS_DIR}/developer_scenarios.txt" << 'EOF'
find all Rust source files
show git status
check for compilation errors
find unused imports
explain how authentication works
locate database connection code
find error handling patterns
check test coverage
find TODO comments
show directory structure
analyze performance bottlenecks
find security vulnerabilities
check code formatting
identify code smells
find deprecated API usage
EOF

    # System admin queries
    cat > "${TEST_RESULTS_DIR}/sysadmin_scenarios.txt" << 'EOF'
show memory usage
display CPU utilization
check disk space
list running processes
find large files
show network status
check system logs
monitor services
find temporary files
check permissions
audit security
show system info
check network connections
find zombie processes
monitor temperature
EOF

    # Mixed workload queries
    cat > "${TEST_RESULTS_DIR}/mixed_scenarios.txt" << 'EOF'
list files in current directory
show disk usage
find all configuration files
check memory usage
display network status
show running processes
find large files over 100MB
check git status
find recently modified files
show environment variables
check system logs
find duplicate files
show file permissions
check service status
find hidden files
EOF

    log_success "Generated scenario files in $TEST_RESULTS_DIR"
}

# Run concurrent load test
run_concurrent_load_test() {
    local concurrent_users=$1
    local duration=$2
    local scenario_type=${3:-mixed}

    log_info "Starting concurrent load test: $concurrent_users users, ${duration}s duration, $scenario_type scenarios"

    local scenario_file
    scenario_file="${TEST_RESULTS_DIR}/${scenario_type}_scenarios.txt"

    if [ ! -f "$scenario_file" ]; then
        log_error "Scenario file not found: $scenario_file"
        return 1
    fi

    local start_time
    start_time=$(date +%s)
    local pids=()
    local results=()

    # Start concurrent users
    for ((user_id=1; user_id<=concurrent_users; user_id++)); do
        simulate_user "$user_id" "$scenario_file" "$duration" &
        pids+=($!)
    done

    # Wait for all users and collect results
    for ((i=0; i<concurrent_users; i++)); do
        local result
        result=$(wait "${pids[$i]}" 2>/dev/null || echo "0,0,0,0")
        results+=("$result")
    done

    local end_time
    end_time=$(date +%s)
    local total_duration
    total_duration=$((end_time - start_time))

    # Aggregate results
    local total_queries=0
    local total_successful=0
    local total_success_rate=0

    for result in "${results[@]}"; do
        IFS=',' read -r user_id queries successful rate <<< "$result"
        total_queries=$((total_queries + queries))
        total_successful=$((total_successful + successful))
    done

    if [ $total_queries -gt 0 ]; then
        total_success_rate=$((total_successful * 100 / total_queries))
    fi

    local throughput=$((total_queries * 3600 / total_duration)) # queries per hour

    log_success "Load test completed in ${total_duration}s"
    log_success "Results: $total_queries total queries, $total_successful successful (${total_success_rate}% success rate)"
    log_success "Throughput: ${throughput} queries/hour, $((total_queries / total_duration)) queries/second"

    # Record metrics
    echo "$(date +%s),load.concurrent_users,$concurrent_users,count" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),load.duration,$total_duration,seconds" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),load.total_queries,$total_queries,count" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),load.successful_queries,$total_successful,count" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),load.success_rate,$total_success_rate,percent" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),load.throughput,$throughput,queries_per_hour" >> "${TEST_RESULTS_DIR}/metrics.log"
}

# Run gradual load increase test
run_ramp_up_test() {
    local max_users=$1
    local duration_per_level=$2

    log_info "Starting ramp-up load test: 1 to $max_users users, ${duration_per_level}s per level"

    for ((users=1; users<=max_users; users++)); do
        log_info "Ramp-up phase: $users concurrent users"
        run_concurrent_load_test "$users" "$duration_per_level" "mixed"

        # Brief pause between levels
        sleep 5
    done

    log_success "Ramp-up test completed"
}

# Run stress test with maximum load
run_stress_test() {
    local max_users=$1
    local duration=$2

    log_info "Starting stress test with maximum load: $max_users users for ${duration}s"

    # Monitor system resources during stress test
    log_info "Monitoring system resources during stress test..."

    local cpu_before
    cpu_before=$(uptime | awk -F'load average:' '{ print $2 }' | cut -d, -f1 | xargs)
    local mem_before
    mem_before=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')

    run_concurrent_load_test "$max_users" "$duration" "mixed"

    local cpu_after
    cpu_after=$(uptime | awk -F'load average:' '{ print $2 }' | cut -d, -f1 | xargs)
    local mem_after
    mem_after=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')

    log_success "Stress test resource usage:"
    log_success "CPU Load: $cpu_before → $cpu_after"
    log_success "Memory Usage: ${mem_before}% → ${mem_after}%"

    # Record resource metrics
    echo "$(date +%s),stress.cpu_before,$cpu_before,load" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),stress.cpu_after,$cpu_after,load" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),stress.mem_before,$mem_before,percent" >> "${TEST_RESULTS_DIR}/metrics.log"
    echo "$(date +%s),stress.mem_after,$mem_after,percent" >> "${TEST_RESULTS_DIR}/metrics.log"
}

# Run endurance test (long duration, moderate load)
run_endurance_test() {
    local users=$1
    local duration=$2

    log_info "Starting endurance test: $users users for ${duration}s"

    local start_time
    start_time=$(date +%s)
    local check_interval=300 # 5 minutes

    # Run the test in background and monitor periodically
    run_concurrent_load_test "$users" "$duration" "mixed" &
    local test_pid=$!

    while kill -0 $test_pid 2>/dev/null; do
        sleep $check_interval

        local elapsed
        elapsed=$(( $(date +%s) - start_time ))
        local mem_usage
        mem_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
        local cpu_load
        cpu_load=$(uptime | awk -F'load average:' '{ print $2 }' | cut -d, -f1 | xargs)

        log_info "Endurance check (${elapsed}s): CPU=$cpu_load, Memory=${mem_usage}%"

        echo "$(date +%s),endurance.elapsed,$elapsed,seconds" >> "${TEST_RESULTS_DIR}/metrics.log"
        echo "$(date +%s),endurance.cpu_load,$cpu_load,load" >> "${TEST_RESULTS_DIR}/metrics.log"
        echo "$(date +%s),endurance.mem_usage,$mem_usage,percent" >> "${TEST_RESULTS_DIR}/metrics.log"

        if [ $elapsed -ge $duration ]; then
            break
        fi
    done

    wait $test_pid 2>/dev/null
    log_success "Endurance test completed"
}

# Main execution
main() {
    local command=${1:-help}

    # Setup
    mkdir -p "$TEST_RESULTS_DIR"
    echo "=== Vibe CLI Load Test Session $(date) ===" > "$LOAD_TEST_LOG"

    case $command in
        "generate-scenarios")
            generate_scenario_files
            ;;
        "concurrent")
            local users=${2:-5}
            local duration=${3:-60}
            local scenario_type=${4:-mixed}
            run_concurrent_load_test "$users" "$duration" "$scenario_type"
            ;;
        "ramp-up")
            local max_users=${2:-10}
            local duration_per_level=${3:-30}
            run_ramp_up_test "$max_users" "$duration_per_level"
            ;;
        "stress")
            local max_users=${2:-20}
            local duration=${3:-120}
            run_stress_test "$max_users" "$duration"
            ;;
        "endurance")
            local users=${2:-3}
            local duration=${3:-1800} # 30 minutes default
            run_endurance_test "$users" "$duration"
            ;;
        "full-suite")
            log_info "Running complete load testing suite..."

            generate_scenario_files

            log_info "Phase 1: Concurrent load test (5 users, 60s)"
            run_concurrent_load_test 5 60 "mixed"

            log_info "Phase 2: Ramp-up test (1-10 users, 30s each)"
            run_ramp_up_test 10 30

            log_info "Phase 3: Stress test (15 users, 120s)"
            run_stress_test 15 120

            log_info "Phase 4: Endurance test (3 users, 300s)"
            run_endurance_test 3 300

            log_success "Complete load testing suite finished"
            ;;
        "help"|*)
            echo "Vibe CLI Load Testing Infrastructure"
            echo ""
            echo "Usage: $0 <command> [options]"
            echo ""
            echo "Commands:"
            echo "  generate-scenarios          Generate realistic scenario files"
            echo "  concurrent <users> <duration> [scenario_type]"
            echo "                              Run concurrent load test"
            echo "  ramp-up <max_users> <duration_per_level>"
            echo "                              Run gradual load increase test"
            echo "  stress <max_users> <duration>"
            echo "                              Run maximum load stress test"
            echo "  endurance <users> <duration>"
            echo "                              Run long-duration endurance test"
            echo "  full-suite                  Run complete testing suite"
            echo "  help                        Show this help message"
            echo ""
            echo "Scenario types: developer, sysadmin, mixed"
            echo "Results: $TEST_RESULTS_DIR"
            ;;
    esac
}

# Run main function with all arguments
main "$@"
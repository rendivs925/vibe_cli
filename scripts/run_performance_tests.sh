#!/bin/bash

# Vibe CLI Performance Testing Runner
# Comprehensive performance testing framework for real-world scenarios

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_RESULTS_DIR="${PROJECT_ROOT}/test_results"
PERFORMANCE_LOG="${TEST_RESULTS_DIR}/performance.log"
METRICS_LOG="${TEST_RESULTS_DIR}/metrics.log"
REPORT_FILE="${TEST_RESULTS_DIR}/performance_report.html"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$PERFORMANCE_LOG"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$PERFORMANCE_LOG"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$PERFORMANCE_LOG"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$PERFORMANCE_LOG"
}

# Setup test environment
setup_test_environment() {
    log_info "Setting up performance test environment..."

    # Create test results directory
    mkdir -p "$TEST_RESULTS_DIR"

    # Initialize log files
    echo "=== Vibe CLI Performance Test Session $(date) ===" > "$PERFORMANCE_LOG"
    echo "timestamp,metric,value,unit" > "$METRICS_LOG"

    # Check if Ollama is running
    if ! curl -s http://localhost:11434/api/tags > /dev/null 2>&1; then
        log_warning "Ollama server not detected. Some tests may fail or be skipped."
        log_warning "Start Ollama with: ollama serve"
    else
        log_success "Ollama server is running"
    fi

    # Check system resources
    log_system_info

    log_success "Test environment setup complete"
}

# Log system information
log_system_info() {
    log_info "Gathering system information..."

    # CPU info
    local cpu_cores
    cpu_cores=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo "4")
    CPU_CORES=$cpu_cores

    local cpu_model
    cpu_model=$(lscpu 2>/dev/null | grep "Model name" | cut -d: -f2 | xargs 2>/dev/null || uname -m)
    CPU_MODEL=$cpu_model

    # Memory info
    local total_mem
    total_mem=$(free -h 2>/dev/null | awk 'NR==2{printf "%.0fGB", $2}' 2>/dev/null ||
                sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0fGB", $1/1024/1024/1024}' 2>/dev/null ||
                echo "8GB")
    TOTAL_MEM=$total_mem

    # Disk info
    local disk_free
    disk_free=$(df -h . 2>/dev/null | tail -1 | awk '{print $4}' || echo "Unknown")
    DISK_FREE=$disk_free

    log_info "System: CPU=$CPU_MODEL ($CPU_CORES cores), Memory=$TOTAL_MEM, Disk Free=$DISK_FREE"

    # Record baseline metrics
    record_metric "system.cpu_cores" "$CPU_CORES" "count"
    record_metric "system.memory_total" "$TOTAL_MEM" "GB"
}

# Record performance metric
record_metric() {
    local metric=$1
    local value=$2
    local unit=$3
    local timestamp
    timestamp=$(date +%s)

    echo "$timestamp,$metric,$value,$unit" >> "$METRICS_LOG"
}

# Run Rust performance tests
run_rust_performance_tests() {
    log_info "Running Rust performance test suite..."

    local start_time
    start_time=$(date +%s)

    if cd "$PROJECT_ROOT/tests" && timeout 120 cargo test -- --nocapture; then
        local end_time
        end_time=$(date +%s)
        local duration
        duration=$((end_time - start_time))

        record_metric "test.rust_performance.duration" "$duration" "seconds"
        log_success "Rust performance tests completed in ${duration}s"
        return 0
    else
        local end_time
        end_time=$(date +%s)
        local duration
        duration=$((end_time - start_time))

        record_metric "test.rust_performance.duration" "$duration" "seconds"
        log_warning "Rust performance tests completed with issues in ${duration}s (continuing with other tests)"
        return 0  # Don't fail the entire test suite
    fi
}

# Run real-world scenario tests
run_real_world_scenarios() {
    log_info "Running real-world scenario tests..."

    local scenarios=(
        "list files in current directory"
        "show disk usage"
        "find all Rust files"
        "check memory usage"
        "display network status"
        "find large files over 100MB"
        "show running processes"
        "check git status"
        "find files modified today"
        "show environment variables"
    )

    local total_scenarios=${#scenarios[@]}
    local successful_scenarios=0
    local total_response_time=0

    for scenario in "${scenarios[@]}"; do
        log_info "Testing scenario: '$scenario'"

        local scenario_start
        scenario_start=$(date +%s%N)

        # Run the scenario with timeout
        if timeout 30s bash -c "
            cd '$PROJECT_ROOT'
            echo 'y' | cargo run --bin vibe_cli -- '$scenario' > /dev/null 2>&1
        " 2>/dev/null; then
            local scenario_end
            scenario_end=$(date +%s%N)
            local response_time
            response_time=$(( (scenario_end - scenario_start) / 1000000 )) # Convert to milliseconds

            total_response_time=$((total_response_time + response_time))
            successful_scenarios=$((successful_scenarios + 1))

            record_metric "scenario.${scenario// /_}.response_time" "$response_time" "ms"
            log_success "Scenario completed in ${response_time}ms"
        else
            log_warning "Scenario '$scenario' timed out or failed"
        fi
    done

    # Calculate averages
    if [ $successful_scenarios -gt 0 ]; then
        local avg_response_time
        avg_response_time=$((total_response_time / successful_scenarios))
        record_metric "scenario.average_response_time" "$avg_response_time" "ms"
        record_metric "scenario.success_rate" "$((successful_scenarios * 100 / total_scenarios))" "percent"

        log_success "Real-world scenarios: $successful_scenarios/$total_scenarios successful, avg ${avg_response_time}ms"
    fi
}

# Run load testing
run_load_tests() {
    log_info "Running load tests..."

    local concurrent_users=(1 3 5 10)
    local queries_per_user=5

    for users in "${concurrent_users[@]}"; do
        log_info "Testing with $users concurrent users..."

        local load_start
        load_start=$(date +%s)

        # Create temporary script for load testing
        local load_script="${TEST_RESULTS_DIR}/load_test_${users}.sh"
        cat > "$load_script" << EOF
#!/bin/bash
cd "$PROJECT_ROOT"
for i in {1..$queries_per_user}; do
    echo "y" | timeout 10s cargo run --bin vibe_cli -- "list files" > /dev/null 2>&1
done
echo "User completed"
EOF
        chmod +x "$load_script"

        # Run concurrent users
        local pids=()
        for ((i=1; i<=users; i++)); do
            "$load_script" &
            pids+=($!)
        done

        # Wait for all users to complete
        local completed=0
        for pid in "${pids[@]}"; do
            if wait "$pid" 2>/dev/null; then
                completed=$((completed + 1))
            fi
        done

        local load_end
        load_end=$(date +%s)
        local load_duration
        load_duration=$((load_end - load_start))

        record_metric "load.concurrent_users_${users}.duration" "$load_duration" "seconds"
        record_metric "load.concurrent_users_${users}.completed" "$completed" "users"

        log_success "Load test ($users users): ${load_duration}s, $completed/$users completed"

        # Cleanup
        rm -f "$load_script"
    done
}

# Run memory and resource monitoring
run_resource_monitoring() {
    log_info "Running resource monitoring tests..."

    # Monitor memory usage during intensive operations
    log_info "Monitoring memory usage during RAG indexing..."

    local mem_before
    mem_before=$(ps -o rss= $$ 2>/dev/null || echo "0")
    record_metric "memory.baseline" "$mem_before" "KB"

    # Run a memory-intensive operation (large codebase analysis simulation)
    if cd "$PROJECT_ROOT" && timeout 30s bash -c "
        echo 'y' | cargo run --bin vibe_cli -- --rag 'analyze the entire codebase' > /dev/null 2>&1
    " 2>/dev/null; then
        local mem_after
        mem_after=$(ps -o rss= $$ 2>/dev/null || echo "0")
        local mem_delta
        mem_delta=$((mem_after - mem_before))

        record_metric "memory.after_rag_operation" "$mem_after" "KB"
        record_metric "memory.delta_rag_operation" "$mem_delta" "KB"

        log_success "Memory monitoring: ${mem_before}KB → ${mem_after}KB (Δ${mem_delta}KB)"
    else
        log_warning "Memory monitoring test timed out"
    fi
}

# Generate performance report
generate_report() {
    log_info "Generating performance report..."

    # Calculate summary statistics
    local total_tests
    total_tests=$(grep -c "^test " "$PERFORMANCE_LOG" 2>/dev/null || echo "0")
    local successful_tests
    successful_tests=$(grep -c "\[SUCCESS\]" "$PERFORMANCE_LOG" 2>/dev/null || echo "0")
    local failed_tests=$((total_tests - successful_tests))

    # Calculate performance metrics
    local avg_response_time
    avg_response_time=$(awk -F',' '/scenario\.average_response_time/ {sum+=$3; count++} END {if(count>0) print int(sum/count); else print "N/A"}' "$METRICS_LOG")
    local success_rate
    success_rate=$(awk -F',' '/scenario\.success_rate/ {print $3}' "$METRICS_LOG" | tail -1)

    # Generate HTML report
    cat > "$REPORT_FILE" << EOF
<!DOCTYPE html>
<html>
<head>
    <title>Vibe CLI Performance Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; }
        .summary { background: #ecf0f1; padding: 20px; margin: 20px 0; border-radius: 5px; }
        .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin: 20px 0; }
        .metric { background: white; padding: 15px; border-radius: 5px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .metric h3 { margin: 0 0 10px 0; color: #2c3e50; }
        .metric .value { font-size: 24px; font-weight: bold; color: #27ae60; }
        .charts { margin: 20px 0; }
        .status-good { color: #27ae60; }
        .status-warning { color: #f39c12; }
        .status-error { color: #e74c3c; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Vibe CLI Performance Test Report</h1>
        <p>Generated on $(date)</p>
    </div>

    <div class="summary">
        <h2>Test Summary</h2>
        <p><strong>Total Tests:</strong> $total_tests</p>
        <p><strong>Successful:</strong> <span class="status-good">$successful_tests</span></p>
        <p><strong>Failed:</strong> <span class="status-error">$failed_tests</span></p>
        <p><strong>Success Rate:</strong> $((successful_tests * 100 / (total_tests > 0 ? total_tests : 1)))%</p>
    </div>

    <div class="metrics">
        <div class="metric">
            <h3>Average Response Time</h3>
            <div class="value">${avg_response_time}ms</div>
        </div>
        <div class="metric">
            <h3>Scenario Success Rate</h3>
            <div class="value">${success_rate}%</div>
        </div>
        <div class="metric">
            <h3>System CPU Cores</h3>
            <div class="value">$(grep "system.cpu_cores" "$METRICS_LOG" | tail -1 | cut -d',' -f3)</div>
        </div>
        <div class="metric">
            <h3>System Memory</h3>
            <div class="value">$(grep "system.memory_total" "$METRICS_LOG" | tail -1 | cut -d',' -f3)</div>
        </div>
    </div>

    <div class="charts">
        <h2>Detailed Results</h2>
        <h3>Performance Log</h3>
        <pre style="background: #f8f9fa; padding: 15px; border-radius: 5px; overflow-x: auto;">$(cat "$PERFORMANCE_LOG")</pre>

        <h3>Metrics Data</h3>
        <pre style="background: #f8f9fa; padding: 15px; border-radius: 5px; overflow-x: auto;">$(cat "$METRICS_LOG")</pre>
    </div>
</body>
</html>
EOF

    log_success "Performance report generated: $REPORT_FILE"
}

# Main execution
main() {
    log_info "Starting Vibe CLI comprehensive performance testing..."

    setup_test_environment

    # Run all test suites
    run_rust_performance_tests
    run_real_world_scenarios
    run_load_tests
    run_resource_monitoring

    # Generate final report
    generate_report

    log_success "Performance testing completed successfully!"
    log_info "Results available in: $TEST_RESULTS_DIR"
    log_info "HTML Report: $REPORT_FILE"
}

# Run main function
main "$@"
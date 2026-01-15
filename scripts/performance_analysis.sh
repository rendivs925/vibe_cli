#!/bin/bash

# Vibe CLI Performance Analysis and Reporting
# Analyzes test results and generates performance insights

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_RESULTS_DIR="${PROJECT_ROOT}/test_results"
ANALYSIS_LOG="${TEST_RESULTS_DIR}/analysis.log"
RECOMMENDATIONS_FILE="${TEST_RESULTS_DIR}/recommendations.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging functions
log_info() {
    echo -e "${BLUE}[ANALYSIS]${NC} $1" | tee -a "$ANALYSIS_LOG"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$ANALYSIS_LOG"
}

log_warning() {
 echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$ANALYSIS_LOG"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$ANALYSIS_LOG"
}

log_highlight() {
    echo -e "${CYAN}[INSIGHT]${NC} $1" | tee -a "$ANALYSIS_LOG"
}

# Performance thresholds (adjustable)
RESPONSE_TIME_THRESHOLD=2000  # ms
SUCCESS_RATE_THRESHOLD=90     # %
MEMORY_GROWTH_THRESHOLD=50    # MB
CPU_USAGE_THRESHOLD=80        # %

# Analyze response times
analyze_response_times() {
    log_info "Analyzing response time patterns..."

    if [ ! -f "${TEST_RESULTS_DIR}/metrics.log" ]; then
        log_error "Metrics log not found. Run performance tests first."
        return 1
    fi

    # Extract response time metrics
    local avg_response_time
    avg_response_time=$(grep "scenario.average_response_time" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)

    if [ -n "$avg_response_time" ] && [ "$avg_response_time" != "N/A" ]; then
        log_info "Average response time: ${avg_response_time}ms"

        if [ "$avg_response_time" -gt "$RESPONSE_TIME_THRESHOLD" ]; then
            log_warning "Response time (${avg_response_time}ms) exceeds threshold (${RESPONSE_TIME_THRESHOLD}ms)"
            echo "- **Response Time Issue**: Average response time of ${avg_response_time}ms exceeds the ${RESPONSE_TIME_THRESHOLD}ms threshold" >> "$RECOMMENDATIONS_FILE"
        else
            log_success "Response time (${avg_response_time}ms) is within acceptable limits"
        fi
    else
        log_warning "No response time data available"
    fi

    # Analyze response time distribution
    local response_times
    response_times=$(grep "scenario\..*\.response_time" "${TEST_RESULTS_DIR}/metrics.log" | cut -d',' -f3)

    if [ -n "$response_times" ]; then
        local max_time
        max_time=$(echo "$response_times" | sort -nr | head -1)
        local min_time
        min_time=$(echo "$response_times" | sort -n | head -1)

        log_info "Response time range: ${min_time}ms - ${max_time}ms"

        if [ "$max_time" -gt $((RESPONSE_TIME_THRESHOLD * 2)) ]; then
            log_warning "Some queries are very slow (max: ${max_time}ms)"
            echo "- **Slow Queries Detected**: Maximum response time of ${max_time}ms indicates potential performance issues" >> "$RECOMMENDATIONS_FILE"
        fi
    fi
}

# Analyze success rates
analyze_success_rates() {
    log_info "Analyzing success rate patterns..."

    local success_rate
    success_rate=$(grep "scenario.success_rate" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)

    if [ -n "$success_rate" ]; then
        log_info "Overall success rate: ${success_rate}%"

        if [ "$success_rate" -lt "$SUCCESS_RATE_THRESHOLD" ]; then
            log_warning "Success rate (${success_rate}%) below threshold (${SUCCESS_RATE_THRESHOLD}%)"
            echo "- **Reliability Issue**: Success rate of ${success_rate}% is below the ${SUCCESS_RATE_THRESHOLD}% threshold" >> "$RECOMMENDATIONS_FILE"
        else
            log_success "Success rate (${success_rate}%) is acceptable"
        fi
    else
        log_warning "No success rate data available"
    fi
}

# Analyze resource usage
analyze_resource_usage() {
    log_info "Analyzing resource utilization..."

    # Memory usage analysis
    local mem_before
    mem_before=$(grep "memory.baseline" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)
    local mem_after
    mem_after=$(grep "memory.after_rag_operation" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)

    if [ -n "$mem_before" ] && [ -n "$mem_after" ]; then
        local mem_growth
        mem_growth=$((mem_after - mem_before))

        log_info "Memory usage: ${mem_before}KB → ${mem_after}KB (Δ${mem_growth}KB)"

        if [ "$mem_growth" -gt $((MEMORY_GROWTH_THRESHOLD * 1024)) ]; then
            log_warning "High memory growth detected (${mem_growth}KB)"
            echo "- **Memory Leak Suspected**: Memory growth of ${mem_growth}KB during operations exceeds ${MEMORY_GROWTH_THRESHOLD}MB threshold" >> "$RECOMMENDATIONS_FILE"
        else
            log_success "Memory usage is stable"
        fi
    fi

    # Load test resource analysis
    local cpu_before
    cpu_before=$(grep "stress.cpu_before" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)
    local cpu_after
    cpu_after=$(grep "stress.cpu_after" "${TEST_RESULTS_DIR}/metrics.log" | tail -1 | cut -d',' -f3)

    if [ -n "$cpu_before" ] && [ -n "$cpu_after" ]; then
        log_info "CPU load during stress test: $cpu_before → $cpu_after"

        # Extract numeric part for comparison
        local cpu_before_num
        cpu_before_num=$(echo "$cpu_before" | sed 's/[^0-9.]//g')
        local cpu_after_num
        cpu_after_num=$(echo "$cpu_after" | sed 's/[^0-9.]//g')

        if (( $(echo "$cpu_after_num > $CPU_USAGE_THRESHOLD" | bc -l 2>/dev/null || echo "0") )); then
            log_warning "High CPU usage during load test (${cpu_after})"
            echo "- **CPU Bottleneck**: CPU usage reached ${cpu_after} during stress testing, indicating potential performance limits" >> "$RECOMMENDATIONS_FILE"
        fi
    fi
}

# Analyze load testing results
analyze_load_testing() {
    log_info "Analyzing load testing performance..."

    local load_metrics
    load_metrics=$(grep "^[0-9]*,load\." "${TEST_RESULTS_DIR}/metrics.log")

    if [ -n "$load_metrics" ]; then
        local max_concurrent
        max_concurrent=$(echo "$load_metrics" | grep "load.concurrent_users" | sort -t',' -k3 -nr | head -1 | cut -d',' -f3)

        local total_queries
        total_queries=$(echo "$load_metrics" | grep "load.total_queries" | awk -F',' '{sum+=$3} END {print sum}')

        local total_successful
        total_successful=$(echo "$load_metrics" | grep "load.successful_queries" | awk -F',' '{sum+=$3} END {print sum}')

        if [ -n "$total_queries" ] && [ "$total_queries" -gt 0 ]; then
            local load_success_rate
            load_success_rate=$((total_successful * 100 / total_queries))

            log_info "Load testing results: $max_concurrent max concurrent users, $total_queries total queries, ${load_success_rate}% success rate"

            if [ "$load_success_rate" -lt "$SUCCESS_RATE_THRESHOLD" ]; then
                log_warning "Load test success rate (${load_success_rate}%) indicates scalability issues"
                echo "- **Scalability Issue**: Load testing showed ${load_success_rate}% success rate with $max_concurrent concurrent users" >> "$RECOMMENDATIONS_FILE"
            else
                log_success "System handles $max_concurrent concurrent users well"
            fi
        fi
    else
        log_warning "No load testing data available"
    fi
}

# Analyze bottlenecks
analyze_bottlenecks() {
    log_info "Identifying performance bottlenecks..."

    # Check for AI inference bottlenecks
    local ollama_errors
    ollama_errors=$(grep -i "ollama\|inference\|model" "${TEST_RESULTS_DIR}/performance.log" | grep -i "error\|timeout\|failed" | wc -l)

    if [ "$ollama_errors" -gt 0 ]; then
        log_warning "Detected $ollama_errors AI inference issues"
        echo "- **AI Inference Bottleneck**: $ollama_errors AI inference errors detected, indicating Ollama or model performance issues" >> "$RECOMMENDATIONS_FILE"
    fi

    # Check for database bottlenecks
    local db_slow_queries
    db_slow_queries=$(grep -i "sqlite\|database\|query" "${TEST_RESULTS_DIR}/performance.log" | grep -i "slow\|timeout" | wc -l)

    if [ "$db_slow_queries" -gt 0 ]; then
        log_warning "Detected $db_slow_queries database performance issues"
        echo "- **Database Bottleneck**: $db_slow_queries slow database queries detected, indicating vector storage performance issues" >> "$RECOMMENDATIONS_FILE"
    fi

    # Check for memory issues
    local mem_issues
    mem_issues=$(grep -i "memory\|leak\|allocation" "${TEST_RESULTS_DIR}/performance.log" | grep -i "error\|warning\|high" | wc -l)

    if [ "$mem_issues" -gt 0 ]; then
        log_warning "Detected $mem_issues memory-related issues"
        echo "- **Memory Management**: $mem_issues memory-related issues detected, indicating potential memory leaks or inefficient allocation" >> "$RECOMMENDATIONS_FILE"
    fi
}

# Generate optimization recommendations
generate_recommendations() {
    log_info "Generating optimization recommendations..."

    cat > "$RECOMMENDATIONS_FILE" << 'EOF'
# Vibe CLI Performance Optimization Recommendations

## Executive Summary
This document contains automated recommendations based on performance test results.

## Key Findings
EOF

    # Add findings from analysis
    echo "- Performance testing completed on $(date)" >> "$RECOMMENDATIONS_FILE"
    echo "- Analysis based on real-world usage patterns" >> "$RECOMMENDATIONS_FILE"
    echo "" >> "$RECOMMENDATIONS_FILE"

    cat >> "$RECOMMENDATIONS_FILE" << 'EOF'
## Detailed Recommendations

### Immediate Actions (High Priority)
- [ ] Review and optimize AI model selection and inference parameters
- [ ] Implement connection pooling for external API calls
- [ ] Add comprehensive error handling and retry logic
- [ ] Optimize memory usage patterns in RAG operations

### Medium Priority Optimizations
- [ ] Implement intelligent caching strategies
- [ ] Add query result caching with semantic similarity
- [ ] Optimize vector database queries and indexing
- [ ] Implement progressive loading for large codebases

### Long-term Improvements
- [ ] Consider distributed caching (Redis) for multi-user scenarios
- [ ] Implement model quantization for faster inference
- [ ] Add horizontal scaling capabilities
- [ ] Implement advanced monitoring and alerting

### Monitoring & Alerting
- [ ] Set up performance regression alerts
- [ ] Implement real-time performance monitoring
- [ ] Add automated performance testing in CI/CD
- [ ] Create performance dashboards for stakeholders

## Implementation Priority Matrix

| Component | Current Performance | Target Improvement | Effort Level |
|-----------|-------------------|-------------------|--------------|
| AI Inference | Variable | < 2s response time | High |
| Database Queries | Moderate | < 500ms query time | Medium |
| Memory Usage | Stable | < 100MB per session | Low |
| Concurrent Users | Limited | 10+ simultaneous | High |

## Next Steps
1. Implement high-priority recommendations
2. Re-run performance tests to validate improvements
3. Set up continuous performance monitoring
4. Plan for production scaling requirements

---
*Generated automatically by performance analysis script*
EOF

    log_success "Recommendations generated: $RECOMMENDATIONS_FILE"
}

# Generate performance summary
generate_summary() {
    log_info "Generating performance summary..."

    local summary_file="${TEST_RESULTS_DIR}/performance_summary.md"

    cat > "$summary_file" << EOF
# Vibe CLI Performance Test Summary

**Test Date:** $(date)
**Environment:** $(uname -a)

## System Information
- **CPU:** $(nproc) cores
- **Memory:** $(free -h | grep Mem | awk '{print $2}')
- **Disk:** $(df -h . | tail -1 | awk '{print $4}') available

## Test Results Overview

### Response Times
EOF

    # Add response time summary
    local avg_response
    avg_response=$(grep "scenario.average_response_time" "${TEST_RESULTS_DIR}/metrics.log" 2>/dev/null | tail -1 | cut -d',' -f3 || echo "N/A")
    echo "- **Average Response Time:** ${avg_response}ms" >> "$summary_file"

    local success_rate
    success_rate=$(grep "scenario.success_rate" "${TEST_RESULTS_DIR}/metrics.log" 2>/dev/null | tail -1 | cut -d',' -f3 || echo "N/A")
    echo "- **Success Rate:** ${success_rate}%" >> "$summary_file"

    cat >> "$summary_file" << EOF

### Load Testing
EOF

    local max_users
    max_users=$(grep "load.concurrent_users" "${TEST_RESULTS_DIR}/metrics.log" 2>/dev/null | sort -t',' -k3 -nr | head -1 | cut -d',' -f3 || echo "N/A")
    echo "- **Max Concurrent Users Tested:** ${max_users}" >> "$summary_file"

    local throughput
    throughput=$(grep "load.throughput" "${TEST_RESULTS_DIR}/metrics.log" 2>/dev/null | tail -1 | cut -d',' -f3 || echo "N/A")
    echo "- **Throughput:** ${throughput} queries/hour" >> "$summary_file"

    cat >> "$summary_file" << EOF

### Resource Usage
EOF

    local mem_growth
    mem_growth=$(grep "memory.delta_rag_operation" "${TEST_RESULTS_DIR}/metrics.log" 2>/dev/null | tail -1 | cut -d',' -f3 || echo "N/A")
    echo "- **Memory Growth:** ${mem_growth}KB during operations" >> "$summary_file"

    cat >> "$summary_file" << EOF

## Recommendations
See recommendations.md for detailed optimization suggestions.

---
*Auto-generated performance summary*
EOF

    log_success "Performance summary generated: $summary_file"
}

# Main execution
main() {
    local command=${1:-full-analysis}

    # Setup
    mkdir -p "$TEST_RESULTS_DIR"
    echo "=== Vibe CLI Performance Analysis $(date) ===" > "$ANALYSIS_LOG"

    case $command in
        "response-times")
            analyze_response_times
            ;;
        "success-rates")
            analyze_success_rates
            ;;
        "resources")
            analyze_resource_usage
            ;;
        "load-testing")
            analyze_load_testing
            ;;
        "bottlenecks")
            analyze_bottlenecks
            ;;
        "full-analysis")
            log_info "Running complete performance analysis..."

            analyze_response_times
            analyze_success_rates
            analyze_resource_usage
            analyze_load_testing
            analyze_bottlenecks

            generate_recommendations
            generate_summary

            log_success "Complete performance analysis finished"
            ;;
        "help"|*)
            echo "Vibe CLI Performance Analysis Tool"
            echo ""
            echo "Usage: $0 <command>"
            echo ""
            echo "Commands:"
            echo "  response-times    Analyze response time patterns"
            echo "  success-rates     Analyze success rate patterns"
            echo "  resources         Analyze resource utilization"
            echo "  load-testing      Analyze load testing results"
            echo "  bottlenecks       Identify performance bottlenecks"
            echo "  full-analysis     Run complete analysis suite"
            echo "  help              Show this help message"
            echo ""
            echo "Results: $TEST_RESULTS_DIR"
            ;;
    esac
}

# Run main function with all arguments
main "$@"
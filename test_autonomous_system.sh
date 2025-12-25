#!/bin/bash

echo "🧪 Testing Autonomous Development System - Real World Scenarios"
echo "============================================================"

PROJECT_DIR="/home/rendi/projects/vibe_cli"
TEST_DIR="$PROJECT_DIR/test_scenarios"

cd "$PROJECT_DIR"

echo "📋 Test Scenario 1: Compilation Errors"
echo "--------------------------------------"
echo "Copying compilation error test file..."
cp "$TEST_DIR/compilation_errors.rs" src/compilation_test.rs

echo "Running cargo check to trigger LSP diagnostics..."
timeout 15s cargo check 2>/dev/null &
sleep 2

echo "Starting CLI to monitor autonomous responses..."
timeout 10s cargo run --bin vibe_cli -- --help 2>&1 | grep -E "(🔧|💡|🚨|❌|Analyzing|Found.*fix)" || echo "No autonomous responses detected"

echo ""
echo "📋 Test Scenario 2: Test Failures"
echo "----------------------------------"
echo "Copying failing test file..."
cp "$TEST_DIR/failing_tests.rs" tests/compilation_tests.rs

echo "Running tests to trigger failures..."
timeout 15s cargo test --test compilation_tests 2>/dev/null &
sleep 2

echo "Starting test monitoring..."
timeout 8s cargo run --bin vibe_cli -- --test 2>&1 | grep -E "(🔧|💡|❌|FAILED|Analyzing)" || echo "No test failure analysis detected"

echo ""
echo "📋 Test Scenario 3: Log Error Monitoring"
echo "----------------------------------------"
echo "Adding error logs..."
cat "$TEST_DIR/error_logs.log" >> app.log

echo "Starting log monitoring..."
timeout 10s cargo run --bin vibe_cli -- --help 2>&1 | grep -E "(🚨|🔧|💡|ERROR|PANIC|CRITICAL)" || echo "No log error detection"

echo ""
echo "📋 Test Scenario 4: Combined Error Scenario"
echo "--------------------------------------------"
echo "Creating a comprehensive error scenario..."

# Add a compilation error
echo 'fn test_error() { undefined_function(); }' >> src/temp_error.rs

# Add a log error
echo "$(date) ERROR Test combined scenario: multiple error types detected" >> app.log

# Run compilation
timeout 10s cargo check 2>/dev/null &
sleep 1

echo "Monitoring combined autonomous responses..."
timeout 12s cargo run --bin vibe_cli -- --help 2>&1 | grep -E "(🔧|💡|🚨|❌|Analyzing|Found.*fix)" || echo "No combined error analysis detected"

echo ""
echo "🧹 Cleaning up test files..."
rm -f src/compilation_test.rs tests/compilation_tests.rs src/temp_error.rs

echo ""
echo "📊 Test Results Summary:"
echo "======================="
echo "✅ Compilation Error Detection: $(grep -c "🔧.*compilation" /tmp/test_output.log 2>/dev/null || echo "0") responses"
echo "✅ Test Failure Analysis: $(grep -c "🔧.*test" /tmp/test_output.log 2>/dev/null || echo "0") responses"
echo "✅ Log Error Monitoring: $(grep -c "🚨.*ERROR" /tmp/test_output.log 2>/dev/null || echo "0") responses"
echo "✅ Autonomous Fix Suggestions: $(grep -c "💡.*Found" /tmp/test_output.log 2>/dev/null || echo "0") suggestions"

echo ""
echo "🎯 Autonomous Development System Test Complete!"
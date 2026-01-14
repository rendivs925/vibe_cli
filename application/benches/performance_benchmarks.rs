use application::{
    advanced_scheduler::{AdvancedScheduler, SchedulingStrategy},
    dynamic_scaling::{DynamicScalingController, ScalingConfig, ScalingPolicy, SystemMetrics},
    parallel_agent::{ParallelAgentOrchestrator, SubTask},
    task_decomposer::{DecompositionStrategy, TaskDecomposer},
};
/// Comprehensive performance benchmarks for advanced features
///
/// Run with: cargo bench
/// Generate HTML reports in: target/criterion/
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shared::{
    batch_processing::{BatchProcessor, VectorBatchOps},
    memory_pool::{BufferPool, ObjectPool},
    zero_copy::{concat_strings, join_with_separator, StringInterner},
};

/// Benchmark advanced scheduler performance
fn bench_scheduler(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");

    for strategy in &[
        SchedulingStrategy::FIFO,
        SchedulingStrategy::Priority,
        SchedulingStrategy::WorkStealing,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", strategy)),
            strategy,
            |b, &strategy| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let scheduler = AdvancedScheduler::new(4, strategy);

                        // Submit 100 tasks
                        for i in 0..100 {
                            let task = SubTask {
                                id: format!("task_{}", i),
                                description: format!("Task {}", i),
                                priority: (i % 10) as u8,
                                dependencies: vec![],
                                estimated_complexity: 0.5,
                            };
                            scheduler.submit_task(task).await.unwrap();
                        }

                        // Retrieve all tasks
                        for worker_id in 0..4 {
                            while let Some(_task) = scheduler.get_next_task(worker_id).await {
                                // Process task
                                black_box(_task);
                            }
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark dynamic scaling decisions
fn bench_dynamic_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("dynamic_scaling");

    for policy in &[
        ScalingPolicy::Conservative,
        ScalingPolicy::Aggressive,
        ScalingPolicy::Adaptive,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", policy)),
            policy,
            |b, &policy| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let mut config = ScalingConfig::new(policy);
                        config.cooldown_period_secs = 0; // Disable for benchmark

                        let controller = DynamicScalingController::new(config);

                        // Record metrics and make scaling decisions
                        for i in 0..100 {
                            let mut metrics = SystemMetrics::new();
                            metrics.cpu_utilization = (i as f32 / 100.0).min(1.0);
                            metrics.memory_utilization = (i as f32 / 150.0).min(1.0);
                            metrics.queue_length = i;
                            metrics.active_workers = 2;

                            controller.record_metrics(metrics.clone()).await;
                            let decision = controller.should_scale(&metrics).await.unwrap();
                            controller.apply_scaling(decision).await.unwrap();
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark string interning
fn bench_string_interner(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_interner");

    group.bench_function("intern_unique_strings", |b| {
        b.iter(|| {
            let interner = StringInterner::new();
            for i in 0..1000 {
                black_box(interner.intern(format!("string_{}", i)));
            }
        });
    });

    group.bench_function("intern_repeated_strings", |b| {
        b.iter(|| {
            let interner = StringInterner::new();
            for _ in 0..1000 {
                black_box(interner.intern("repeated_string"));
            }
        });
    });

    group.finish();
}

/// Benchmark string concatenation
fn bench_string_concat(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_concat");

    let parts = vec![
        "Hello", " ", "World", "!", " ", "This", " ", "is", " ", "a", " ", "test",
    ];

    group.bench_function("concat_strings", |b| {
        b.iter(|| {
            black_box(concat_strings(&parts));
        });
    });

    group.bench_function("join_with_separator", |b| {
        let words = vec!["one", "two", "three", "four", "five"];
        b.iter(|| {
            black_box(join_with_separator(words.clone(), ", "));
        });
    });

    group.finish();
}

/// Benchmark object pooling
fn bench_object_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_pool");

    group.bench_function("pool_acquire_release", |b| {
        let pool = ObjectPool::new(|| Vec::<i32>::with_capacity(1024), 100);
        b.iter(|| {
            let mut obj = pool.acquire();
            for i in 0..100 {
                obj.push(i);
            }
            drop(obj); // Return to pool
        });
    });

    group.bench_function("pool_vs_new_allocation", |b| {
        b.iter(|| {
            let mut vec = Vec::<i32>::with_capacity(1024);
            for i in 0..100 {
                vec.push(i);
            }
        });
    });

    group.finish();
}

/// Benchmark buffer pooling
fn bench_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");

    group.bench_function("buffer_pool_reuse", |b| {
        let pool = BufferPool::new(4096, 50);
        b.iter(|| {
            let mut buffer = pool.acquire();
            buffer.extend_from_slice(b"test data");
            drop(buffer);
        });
    });

    group.finish();
}

/// Benchmark batch processing
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");

    group.bench_function("parallel_transform", |b| {
        let items: Vec<i32> = (0..10000).collect();
        b.iter(|| {
            black_box(VectorBatchOps::transform(items.clone(), |x| x * 2));
        });
    });

    group.bench_function("parallel_filter", |b| {
        let items: Vec<i32> = (0..10000).collect();
        b.iter(|| {
            black_box(VectorBatchOps::filter(items.clone(), |x| x % 2 == 0));
        });
    });

    group.bench_function("parallel_sum", |b| {
        let items: Vec<i32> = (0..10000).collect();
        b.iter(|| {
            black_box(VectorBatchOps::sum(items.clone()));
        });
    });

    group.bench_function("batch_processor", |b| {
        let processor = BatchProcessor::new(100);
        let items: Vec<i32> = (0..10000).collect();
        b.iter(|| {
            black_box(processor.process(items.clone(), |x| x * 2));
        });
    });

    group.finish();
}

/// Benchmark task decomposition
fn bench_task_decomposition(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_decomposition");

    for strategy in &[
        DecompositionStrategy::ByFile,
        DecompositionStrategy::ByFeature,
        DecompositionStrategy::ByLayer,
        DecompositionStrategy::Intelligent,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", strategy)),
            strategy,
            |b, &strategy| {
                let decomposer = TaskDecomposer::new(strategy);
                b.iter(|| {
                    black_box(decomposer.decompose("Build a complex authentication system with OAuth2, JWT tokens, role-based access control, and audit logging").unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel agent execution
fn bench_parallel_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_agent");

    group.bench_function("parallel_execution_4_tasks", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let orchestrator = ParallelAgentOrchestrator::new(4);

                let tasks = vec![
                    SubTask {
                        id: "task_1".to_string(),
                        description: "Task 1".to_string(),
                        priority: 10,
                        dependencies: vec![],
                        estimated_complexity: 0.5,
                    },
                    SubTask {
                        id: "task_2".to_string(),
                        description: "Task 2".to_string(),
                        priority: 9,
                        dependencies: vec![],
                        estimated_complexity: 0.5,
                    },
                    SubTask {
                        id: "task_3".to_string(),
                        description: "Task 3".to_string(),
                        priority: 8,
                        dependencies: vec![],
                        estimated_complexity: 0.5,
                    },
                    SubTask {
                        id: "task_4".to_string(),
                        description: "Task 4".to_string(),
                        priority: 7,
                        dependencies: vec![],
                        estimated_complexity: 0.5,
                    },
                ];

                let executor = |_task: SubTask| async move {
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    Ok(application::parallel_agent::SubTaskResult {
                        task_id: "test".to_string(),
                        success: true,
                        output: "done".to_string(),
                        execution_time_ms: 1,
                        error: None,
                    })
                };

                black_box(
                    orchestrator
                        .execute_parallel(tasks, executor)
                        .await
                        .unwrap(),
                );
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scheduler,
    bench_dynamic_scaling,
    bench_string_interner,
    bench_string_concat,
    bench_object_pool,
    bench_buffer_pool,
    bench_batch_processing,
    bench_task_decomposition,
    bench_parallel_agent,
);

criterion_main!(benches);

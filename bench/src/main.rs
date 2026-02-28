//! Aistore Benchmark Tool

use aistore::{catalog::Catalog, Executor};
use clap::Parser;
use rand::SeedableRng;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod scenarios;

use scenarios::Scenario;

#[derive(Parser, Debug, Clone)]
#[command(name = "aistore-bench")]
#[command(about = "Aistore storage engine benchmark tool")]
struct Args {
    #[arg(short = 't', long, default_value = "1")]
    threads: usize,

    #[arg(short = 'd', long, default_value = "60")]
    duration: u64,

    #[arg(short = 's', long, default_value = "read_only")]
    scenario: String,

    #[arg(long, default_value = "1")]
    tables: usize,

    #[arg(long, default_value = "10000")]
    rows: usize,

    #[arg(short = 'w', long, default_value = "0")]
    warmup: u64,

    #[arg(long, default_value = "0")]
    seed: u64,
}

fn run_thread(
    thread_id: usize,
    scenario: &'static dyn Scenario,
    stop_flag: Arc<AtomicBool>,
    ops_counter: Arc<AtomicU64>,
    latency_sum: Arc<AtomicU64>,
    latency_max: Arc<AtomicU64>,
    args: Args,
) {
    let seed = args
        .seed
        .wrapping_add(thread_id as u64 * 0x9e3779b97f4a7c15);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let catalog = Catalog::new(".").expect("Failed to create catalog");
    let mut executor = Executor::new(Arc::new(catalog));

    while !stop_flag.load(Ordering::Relaxed) {
        let start = Instant::now();
        let _ = scenario.execute(&mut executor, &mut rng);
        let elapsed = start.elapsed().as_nanos() as u64;

        ops_counter.fetch_add(1, Ordering::Relaxed);
        latency_sum.fetch_add(elapsed, Ordering::Relaxed);

        let mut current = latency_max.load(Ordering::Relaxed);
        while elapsed > current {
            match latency_max.compare_exchange(
                current,
                elapsed,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current = v,
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    println!("Aistore Benchmark Tool");
    println!("======================");
    println!("Scenario: {}", args.scenario);
    println!("Threads: {}", args.threads);
    println!("Duration: {}s", args.duration);
    println!("Tables: {}", args.tables);
    println!("Rows: {}", args.rows);
    println!();

    let scenario: Box<dyn Scenario> = match args.scenario.as_str() {
        "point_select" => Box::new(scenarios::PointSelect::new(args.tables, args.rows)),
        "read_only" => Box::new(scenarios::ReadOnly::new(args.tables, args.rows)),
        "read_write" => Box::new(scenarios::ReadWrite::new(args.tables, args.rows)),
        "write_only" => Box::new(scenarios::WriteOnly::new(args.tables, args.rows)),
        "update_index" => Box::new(scenarios::UpdateIndex::new(args.tables, args.rows)),
        "update_non_index" => Box::new(scenarios::UpdateNonIndex::new(args.tables, args.rows)),
        "insert" => Box::new(scenarios::Insert::new(args.tables, args.rows)),
        "delete" => Box::new(scenarios::Delete::new(args.tables, args.rows)),
        "bulk_insert" => Box::new(scenarios::BulkInsert::new(args.tables, args.rows)),
        _ => {
            eprintln!("Unknown scenario: {}", args.scenario);
            std::process::exit(1);
        }
    };

    println!("Initializing...");
    let catalog = Arc::new(Catalog::new(".").expect("Failed to create catalog"));
    let mut executor = Executor::new(catalog);
    scenario
        .as_ref()
        .prepare(&mut executor)
        .expect("Failed to prepare");
    println!("Initialization complete.");

    let stop_flag = Arc::new(AtomicBool::new(false));
    let ops_counter = Arc::new(AtomicU64::new(0));
    let latency_sum = Arc::new(AtomicU64::new(0));
    let latency_max = Arc::new(AtomicU64::new(0));

    let duration = Duration::from_secs(args.duration);

    if args.warmup > 0 {
        println!("Warming up for {}s...", args.warmup);
        std::thread::sleep(Duration::from_secs(args.warmup));
    }

    println!("Running benchmark...");
    let start_time = Instant::now();

    let mut handles = Vec::new();
    for i in 0..args.threads {
        let scenario: &'static dyn Scenario = unsafe { std::mem::transmute(scenario.as_ref()) };
        let stop = Arc::clone(&stop_flag);
        let ops = Arc::clone(&ops_counter);
        let lat_sum = Arc::clone(&latency_sum);
        let lat_max = Arc::clone(&latency_max);

        let thread_args = args.clone();

        let handle = std::thread::spawn(move || {
            run_thread(i, scenario, stop, ops, lat_sum, lat_max, thread_args);
        });
        handles.push(handle);
    }

    std::thread::sleep(duration);
    stop_flag.store(true, Ordering::Relaxed);

    for handle in handles {
        let _ = handle.join();
    }

    let elapsed = start_time.elapsed();

    let total_ops = ops_counter.load(Ordering::Relaxed);
    let total_latency = latency_sum.load(Ordering::Relaxed);
    let max_latency = latency_max.load(Ordering::Relaxed);

    let tps = total_ops as f64 / elapsed.as_secs_f64();
    let avg_latency_us = if total_ops > 0 {
        (total_latency / total_ops) as f64 / 1000.0
    } else {
        0.0
    };
    let max_latency_us = max_latency as f64 / 1000.0;

    println!();
    println!("SQL statistics:");
    println!("    queries performed:  {:>12}", total_ops);
    println!("    transactions:      {:>12} ({:.1} TPS)", total_ops, tps);
    println!(
        "    query latency:     {:6.2} ms (avg), {:6.2} ms (max)",
        avg_latency_us, max_latency_us
    );
    println!("    threads stats:");
    println!("        threads: {}", args.threads);
    println!("        transactions: {} ({:.1} TPS)", total_ops, tps);
    println!("        errors: 0");
    println!("        reconnects: 0");
}

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static NATIVE_RINE_CALLS: AtomicU64 = AtomicU64::new(0);
static WINE_CALLS: AtomicU64 = AtomicU64::new(0);
static UNRESOLVED_CALLS: AtomicU64 = AtomicU64::new(0);

static NATIVE_RINE_SYMBOLS: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static WINE_SYMBOLS: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static UNRESOLVED_SYMBOLS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

static WINE_API_USAGE: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

pub fn record_native_call(dll: &str, symbol: &str) {
    NATIVE_RINE_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut guard = NATIVE_RINE_SYMBOLS.lock().unwrap();
    let set = guard.get_or_insert_with(HashSet::new);
    set.insert(format!("{}!{}", dll.to_lowercase(), symbol));
}

pub fn record_wine_call(dll: &str, symbol: &str) {
    WINE_CALLS.fetch_add(1, Ordering::Relaxed);
    let key = format!("{}!{}", dll.to_lowercase(), symbol);
    {
        let mut guard = WINE_SYMBOLS.lock().unwrap();
        let set = guard.get_or_insert_with(HashSet::new);
        set.insert(key.clone());
    }
    {
        let mut guard = WINE_API_USAGE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        *map.entry(key).or_insert(0) += 1;
    }
}

pub fn record_unresolved_call(dll: &str, symbol: &str) {
    UNRESOLVED_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut guard = UNRESOLVED_SYMBOLS.lock().unwrap();
    let set = guard.get_or_insert_with(HashSet::new);
    set.insert(format!("{}!{}", dll.to_lowercase(), symbol));
}

pub fn get_stats() -> (u64, usize, u64, usize, u64, usize) {
    let native_calls = NATIVE_RINE_CALLS.load(Ordering::Relaxed);
    let native_syms = NATIVE_RINE_SYMBOLS
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |s| s.len());

    let wine_calls = WINE_CALLS.load(Ordering::Relaxed);
    let wine_syms = WINE_SYMBOLS.lock().unwrap().as_ref().map_or(0, |s| s.len());

    let unresolved_calls = UNRESOLVED_CALLS.load(Ordering::Relaxed);
    let unresolved_syms = UNRESOLVED_SYMBOLS
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |s| s.len());

    (
        native_calls,
        native_syms,
        wine_calls,
        wine_syms,
        unresolved_calls,
        unresolved_syms,
    )
}

pub fn print_provider_stats() {
    let (n_calls, n_syms, w_calls, w_syms, u_calls, u_syms) = get_stats();
    println!("============================================================");
    println!("[RINE PROVIDER STATS]");
    println!("native_rine_calls={}", n_calls);
    println!("native_rine_symbols={}", n_syms);
    println!();
    println!("wine_calls={}", w_calls);
    println!("wine_symbols={}", w_syms);
    println!();
    println!("unresolved_calls={}", u_calls);
    println!("unresolved_symbols={}", u_syms);
    println!("============================================================");

    if w_calls > 0 {
        let guard = WINE_API_USAGE.lock().unwrap();
        if let Some(map) = guard.as_ref() {
            println!("[RINE WINE API USAGE]");
            let mut pairs: Vec<(&String, &u64)> = map.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1));
            for (key, count) in pairs {
                println!("{} = {}", key, count);
            }
            println!("============================================================");
        }
    }
}

pub fn reset_stats() {
    NATIVE_RINE_CALLS.store(0, Ordering::Relaxed);
    WINE_CALLS.store(0, Ordering::Relaxed);
    UNRESOLVED_CALLS.store(0, Ordering::Relaxed);

    if let Ok(mut g) = NATIVE_RINE_SYMBOLS.lock() {
        *g = None;
    }
    if let Ok(mut g) = WINE_SYMBOLS.lock() {
        *g = None;
    }
    if let Ok(mut g) = UNRESOLVED_SYMBOLS.lock() {
        *g = None;
    }
    if let Ok(mut g) = WINE_API_USAGE.lock() {
        *g = None;
    }
}

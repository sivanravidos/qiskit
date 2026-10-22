// This code is part of Qiskit.
//
// (C) Copyright IBM 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at https://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

//! Memory layout and deep memory measurement of MIR types and containers.
//!
//! Run with:
//! ```bash
//! cargo run --example mir_memory -p qiskit-mir --features mem_dbg
//! ```

#![allow(clippy::print_stdout)]

use mem_dbg::{DbgFlags, MemDbg, MemSize, SizeFlags};
use qiskit_mir::line_graph::{Index, LineGraph, Node, Slot};
use qiskit_mir::qid::{Qid, QidBit};
use qiskit_mir::symbol_table::{Symbol, SymbolId, SymbolTable};
use std::mem::{align_of, size_of};
use std::num::NonZeroU32;

/// Print one row of the type size table.  `Option<T>` is the same size as `T` when `T` has an
/// unused value for `None` to use.
fn size_of_type_row<T>(name: &str) {
    println!(
        "{:<14} {:>4} B {:>6} {:>9} B",
        name,
        size_of::<T>(),
        align_of::<T>(),
        size_of::<Option<T>>(),
    );
}

/// Build a `LineGraph<T>` of `n` nodes, with weights `make(0)..make(n - 1)`.
fn build_line_graph<T>(n: u32, make: impl Fn(u32) -> T) -> LineGraph<T> {
    let mut graph = LineGraph::<T>::new();
    for val in 0..n {
        graph.push_back(make(val));
    }
    graph
}

/// `size / n` formatted as bytes, or `-` when `n` is 0 (per-item cost is undefined when empty).
fn per_item(size: usize, n: usize) -> String {
    if n == 0 {
        "-".to_string()
    } else {
        format!("{:.2} B", size as f64 / n as f64)
    }
}

/// Print one row of the `LineGraph<T>` of `n` nodes table:
/// - `N`: number of nodes in the graph
/// - `capacity`: total nodes the graph can hold before it has to reallocate
/// - `used`: header + slots in use (excludes spare capacity)
/// - `allocated`: actual allocated memory (including spare capacity)
/// - `used/node`, `alloc/node`: the two sizes divided by the node count, including the header
fn line_graph_summary_row<T>(name: &str, n: u32, make: impl Fn(u32) -> T)
where
    LineGraph<T>: MemSize,
{
    let graph = build_line_graph(n, make);
    let used = graph.mem_size(SizeFlags::default());
    let allocated = graph.mem_size(SizeFlags::CAPACITY);
    let nodes = graph.node_count();
    println!(
        "{:<12}  {:>6}  {:>8}  {:>8} B  {:>9} B  {:>9}  {:>10}",
        name,
        nodes,
        graph.capacity(),
        used,
        allocated,
        per_item(used, nodes),
        per_item(allocated, nodes),
    );
}

/// Build a `SymbolTable` of `n` symbols named `v0`, `v1`, ...
fn build_symbol_table(n: u32) -> SymbolTable {
    let mut table = SymbolTable::default();
    for i in 0..n {
        table
            .add(Symbol::new(format!("v{i}")))
            .expect("new symbols have no id yet");
    }
    table
}

/// Print one row of the `SymbolTable` table:
/// - `N`: number of symbols in the table
/// - `used`: header + symbols in use + name bytes (excludes spare capacity)
/// - `allocated`: actual allocated memory (including spare capacity)
/// - `used/symbol`, `alloc/symbol`: the two sizes divided by `N`, including the header
fn symbol_table_summary_row(n: u32) {
    let table = build_symbol_table(n);
    let used = table.mem_size(SizeFlags::default());
    let allocated = table.mem_size(SizeFlags::CAPACITY);
    println!(
        "{:>6}  {:>8} B  {:>9} B  {:>11}  {:>12}",
        n,
        used,
        allocated,
        per_item(used, n as usize),
        per_item(allocated, n as usize),
    );
}

fn main() {
    println!("=== Qiskit MIR Memory Analysis ===");
    println!();

    println!("--- 1. Type Sizes (size_of) ---");
    println!(
        "{:<14} {:>6} {:>6} {:>11}",
        "Type", "size", "align", "Option<T>"
    );
    size_of_type_row::<Index>("Index");
    size_of_type_row::<QidBit>("QidBit");
    size_of_type_row::<Qid>("Qid");
    // `Slot - Node` is the enum tag (+ padding) of `Slot`.  The `LineGraph` header doesn't contain a
    // `T`, so its size is the same for every payload.
    size_of_type_row::<Node<Qid>>("Node<Qid>");
    size_of_type_row::<Slot<Qid>>("Slot<Qid>");
    size_of_type_row::<LineGraph<Qid>>("LineGraph<Qid>");
    size_of_type_row::<SymbolId>("SymbolId");
    size_of_type_row::<Symbol>("Symbol");
    size_of_type_row::<SymbolTable>("SymbolTable");
    println!();

    // `Qid` is a placeholder payload until MIR has an instruction type.  `u32` is its reference
    // (`Qid` is a plain `u32` inside), and `NonZeroU32` shows the cost with an unused value for the
    // `Slot` tag.  `+ 1` because `NonZeroU32` can't hold 0.
    let non_zero = |i: u32| NonZeroU32::new(i + 1).expect("i + 1 is never zero");

    let n = 1000;
    println!("--- 2. LineGraph<T> of N nodes (mem_size) ---");
    println!(
        "{:<12}  {:>6}  {:>8}  {:>10}  {:>11}  {:>9}  {:>10}",
        "T", "N", "capacity", "used", "allocated", "used/node", "alloc/node"
    );
    line_graph_summary_row("NonZeroU32", n, non_zero);
    line_graph_summary_row("u32", n, |i| i);
    line_graph_summary_row("Qid", 0, Qid::virt);
    line_graph_summary_row("Qid", 1, Qid::virt);
    line_graph_summary_row("Qid", n, Qid::virt);
    println!();

    println!("LineGraph<Qid>, {n} nodes:");
    build_line_graph(n, Qid::virt)
        .mem_dbg(DbgFlags::default() | DbgFlags::CAPACITY)
        .expect("failed to print mem_dbg tree");
    println!();

    println!("--- 3. SymbolTable of N symbols (mem_size, names \"v0\", \"v1\", ...) ---");
    println!(
        "{:>6}  {:>10}  {:>11}  {:>11}  {:>12}",
        "N", "used", "allocated", "used/symbol", "alloc/symbol"
    );
    symbol_table_summary_row(0);
    symbol_table_summary_row(1);
    symbol_table_summary_row(n);
    println!();

    println!("SymbolTable, {n} symbols:");
    build_symbol_table(n)
        .mem_dbg(DbgFlags::default() | DbgFlags::CAPACITY)
        .expect("failed to print mem_dbg tree");
}

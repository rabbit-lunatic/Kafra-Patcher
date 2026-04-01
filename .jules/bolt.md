
## Optimization: Process Argument Construction (kpatcher/src/process.rs)
- Replaced `.fold(String::new(), |a, b| a + " " + b.as_ref() + "")` with `args.join(" ")`
- The iterative fold caused multiple allocations and `O(N^2)` memory copying string re-allocations for N arguments.
- By using `.collect::<Vec<String>>()` and `.join(" ")`, we do only a single final allocation for the combined string, increasing execution speed and reducing memory fragmentation.
- The legacy behavior inherently added a leading space to the beginning of the arguments string. This was carefully preserved using `format!(" {}", args.join(" "))` to ensure compatibility with `win32_spawn_process_runas`.
- As this execution path is not repeatedly run (only once per application launch or setup), micro-benchmarking was deemed unnecessary.

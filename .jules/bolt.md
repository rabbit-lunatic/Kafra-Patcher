
## Optimization: Process Argument Construction (kpatcher/src/process.rs)
- Replaced `.fold(String::new(), |a, b| a + " " + b.as_ref() + "")` with `args.join(" ")`
- The iterative fold caused multiple allocations and `O(N^2)` memory copying string re-allocations for N arguments.
- By using `.collect::<Vec<String>>()` and `.join(" ")`, we do only a single final allocation for the combined string, increasing execution speed and reducing memory fragmentation.
- The legacy behavior inherently added a leading space to the beginning of the arguments string. This was carefully preserved using `format!(" {}", args.join(" "))` to ensure compatibility with `win32_spawn_process_runas`.
- As this execution path is not repeatedly run (only once per application launch or setup), micro-benchmarking was deemed unnecessary.

## Optimization: Selective GRF Entry Collection (kpatcher/src/patcher/core.rs)
- Moved `.take(sample_size)` before `.cloned().collect()` in `verify_grf_integrity`.
- The previous implementation collected and cloned ALL entries from the GRF archive into a `Vec`, only to then take a sample of at most 10 entries for verification.
- For large GRFs with hundreds of thousands of entries, this resulted in significant unnecessary memory allocations and CPU overhead due to cloning `GrfFileEntry` structs (which include `String` paths).
- By applying the limit before collection, we reduce the operation from `O(N)` (where N is the number of files in the archive) to `O(S)` (where S is the sample size, typically 10), providing a constant-time performance regardless of the archive size.

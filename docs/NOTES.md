# rust-gpu Notes and Gotchas

This document captures lessons learned and common pitfalls when working with rust-gpu.

## Array Iteration Patterns

### The Problem

rust-gpu has significant limitations around iterating over arrays. Certain patterns that work fine in regular Rust will cause GPU crashes ("Parent device is lost" errors) when compiled to SPIR-V.

### What Doesn't Work

1. **`for..in` over arrays:**
   ```rust
   let items: [T; N] = [...];
   for item in items {  // Crashes GPU
       // ...
   }
   ```

2. **Index-based loops over arrays of complex types:**
   ```rust
   let data: [(Point3, usize, PrimitiveKind); N] = [...];
   for i in 0..N {
       let (pos, idx, kind) = data[i];  // Still crashes GPU
       // ...
   }
   ```

### What Does Work

Hardcode each element access individually rather than using any loop construct:

```rust
// Instead of a loop, explicitly handle each case:
scene.push(Instance::sphere(pos1, radius1, mat1));
scene.push(Instance::sphere(pos2, radius2, mat2));
scene.push(Instance::sphere(pos3, radius3, mat3));
// ... etc
```

### Why This Happens

The SPIR-V backend appears to have difficulty with:
- Iterator machinery over arrays
- Complex indexing patterns with tuples or structs
- Potentially related to how the compiler unrolls or handles stack allocations for array temporaries

### Workarounds

1. **Hardcode individual operations** when the number of items is known and reasonable
2. **Use uniform buffers** to pass data from CPU rather than building arrays in shader code
3. **Keep array element types simple** (primitives rather than tuples/structs) if loops are necessary

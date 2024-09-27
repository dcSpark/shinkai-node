# Shinkai Vector Resources

A powerful native Rust fully in-memory/serializable Vector Search solution.

A Vector Resource is made up of a hierarchy of nodes, where each node can either hold a piece of `Text` or another `Vector Resource`. Vector Searching can be performed starting from the root resource or from any path throughout the entire hierarchy, and with the easy-to-use enum-based TraversalMethod interface, offers great customizability of how results are found/scored.

## Importing Into Your Project

By default the library includes both async & blocking interfaces for parsing files into Vector Resources automatically (with hierarchy/embeddings auto-generated + batched). This ingestion is non-wasm compatible, and is included as a default feature called `desktop-only`.

Past ingestion & generation of the Vector Resources themselves (which requires sending requests to Embedding Generation Server), all other parts of the library are pure Rust and are wasm compatible. As such, you can generate Vector Resources in non-wasm code and simply serialize/send them into the wasm side, to then use them freely with no issues.

To disable `desktop-only` & support wasm, simply import as such:

```
shinkai_vector_resources = { path = "../shinkai-vector-resources", default-features = false }
```

Otherwise if you wish to include the file ingestion/Vector Resource generation interface:

```
shinkai_vector_resources = { path = "../shinkai-vector-resources" }
```

## How To Use Vector Resources

Reference `vector_resource_tests.rs` to see the examples of the basic flow of:

1. Ingesting a source document (pdf/txt/epub/...) into a Vector Resource
2. Generating a query
3. Performing a vector search on the resource using the query
4. Examining results

Reference `vector_resource_tests.rs` to see examples of how to use advanced capabilities such as:

1. Differences between DocumentVectorResource vs. MapVectorResource
2. How pathing works through the hierarchy (and making searches starting at arbitrary paths)
3. Different TaversalMethods available when making a Vector Search
4. Syntactic Vector Searches
5. Manual Vector Resource building (including manual hierarchy building)

## Tests

### Running Tests

Of note, the tests read files which are held outside of this crate in the actual repo. In other words, they will fail if testing the crate alone.

As such, if outside of the repo run:

```
cargo test --test vector_resource_tests
```

Else run:

```
cargo test
```

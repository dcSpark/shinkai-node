# OpenSea Posts
Timestamp: {{{timestamp:2024-04-15T18:32:45+00:00}}}
Username: {{{username:kendale}}}
Content: 🚀 KoL Token has successfully launched! Trading Party on OpenSea!
https://opensea.io/collection/cnft-of-kol-token
Likes: {{{likes:0}}}
Recasts: {{{recasts:0}}}
Replies: {{{replies:0}}}
Post Hash: {{{hash:0x43b9a4bc24246855e3d5f4459a7a3d79e50505e6}}}
Parent Hash: {{{parenthash:0x43b9a4bc24246855e3d5f4459a7a3d79e50505e6}}}
Thread Hash: {{{threadHash:0x43b9a4bc24246855e3d5f4459a7a3d79e50505e6}}}
Date: {{{date:2024-04}}}

# Farcaster Posts
Timestamp: {{{timestamp:2024-04-18T13:41:13+00:00}}}
Username: {{{username:baubergo-}}}
Content: Pepe Runner 2049
Likes: {{{likes:2}}}
Recasts: {{{recasts:0}}}
Replies: {{{replies:0}}}
Post Hash: {{{hash:0xa64f092dbc8163e67b5a6d4555800762049cd6a9}}}
Parent Hash: {{{parenthash:0xa64f092dbc8163e67b5a6d4555800762049cd6a9}}}
Thread Hash: {{{threadHash:0xa64f092dbc8163e67b5a6d4555800762049cd6a9}}}
Date: {{{date:2024-04}}}

---

Timestamp: {{{timestamp:2024-04-18T04:56:14+00:00}}}
Username: {{{username:rish}}}
Content: i never remember whether its spelled guage or gauge... 

its obvious in hindsight but somehow not obvious when typing
Likes: {{{likes:66}}}
Recasts: {{{recasts:7}}}
Replies: {{{replies:19}}}
Post Hash: {{{hash:0xc35ed69461ab11dd88b76e1715d0fd4a7a50dba1}}}
Parent Hash: {{{parenthash:0xc35ed69461ab11dd88b76e1715d0fd4a7a50dba1}}}
Thread Hash: {{{threadHash:0xc35ed69461ab11dd88b76e1715d0fd4a7a50dba1}}}
Date: {{{date:2024-04}}}

---

Timestamp: {{{timestamp:2024-04-18T02:37:18+00:00}}}
Username: {{{username:rish}}}
Content: legenday pplpleasr + jtgi combo

big fan of both their work

wowow.shibuya.xyz/
Likes: {{{likes:25}}}
Recasts: {{{recasts:1}}}
Replies: {{{replies:3}}}
Post Hash: {{{hash:0xbf6b8fd8dec25cf4e2cbdcd03d5bfc86bfa547ed}}}
Parent Hash: {{{parenthash:0xbf6b8fd8dec25cf4e2cbdcd03d5bfc86bfa547ed}}}
Thread Hash: {{{threadHash:0xbf6b8fd8dec25cf4e2cbdcd03d5bfc86bfa547ed}}}
Date: {{{date:2024-04}}}


# Shinkai Vector Resources

A powerful native Rust fully in-memory/serializable Vector Search solution.

A Vector Resource is made up of a hierarchy of nodes, where each node can either hold a piece of `Text` or another `Vector Resource`.

## Importing Into Your Project

To disable [desktop-only](https://www.shinkai.com/) & support ![wasm](https://upload.wikimedia.org/wikipedia/commons/thumb/1/1f/WebAssembly_Logo.svg/1200px-WebAssembly_Logo.svg.png), simply import <b>as such<b>:

```
shinkai_vector_resources = { path = "../shinkai-vector-resources", default-features = false }
```

## How To Use Vector Resources

Reference `unstructured_tests.rs` to see the examples of the basic flow of:

1. Ingesting a source document (pdf/txt/epub/...) into a Vector Resource
2. Generating a query
3. Performing a vector search on the resource using the query
4. Examining results

Reference `vector_resource_tests.rs` to see examples of how to use advanced capabilities such as:

- Differences between DocumentVectorResource vs. MapVectorResource
- How pathing works through the hierarchy (and making searches starting at arbitrary paths)
- Different TaversalMethods available when making a Vector Search
- Syntactic Vector Searches
- Manual Vector Resource building (including manual hierarchy building)

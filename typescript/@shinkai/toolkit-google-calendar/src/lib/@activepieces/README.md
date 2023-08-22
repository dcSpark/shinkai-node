# @shinkai/toolkit + activepieces

This is a (incomplete) Activepieces Adapter for Shinkai Tools.  
Exposes `@activepieces/pieces-common` and `@activepieces/pieces-framework` interfaces.

## Interface Setup

Implement `SHINKAI_AP_HEADERS` and `SHINKAI_AP_INPUT` to interface Shinkai-Toolkit and Activepieces Adapter. 

## Project Setup 
This toolkit should use `tsconfig-paths-webpack-plugin` package and set `tsconfig.json`

```
...
"paths": {
  "@activepieces/pieces-common": ["./src/lib/@activepieces/pieces-common"],
  "@activepieces/pieces-framework": ["./src/lib/@activepieces/pieces-framework"], 
}, 
...
```

Please read `tsconfig-paths-webpack-plugin` for further setup.
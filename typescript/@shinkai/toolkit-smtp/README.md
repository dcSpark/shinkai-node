# Shinkai-Toolkit

## Building & Testing

### Compile 
This command generates the complete packaged output toolkit in `./dist/packaged-shinkai-toolkit.js`

```bash
npm run compile
```

Note, before submitting the packaged toolkit anywhere, we recommend renaming the file to include your toolkit name.

### Test

To test the toolkit you can run:

```bash
npm run test
```


### Generate Toolkit JSON

Generate the Toolkit JSON which holds all relevant definitions for running the toolkit. Your compiled toolkit includues the JSON internally (which the Shinkai node extracts itself), so this is mainly for testing/verifying everything is in order.

You can either run it from the source code using npm:
```bash
npm run toolkit-json
```

Or extract it from the packaged toolkit:

```bash
EMIT_TOOLS=1 node dist/packaged-shinkai-toolkit.js
```

#### Output Example

```json

{
  "toolkit-name": "Shinkai Toolkit",
  "author": "local.shinkai",
  "version": "0.0.1",
  "tools": [
    {
      "name": "isEven",
      "description": "Check if a number is even",
      "input": [
        {
          "name": "number",
          "type": "INT",
          "description": "Integer number to check if is even.",
          "isOptional": false,
          "wrapperType": "none"
        }
      ],
      "output": [
        {
          "name": "isEven",
          "type": "BOOL",
          "description": "Result of the check. True if the number is even.",
          "isOptional": false,
          "wrapperType": "none"
        }
      ]
    }
  ]
}
```

## Development

### Available Decorators
#### Interfaces
  `@isTool` : Define tool
  `@output(string)` : Define input for tool 
  `@input(string)` : Define output for tool

#### Input/Output Fields
  `@isOptional` : Field is optional (undefined)  
  `@isRequired` : Field is required  
  `@description(string)` : Annotate field description    
 
  `@isChar(string?)` : Field is Char (TS interpretation as string). Allows optional description.  
  `@isJSON(string?)` : Field is JSON (TS interpretation as string). Allows optional description.  
  `@isBoolean(string?)` : Field is Boolean. Allows optional description.  
  `@isFloat(string?)` : Field is Float (TS interpretation as number). Allows optional description.  
  `@isInteger(string?)`: Field is Integer (TS interpretation as number). Allows optional description.  
  `@isString(string?)` : Field is String. Allows optional description.  
  `@isEnum(string[], string?)` : Field is ENUM. First field for valid values. Allows optional description.
  `@isArray` : Mark type as Array.  
  
NOTE: string, number, boolean types can be inferred. e.g., 
```
@description('Weight in KG')
weight: number
```

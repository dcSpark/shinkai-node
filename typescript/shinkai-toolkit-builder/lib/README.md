# Shinkai-Toolkit

## Generate Tools Definition

From source: `npm run tools`  
or  
From compiled code: `EMIT_TOOLS=1 node dist/shinkai-tools.js`

Example:
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

## Available Decorators
### Interfaces
  `@isTool` : Define tool
  `@output(string)` : Define input for tool 
  `@input(string)` : Define output for tool

### Input/Output Fields
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
  
NOTE: string, number, boolean types can be infered. e.g., 
```
@description('Weight in KG')
weight: number
```
## Compile Output
`npm run compile`

This command generates the complete source in `./dist/shinkai-tools.js`

## Test
`npm run test`


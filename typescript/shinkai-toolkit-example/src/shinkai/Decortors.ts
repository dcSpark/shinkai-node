import 'reflect-metadata';

import {ToolKit} from '../setup';
import {BaseInput, BaseOutput, BaseTool} from './BaseTool';
import Joi from 'joi';

enum DATA_TYPES {
  BOOLEAN = 'BOOL',
  INTEGER = 'INT',
  FLOAT = 'FLOAT',
  STRING = 'STRING',
  ENUM = 'ENUM',
  CHAR = 'CHAR',
  JSON = 'JSON',
}

export class DecoratorsTools {
  // Store ToolName: {name, description}
  static tools: Record<string, {name: string; description: string}> = {};

  // Store ToolName: [Input Name, Output Name]
  static toolsInOut: Record<string, [string?, string?]> = {};

  // Store ToolName: InputClass
  static classMap: Record<string, typeof BaseInput> = {};

  // Store ToolName: Input JoiSchema Validator
  static validators: Record<string, Joi.ObjectSchema> = {};

  // Store ClassName.FieldName : {type, description ...}
  static ebnf: Record<
    string,
    {
      context?: string;
      type?: DATA_TYPES;
      isOptional?: boolean;
      enum?: string[];
      description?: string;
      wrapperType?: 'none' | 'array';
    }
  > = {};

  public static getInputValidator(toolName: string): Joi.ObjectSchema {
    // This returns a Joi Schema.
    return DecoratorsTools.validators[toolName];
  }

  static generateValidator() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const joiObjects: Record<string, Record<string, any>> = {};

    const fieldNames: string[] = Object.keys(this.ebnf);
    fieldNames.forEach(fullFieldName => {
      const [prefix, fieldName] = fullFieldName.split('.');
      const fieldData = this.ebnf[fullFieldName];

      // From the input, find the tool name.
      let toolName = '';
      Object.keys(DecoratorsTools.toolsInOut).forEach(toolName_ => {
        const x = DecoratorsTools.toolsInOut[toolName_];
        if (x[0] === fieldData.context) {
          toolName = toolName_;
          if (!joiObjects[toolName]) {
            joiObjects[toolName] = {};
          }
        }
      });

      if (!toolName) {
        // Field is output type.
        return;
      }

      // Generate the Joi validation for each field
      const required = !fieldData.isOptional;
      switch (fieldData.type) {
        case DATA_TYPES.BOOLEAN:
          joiObjects[toolName][fieldName] = required
            ? Joi.boolean().required()
            : Joi.boolean();
          break;
        case DATA_TYPES.INTEGER:
          joiObjects[toolName][fieldName] = required
            ? Joi.number().integer().required()
            : Joi.number().integer();
          break;
        case DATA_TYPES.FLOAT:
          joiObjects[toolName][fieldName] = required
            ? Joi.number().required()
            : Joi.number();
          break;
        case DATA_TYPES.STRING:
          joiObjects[toolName][fieldName] = required
            ? Joi.string().required()
            : Joi.string();
          break;
        case DATA_TYPES.ENUM:
          {
            const enm = fieldData.enum as string[];
            joiObjects[toolName][fieldName] = required
              ? Joi.string()
                  .valid(...enm)
                  .required()
              : Joi.string().valid(...enm);
          }
          break;
        case DATA_TYPES.CHAR:
          joiObjects[toolName][fieldName] = required
            ? Joi.string().length(1).required()
            : Joi.string().length(1);
          break;
        case DATA_TYPES.JSON:
          joiObjects[toolName][fieldName] = required
            ? Joi.object().required()
            : Joi.object();
          break;
      }
    });

    // Build the Input Object Validators
    Object.keys(DecoratorsTools.classMap).forEach(className => {
      DecoratorsTools.validators[className] = Joi.object(joiObjects[className]);
    });
  }

  static validate() {
    const interfaces = Object.keys(DecoratorsTools.toolsInOut)
      .map(toolName => DecoratorsTools.toolsInOut[toolName])
      .flat();

    const fieldNames: string[] = Object.keys(this.ebnf);
    fieldNames.forEach(fieldName => {
      const fieldData = this.ebnf[fieldName];

      // Each field requires: context, type and description.
      if (!fieldData.context || !interfaces.includes(fieldData.context)) {
        throw new Error(
          `Field "${fieldName}" has no valid context. 
Use @input or @output to mark the class.`
        );
      }

      if (!fieldData.type) {
        throw new Error(
          `Field "${fieldName}" has no valid type.
Use @isBoolean, @isInteger, @isFloat, @isString, @isChar, @isEnum([]) or @isJSON`
        );
      }

      if (!fieldData.description) {
        throw new Error(
          `Field "${fieldName}" requires a description.
Use @description('') to add a description.`
        );
      }
    });
  }

  static run() {
    // setTimeout is to execute this once all files are loaded, and all decorators are executed.
    setTimeout(() => {
      DecoratorsTools.validate();
      DecoratorsTools.generateValidator();
      if (process.env.EMIT_TOOLS) {
        const config = DecoratorsTools.generateConfig();
        console.log(JSON.stringify(config, null, 2));
      }
    });
  }

  static generateConfig() {
    const toolData = Object.keys(DecoratorsTools.tools).map(toolName => {
      const extract = (contextName: string | undefined) => {
        if (!contextName) throw new Error('No context name provided');
        return Object.keys(DecoratorsTools.ebnf)
          .filter(field => DecoratorsTools.ebnf[field].context === contextName)
          .map(field => {
            const [prefix, fieldName] = field.split('.'); // [input, field
            const f = DecoratorsTools.ebnf[field];
            return {
              name: fieldName,
              type: f.type,
              description: f.description,
              isOptional: f.isOptional || false,
              wrapperType: f.wrapperType || 'none',
            };
          });
      };

      return {
        name: toolName,
        description: DecoratorsTools.tools[toolName].description,
        input: extract(DecoratorsTools.toolsInOut[toolName][0]),
        output: extract(DecoratorsTools.toolsInOut[toolName][1]),
      };
    });
    return {
      'toolkit-name': ToolKit._name,
      author: ToolKit._author,
      version: ToolKit._version,
      tools: toolData,
    };
  }

  static registerField(key: string, contextName: string) {
    if (!DecoratorsTools.ebnf[key]) {
      DecoratorsTools.ebnf[key] = {
        context: contextName,
      };
    }
  }

  static registerFieldAutoType(
    key: string,
    contextName: string,
    type: DATA_TYPES
  ) {
    DecoratorsTools.registerField(key, contextName);
    // Do not override type if already set
    if (!DecoratorsTools.ebnf[key].type) {
      DecoratorsTools.ebnf[key].type = type;
    }
  }

  static registerFieldArray(key: string, contextName: string) {
    DecoratorsTools.registerField(key, contextName);
    DecoratorsTools.ebnf[key].wrapperType = 'array';
  }

  static registerFieldType(key: string, contextName: string, type: DATA_TYPES) {
    DecoratorsTools.registerField(key, contextName);
    DecoratorsTools.ebnf[key].type = type;
  }

  static registerFieldEnumData(key: string, enumValues: string[]) {
    DecoratorsTools.ebnf[key].enum = enumValues;
  }

  static registerFieldOptional(key: string, contextName: string) {
    DecoratorsTools.registerField(key, contextName);
    DecoratorsTools.ebnf[key].isOptional = true;
  }

  static registerFieldRequired(key: string, contextName: string) {
    DecoratorsTools.registerField(key, contextName);
    DecoratorsTools.ebnf[key].isOptional = false;
  }

  static registerFieldDescription(
    key: string,
    contextName: string,
    description: string
  ) {
    DecoratorsTools.registerField(key, contextName);
    DecoratorsTools.ebnf[key].description = description;
  }

  static registerTool(toolName: string, description: string) {
    if (!DecoratorsTools.tools[toolName]) {
      DecoratorsTools.tools[toolName] = {name: toolName, description};
    }
    DecoratorsTools.tools[toolName].name = toolName;
    DecoratorsTools.tools[toolName].description = description;
  }

  static registerClass(className: string, classRef: typeof BaseInput) {
    DecoratorsTools.classMap[className] = classRef;
  }

  static registerToolInput(inputOutputName: string, toolName: string) {
    if (DecoratorsTools.toolsInOut[toolName]?.[0]) {
      throw new Error(`Duplicated input name: "${toolName}"`);
    }
    DecoratorsTools.toolsInOut[toolName] = [
      inputOutputName,
      DecoratorsTools.toolsInOut[toolName]
        ? DecoratorsTools.toolsInOut[toolName][1]
        : undefined,
    ];
  }

  static registerToolOutput(inputOutputName: string, toolName: string) {
    if (DecoratorsTools.toolsInOut[toolName]?.[1]) {
      throw new Error(`Duplicated output name: "${toolName}"`);
    }
    DecoratorsTools.toolsInOut[toolName] = [
      DecoratorsTools.toolsInOut[toolName]
        ? DecoratorsTools.toolsInOut[toolName][0]
        : undefined,
      inputOutputName,
    ];
  }
}

// This async function processes the decorators and
// generates the tool descriptions and validators.
DecoratorsTools.run();

// Decorator for main tool description
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isTool(classDef: any) {
  // Tool description is a non-static member.
  // TODO Find a way to make it static.
  //      abstract static is not allowed by TS.
  const tool = new classDef();
  DecoratorsTools.registerTool(classDef.name, tool.description);
}

// Decorator for input class
export function input(className: string) {
  return function (classDef: typeof BaseInput) {
    const key = classDef.name;
    DecoratorsTools.registerToolInput(key, className);
    DecoratorsTools.registerClass(className, classDef);
  };
}

// Decorator for output class
export function output(className: string) {
  return function (classDef: typeof BaseOutput) {
    const key = classDef.name;
    DecoratorsTools.registerToolOutput(key, className);
  };
}

// Decorator for field description
//
// Description can be set with @description("some description")
// or with type decorators as @isString("some description"),
// @isNumber("some description"), @isEnum([values], "some description"), etc...
export function description(description: string) {
  return function (context: Object, propertyKey: string) {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldDescription(
      fieldName,
      contextName,
      description
    );
    const type = extractTypeFromDecorator(context, propertyKey);
    if (type) {
      DecoratorsTools.registerFieldAutoType(fieldName, contextName, type);
    }
  };
}

function buildFieldName(context: Object, propertyKey: string) {
  return `${context.constructor.name}.${propertyKey}`;
}

// Decorator to mark field as array.
export function isArray(context: Object, propertyKey: string) {
  const contextName = context.constructor.name;
  const fieldName = buildFieldName(context, propertyKey);
  DecoratorsTools.registerFieldArray(fieldName, contextName);
}

// Decorator for String field
export function isString(description?: string) {
  return function (context: Object, propertyKey: string): void {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.STRING
    );
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for Enum field
// @param1 enumValues: string[] - array of possible values
// @param2 description?: string - optional description
export function isEnum(enumValues: string[], description?: string) {
  return (context: Object, propertyKey: string) => {
    const fieldName = buildFieldName(context, propertyKey);

    const contextName = context.constructor.name;
    DecoratorsTools.registerFieldType(fieldName, contextName, DATA_TYPES.ENUM);
    DecoratorsTools.registerFieldEnumData(fieldName, enumValues);
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for Character field
export function isChar(enumValues: string[], description?: string) {
  return (context: Object, propertyKey: string) => {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(fieldName, contextName, DATA_TYPES.CHAR);
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for JSON field
export function isJSON(description?: string) {
  return (context: Object, propertyKey: string) => {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(fieldName, contextName, DATA_TYPES.JSON);
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for Boolean Field
export function isBoolean(description?: string) {
  return function (context: Object, propertyKey: string): void {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.BOOLEAN
    );
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for Integer field
export function isInteger(description?: string) {
  return function (context: Object, propertyKey: string): void {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.INTEGER
    );
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator for Float field
export function isFloat(description?: string) {
  return function (context: Object, propertyKey: string): void {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    DecoratorsTools.registerFieldType(fieldName, contextName, DATA_TYPES.FLOAT);
    if (description) {
      DecoratorsTools.registerFieldDescription(
        fieldName,
        contextName,
        description
      );
    }
  };
}

// Decorator to mark field as Optional
// By default all fields are required.
export function isOptional(context: Object, propertyKey: string): void {
  const contextName = context.constructor.name;
  const fieldName = buildFieldName(context, propertyKey);

  DecoratorsTools.registerFieldOptional(fieldName, contextName);
  const type = extractTypeFromDecorator(context, propertyKey);
  if (type) {
    DecoratorsTools.registerFieldAutoType(fieldName, contextName, type);
  }
}

// Decorator to mark field as Required.
// By default all fields are required (so this decorator is not necessary)
export function isRequired(context: Object, propertyKey: string): void {
  const contextName = context.constructor.name;
  const fieldName = buildFieldName(context, propertyKey);

  DecoratorsTools.registerFieldRequired(fieldName, contextName);
  const type = extractTypeFromDecorator(context, propertyKey);
  if (type) {
    DecoratorsTools.registerFieldAutoType(fieldName, contextName, type);
  }
}

function extractTypeFromDecorator(
  context: Object,
  propertyKey: string
): DATA_TYPES | undefined {
  const typeInfo = Reflect.getMetadata('design:type', context, propertyKey);
  switch (typeInfo.name) {
    case 'String':
      return DATA_TYPES.STRING;
    case 'Number':
      return DATA_TYPES.INTEGER;
    case 'Boolean':
      return DATA_TYPES.BOOLEAN;
    case 'Array':
    case 'Object':
    default:
      return undefined;
  }
}

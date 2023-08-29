// Decorator for toolkit description

import {BaseInput, BaseOutput} from './BaseTool';
import {ShinkaiToolkitLib} from './ShinkaiToolkitLib';
import {DATA_TYPES} from './types';
import 'reflect-metadata';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isToolKit(classDef: any) {
  ShinkaiToolkitLib.registerToolKit(new classDef());
}

// Decorator for tool description
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isTool(classDef: any) {
  // Tool description is a non-static member.
  // TODO Find a way to make it static.
  //      abstract static is not allowed by TS.
  const tool = new classDef();
  ShinkaiToolkitLib.registerTool(classDef.name, tool.description);
}

// Decorator for input class
export function isInput(className: string) {
  return function (classDef: typeof BaseInput) {
    const key = classDef.name;
    ShinkaiToolkitLib.registerToolInput(key, className);
    ShinkaiToolkitLib.registerInputClass(className, classDef);
  };
}

// Decorator for output class
export function isOutput(className: string) {
  return function (classDef: typeof BaseOutput) {
    const key = classDef.name;
    ShinkaiToolkitLib.registerToolOutput(key, className);
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

    ShinkaiToolkitLib.registerFieldDescription(
      fieldName,
      contextName,
      description
    );
    const type = extractTypeFromDecorator(context, propertyKey);
    if (type) {
      ShinkaiToolkitLib.registerFieldAutoType(fieldName, contextName, type);
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
  ShinkaiToolkitLib.registerFieldArray(fieldName, contextName);
}

// Decorator for String field
export function isString(description?: string) {
  return function (context: Object, propertyKey: string): void {
    const contextName = context.constructor.name;
    const fieldName = buildFieldName(context, propertyKey);

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.STRING
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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
    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.ENUM
    );
    ShinkaiToolkitLib.registerFieldEnumData(fieldName, enumValues);
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.CHAR
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.JSON
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.BOOLEAN
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.INTEGER
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

    ShinkaiToolkitLib.registerFieldType(
      fieldName,
      contextName,
      DATA_TYPES.FLOAT
    );
    if (description) {
      ShinkaiToolkitLib.registerFieldDescription(
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

  ShinkaiToolkitLib.registerFieldOptional(fieldName, contextName);
  const type = extractTypeFromDecorator(context, propertyKey);
  if (type) {
    ShinkaiToolkitLib.registerFieldAutoType(fieldName, contextName, type);
  }
}

// Decorator to mark field as Required.
// By default all fields are required (so this decorator is not necessary)
export function isRequired(context: Object, propertyKey: string): void {
  const contextName = context.constructor.name;
  const fieldName = buildFieldName(context, propertyKey);

  ShinkaiToolkitLib.registerFieldRequired(fieldName, contextName);
  const type = extractTypeFromDecorator(context, propertyKey);
  if (type) {
    ShinkaiToolkitLib.registerFieldAutoType(fieldName, contextName, type);
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

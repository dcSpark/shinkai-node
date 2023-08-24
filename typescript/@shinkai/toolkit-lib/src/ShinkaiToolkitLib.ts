import 'reflect-metadata';
import Joi from 'joi';

import {BaseInput} from './BaseTool';
import {DATA_TYPES, ShinkaiFieldIO, ShinkaiFieldHeader} from './types';
import {ShinkaiSetup} from './ShinkaiSetup';

const DEBUG = !!process.env.DEBUG_TOOLKIT;
/**
 * This class is used to:
 *  1. Process the decorators
 *  2. Validate the config
 *  3. Generate the toolkit config
 *
 * IMPORTANT: as decorators are processed at runtime,
 * this class will only have all the data after ALL files
 * are loaded. So validation and emition must be run after
 * all is loaded. To do so, setTimeout(F, 0) can be used
 * to ensure this condition.
 */
export class ShinkaiToolkitLib {
  // ToolKit description
  static toolkit: ShinkaiSetup;

  // Store ToolName: {name, description}
  static tools: Record<
    string,
    {
      name: string;
      description: string;
    }
  > = {};

  // Store ToolName: [Input Name, Output Name]
  private static toolsInOut: Record<string, [string?, string?]> = {};

  // Store ToolName: InputClass
  private static inputClass: Record<string, typeof BaseInput> = {};

  // Store ToolName: Input JoiSchema Validator
  private static validators: Record<string, Joi.ObjectSchema> = {};

  // Store header JoiSchema Validator
  private static headerValidator: Joi.ObjectSchema = Joi.object();

  // Store header x-shinkai-* transformer
  private static headerTransformer: Record<
    string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (input: string) => Record<string, any>
  > = {};

  // Store ClassName.FieldName : {type, description ...}
  private static ebnf: Record<string, ShinkaiFieldIO> = {};

  private static isLibReady = false;

  public static async waitForLib() {
    const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

    while (!ShinkaiToolkitLib.isLibReady) {
      await wait(1);
    }
  }

  public static async getInputValidator(
    toolName: string
  ): Promise<Joi.ObjectSchema> {
    await ShinkaiToolkitLib.waitForLib();

    const validator = ShinkaiToolkitLib.validators[toolName];
    if (!validator) {
      throw new Error(`No validator for ${toolName}`);
    }
    return validator;
  }

  public static async getHeadersValidator(): Promise<{
    validator: Joi.ObjectSchema;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    transformer: Record<string, (input: string) => any>;
  }> {
    await ShinkaiToolkitLib.waitForLib();
    return {
      validator: ShinkaiToolkitLib.headerValidator,
      transformer: ShinkaiToolkitLib.headerTransformer,
    };
  }

  // Main function to generate validators in runtime.
  public static async start() {
    const wait = (ms: number) =>
      new Promise(resolve => setTimeout(() => resolve(null), ms));

    let maxRetries = 100;
    while (!ShinkaiToolkitLib.toolkit) {
      await wait(10);
      maxRetries -= 1;
      if (maxRetries === 0) {
        throw new Error(`No toolkit description provided.
1. Verify that @isToolKit is used. 
2. Verify that Tool is imported through /registry.js and not directly from the /package/tool)'`);
      }
    }
    try {
      await ShinkaiToolkitLib.validate();
      await ShinkaiToolkitLib.generateValidator();
      await ShinkaiToolkitLib.generateHeaderValidator();
    } catch (e) {
      console.log('Error at lib autosetup', e);
      throw e;
    }

    if (DEBUG) {
      console.log('ShinkaiToolkitLib: Toolkit ready');
    }
    ShinkaiToolkitLib.isLibReady = true;
  }

  // Emit the toolkit config.
  public static async emitConfig(): Promise<string> {
    await ShinkaiToolkitLib.waitForLib();
    const config = ShinkaiToolkitLib.generateConfig();
    return JSON.stringify(config, null, 2);
  }

  private static buildFieldJoiValidator(
    type: DATA_TYPES,
    required: boolean,
    isArray: boolean,
    enumList: string[]
  ) {
    switch (type) {
      case DATA_TYPES.BOOLEAN: {
        const val = required ? Joi.boolean().required() : Joi.boolean();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.INTEGER: {
        const val = required
          ? Joi.number().integer().required()
          : Joi.number().integer();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.FLOAT: {
        const val = required ? Joi.number().required() : Joi.number();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.OAUTH:
      case DATA_TYPES.STRING: {
        const val = required ? Joi.string().required() : Joi.string();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.ENUM: {
        if (!enumList) throw new Error('Enum list is requried.');
        const val = required
          ? Joi.string()
              .valid(...enumList)
              .required()
          : Joi.string().valid(...enumList);
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.CHAR: {
        const val = required
          ? Joi.string().length(1).required()
          : Joi.string().length(1);
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.JSON: {
        const val = required ? Joi.object().required() : Joi.object();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      case DATA_TYPES.ISODATE: {
        const val = required ? Joi.date().iso().required() : Joi.date().iso();
        if (required && isArray)
          return Joi.array().min(1).items(val).required();
        return isArray ? Joi.array().items(val) : val;
      }

      default:
        throw new Error(`Unknown type ${type}`);
    }
  }

  private static async generateHeaderValidator() {
    const joiObjects: Record<string, Joi.AnySchema> = {};
    const fields = ShinkaiToolkitLib.toolkit.toolkitHeaders || [];
    fields.forEach((field: ShinkaiFieldHeader) => {
      const header = ShinkaiToolkitLib.fieldNameToHeaderName(field.name);
      if (field.oauth) field.type = DATA_TYPES.OAUTH;
      switch (field.type) {
        case DATA_TYPES.CHAR:
        case DATA_TYPES.ENUM:
        case DATA_TYPES.OAUTH:
        case DATA_TYPES.STRING: {
          ShinkaiToolkitLib.headerTransformer[header] = (input: string) => ({
            [header]: input,
            [field.name]: input,
          });
          break;
        }
        case DATA_TYPES.BOOLEAN: {
          ShinkaiToolkitLib.headerTransformer[header] = (input: string) => ({
            [header]: input === 'true' ? true : input === 'false' ? false : '?',
            [field.name]:
              input === 'true' ? true : input === 'false' ? false : '?',
          });
          break;
        }
        case DATA_TYPES.INTEGER:
        case DATA_TYPES.FLOAT: {
          ShinkaiToolkitLib.headerTransformer[header] = (input: string) => ({
            [header]: +input,
            [field.name]: +input,
          });
          break;
        }
        case DATA_TYPES.JSON: {
          ShinkaiToolkitLib.headerTransformer[header] = (input: string) => ({
            [header]: JSON.parse(input),
            [field.name]: JSON.parse(input),
          });
          break;
        }
        case DATA_TYPES.ISODATE: {
          ShinkaiToolkitLib.headerTransformer[header] = (input: string) => ({
            [header]: new Date(input).toISOString(),
            [field.name]: new Date(input).toISOString(),
          });
          break;
        }

        default:
          throw new Error(`Unknown type ${JSON.stringify(field)}`);
      }
    });

    fields.forEach((field: ShinkaiFieldHeader) => {
      const header = ShinkaiToolkitLib.fieldNameToHeaderName(field.name);
      const validator = ShinkaiToolkitLib.buildFieldJoiValidator(
        field.type!,
        !field.isOptional,
        field.wrapperType === 'array',
        field.enum || []
      );
      joiObjects[field.name] = validator;
      joiObjects[header] = validator;
    });
    ShinkaiToolkitLib.headerValidator = Joi.object(joiObjects);
  }

  private static async generateValidator() {
    const joiObjects: Record<string, Record<string, Joi.AnySchema>> = {};

    const fieldNames: string[] = Object.keys(this.ebnf);
    fieldNames.forEach(fullFieldName => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const [prefix, fieldName] = fullFieldName.split('.');
      const fieldData = this.ebnf[fullFieldName];

      // From the input, find the tool name.
      let toolName = '';
      Object.keys(ShinkaiToolkitLib.toolsInOut).forEach(toolName_ => {
        const inputName = ShinkaiToolkitLib.toolsInOut[toolName_][0];
        if (inputName === fieldData.context) {
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
      joiObjects[toolName][fieldName] =
        ShinkaiToolkitLib.buildFieldJoiValidator(
          fieldData.type!,
          !fieldData.isOptional,
          fieldData.wrapperType === 'array',
          fieldData.enum || []
        );
    });

    // Build the Input Object Validators
    Object.keys(ShinkaiToolkitLib.inputClass).forEach(className => {
      ShinkaiToolkitLib.validators[className] = Joi.object(
        joiObjects[className]
      );
    });
  }

  private static async validate() {
    const interfaces = Object.keys(ShinkaiToolkitLib.toolsInOut)
      .map(toolName => ShinkaiToolkitLib.toolsInOut[toolName])
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

  private static generateBNF(fieldName: string, field: ShinkaiFieldIO) {
    const op = field.isOptional ? '?' : '';
    const array = field.wrapperType === 'array';
    const buildBNF = (type: string) => {
      return `${array ? `[${type} {, ${type}}]` : type}${op}`;
    };

    switch (field.type) {
      case DATA_TYPES.BOOLEAN: {
        return buildBNF('("true"|"false")');
      }
      case DATA_TYPES.INTEGER:
        return buildBNF('(-?[0-9]+)');
      case DATA_TYPES.FLOAT:
        return buildBNF('(-?[0-9]+(.[0-9]+)?)');
      case DATA_TYPES.OAUTH:
      case DATA_TYPES.STRING:
        return buildBNF('([a-zA-Z0-9_]+)');
      case DATA_TYPES.ENUM:
        if (!field.enum)
          throw new Error('Enum types not defined for ' + fieldName);
        return buildBNF('(' + field.enum.map(x => `"${x}"`).join(' | ') + ')');
      case DATA_TYPES.CHAR:
        return buildBNF('([a-zA-Z0-9_])');
      case DATA_TYPES.JSON:
        return buildBNF('(( "{" .* "}" ) | ( "[" .* "]" ))');
      case DATA_TYPES.ISODATE:
        return buildBNF('([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]+:[0-9]+Z)');
      default:
        throw new Error('Unknown type ' + field.type);
    }
  }

  public static fieldNameToHeaderName(fieldName: string) {
    const validHeader = fieldName
      .toLocaleLowerCase()
      .replace(/[^a-z0-9_-]/g, '')
      .replace(/_/g, '-');
    return `x-shinkai-${validHeader}`;
  }

  private static generatetoolkitHeadersFields() {
    const setup: typeof ShinkaiToolkitLib.toolkit = JSON.parse(
      JSON.stringify(ShinkaiToolkitLib.toolkit)
    );
    // Setup setup vars & headers
    if (!setup.toolkitHeaders) return {};

    setup.toolkitHeaders.forEach((field: ShinkaiFieldHeader) => {
      field.header = ShinkaiToolkitLib.fieldNameToHeaderName(field.name);
      if (field.oauth) {
        field.type = DATA_TYPES.OAUTH;
        field.description = field.description || field.oauth.description;
      }
    });
    return setup.toolkitHeaders;
  }

  private static generateConfig() {
    const inputEBNF: string[] = [];
    const toolData = Object.keys(ShinkaiToolkitLib.tools).map(toolName => {
      const extract = (
        contextName: string | undefined,
        allowUndefined = false
      ) => {
        if (!contextName) {
          if (allowUndefined) {
            return [];
          }
          throw new Error('No context name provided');
        }
        return Object.keys(ShinkaiToolkitLib.ebnf)
          .filter(
            field => ShinkaiToolkitLib.ebnf[field].context === contextName
          )
          .map(field => {
            // eslint-disable-next-line @typescript-eslint/no-unused-vars
            const [prefix, fieldName] = field.split('.'); // [input, field
            const f = ShinkaiToolkitLib.ebnf[field];
            inputEBNF.push(
              `${fieldName} ::= ${ShinkaiToolkitLib.generateBNF(fieldName, f)}`
            );
            return {
              name: fieldName,
              type: f.type,
              description: f.description,
              isOptional: f.isOptional || false,
              wrapperType: f.wrapperType || 'none',
              enum: f.enum,
              ebnf: ShinkaiToolkitLib.generateBNF(fieldName, f),
            };
          });
      };

      const input = extract(ShinkaiToolkitLib.toolsInOut[toolName][0]);
      const output = extract(ShinkaiToolkitLib.toolsInOut[toolName][1]);

      return {
        name: toolName,
        description: ShinkaiToolkitLib.tools[toolName].description,
        input,
        output,
        inputEBNF: inputEBNF.join('\n'),
      };
    });

    return {
      ...ShinkaiToolkitLib.toolkit,
      toolkitHeaders: ShinkaiToolkitLib.generatetoolkitHeadersFields(),
      tools: toolData,
    };
  }

  /* Function for decorators to register data */
  static registerField(key: string, contextName: string) {
    if (!ShinkaiToolkitLib.ebnf[key]) {
      ShinkaiToolkitLib.ebnf[key] = {
        name: key,
        context: contextName,
      };
    }
  }

  static registerFieldAutoType(
    key: string,
    contextName: string,
    type: DATA_TYPES
  ) {
    ShinkaiToolkitLib.registerField(key, contextName);
    // Do not override type if already set
    if (!ShinkaiToolkitLib.ebnf[key].type) {
      ShinkaiToolkitLib.ebnf[key].type = type;
    }
  }

  static registerFieldArray(key: string, contextName: string) {
    ShinkaiToolkitLib.registerField(key, contextName);
    ShinkaiToolkitLib.ebnf[key].wrapperType = 'array';
  }

  static registerFieldType(key: string, contextName: string, type: DATA_TYPES) {
    ShinkaiToolkitLib.registerField(key, contextName);
    ShinkaiToolkitLib.ebnf[key].type = type;
  }

  static registerFieldEnumData(key: string, enumValues: string[]) {
    ShinkaiToolkitLib.ebnf[key].enum = enumValues;
  }

  static registerFieldOptional(key: string, contextName: string) {
    ShinkaiToolkitLib.registerField(key, contextName);
    ShinkaiToolkitLib.ebnf[key].isOptional = true;
  }

  static registerFieldRequired(key: string, contextName: string) {
    ShinkaiToolkitLib.registerField(key, contextName);
    ShinkaiToolkitLib.ebnf[key].isOptional = false;
  }

  static registerFieldDescription(
    key: string,
    contextName: string,
    description: string
  ) {
    ShinkaiToolkitLib.registerField(key, contextName);
    ShinkaiToolkitLib.ebnf[key].description = description;
  }

  static registerToolKit(setup: ShinkaiSetup) {
    if (DEBUG) {
      // console.log(`Registering toolkit: ${setup[toolkitName]}`);
    }
    ShinkaiToolkitLib.toolkit = setup;
  }

  static registerTool(toolName: string, description: string) {
    if (ShinkaiToolkitLib.tools[toolName]) {
      throw new Error(`Duplicated tool name: "${toolName}"`);
    }
    if (DEBUG) {
      console.log(`Registering tool: ${toolName}`);
    }
    ShinkaiToolkitLib.tools[toolName] = {
      name: toolName,
      description,
    };
  }

  static registerInputClass(className: string, classRef: typeof BaseInput) {
    if (ShinkaiToolkitLib.inputClass[className]) {
      throw new Error(`Duplicated input class name: "${className}"`);
    }
    if (DEBUG) {
      console.log(`Registering input class: ${className}`);
    }
    ShinkaiToolkitLib.inputClass[className] = classRef;
  }

  static registerToolInput(inputOutputName: string, toolName: string) {
    if (ShinkaiToolkitLib.toolsInOut[toolName]?.[0]) {
      throw new Error(`Duplicated input name: "${toolName}"`);
    }
    if (DEBUG) {
      console.log(`Registering input: ${inputOutputName} for ${toolName}`);
    }
    ShinkaiToolkitLib.toolsInOut[toolName] = [
      inputOutputName,
      ShinkaiToolkitLib.toolsInOut[toolName]
        ? ShinkaiToolkitLib.toolsInOut[toolName][1]
        : undefined,
    ];
  }

  static registerToolOutput(inputOutputName: string, toolName: string) {
    if (ShinkaiToolkitLib.toolsInOut[toolName]?.[1]) {
      throw new Error(`Duplicated output name: "${toolName}"`);
    }
    if (DEBUG) {
      console.log(`Registering output: ${inputOutputName} for ${toolName}`);
    }
    ShinkaiToolkitLib.toolsInOut[toolName] = [
      ShinkaiToolkitLib.toolsInOut[toolName]
        ? ShinkaiToolkitLib.toolsInOut[toolName][0]
        : undefined,
      inputOutputName,
    ];
  }
}

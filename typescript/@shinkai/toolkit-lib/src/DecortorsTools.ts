import 'reflect-metadata';
import Joi from 'joi';

import {BaseInput} from './BaseTool';
import {DATA_TYPES, ShinkaiField, ShinkaiSetup} from './types';

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
export class DecoratorsTools {
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
  private static classMap: Record<string, typeof BaseInput> = {};

  // Store ToolName: Input JoiSchema Validator
  private static validators: Record<string, Joi.ObjectSchema> = {};

  // Store header JoiSchema Validator
  private static headerValidator: Joi.ObjectSchema = Joi.object();

  // Store ClassName.FieldName : {type, description ...}
  private static ebnf: Record<string, ShinkaiField> = {};

  public static async getInputValidator(
    toolName: string
  ): Promise<Joi.ObjectSchema> {
    // If not found, wait until end of event loop.
    if (!DecoratorsTools.validators[toolName]) {
      await wait(0);
    }
    const validator = DecoratorsTools.validators[toolName];
    if (!validator) {
      throw new Error(`No validator for ${toolName}`);
    }
    return validator;
  }

  public static async getHeadersValidator(): Promise<Joi.ObjectSchema> {
    // If not found, wait until end of event loop.
    if (!DecoratorsTools.headerValidator) {
      await wait(0);
    }
    return DecoratorsTools.headerValidator;
  }

  // Main function to generate validators in runtime.
  public static start() {
    DecoratorsTools.validate();
    DecoratorsTools.generateValidator();
  }

  // Emit the toolkit config.
  public static async emitConfig(): Promise<string> {
    return new Promise(resolve => {
      setTimeout(() => {
        // ShinkaiSetup
        const config = DecoratorsTools.generateConfig();
        resolve(JSON.stringify(config, null, 2));
      }, 0);
    });
  }

  private static generateValidator() {
    const joiObjects: Record<string, Record<string, Joi.AnySchema>> = {};

    const fieldNames: string[] = Object.keys(this.ebnf);
    fieldNames.forEach(fullFieldName => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const [prefix, fieldName] = fullFieldName.split('.');
      const fieldData = this.ebnf[fullFieldName];

      // From the input, find the tool name.
      let toolName = '';
      Object.keys(DecoratorsTools.toolsInOut).forEach(toolName_ => {
        const inputName = DecoratorsTools.toolsInOut[toolName_][0];
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
        case DATA_TYPES.ISODATE:
          joiObjects[toolName][fieldName] = required
            ? Joi.date().iso().required()
            : Joi.date().iso();
          break;
        default:
          throw new Error(`Unknown type ${fieldData.type}`);
      }
    });

    // Build the Input Object Validators
    Object.keys(DecoratorsTools.classMap).forEach(className => {
      DecoratorsTools.validators[className] = Joi.object(joiObjects[className]);
    });
  }

  private static validate() {
    if (!DecoratorsTools.toolkit) {
      throw new Error('No toolkit description provided. Please add @isToolKit');
    }

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

  private static generateBNF(fieldName: string, field: ShinkaiField) {
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
      default:
        throw new Error('Unknown type ' + field.type);
    }
  }

  private static generateExecutionSetupFields() {
    const setup: typeof DecoratorsTools.toolkit = JSON.parse(
      JSON.stringify(DecoratorsTools.toolkit)
    );
    // Setup setup vars & headers
    if (!setup.executionSetup) return {};

    setup.executionSetup.forEach((field: ShinkaiField) => {
      field.ebnf = DecoratorsTools.generateBNF(field.name, field);
      const validHeader = field.name
        .toLocaleLowerCase()
        .replace(/[^a-z0-9_-]/g, '')
        .replace(/_/g, '-');
      field.header = `x-shinkai-${validHeader}`;
    });

    // Add oauth header.
    if (DecoratorsTools.toolkit.oauth?.authUrl) {
      if (!setup.executionSetup) setup.executionSetup = [];

      const field: ShinkaiField = {
        name: 'OAUTH',
        type: DATA_TYPES.STRING,
        description: DecoratorsTools.toolkit.oauth.description,
        header: 'x-shinkai-oauth',
      };
      field.ebnf = DecoratorsTools.generateBNF('x-shinkai-oauth', field);

      setup.executionSetup.push(field);
    }
    return setup.executionSetup;
  }

  private static generateConfig() {
    const inputEBNF: string[] = [];
    const toolData = Object.keys(DecoratorsTools.tools).map(toolName => {
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
        return Object.keys(DecoratorsTools.ebnf)
          .filter(field => DecoratorsTools.ebnf[field].context === contextName)
          .map(field => {
            // eslint-disable-next-line @typescript-eslint/no-unused-vars
            const [prefix, fieldName] = field.split('.'); // [input, field
            const f = DecoratorsTools.ebnf[field];
            inputEBNF.push(
              `${fieldName} ::= ${DecoratorsTools.generateBNF(fieldName, f)}`
            );
            return {
              name: fieldName,
              type: f.type,
              description: f.description,
              isOptional: f.isOptional || false,
              wrapperType: f.wrapperType || 'none',
              enum: f.enum,
              ebnf: DecoratorsTools.generateBNF(fieldName, f),
            };
          });
      };

      const input = extract(DecoratorsTools.toolsInOut[toolName][0]);
      const output = extract(DecoratorsTools.toolsInOut[toolName][1]);

      return {
        name: toolName,
        description: DecoratorsTools.tools[toolName].description,
        input,
        output,
        inputEBNF: inputEBNF.join('\n'),
      };
    });

    return {
      ...DecoratorsTools.toolkit,
      executionSetup: DecoratorsTools.generateExecutionSetupFields(),
      tools: toolData,
    };
  }

  /* Function for decorators to register data */
  static registerField(key: string, contextName: string) {
    if (!DecoratorsTools.ebnf[key]) {
      DecoratorsTools.ebnf[key] = {
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

  static registerToolKit(setup: ShinkaiSetup) {
    DecoratorsTools.toolkit = setup;
  }

  static registerTool(toolName: string, description: string) {
    if (!DecoratorsTools.tools[toolName]) {
      DecoratorsTools.tools[toolName] = {
        name: toolName,
        description,
      };
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

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms));

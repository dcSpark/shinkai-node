import {
  isTool,
  isInput,
  isOutput,
  isEnum,
  isInteger,
  BaseTool,
  BaseInput,
  BaseOutput,
} from '@shinkai/toolkit-lib';

@isInput('CompareNumbers')
class CompareInput extends BaseInput {
  @isInteger('Number to check if greater than, lower than or equal than.')
  number!: number;

  @isInteger('Number to compare with.')
  numberToCompare!: number;
}

@isOutput('CompareNumbers')
class CompareOutput extends BaseOutput {
  @isEnum(['GT', 'LT', 'EQ'], 'Result of the comparison.')
  comparison!: string;
}

@isTool
export class CompareNumbers extends BaseTool<CompareInput, CompareOutput> {
  description =
    'Check if number is greater than, lower than or equal to another number.';

  async run(input: CompareInput): Promise<CompareOutput> {
    const out = new CompareOutput();
    if (input.number > input.numberToCompare) {
      out.comparison = 'GT';
    } else if (input.number < input.numberToCompare) {
      out.comparison = 'LT';
    } else {
      out.comparison = 'EQ';
    }

    return out;
  }
}

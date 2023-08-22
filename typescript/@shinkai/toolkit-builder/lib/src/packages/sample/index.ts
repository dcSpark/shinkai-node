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

@isInput('Sample')
class SampleInput extends BaseInput {
  @isInteger('Number to check if is greater than, lower than or equal than.')
  number!: number;

  @isInteger('Number to compare with.')
  numberToCompare!: number;
}

@isOutput('Sample')
class SampleOutput extends BaseOutput {
  @isEnum(['GT', 'LT', 'EQ'], 'Result of the comparison.')
  comparison!: string;
}

@isTool
export class Sample extends BaseTool<SampleInput, SampleOutput> {
  description =
    'Check if number is greater than, lower than or equal to another number.';

  async run(input: SampleInput): Promise<SampleOutput> {
    const out = new SampleOutput();

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

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
  @isInteger('Number to check if greater than, lower than or equal than.')
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
  description = 'Check if a number is even';

  async run(input: SampleInput): Promise<SampleOutput> {
    this.validate(input);

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

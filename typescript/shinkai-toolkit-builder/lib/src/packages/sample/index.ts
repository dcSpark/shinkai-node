import {
  isTool,
  output,
  input,
  isBoolean,
  isInteger,
} from '../../shinkai/Decortors';
import {BaseTool, BaseInput, BaseOutput} from '../../shinkai/BaseTool';

@input('Sample')
class SampleInput extends BaseInput {
  @isInteger('Integer number to check if is even.')
  number!: number;
}

@output('Sample')
class SampleOutput extends BaseOutput {
  @isBoolean('Example: check if value is integer.')
  isInteger!: boolean;
}

@isTool
export class Sample extends BaseTool<SampleInput, SampleOutput> {
  description = 'Check if a number is even';

  async run(input: SampleInput): Promise<SampleOutput> {
    this.validate(input);

    const out = new SampleOutput();
    const convertedToNumber = parseInt(String(input.number), 10);
    // Allow "2" or 2
    out.isInteger = String(convertedToNumber) === String(input.number); 
    return out;
  }
}

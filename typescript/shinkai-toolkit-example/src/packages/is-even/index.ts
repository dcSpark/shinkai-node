import {
  isTool,
  output,
  input,
  isBoolean,
  isInteger,
} from '../../shinkai/Decortors';
import {BaseTool, BaseInput, BaseOutput} from '../../shinkai/BaseTool';

@input('isEven')
class isEvenInput extends BaseInput {
  @isInteger('Integer number to check if is even.')
  number!: number;
}

@output('isEven')
class isEvenOutput extends BaseOutput {
  @isBoolean('Result of the check. True if the number is even.')
  isEven!: boolean;
}

@isTool
export class isEven extends BaseTool<isEvenInput, isEvenOutput> {
  description = 'Check if a number is even';

  async run(input: isEvenInput): Promise<isEvenOutput> {
    this.validate(input);

    const isEven = (input.number || 0) % 2 === 0;
    return {isEven} as any;
  }
}

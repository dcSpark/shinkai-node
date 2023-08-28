# Build Error Data

#!/bin/bash

```
node generate-cmd.js -o fill-memory.json -t ErrorGenerator -i '{"error":"fill-memory"}' -p './dist/packaged-shinkai-toolkit.js'
node generate-cmd.js -o io-block.json -t ErrorGenerator -i '{"error":"io-block"}' -p './dist/packaged-shinkai-toolkit.js'
node generate-cmd.js -o none.json -t ErrorGenerator -i '{"error":"none"}' -p './dist/packaged-shinkai-toolkit.js'
node generate-cmd.js -o terminate.json -t ErrorGenerator -i '{"error":"terminate"}' -p './dist/packaged-shinkai-toolkit.js'
node generate-cmd.js -o throw-exception.json -t ErrorGenerator -i '{"error":"throw-exception"}' -p './dist/packaged-shinkai-toolkit.js'
node generate-cmd.js -o timeout.json -t ErrorGenerator -i '{"error":"timeout"}' -p './dist/packaged-shinkai-toolkit.js'

cp fill-memory.json io-block.json none.json terminate.json throw-exception.json timeout.json ../shinkai-executor/test/errors/
```


# ErrorGenerator Source
```
import {
  isInput,
  BaseInput,
  isOutput,
  BaseOutput,
  isString,
  isTool,
  BaseTool,
  isEnum,
} from '@shinkai/toolkit-lib';

@isInput('ErrorGenerator')
class ErrorGeneratorInput extends BaseInput {
  @isEnum(
    [
      'fill-memory',
      'timeout',
      'io-block',
      'throw-exception',
      'terminate',
      'none',
    ],
    'Error type'
  )
  error:
    | 'fill-memory'
    | 'timeout'
    | 'io-block'
    | 'throw-exception'
    | 'terminate'
    | 'none' = 'none';
}

@isOutput('ErrorGenerator')
class ErrorGeneratorOutput extends BaseOutput {
  @isString('Result')
  ErrorOutput!: string;
}

@isTool
export class ErrorGenerator extends BaseTool<
  ErrorGeneratorInput,
  ErrorGeneratorOutput
> {
  description = 'Check if a number is even';

  async run(input: ErrorGeneratorInput): Promise<ErrorGeneratorOutput> {
    switch (input.error) {
      case 'fill-memory': {
        const array = [];
        // eslint-disable-next-line no-constant-condition
        while (true) {
          array.push(new Array(1000000));
        }
      }
      case 'timeout':
        // eslint-disable-next-line no-constant-condition
        while (true) {
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      case 'io-block': {
        // eslint-disable-next-line no-constant-condition, no-empty
        while (true) {}
      }
      case 'throw-exception':
        throw new Error('ErrorGenerator: throw-exception');
      case 'terminate':
        // eslint-disable-next-line no-process-exit
        process.exit(1);
    }
    const out = new ErrorGeneratorOutput();
    out.ErrorOutput = input.error;
    return out;
  }
}
```

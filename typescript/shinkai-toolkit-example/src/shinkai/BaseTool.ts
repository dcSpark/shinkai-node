export abstract class BaseInput {}
export abstract class BaseOutput {}

export abstract class BaseTool<I extends BaseInput, O extends BaseOutput> {
  abstract description: string;
  abstract run(input: I): Promise<O>;
  protected validate(input: I) {}
}

import {
  isTool,
  isInput,
  isOutput,
  BaseTool,
  BaseInput,
  BaseOutput,
  isString,
  isArray,
  isOptional,
} from '@shinkai/toolkit-lib';

import {smtp} from '../../lib/smtp/src';

@isInput('SMTP')
class SMTPInput extends BaseInput {
  @isString('From email address')
  from!: string;

  @isString('TO email address')
  @isArray
  to!: string[];

  @isString('CC email address')
  @isArray
  @isOptional
  cc?: string[] | undefined;

  @isString('Reply to email address')
  @isOptional
  replyTo?: string | undefined;

  @isString('BCC email address')
  @isArray
  @isOptional
  bcc?: string[] | undefined;

  @isString('Email subject')
  subject!: string;

  @isString('Email body')
  body!: string;
}

@isOutput('SMTP')
class SMTPOutput extends BaseOutput {
  @isString('SMTP result')
  result!: string;
}

@isTool
export class SMTP extends BaseTool<SMTPInput, SMTPOutput> {
  description = 'Send email through smtp connection.';

  async run(
    input: SMTPInput,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    headers: Record<string, any>
  ): Promise<SMTPOutput> {
    const emailStatus = await smtp.actions[0].run({
      auth: {
        host: headers.HOST,
        port: headers.PORT,
        TLS: headers.TLS,
        email: headers.EMAIL,
        password: headers.PASSWORD,
      },
      propsValue: {
        from: input.from,
        to: input.to,
        cc: input.cc || [],
        replyTo: input.replyTo || input.from,
        bcc: input.bcc || [],
        subject: input.subject,
        body: input.body,
      },
    });

    const out = new SMTPOutput();
    out.result = emailStatus;
    return out;
  }
}

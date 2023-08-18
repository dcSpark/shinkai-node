import {
  isTool,
  isOutput,
  isInput,
  isString,
  isOptional,
  BaseTool,
  BaseInput,
  BaseOutput,
  SHINKAI_OAUTH,
  isArray,
} from '@shinkai/toolkit-lib';
import {gmail} from '../../lib/gmail/src/index';
import {Context} from '@activepieces/pieces-framework';
import {SHINKAI_AP_INPUT} from '../../lib/@activepieces/shinkai-activepieces-interface';

@isInput('GmailSendEmail')
class APGmailInput extends BaseInput {
  @isString("Recipients' email addresses")
  @isArray
  receiver!: string[];

  @isString("Email's subject")
  subject!: string;

  @isString("Email's body in plain text")
  body_text!: string;

  @isString('Sender')
  @isArray
  reply_to!: string[];

  @isString("Email's body in HTML format")
  @isOptional
  body_html: string | undefined;

  @isString('Attachment Filename')
  @isOptional
  attachment_filename: string | undefined;

  @isString('Attachment content in base64 format')
  @isOptional
  attachment_base64: string | undefined;

  @isString('Attachement file extension')
  @isOptional
  attachment_extension: string | undefined;
}

@isOutput('GmailSendEmail')
class APGmailOutput extends BaseOutput {
  @isString('Response')
  response!: string;
}

@isTool
export class GmailSendEmail extends BaseTool<APGmailInput, APGmailOutput> {
  description = 'Activepieces Gmail Send Email';
  async run(
    input: APGmailInput,
    headers: Record<string, string>
  ): Promise<APGmailOutput> {
    const sendEmail = gmail.actions[0];

    const attachment =
      input.attachment_base64 &&
      input.attachment_extension &&
      input.attachment_filename
        ? {
            filename: input.attachment_filename,
            base64: input.attachment_base64,
            extension: input.attachment_extension,
          }
        : undefined;

    const setup: Context = {
      auth: {
        access_token: headers['x-shinkai-oauth'] || headers[SHINKAI_OAUTH],
      },
      propsValue: {
        receiver: input.receiver,
        subject: input.subject,
        body_text: input.body_text,
        reply_to: input.reply_to,
        body_html: input.body_html || input.body_text,
        attachment,
      } as SHINKAI_AP_INPUT,
    };
    const response = await sendEmail.run(setup);
    const out = new APGmailOutput();
    out.response = JSON.stringify(response.body);
    return out;
  }
}

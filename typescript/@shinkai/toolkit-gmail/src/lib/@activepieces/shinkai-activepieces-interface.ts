import {GmailLabel, GmailMessageFormat} from '../gmail/src/lib/common/models';

export interface SHINKAI_AP_HEADERS {
  access_token: string;
}

export interface SHINKAI_AP_INPUT {
  subject: string;
  attachment:
    | {
        filename: string;
        base64: string;
        extension: string;
      }
    | undefined;
  reply_to: string[];
  receiver: string[];
  body_text: string;
  body_html: string;
  from: string;
  to: string;
  label: GmailLabel;
  category: string;
  message_id: string;
  format: GmailMessageFormat;
  thread_id: string;
}

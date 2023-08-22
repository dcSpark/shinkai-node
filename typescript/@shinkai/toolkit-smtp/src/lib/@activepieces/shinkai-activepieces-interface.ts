export interface SHINKAI_AP_HEADERS {
  host: string;
  port: number;
  TLS: boolean;
  email: string;
  password: string;
}

export interface SHINKAI_AP_INPUT {
  from: string;
  to: string[];
  cc: string[];
  replyTo: string;
  bcc: string[];
  subject: string;
  body: string;
}

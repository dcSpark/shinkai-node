export interface SHINKAI_AP_HEADERS {
  access_token: string;
}

export interface SHINKAI_AP_INPUT {
  calendar_id: string;
  text: string;
  send_updates: 'none' | 'all' | 'externalOnly';

  title?: string;
  start_date_time?: string;
  end_date_time?: string;
}

import axios, {AxiosResponse} from 'axios';
import querystring from 'querystring';

export interface Polling<T, P> {
  strategy: DedupeStrategy;
  items: (setup: {
    auth: any;
    propsValue: any;
    lastFetchEpochMS: any;
  }) => Promise<{epochMilliSeconds: number; data: any}[]>;
}

/*
const polling: Polling<PiecePropValueSchema<typeof gmailAuth>, PropsValue> = {
  strategy: DedupeStrategy.TIMEBASED,
  items: async ({auth, propsValue, lastFetchEpochMS}) => {
    const items = await getEmail(
      lastFetchEpochMS === 0 ? 5 : 100,
      Math.floor(lastFetchEpochMS / 1000),
      */
export const pollingHelper = {
  onEnable: (
    polling: any,
    setup: {auth: any; store: any; propsValue: any}
  ) => {},
  onDisable: (
    polling: any,
    setup: {auth: any; store: any; propsValue: any}
  ) => {},
  test: (polling: any, setup: {auth: any; store: any; propsValue: any}) => {},
  poll: (polling: any, setup: {auth: any; store: any; propsValue: any}) => {},
};
export enum DedupeStrategy {
  TIMEBASED = 'TIMEBASED',
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type HttpRequest<RequestBody extends {} = any> = {
  method: HttpMethod;
  url: string;
  body?: RequestBody | undefined;
  authentication?:
    | {
        type: AuthenticationType;
        token: string;
      }
    | undefined;
  queryParams?: Record<string, string> | undefined;
  headers?: HttpHeaders;
  timeout?: number;
};

type HttpHeaders = Record<string, string | string[] | undefined>;

export enum HttpMethod {
  POST = 'post',
  GET = 'get',
  PUT = 'put',
  DELETE = 'delete',
}

export enum AuthenticationType {
  BEARER_TOKEN = 'BEARER_TOKEN',
}

export class httpClient {
  public static async sendRequest<T>(request: HttpRequest): Promise<{body: T}> {
    const config = {
      method: request.method,
      url: request.queryParams
        ? request.url + '?' + querystring.stringify(request.queryParams)
        : request.url,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${request.authentication?.token}`,
      },
      data: request.body,
    };
    const response: AxiosResponse<T> = await axios<T>(config);
    return {body: response.data};
  }
}

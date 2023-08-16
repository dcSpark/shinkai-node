import axios, {AxiosResponse} from 'axios';
import querystring from 'querystring';

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

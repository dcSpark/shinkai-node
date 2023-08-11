import axios, {AxiosResponse} from 'axios';

export type HttpRequest = {
  method: HttpMethod;
  url: string;
  body?: Record<string, any>;
  authentication?: {
    type: AuthenticationType;
    token: string;
  };
  queryParams?: Record<string, string>;
};

export enum HttpMethod {
  POST = 'post',
  GET = 'get',
  PUT = 'put',
  DELETE = 'delete',
}

export enum AuthenticationType {
  BEARER_TOKEN = 'BEARER_TOKEN',
}
const querystring = require('querystring');

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

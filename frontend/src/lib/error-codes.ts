export interface ErrorCodeDefinition {
  status: number;
  title: string;
  description: string;
  guidance: string;
}

export const ERROR_CODE_CATALOG: ErrorCodeDefinition[] = [
  {
    status: 400,
    title: 'Bad Request',
    description: 'The request could not be processed because it is malformed or missing required fields.',
    guidance: 'Check query parameters, request payload, and URL encoding.'
  },
  {
    status: 401,
    title: 'Unauthorized',
    description: 'The server rejected the request due to missing or invalid credentials.',
    guidance: 'Verify credentials for private repositories and retry.'
  },
  {
    status: 403,
    title: 'Forbidden',
    description: 'The request was understood, but access is not allowed for this resource.',
    guidance: 'Confirm repository permissions and host-level access rules.'
  },
  {
    status: 404,
    title: 'Not Found',
    description: 'The requested route, repository, or file could not be found.',
    guidance: 'Check repository alias, branch/ref, and file path.'
  },
  {
    status: 408,
    title: 'Request Timeout',
    description: 'The server timed out while waiting for the request to complete.',
    guidance: 'Retry the request and verify network reachability to the git host.'
  },
  {
    status: 409,
    title: 'Conflict',
    description: 'The operation conflicts with the current state of the resource.',
    guidance: 'Refresh data and retry with updated repository state.'
  },
  {
    status: 422,
    title: 'Unprocessable Entity',
    description: 'The payload format is valid, but semantic validation failed.',
    guidance: 'Review provided values and ensure they satisfy endpoint requirements.'
  },
  {
    status: 429,
    title: 'Too Many Requests',
    description: 'Rate limits were exceeded and the request was throttled.',
    guidance: 'Wait briefly, then retry with reduced request frequency.'
  },
  {
    status: 500,
    title: 'Internal Server Error',
    description: 'An unexpected server failure occurred while handling your request.',
    guidance: 'Retry and check backend logs for stack traces and git errors.'
  },
  {
    status: 502,
    title: 'Bad Gateway',
    description: 'The server received an invalid response from an upstream service.',
    guidance: 'Validate reverse proxy and upstream server health.'
  },
  {
    status: 503,
    title: 'Service Unavailable',
    description: 'The service is temporarily unavailable or overloaded.',
    guidance: 'Retry shortly and verify runtime and dependency health.'
  },
  {
    status: 504,
    title: 'Gateway Timeout',
    description: 'An upstream service did not respond in time.',
    guidance: 'Check network latency and upstream git host responsiveness.'
  }
];

export function getErrorDefinition(status: number): ErrorCodeDefinition | undefined {
  return ERROR_CODE_CATALOG.find((entry) => entry.status === status);
}


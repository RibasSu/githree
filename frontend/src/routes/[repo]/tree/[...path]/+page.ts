import type { PageLoad } from './$types';

export const load: PageLoad = ({ params, url }) => {
  return {
    repo: params.repo,
    path: params.path,
    refName: url.searchParams.get('ref') ?? ''
  };
};

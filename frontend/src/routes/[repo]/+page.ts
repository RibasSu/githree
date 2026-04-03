import type { PageLoad } from './$types';

export const load: PageLoad = ({ params, url }) => {
  return {
    repo: params.repo,
    refName: url.searchParams.get('ref') ?? ''
  };
};

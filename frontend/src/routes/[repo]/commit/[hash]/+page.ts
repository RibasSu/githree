import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
  return {
    repo: params.repo,
    hash: params.hash
  };
};

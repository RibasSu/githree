import { error } from '@sveltejs/kit';
import { getErrorDefinition } from '$lib/error-codes';

export function load({ params }) {
  const parsed = Number.parseInt(params.code, 10);
  if (!Number.isFinite(parsed)) {
    throw error(404, 'Error code not found.');
  }

  const definition = getErrorDefinition(parsed);
  if (!definition) {
    throw error(404, `Error code ${params.code} is not mapped in Githree.`);
  }

  throw error(parsed, definition.description);
}


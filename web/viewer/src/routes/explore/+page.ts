import { listPublicSessions } from '$lib/api';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ url }) => {
	const page = parseInt(url.searchParams.get('page') || '1');
	const limit = 20;
	const offset = (page - 1) * limit;

	const data = await listPublicSessions(limit, offset);

	return {
		...data,
		page
	};
};

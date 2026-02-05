import { listPublicSessions } from '$lib/api';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
	try {
		const data = await listPublicSessions(8, 0);
		return { recentSessions: data.sessions };
	} catch {
		// Don't break the landing page if the API is down
		return { recentSessions: [] };
	}
};

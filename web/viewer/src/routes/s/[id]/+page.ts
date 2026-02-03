import { getSession, getSessionContent, ApiError } from '$lib/api';
import { parseSpool } from '$lib/parser';
import { error } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
	const { id } = params;

	try {
		// Fetch metadata and content in parallel
		const [metadata, content] = await Promise.all([getSession(id), getSessionContent(id)]);

		// Parse the .spool content
		const spool = parseSpool(content);

		return {
			id,
			metadata,
			spool
		};
	} catch (e) {
		if (e instanceof ApiError) {
			if (e.status === 404) {
				error(404, 'Session not found');
			}
			if (e.status === 403) {
				error(403, 'This session is private');
			}
			error(e.status, e.message);
		}
		throw e;
	}
};

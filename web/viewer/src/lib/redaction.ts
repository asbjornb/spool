/**
 * Client-side secret detection and redaction.
 *
 * Port of crates/spool-format/src/redaction.rs.
 * All processing runs in the browser — nothing is sent to the server.
 */

export type SecretCategory =
	| 'api_key'
	| 'email'
	| 'phone'
	| 'ip_address'
	| 'private_key'
	| 'aws_key'
	| 'github_token'
	| 'jwt_token';

export interface DetectedSecret {
	start: number;
	end: number;
	category: SecretCategory;
	matched: string;
	/** Whether the user has confirmed this should be redacted */
	confirmed: boolean;
}

const REPLACEMENT: Record<SecretCategory, string> = {
	api_key: '[REDACTED:api_key]',
	email: '[REDACTED:email]',
	phone: '[REDACTED:phone]',
	ip_address: '[REDACTED:ip_address]',
	private_key: '[REDACTED:private_key]',
	aws_key: '[REDACTED:aws_key]',
	github_token: '[REDACTED:github_token]',
	jwt_token: '[REDACTED:jwt_token]'
};

const PATTERNS: [RegExp, SecretCategory][] = [
	// Anthropic API key
	[/sk-ant-api\d{2}-[a-zA-Z0-9_-]{40,}/g, 'api_key'],
	// OpenAI API key
	[/sk-[a-zA-Z0-9]{32,}/g, 'api_key'],
	// Generic key=value
	[/['"](api[_-]?)?key['"]?\s*[:=]\s*['"][a-zA-Z0-9_-]{20,}['"]/g, 'api_key'],
	// Email
	[/[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g, 'email'],
	// US phone
	[/\b\d{3}[-.]?\d{3}[-.]?\d{4}\b/g, 'phone'],
	// International phone
	[/\+\d{1,3}[-.\s]?\d{1,14}/g, 'phone'],
	// IPv4
	[/\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/g, 'ip_address'],
	// Private key
	[/-----BEGIN [A-Z ]+ PRIVATE KEY-----/g, 'private_key'],
	// AWS key
	[/AKIA[0-9A-Z]{16}/g, 'aws_key'],
	// GitHub tokens
	[/ghp_[a-zA-Z0-9]{36}/g, 'github_token'],
	[/github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}/g, 'github_token'],
	// JWT
	[/eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+/g, 'jwt_token']
];

/** Detect all secrets in the given text */
export function detectSecrets(text: string): DetectedSecret[] {
	const secrets: DetectedSecret[] = [];

	for (const [pattern, category] of PATTERNS) {
		// Reset regex lastIndex for global patterns
		pattern.lastIndex = 0;
		let match;
		while ((match = pattern.exec(text)) !== null) {
			secrets.push({
				start: match.index,
				end: match.index + match[0].length,
				category,
				matched: match[0],
				confirmed: true // default to redacting
			});
		}
	}

	// Sort by start position and deduplicate overlapping
	secrets.sort((a, b) => a.start - b.start);
	deduplicateOverlapping(secrets);

	return secrets;
}

/** Get the replacement text for a secret category */
export function getReplacement(category: SecretCategory): string {
	return REPLACEMENT[category];
}

/** Apply confirmed redactions to text (redactions sorted by position descending) */
export function applyRedactions(text: string, secrets: DetectedSecret[]): string {
	// Sort descending by start so replacements don't shift offsets
	const sorted = [...secrets].filter((s) => s.confirmed).sort((a, b) => b.start - a.start);

	let result = text;
	for (const secret of sorted) {
		if (secret.start < result.length && secret.end <= result.length) {
			result = result.slice(0, secret.start) + REPLACEMENT[secret.category] + result.slice(secret.end);
		}
	}
	return result;
}

function deduplicateOverlapping(secrets: DetectedSecret[]): void {
	let i = 0;
	while (i + 1 < secrets.length) {
		if (secrets[i].end > secrets[i + 1].start) {
			// Overlapping — keep the longer one
			const lenA = secrets[i].end - secrets[i].start;
			const lenB = secrets[i + 1].end - secrets[i + 1].start;
			if (lenA >= lenB) {
				secrets.splice(i + 1, 1);
			} else {
				secrets.splice(i, 1);
			}
		} else {
			i++;
		}
	}
}

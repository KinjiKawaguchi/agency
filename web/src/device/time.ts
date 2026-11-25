/** RFC 3339 in UTC to the second, as the protocol writes times (`2026-11-24T12:00:00Z`). */
export const timestampOf = (at: Date): string => at.toISOString().replace(/\.\d{3}Z$/, 'Z')

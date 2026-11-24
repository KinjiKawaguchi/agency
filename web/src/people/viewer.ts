import type { PersonStatus, TeamView } from '../api/teamview'

/** The person a team runtime calls `name`, if the team lists them. */
export const personOf = (view: TeamView, name: string): PersonStatus | undefined => view.people.find((person) => person.id === name)
